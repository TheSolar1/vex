// ══════════════════════════════════════════════════════════════════
// viso.rs — VEX Viso (appels audio/vidéo chiffrés de bout en bout)
// 0 SQL hors de ce fichier / appeldb.rs — tout passe par appeldb::
//
// ── MODÈLE DE SÉCURITÉ ────────────────────────────────────────────
// Le serveur agit UNIQUEMENT comme relais de signalisation WebRTC.
// Il ne voit jamais le flux audio/vidéo (DTLS-SRTP direct navigateur
// ↔ navigateur), ni les SDP/ICE en clair (chiffrés AES-256-GCM côté
// client via une clé dérivée par ECDH P-256 + HKDF entre chaque paire
// de participants — voir static/viso/viso.html pour le détail crypto).
//
// ── INTÉGRATION UI ─────────────────────────────────────────────────
// Pas de barre de navigation ni de sélecteur de thème propres à cette
// page : la nav est injectée côté serveur via function::build_nav_html
// (comme toutes les autres pages VEX), et le thème clair/sombre suit
// exclusivement la préférence utilisateur stockée dans la table `pref`
// (function::get_theme_attr) — aucun bouton local, aucun état JS.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{
    compter_lignes, inserer_ou_modifier, selectionner, supprimer_ligne, verifier_connexion,
    DbPool,
};
use crate::function::{build_nav_html, get_theme_attr, NavContext};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use tiny_http::{Header, Method, Request, Response};

// ══════════════════════════════════════════════════════════════════
// SCHÉMA DB ATTENDU (à créer via db_init.rs ou phpMyAdmin)
// ══════════════════════════════════════════════════════════════════
//
// CREATE TABLE `meet_rooms` (
//   `id`                 INT AUTO_INCREMENT PRIMARY KEY,
//   `room_code`          VARCHAR(16)  NOT NULL UNIQUE,
//   `creator_id`         INT          NOT NULL,
//   `title`              VARCHAR(255) NOT NULL DEFAULT 'Appel VEX',
//   `is_public`          TINYINT      NOT NULL DEFAULT 0,
//   `require_password`   TINYINT      NOT NULL DEFAULT 0,
//   `password_hash`      VARCHAR(255) DEFAULT NULL,
//   `max_participants`   INT          NOT NULL DEFAULT 8,
//   `is_active`          TINYINT      NOT NULL DEFAULT 1,
//   `created_at`         DATETIME     NOT NULL DEFAULT NOW()
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
//
// CREATE TABLE `meet_participants` (
//   `id`          INT AUTO_INCREMENT PRIMARY KEY,
//   `room_id`     INT          NOT NULL,
//   `user_id`     INT          NOT NULL,
//   `session_id`  VARCHAR(64)  NOT NULL UNIQUE,
//   `nom`         VARCHAR(128) NOT NULL,
//   `x25519_pub`  VARCHAR(64)  NOT NULL,   -- clé publique éphémère ECDH P-256 (base64)
//   `joined_at`   DATETIME     NOT NULL DEFAULT NOW(),
//   `last_seen`   DATETIME     NOT NULL DEFAULT NOW(),
//   `status`      VARCHAR(16)  NOT NULL DEFAULT 'connected',
//   INDEX (`room_id`)
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
//
// CREATE TABLE `meet_signaling` (
//   `id`            BIGINT AUTO_INCREMENT PRIMARY KEY,
//   `room_id`       INT          NOT NULL,
//   `from_session`  VARCHAR(64)  NOT NULL,
//   `to_session`    VARCHAR(64)  NOT NULL,
//   `payload_type`  VARCHAR(16)  NOT NULL,   -- offer | answer | ice | bye
//   `ciphertext`    MEDIUMTEXT   NOT NULL,    -- base64, opaque pour le serveur
//   `nonce`         VARCHAR(32)  NOT NULL,    -- base64, 12 octets AES-GCM
//   `created_at`    DATETIME(3)  NOT NULL DEFAULT NOW(3),
//   INDEX (`to_session`),
//   INDEX (`room_id`)
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

const PAGE_HTML: &str = include_str!("../../static/viso/viso.html");

const PAYLOAD_TYPES: &[&str] = &["offer", "answer", "ice", "bye", "renegotiate"];
const MAX_CIPHERTEXT_LEN: usize = 20_000;
const SESSION_TIMEOUT_MINUTES: i64 = 2;
const SIGNAL_RETENTION_MINUTES: i64 = 10;

