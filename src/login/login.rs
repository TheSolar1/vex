// ══════════════════════════════════════════════════════════════════
// login/login.rs — VEX Login + Inscription — SRP-6a (RFC 5054, SHA-256)
//
// - Le mot de passe NE QUITTE JAMAIS le navigateur, sous AUCUNE forme,
//   même hashée : ni en clair, ni en hash réutilisable pour se connecter.
// - À l'inscription, le client calcule un `salt` + un `verifier`
//   mathématiques (voir crate::srp) et n'envoie que ça — le serveur ne
//   peut pas en retrouver le mot de passe, et le verifier seul ne
//   permet pas de se connecter (pas de "pass-the-hash").
// - À la connexion, un échange en 2 étapes (SRP step1/step2) prouve
//   que le client connaît le mot de passe sans jamais le transmettre
//   ni transmettre une valeur replayable.
// - Le mdp en clair reste en mémoire JS côté client, pour dériver la clé
//   de chiffrement des fichiers (inchangé, géré par vex-crypto.js).
//
// SCHÉMA DB REQUIS (migration depuis l'ancienne colonne `motdepass`) :
//
//   ALTER TABLE `login`
//     DROP COLUMN `motdepass`,
//     ADD COLUMN `srp_salt`     VARCHAR(64)  NOT NULL,   -- hex, 32 octets
//     ADD COLUMN `srp_verifier` VARCHAR(512) NOT NULL;   -- hex, 256 octets
//
//   CREATE TABLE `srp_sessions` (
//     `token`      VARCHAR(64)  NOT NULL PRIMARY KEY,   -- corrèle step1 → step2
//     `email`      VARCHAR(255) NOT NULL,
//     `b_hex`      VARCHAR(64)  NOT NULL,                -- exposant privé serveur (éphémère)
//     `created_at` DATETIME     NOT NULL DEFAULT NOW()
//   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
//
//   Nettoyage périodique recommandé (cron, comme meet_signaling) :
//     DELETE FROM srp_sessions WHERE created_at < NOW() - INTERVAL 5 MINUTE;
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{compter_lignes, inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::config_loader::VexConfig;
use crate::srp::{
    self, bigint_from_hex, compute_b_public, compute_k, compute_m1, compute_m2, compute_s_server,
    compute_u, constant_time_eq, generate_b, group, hex_decode, hex_encode, is_safe_public_value,
};
use crate::utils::{strip_port, url_decode};
use serde_json::json;
use std::collections::HashMap;
use tiny_http::{Request, Response};

/// Durée de vie max d'une session SRP éphémère (step1 → step2).
const SRP_SESSION_MAX_AGE_SECONDS: i64 = 300; // 5 minutes

pub fn handle_request(mut request: Request, pool: &DbPool, config: &VexConfig, remote_full: &str) {
    let remote_ip = strip_port(remote_full);
    let method = request.method().to_string();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    // ── Config publique ──────────────────────────────────────────
    if path == "/api/login/config" {
        let referer = get_header(&request, "Referer");
        let host = get_header(&request, "Host");
        if !referer.is_empty() && !referer.contains(&host) {
            respond_json(request, json!({"error":"Forbidden"}), 403);
            return;
        }
        respond_json(
            request,
            json!({
                "registration_mode":       config.users.registration_mode,
                "activation_key_required": config.users.activation_key_required,
                "password_min_length":     config.security.password_min_length,
            }),
            200,
        );
        return;
    }

    // ── Déjà connecté → redirige ─────────────────────────────────
    if method == "GET" && !cookie_val.is_empty() {
        if crate::c::verifier_session(pool, &cookie_val, &remote_ip, &user_agent).connecte {
            redirect(request, "/login/dashboard");
            return;
        }
    }

    // ── Premier lancement → redirige vers first_setup ────────────
    if method == "GET" && compter_lignes(pool, "login", &[]) == 0 {
        redirect(request, "/login/first_setup");
        return;
    }

    // ── GET → sert le HTML statique ──────────────────────────────
    if method == "GET" {
        serve_static_html(request, "static/login/login.html");
        return;
    }

    // ── IP blacklist / whitelist ──────────────────────────────────
    if config.security.ip_blacklist_enabled {
        if config.security.ip_blacklist.iter().any(|ip| ip == &remote_ip) {
            respond_json(
                request,
                json!({"success":false,"error":"Accès refusé : IP bloquée."}),
                403,
            );
            return;
        }
    }
    if config.security.ip_whitelist_enabled && !config.security.ip_whitelist.is_empty() {
        if !config.security.ip_whitelist.iter().any(|ip| ip == &remote_ip) {
            respond_json(
                request,
                json!({"success":false,"error":"Accès refusé : IP non autorisée."}),
                403,
            );
            return;
        }
    }

    let body = read_body(&mut request);
    let action = body.get("action").map(|s| s.as_str()).unwrap_or("");

    match action {
        // ── Nouveau flux SRP-6a en 2 étapes ────────────────────────
        "srp_step1" => handle_srp_step1(request, pool, &body),
        "srp_step2" => handle_srp_step2(request, pool, &body, &remote_ip, &user_agent),
        "signup" => handle_signup(request, pool, config, &body),
        _ => respond_json(
            request,
            json!({"success":false,"error":"Action inconnue"}),
            400,
        ),
    }
}

// ══════════════════════════════════════════════════════════════════
// SRP — Étape 1 : le client envoie son email, le serveur répond avec
// le salt, sa valeur publique B, et un token pour corréler l'étape 2.
// ══════════════════════════════════════════════════════════════════
fn handle_srp_step1(request: Request, pool: &DbPool, body: &HashMap<String, String>) {
    let email = body.get("email").cloned().unwrap_or_default();

    // FIX (défense en profondeur, même logique que step2) : borne la
    // taille de l'email avant tout hash/lookup — évite qu'une entrée
    // anormalement longue serve à faire travailler inutilement le
    // hachage SHA-256 ou les fonctions de la table `login`.
    if email.is_empty() || email.len() > 255 {
        respond_json(request, json!({"success":false,"error":"Email invalide."}), 400);
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["srp_salt", "srp_verifier"],
        None,
        Some(1),
    );

    // ── Anti-énumération de comptes ───────────────────────────────
    // Si le compte n'existe pas, on NE DOIT PAS répondre différemment
    // (sinon on révèle l'existence de l'email). On génère un salt/verifier
    // factices mais déterministes-par-email (donc stables si l'attaquant
    // retente), pour que le comportement soit indistinguable d'un vrai
    // compte du point de vue du timing/format de réponse.
    let (salt_hex, verifier_hex) = if let Some(row) = rows.into_iter().next() {
        let s = row.get("srp_salt").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let v = row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
        (s, v)
    } else {
        fake_salt_and_verifier(&email)
    };

    let grp = group();
    let Some(v_big) = bigint_from_hex(&verifier_hex) else {
        respond_json(request, json!({"success":false,"error":"Erreur interne (verifier)."}), 500);
        return;
    };

    let b = generate_b();
    let b_pub = compute_b_public(&grp, &v_big, &b);

    let token = hex_encode(&srp::random_bytes(24));
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    inserer_ou_modifier(
        pool,
        "srp_sessions",
        &[
            ("token", mysql::Value::from(token.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("b_hex", mysql::Value::from(hex_encode(&b.to_bytes_be()).as_str())),
            ("created_at", mysql::Value::from(now.as_str())),
        ],
        &[],
    );

    respond_json(
        request,
        json!({
            "success": true,
            "salt":    salt_hex,
            "B":       hex_encode(&b_pub.to_bytes_be()),
            "token":   token,
        }),
        200,
    );
}

/// Génère un salt/verifier factices mais stables pour un email donné,
/// pour que /srp_step1 sur un compte inexistant se comporte comme un
/// vrai compte (anti-énumération). Dérivé de SHA-256(email) — jamais
/// utilisé pour un vrai calcul de mot de passe, juste pour la forme.
fn fake_salt_and_verifier(email: &str) -> (String, String) {
    let h = srp::sha256(email.to_lowercase().as_bytes());
    let salt_hex = hex_encode(&h[..16]);
    // "Verifier" factice = hash étendu, jamais un vrai g^x mod N, mais de
    // la bonne forme hex pour ne pas planter bigint_from_hex côté step1.
    let mut extended = h.clone();
    extended.extend_from_slice(&srp::sha256(&h));
    extended.extend_from_slice(&srp::sha256(&extended[..32]));
    extended.truncate(srp::N_LEN_BYTES);
    (salt_hex, hex_encode(&extended))
}

// ══════════════════════════════════════════════════════════════════
// SRP — Étape 2 : le client prouve qu'il connaît le mot de passe (M1),
// le serveur vérifie et renvoie sa propre preuve (M2) + ouvre la session.
// ══════════════════════════════════════════════════════════════════
fn handle_srp_step2(
    request: Request,
    pool: &DbPool,
    body: &HashMap<String, String>,
    remote_ip: &str,
    user_agent: &str,
) {
    let token = body.get("token").cloned().unwrap_or_default();
    let email = body.get("email").cloned().unwrap_or_default();
    let a_hex = body.get("A").cloned().unwrap_or_default();
    let m1_hex = body.get("M1").cloned().unwrap_or_default();

    // ── FIX SÉCURITÉ (DoS) ──────────────────────────────────────────
    // Avant TOUT parsing en BigUint, on borne strictement la taille de
    // A et M1. Sans ce contrôle, un client pouvait envoyer un `A` de
    // plusieurs Mo de texte hex : ça se transforme en un BigUint géant,
    // et le modpow + hash qui suivent deviennent arbitrairement coûteux
    // (CPU/mémoire) — un déni de service trivial en une seule requête,
    // qui ne se voit jamais en usage normal (le login continue de
    // fonctionner), donc casse la sécurité "en silence".
    //   - A : au plus 512 car. hex (256 octets = taille de N)
    //   - M1 : exactement 64 car. hex (32 octets = SHA-256)
    if a_hex.is_empty() || a_hex.len() > 512 || !a_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Valeur d'authentification invalide."}), 400);
        return;
    }
    if m1_hex.len() != 64 || !m1_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Preuve d'authentification invalide."}), 400);
        return;
    }
    if token.len() != 48 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Session d'authentification invalide."}), 400);
        return;
    }

    // ── Récupère la session éphémère (b, corrélée au token) ───────
    let sess_rows = selectionner(
        pool,
        "srp_sessions",
        &[
            ("token", mysql::Value::from(token.as_str())),
            ("email", mysql::Value::from(email.as_str())),
        ],
        &["b_hex", "created_at"],
        None,
        Some(1),
    );
    let Some(sess) = sess_rows.into_iter().next() else {
        respond_json(request, json!({"success":false,"error":"Session d'authentification invalide ou expirée."}), 200);
        return;
    };
    // Session à usage unique — on la supprime immédiatement, qu'elle
    // réussisse ou échoue, pour empêcher tout replay de step2.
    supprimer_ligne(pool, "srp_sessions", "token", mysql::Value::from(token.as_str()));

    let created_at = sess.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
    if !crate::c::is_recent_local(created_at, SRP_SESSION_MAX_AGE_SECONDS) {
        respond_json(request, json!({"success":false,"error":"Session d'authentification expirée."}), 200);
        return;
    }
    let b_hex = sess.get("b_hex").and_then(|v| v.as_str()).unwrap_or("");

    // ── Récupère le compte (peut ne pas exister si fake step1) ────
    let user_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id", "nom", "email", "srp_verifier", "vip", "privilege"],
        None,
        Some(1),
    );
    let Some(user_row) = user_rows.into_iter().next() else {
        // Compte inexistant : réponse volontairement identique à un
        // mauvais mot de passe, pour ne pas révéler l'absence du compte.
        respond_json(request, json!({"success":false,"error":"Email ou mot de passe incorrect."}), 200);
        return;
    };

    let verifier_hex = user_row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("");
    let (Some(v_big), Some(b_bytes), Some(a_bytes), Some(m1_client)) = (
        bigint_from_hex(verifier_hex),
        hex_decode(b_hex),
        hex_decode(&a_hex),
        hex_decode(&m1_hex),
    ) else {
        respond_json(request, json!({"success":false,"error":"Format d'authentification invalide."}), 400);
        return;
    };

    let grp = group();
    let b = num_bigint::BigUint::from_bytes_be(&b_bytes);
    let a_pub = num_bigint::BigUint::from_bytes_be(&a_bytes);

    // ── Vérification anti-fuite : A ne doit jamais être 0 mod N ───
    if !is_safe_public_value(&a_pub, &grp.n) {
        respond_json(request, json!({"success":false,"error":"Valeur d'authentification invalide."}), 400);
        return;
    }

    let b_pub = compute_b_public(&grp, &v_big, &b);
    let u = compute_u(&a_pub, &b_pub);
    let s_server = compute_s_server(&grp, &a_pub, &v_big, &u, &b);
    let k_bytes = compute_k(&s_server);

    // Le salt réel n'est pas re-stocké ici (déjà envoyé en step1) —
    // on doit néanmoins le relire pour calculer M1 côté serveur.
    let salt_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["srp_salt"],
        None,
        Some(1),
    );
    let salt_hex = salt_rows
        .into_iter()
        .next()
        .and_then(|r| r.get("srp_salt").and_then(|v| v.as_str().map(|s| s.to_string())))
        .unwrap_or_default();
    let Some(salt_bytes) = hex_decode(&salt_hex) else {
        respond_json(request, json!({"success":false,"error":"Erreur interne (salt)."}), 500);
        return;
    };

    let m1_expected = compute_m1(&grp, &email, &salt_bytes, &a_pub, &b_pub, &k_bytes);

    if !constant_time_eq(&m1_client, &m1_expected) {
        respond_json(request, json!({"success":false,"error":"Email ou mot de passe incorrect."}), 200);
        return;
    }

    // ── Authentification réussie : preuve serveur + ouverture session ──
    let m2 = compute_m2(&a_pub, &m1_expected, &k_bytes);

    let user_id = user_row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let nom = user_row.get("nom").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let email_db = user_row.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let _ = user_id; // conservé pour lisibilité / usage futur (logs, etc.)

    let cookie_value = generate_session_token(32);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    inserer_ou_modifier(
        pool,
        "loginc",
        &[
            ("idcokier", mysql::Value::from(cookie_value.as_str())),
            ("datecra", mysql::Value::from(now.as_str())),
            ("pc", mysql::Value::from(remote_ip)),
            ("navi", mysql::Value::from(user_agent)),
            ("email", mysql::Value::from(email_db.as_str())),
            ("nom", mysql::Value::from(nom.as_str())),
        ],
        &[],
    );

    let expires = chrono::Local::now() + chrono::Duration::days(30);
    let cookie_str = format!(
        "connexion_cookie={}; Path=/; HttpOnly; SameSite=Strict; Expires={}",
        cookie_value,
        expires.format("%a, %d %b %Y %H:%M:%S GMT")
    );

    let body_json = serde_json::to_string(&json!({
        "success":  true,
        "M2":       hex_encode(&m2),
        "redirect": "/login/dashboard",
    }))
    .unwrap_or_default();

    let _ = request.respond(
        Response::from_string(body_json)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Set-Cookie", cookie_str.as_str()).unwrap()),
    );
}