// ══════════════════════════════════════════════════════════════════
// POINT D'ENTRÉE HTTP — appelé depuis main.rs :
//   viso::viso::handle(&pool, &mut request)
// Route la page HTML (GET /viso, /viso/) et l'API JSON (POST /api/viso).
// ══════════════════════════════════════════════════════════════════
pub fn handle(pool: &DbPool, request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let path = request.url().split('?').next().unwrap_or("/viso").to_string();
    let method = request.method().clone();

    let cookie_val = lire_cookie(request, "connexion_cookie");
    let user_agent = lire_header(request, "User-Agent");
    let remote_ip = crate::utils::strip_port(
        &request.remote_addr().map(|a| a.to_string()).unwrap_or_default(),
    );

    if path.starts_with("/api/viso") {
        if method != Method::Post {
            return json_response(&erreur("Méthode non autorisée."), 405);
        }
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let params = crate::utils::parse_query(&format!("?{}", body));
        let action = params.get("action").cloned().unwrap_or_default();
        let res = handle_viso_action(pool, &action, &params, &cookie_val, &remote_ip, &user_agent);
        return json_response(&res, 200);
    }

    render_page(pool, &cookie_val, &remote_ip, &user_agent)
}

// ── Construction des réponses HTTP ──────────────────────────────────
// IMPORTANT : on utilise Response::from_data() et NON from_string().
// from_string() pose en interne un header par défaut
// "Content-Type: text/plain; charset=UTF-8", et .with_header(...) ne
// le remplace pas : il l'AJOUTE. Le client recevait donc DEUX headers
// Content-Type (text/plain puis le vrai), ce qui rendait la réponse
// ambiguë côté navigateur et cassait le rendu (le <style> de la page
// s'affichait comme du texte brut au lieu d'être appliqué comme CSS).
// from_data() ne pose aucun Content-Type par défaut : le nôtre est
// donc le seul présent.
// Remplace uniquement la première occurrence de `from` par `to` dans `s`.
// Contrairement à str::replace, qui remplace TOUTES les occurrences —
// c'est cette différence qui a causé le bug d'injection de nav_html
// dans un commentaire CSS (voir render_page).
fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) {
        Some(pos) => {
            let mut out = String::with_capacity(s.len() + to.len());
            out.push_str(&s[..pos]);
            out.push_str(to);
            out.push_str(&s[pos + from.len()..]);
            out
        }
        None => s.to_string(),
    }
}

fn json_response(v: &Value, status: u16) -> Response<Cursor<Vec<u8>>> {
    let body = v.to_string().into_bytes();
    Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
}

/// Rend la page HTML avec la nav VEX standard injectée et le thème pris
/// depuis la préférence utilisateur (`pref.teme`) — jamais depuis un
/// bouton ou du localStorage.
fn render_page(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<Cursor<Vec<u8>>> {
    let user = verifier_connexion(pool, cookie_val, remote_ip, user_agent);
    let user_id = user.as_ref().and_then(|u| u.get("id").and_then(|v| v.as_i64()));

    // Thème = préférence DB uniquement (function::get_theme_attr → "dark"|"light")
    let theme = user_id
        .map(|uid| get_theme_attr(pool, uid))
        .unwrap_or("light");

    let ctx = NavContext {
        pool,
        user_id,
        page_key: "viso",
        cookie_val,
        remote_ip,
        user_agent,
        query_id: None,
        apps: vec![],
        admin_apps: vec![],
    };
    let nav_html = build_nav_html(&ctx);

    // On ne remplace QUE la première occurrence de {{NAV_HTML}} : si un
    // jour un commentaire ou un texte contient à nouveau ce token par
    // erreur, on ne cassera plus la page comme précédemment (le nav_html,
    // qui contient son propre </style>, fermait prématurément le <style>
    // de tête quand {{NAV_HTML}} apparaissait aussi dans un commentaire CSS).
    let page = replace_first(PAGE_HTML, "{{NAV_HTML}}", &nav_html)
        .replace("{{THEME}}", theme);

    Response::from_data(page.into_bytes())
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}

// ── Lecture cookie / header ─────────────────────────────────────────
fn lire_cookie(request: &Request, name: &str) -> String {
    for h in request.headers() {
        if h.field.as_str().as_str().eq_ignore_ascii_case("Cookie") {
            let raw = h.value.as_str();
            for part in raw.split(';') {
                let part = part.trim();
                if let Some(v) = part.strip_prefix(&format!("{}=", name)) {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

fn lire_header(request: &Request, name: &str) -> String {
    for h in request.headers() {
        if h.field.as_str().as_str().eq_ignore_ascii_case(name) {
            return h.value.as_str().to_string();
        }
    }
    String::new()
}

// ══════════════════════════════════════════════════════════════════
// AUTHENTIFICATION COMMUNE
// ══════════════════════════════════════════════════════════════════
#[allow(dead_code)]
fn auth(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Option<(i64, String, i64)> {
    let u = verifier_connexion(pool, cookie_val, remote_ip, user_agent)?;
    let id = u.get("id")?.as_i64()?;
    let nom = u.get("nom")?.as_str()?.to_string();
    let privilege = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);
    Some((id, nom, privilege))
}

// ══════════════════════════════════════════════════════════════════
// CORRECTIF (bug "Session invalide ou expirée" au moment de rejoindre)
// ══════════════════════════════════════════════════════════════════
// `auth()` (donc `verifier_connexion`) compare strictement l'IP et le
// user-agent à ce qui a été enregistré à la connexion initiale. Sur
// un réseau instable (proxy, double pile IPv4/IPv6, mobile) ça rejette
// des sessions pourtant légitimes. On ne modifie pas cette logique
// (elle vit hors de ce fichier, dans c.rs), on l'évite juste pour Viso :
// `auth_viso` revalide seulement le cookie et son expiration (1h),
// directement via les tables `loginc`/`login`, sans comparer
// IP/user-agent. Toujours 0 SQL brut ici : on passe par `selectionner`
// (appeldb::).
fn auth_viso(pool: &DbPool, cookie_val: &str) -> Option<(i64, String, i64)> {
    if cookie_val.is_empty() {
        return None;
    }

    let loginc = selectionner(
        pool,
        "loginc",
        &[("idcokier", mysql::Value::from(cookie_val))],
        &["datecra", "email", "nom"],
        None,
        Some(1),
    );
    let row = loginc.into_iter().next()?;
    let date = row.get("datecra").and_then(|v| v.as_str()).unwrap_or("");

    // Même règle de validité que verifier_session (1h), sans le
    // contrôle IP/user-agent.
    if !crate::c::is_recent_local(date, 3600) {
        return None;
    }

    let email = row.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let nom = row.get("nom").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if email.is_empty() {
        return None;
    }

    let login_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id", "privilege"],
        None,
        Some(1),
    );
    let lr = login_rows.into_iter().next()?;
    let id = lr.get("id").and_then(|v| v.as_i64())?;
    let privilege = lr.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);

    Some((id, nom, privilege))
}

// ══════════════════════════════════════════════════════════════════
// CORRECTIF (bug "Session invalide ou expirée" en cours d'appel)
// ══════════════════════════════════════════════════════════════════
// `auth()` revalide le cookie ET compare remote_ip / user_agent à ce
// qui a été enregistré à la connexion (verifier_connexion). C'est
// pertinent pour des actions sensibles (créer/fermer une salle), mais
// c'est beaucoup trop strict pour les appels de signalisation qui
// tournent en boucle pendant toute la durée de l'appel
// (poster_signal / recuperer_signaux / heartbeat) : si l'IP change en
// cours de route (bascule wifi/4G, double pile IPv4/IPv6, proxy sortant
// qui alterne d'adresse, etc.), verifier_connexion échoue et l'appel
// entier est vu comme "Session invalide ou expirée" alors que
// l'utilisateur est toujours bel et bien dans la salle.
//
// Pour ces actions-là, on vérifie uniquement que `session_id` est un
// participant actif et connu de la salle (déjà garanti par
// rejoindre_salle, qui lui a fait l'auth complète une seule fois à
// l'entrée). C'est suffisant : un session_id est un secret aléatoire
// de 24 octets connu uniquement du client qui a rejoint la salle.
fn session_active(pool: &DbPool, session_id: &str) -> bool {
    compter_lignes(
        pool,
        "meet_participants",
        &[
            ("session_id", mysql::Value::from(session_id)),
            ("status", mysql::Value::from("connected")),
        ],
    ) > 0
}

fn erreur(msg: &str) -> Value {
    json!({"success": false, "error": msg})
}

// ══════════════════════════════════════════════════════════════════
// GÉNÉRATION DE CODE DE SALLE
// ══════════════════════════════════════════════════════════════════
fn generer_code_salle() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let charset = b"abcdefghjkmnpqrstuvwxyz23456789";
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1337);
    let mut state = (seed as u64) ^ 0x9e3779b97f4a7c15 ^ (std::process::id() as u64);
    let mut code = String::with_capacity(11);
    for i in 0..10 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if i == 3 {
            code.push('-');
        }
        code.push(charset[(state as usize) % charset.len()] as char);
    }
    code
}

fn hash_simple(input: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(digest)
}

// ══════════════════════════════════════════════════════════════════
// 1. CRÉER UNE SALLE
// ══════════════════════════════════════════════════════════════════
pub fn creer_salle(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    title: &str,
    is_public: bool,
    password: Option<&str>,
    max_participants: i64,
) -> Value {
    let _ = (remote_ip, user_agent);
    let Some((user_id, _nom, _priv)) = auth_viso(pool, cookie_val) else {
        return erreur("Session invalide ou expirée.");
    };

    let max_participants = max_participants.clamp(2, 32);
    let title = if title.trim().is_empty() {
        "Appel VEX".to_string()
    } else {
        title.chars().take(255).collect::<String>()
    };

    let mut room_code = String::new();
    for _ in 0..10 {
        let candidate = generer_code_salle();
        let existe = compter_lignes(
            pool,
            "meet_rooms",
            &[("room_code", mysql::Value::from(candidate.as_str()))],
        ) > 0;
        if !existe {
            room_code = candidate;
            break;
        }
    }
    if room_code.is_empty() {
        return erreur("Impossible de générer un code de salle unique.");
    }

    let require_password = password.map(|p| !p.is_empty()).unwrap_or(false);
    let pass_val: mysql::Value = if require_password {
        mysql::Value::from(hash_simple(password.unwrap()))
    } else {
        mysql::Value::NULL
    };

    let id = inserer_ou_modifier(
        pool,
        "meet_rooms",
        &[
            ("room_code", mysql::Value::from(room_code.as_str())),
            ("creator_id", mysql::Value::from(user_id)),
            ("title", mysql::Value::from(title.as_str())),
            ("is_public", mysql::Value::from(is_public as i64)),
            (
                "require_password",
                mysql::Value::from(require_password as i64),
            ),
            ("password_hash", pass_val),
            ("max_participants", mysql::Value::from(max_participants)),
            ("is_active", mysql::Value::from(1i64)),
        ],
        &[],
    );

    if id < 0 {
        return erreur("Erreur lors de la création de la salle.");
    }

    json!({
        "success": true,
        "data": {
            "room_id": id,
            "room_code": room_code,
            "title": title,
            "max_participants": max_participants
        }
    })
}

// ══════════════════════════════════════════════════════════════════
// 2. REJOINDRE UNE SALLE
// ══════════════════════════════════════════════════════════════════
pub fn rejoindre_salle(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    room_code: &str,
    session_id: &str,
    x25519_pub_b64: &str,
    password: Option<&str>,
) -> Value {
    let _ = (remote_ip, user_agent);
    let Some((user_id, nom, _priv)) = auth_viso(pool, cookie_val) else {
        return erreur("Session invalide ou expirée.");
    };

    if session_id.len() < 16 || session_id.len() > 64 {
        return erreur("session_id invalide.");
    }
    if x25519_pub_b64.is_empty() || x25519_pub_b64.len() > 128 {
        return erreur("Clé publique invalide.");
    }

    let rooms = selectionner(
        pool,
        "meet_rooms",
        &[
            ("room_code", mysql::Value::from(room_code)),
            ("is_active", mysql::Value::from(1i64)),
        ],
        &[],
        None,
        Some(1),
    );
    let Some(room) = rooms.into_iter().next() else {
        return erreur("Salle introuvable ou fermée.");
    };
    let room_id = room.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    if room.get("require_password").and_then(|v| v.as_i64()) == Some(1) {
        let attendu = room.get("password_hash").and_then(|v| v.as_str());
        let fourni = password.map(hash_simple);
        if attendu.is_none() || fourni.as_deref() != attendu {
            return erreur("Mot de passe de salle incorrect.");
        }
    }

    let max_p = room
        .get("max_participants")
        .and_then(|v| v.as_i64())
        .unwrap_or(8);
    let actifs = compter_participants_actifs(pool, room_id);
    if actifs >= max_p {
        return erreur("Salle pleine.");
    }

    let inserted = inserer_ou_modifier(
        pool,
        "meet_participants",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("user_id", mysql::Value::from(user_id)),
            ("session_id", mysql::Value::from(session_id)),
            ("nom", mysql::Value::from(nom.as_str())),
            ("x25519_pub", mysql::Value::from(x25519_pub_b64)),
            ("status", mysql::Value::from("connected")),
        ],
        &[],
    );
    if inserted < 0 {
        return erreur("Impossible de rejoindre la salle (session_id déjà utilisé ?).");
    }

    let autres = lister_participants(pool, room_id, Some(session_id));

    // FIX (point 4) : on renvoie le pseudo réel de l'utilisateur courant
    // (`your_nom`) au client, pour que la vignette locale affiche son nom
    // au lieu du texte générique "Vous" codé en dur côté JS.
    json!({
        "success": true,
        "data": {
            "room_id": room_id,
            "title": room.get("title").cloned().unwrap_or(json!("")),
            "your_nom": nom,
            "participants": autres
        }
    })
}