// ══════════════════════════════════════════════════════════════════
// Inscription — le client envoie salt + verifier (calculés côté JS),
// jamais le mot de passe, jamais un hash équivalent au mot de passe.
// ══════════════════════════════════════════════════════════════════
fn handle_signup(request: Request, pool: &DbPool, config: &VexConfig, body: &HashMap<String, String>) {
    let reg_mode = config.users.registration_mode.as_str();
    let activation_req = config.users.activation_key_required;
    let activation_key = config.users.activation_key.as_str();
    let max_users = config.users.max_users;

    if reg_mode == "closed" {
        respond_json(request, json!({"success":false,"error":"Les inscriptions sont fermées."}), 200);
        return;
    }
    if reg_mode == "invitation" && activation_req {
        let key = body.get("activation_key").map(|s| s.as_str()).unwrap_or("");
        if key != activation_key {
            respond_json(request, json!({"success":false,"error":"Clé d'activation invalide."}), 200);
            return;
        }
    }
    if body.get("scales").is_none() {
        respond_json(
            request,
            json!({"success":false,"error":"Veuillez accepter la politique de confidentialité."}),
            200,
        );
        return;
    }
    if compter_lignes(pool, "login", &[]) >= max_users {
        respond_json(
            request,
            json!({"success":false,"error":format!("Nombre maximum d'utilisateurs ({}) atteint.", max_users)}),
            200,
        );
        return;
    }

    let nom = html_escape(body.get("nom").cloned().unwrap_or_default().trim());
    let email = html_escape(body.get("email").cloned().unwrap_or_default().trim());
    let salt_hex = body.get("srp_salt").cloned().unwrap_or_default();
    let verifier_hex = body.get("srp_verifier").cloned().unwrap_or_default();

    // Validation de forme : salt = 16 octets hex (32 car.), verifier = 256 octets hex (512 car.)
    if salt_hex.len() != 32 || !salt_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Format de salt invalide."}), 400);
        return;
    }
    if verifier_hex.len() > 512 || verifier_hex.is_empty() || !verifier_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Format de verifier invalide."}), 400);
        return;
    }

    let existing = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id"],
        None,
        Some(1),
    );
    let existing_nom = selectionner(
        pool,
        "login",
        &[("nom", mysql::Value::from(nom.as_str()))],
        &["id"],
        None,
        Some(1),
    );

    if !existing.is_empty() || !existing_nom.is_empty() {
        respond_json(request, json!({"success":false,"error":"Ce nom ou cet email existe déjà."}), 200);
        return;
    }

    let result = inserer_ou_modifier(
        pool,
        "login",
        &[
            ("nom", mysql::Value::from(nom.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("srp_salt", mysql::Value::from(salt_hex.as_str())),
            ("srp_verifier", mysql::Value::from(verifier_hex.as_str())),
            ("vip", mysql::Value::from(0i64)),
        ],
        &[],
    );

    if result > 0 {
        respond_json(
            request,
            json!({"success":true,"message":"Inscription réussie. Connectez-vous."}),
            200,
        );
    } else {
        respond_json(request, json!({"success":false,"error":"Erreur lors de l'inscription."}), 200);
    }
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

fn serve_static_html(request: Request, path: &str) {
    match std::fs::read_to_string(path) {
        Ok(html) => {
            // Pas de session avant connexion → thème par défaut "light".
            // (Si tu veux respecter un thème mémorisé pré-connexion, il
            // faudrait un cookie non-HttpOnly dédié — hors scope ici.)
            let html = html.replace("{{THEME}}", "light");
            let _ = request.respond(Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(
                Response::from_string(format!("Fichier introuvable : {}", path)).with_status_code(500),
            );
        }
    }
}

fn respond_json(request: Request, body: serde_json::Value, status: u16) {
    let _ = request.respond(
        Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
            ),
    );
}

fn redirect(request: Request, location: &str) {
    let _ = request.respond(
        Response::empty(302).with_header(tiny_http::Header::from_bytes("Location", location).unwrap()),
    );
}

/// Token de session (cookie) — utilise le générateur cryptographique
/// getrandom (srp::random_bytes), PAS le PRNG faible XorShift qui servait
/// avant ici. Un cookie de session prévisible = compromission totale.
fn generate_session_token(len_bytes: usize) -> String {
    hex_encode(&srp::random_bytes(len_bytes))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn read_body(request: &mut Request) -> HashMap<String, String> {
    let mut body = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);
    let mut map = HashMap::new();
    for pair in body.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            map.insert(url_decode(k), url_decode(v));
        }
    }
    map
}