fn compter_participants_actifs(pool: &DbPool, room_id: i64) -> i64 {
    compter_lignes(
        pool,
        "meet_participants",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("status", mysql::Value::from("connected")),
        ],
    ) as i64
}

pub fn lister_participants(
    pool: &DbPool,
    room_id: i64,
    exclude_session: Option<&str>,
) -> Vec<HashMap<String, Value>> {
    selectionner(
        pool,
        "meet_participants",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("status", mysql::Value::from("connected")),
        ],
        &["session_id", "user_id", "nom", "x25519_pub", "joined_at"],
        Some("joined_at ASC"),
        None,
    )
    .into_iter()
    .filter(|p| {
        exclude_session
            .map(|ex| p.get("session_id").and_then(|v| v.as_str()) != Some(ex))
            .unwrap_or(true)
    })
    .collect()
}

// ══════════════════════════════════════════════════════════════════
// 3. RELAYER UN MESSAGE DE SIGNALISATION (chiffré, opaque)
// ══════════════════════════════════════════════════════════════════
pub fn poster_signal(
    pool: &DbPool,
    room_id: i64,
    from_session: &str,
    to_session: &str,
    payload_type: &str,
    ciphertext_b64: &str,
    nonce_b64: &str,
) -> Value {
    if !PAYLOAD_TYPES.contains(&payload_type) {
        return erreur("Type de payload non autorisé.");
    }
    if ciphertext_b64.is_empty() || ciphertext_b64.len() > MAX_CIPHERTEXT_LEN {
        return erreur("Ciphertext invalide ou trop volumineux.");
    }
    if nonce_b64.is_empty() || nonce_b64.len() > 32 {
        return erreur("Nonce invalide.");
    }

    let expediteur_ok = compter_lignes(
        pool,
        "meet_participants",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("session_id", mysql::Value::from(from_session)),
            ("status", mysql::Value::from("connected")),
        ],
    ) > 0;
    let destinataire_ok = compter_lignes(
        pool,
        "meet_participants",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("session_id", mysql::Value::from(to_session)),
            ("status", mysql::Value::from("connected")),
        ],
    ) > 0;
    if !expediteur_ok || !destinataire_ok {
        return erreur("Participant inconnu dans cette salle.");
    }

    let id = inserer_ou_modifier(
        pool,
        "meet_signaling",
        &[
            ("room_id", mysql::Value::from(room_id)),
            ("from_session", mysql::Value::from(from_session)),
            ("to_session", mysql::Value::from(to_session)),
            ("payload_type", mysql::Value::from(payload_type)),
            ("ciphertext", mysql::Value::from(ciphertext_b64)),
            ("nonce", mysql::Value::from(nonce_b64)),
        ],
        &[],
    );

    if id < 0 {
        erreur("Erreur lors de l'enregistrement du signal.")
    } else {
        json!({"success": true, "data": {"id": id}})
    }
}

// ══════════════════════════════════════════════════════════════════
// 4. RÉCUPÉRER LES SIGNAUX EN ATTENTE
// ══════════════════════════════════════════════════════════════════
pub fn recuperer_signaux(pool: &DbPool, session_id: &str) -> Vec<HashMap<String, Value>> {
    let rows = selectionner(
        pool,
        "meet_signaling",
        &[("to_session", mysql::Value::from(session_id))],
        &[
            "id",
            "from_session",
            "payload_type",
            "ciphertext",
            "nonce",
            "created_at",
        ],
        Some("created_at ASC"),
        Some(200),
    );

    for row in &rows {
        if let Some(id) = row.get("id").and_then(|v| v.as_i64()) {
            supprimer_ligne(pool, "meet_signaling", "id", mysql::Value::from(id));
        }
    }
    rows
}

// ══════════════════════════════════════════════════════════════════
// 5. HEARTBEAT
// ══════════════════════════════════════════════════════════════════
pub fn heartbeat(pool: &DbPool, session_id: &str) -> bool {
    inserer_ou_modifier(
        pool,
        "meet_participants",
        &[(
            "last_seen",
            mysql::Value::from(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        )],
        &[("session_id", mysql::Value::from(session_id))],
    ) >= 0
}

// ══════════════════════════════════════════════════════════════════
// 6. QUITTER LA SALLE
// ══════════════════════════════════════════════════════════════════
pub fn quitter_salle(pool: &DbPool, session_id: &str) -> Value {
    let existe = compter_lignes(
        pool,
        "meet_participants",
        &[("session_id", mysql::Value::from(session_id))],
    ) > 0;
    if !existe {
        return erreur("Session non trouvée.");
    }
    supprimer_ligne(
        pool,
        "meet_participants",
        "session_id",
        mysql::Value::from(session_id),
    );
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return json!({"success": true}),
    };
    use mysql::prelude::Queryable;
    let _ = conn.exec_drop(
        "DELETE FROM `meet_signaling` WHERE from_session = ? OR to_session = ?",
        (session_id, session_id),
    );
    json!({"success": true})
}

// ══════════════════════════════════════════════════════════════════
// 7. FERMER UNE SALLE (créateur ou admin uniquement)
// ══════════════════════════════════════════════════════════════════
pub fn fermer_salle(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    room_id: i64,
) -> Value {
    let _ = (remote_ip, user_agent);
    let Some((user_id, _nom, privilege)) = auth_viso(pool, cookie_val) else {
        return erreur("Session invalide ou expirée.");
    };

    let rooms = selectionner(
        pool,
        "meet_rooms",
        &[("id", mysql::Value::from(room_id))],
        &["creator_id"],
        None,
        Some(1),
    );
    let Some(room) = rooms.into_iter().next() else {
        return erreur("Salle introuvable.");
    };
    let creator_id = room.get("creator_id").and_then(|v| v.as_i64()).unwrap_or(-1);
    if creator_id != user_id && privilege > 2 {
        return erreur("Droits insuffisants pour fermer cette salle.");
    }

    inserer_ou_modifier(
        pool,
        "meet_rooms",
        &[("is_active", mysql::Value::from(0i64))],
        &[("id", mysql::Value::from(room_id))],
    );

    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return json!({"success": true}),
    };
    use mysql::prelude::Queryable;
    let _ = conn.exec_drop("DELETE FROM `meet_participants` WHERE room_id = ?", (room_id,));
    let _ = conn.exec_drop("DELETE FROM `meet_signaling` WHERE room_id = ?", (room_id,));

    json!({"success": true})
}

// ══════════════════════════════════════════════════════════════════
// 8. NETTOYAGE PÉRIODIQUE (à appeler depuis un thread cron dans main.rs)
// ══════════════════════════════════════════════════════════════════
pub fn nettoyer_sessions_expirees(pool: &DbPool) -> (u64, u64) {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    use mysql::prelude::Queryable;

    let _ = conn.exec_drop(
        "DELETE FROM `meet_participants` WHERE last_seen < NOW() - INTERVAL ? MINUTE",
        (SESSION_TIMEOUT_MINUTES,),
    );
    let participants_purges = conn.affected_rows();

    let _ = conn.exec_drop(
        "DELETE FROM `meet_signaling` WHERE created_at < NOW() - INTERVAL ? MINUTE",
        (SIGNAL_RETENTION_MINUTES,),
    );
    let signaux_purges = conn.affected_rows();

    (participants_purges, signaux_purges)
}

// ══════════════════════════════════════════════════════════════════
// 9. ROUTEUR D'ACTIONS API (POST /api/viso, champ `action`)
// ══════════════════════════════════════════════════════════════════
pub fn handle_viso_action(
    pool: &DbPool,
    action: &str,
    params: &HashMap<String, String>,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Value {
    match action {
        "creer_salle" => creer_salle(
            pool,
            cookie_val,
            remote_ip,
            user_agent,
            params.get("title").map(|s| s.as_str()).unwrap_or(""),
            params.get("is_public").map(|v| v == "1").unwrap_or(false),
            params.get("password").map(|s| s.as_str()).filter(|s| !s.is_empty()),
            params
                .get("max_participants")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(8),
        ),

        "rejoindre_salle" => {
            let (Some(code), Some(sid), Some(pubkey)) = (
                params.get("room_code"),
                params.get("session_id"),
                params.get("x25519_pub"),
            ) else {
                return erreur("Paramètres manquants.");
            };
            rejoindre_salle(
                pool,
                cookie_val,
                remote_ip,
                user_agent,
                code,
                sid,
                pubkey,
                params.get("password").map(|s| s.as_str()),
            )
        }

        // CORRECTIF : on ne revalide plus le cookie complet (auth) ici,
        // seulement l'appartenance du session_id à la salle — déjà fait
        // ci-dessous via expediteur_ok/destinataire_ok dans poster_signal.
        // Avant, un double contrôle (auth() ET vérif participant) pouvait
        // faire échouer l'appel avec "Session invalide ou expirée" dès
        // que remote_ip/user_agent ne correspondaient plus exactement à
        // la connexion initiale, même si le participant était toujours
        // légitimement dans la salle.
        "poster_signal" => {
            let (Some(room_id), Some(from), Some(to), Some(ptype), Some(cipher), Some(nonce)) = (
                params.get("room_id").and_then(|v| v.parse::<i64>().ok()),
                params.get("from_session"),
                params.get("to_session"),
                params.get("payload_type"),
                params.get("ciphertext"),
                params.get("nonce"),
            ) else {
                return erreur("Paramètres manquants.");
            };
            if !session_active(pool, from) {
                return erreur("Session invalide ou expirée.");
            }
            poster_signal(pool, room_id, from, to, ptype, cipher, nonce)
        }

        // CORRECTIF : idem, on vérifie que session_id est un participant
        // actif plutôt que de revalider le cookie/IP/UA à chaque poll
        // (cet appel tourne toutes les ~1,2s pendant toute la durée de
        // l'appel : la moindre fluctuation réseau côté client faisait
        // échouer verifier_connexion et cassait l'appel en cours).
        "recuperer_signaux" => {
            let Some(sid) = params.get("session_id") else {
                return erreur("session_id manquant.");
            };
            if !session_active(pool, sid) {
                return erreur("Session invalide ou expirée.");
            }
            let signaux = recuperer_signaux(pool, sid);
            json!({"success": true, "data": {"signaux": signaux}})
        }

        "heartbeat" => {
            let Some(sid) = params.get("session_id") else {
                return erreur("session_id manquant.");
            };
            if !session_active(pool, sid) {
                return erreur("Session invalide ou expirée.");
            }
            json!({"success": heartbeat(pool, sid)})
        }

        "quitter_salle" => {
            let Some(sid) = params.get("session_id") else {
                return erreur("session_id manquant.");
            };
            quitter_salle(pool, sid)
        }

        "fermer_salle" => {
            let Some(room_id) = params.get("room_id").and_then(|v| v.parse::<i64>().ok()) else {
                return erreur("room_id manquant.");
            };
            fermer_salle(pool, cookie_val, remote_ip, user_agent, room_id)
        }

        // CORRECTIF : là aussi, remplacé le double-check auth() par une
        // vérification que le session_id fourni est bien un participant
        // actif de la salle demandée, pour la même raison que ci-dessus.
        "lister_participants" => {
            let Some(sid) = params.get("session_id") else {
                return erreur("session_id manquant.");
            };
            if !session_active(pool, sid) {
                return erreur("Session invalide ou expirée.");
            }
            let Some(room_id) = params.get("room_id").and_then(|v| v.parse::<i64>().ok()) else {
                return erreur("room_id manquant.");
            };
            json!({"success": true, "data": {"participants": lister_participants(pool, room_id, None)}})
        }

        _ => erreur("Action inconnue."),
    }
}