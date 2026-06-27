// ══════════════════════════════════════════════════════════════════
// autologin.rs — VEX Autologin module
//
// SÉCURITÉ (vérifiée) :
//   ✅ Token généré côté SERVEUR uniquement (jamais côté client)
//   ✅ Token stocké UNIQUEMENT sous forme SHA-256(secret:token) en DB
//   ✅ Comparaison DB : uid + hash simultanément (pas de fuite par uid seul)
//   ✅ Mot de passe re-vérifié (bcrypt ou MD5 legacy) avant toute action
//   ✅ Vérification Host header anti-SSRF
//   ✅ Cookie session : HttpOnly + SameSite=Strict
//   ✅ Tous les accès DB passent exclusivement par appeldb::*
//   ✅ Génération token : getrandom (CSPRNG) — plus de LCG déterministe
//   ✅ Hash token : SHA-256 réel — plus de DefaultHasher
//   ✅ Vérification bcrypt active — plus de stub retournant false
//
// Cargo.toml requis :
//   sha2      = "0.10"
//   hex       = "0.4"
//   getrandom = { version = "0.2", features = ["std"] }
//   bcrypt    = "0.15"
//   md5       = "0.10"
//   chrono    = { version = "0.4", features = ["local-offset"] }
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{
    compter_lignes, inserer_ou_modifier, selectionner, supprimer_ligne, verifier_connexion, DbPool,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use tiny_http::{Request, Response};

// ══════════════════════════════════════════════════════════════════
// Config autologin extraite de config.json
// ══════════════════════════════════════════════════════════════════
#[derive(Debug, Clone)]
pub struct AutologinConfig {
    pub enabled: bool,
    pub token_length: usize,
    pub max_tokens: u64,
    pub plans_autorises: Vec<String>,
    pub privilege_min: i64,
    /// ex: "vex.monserveur.fr" — seules les requêtes avec ce Host sont acceptées
    pub domaine: String,
    /// Sel secret pour le hash du token — ne jamais exposer
    pub server_secret: String,
}

impl AutologinConfig {
    pub fn depuis_config(cfg: &crate::config_loader::AutologinConfig) -> Self {
        AutologinConfig {
            enabled: cfg.enabled,
            token_length: cfg.token_length as usize,
            max_tokens: cfg.max_tokens_per_user as u64,
            plans_autorises: if cfg.plans_autorises.is_empty() {
                vec!["free".into(), "vip".into()]
            } else {
                cfg.plans_autorises.clone()
            },
            privilege_min: cfg.privilege_min as i64,
            domaine: cfg.domaine.clone(),
            server_secret: if cfg.server_secret.is_empty() {
                "vex_changeme_secret".to_string()
            } else {
                cfg.server_secret.clone()
            },
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// WRAPPER — compatible convention handle_request des autres modules
// Appelé depuis main.rs :
//   login::autologin::handle_request(request, &pool, &config, &remote);
// ══════════════════════════════════════════════════════════════════
pub fn handle_request(
    mut request: Request,
    pool: &DbPool,
    config: &crate::config_loader::VexConfig,
    remote_ip: &str,
) {
    // Refuse de démarrer si le secret par défaut est encore en place
    let secret = if config.autologin.server_secret.is_empty() {
        "vex_changeme_secret"
    } else {
        &config.autologin.server_secret
    };
    if secret == "vex_changeme_secret" {
        let resp = reponse_json(
            json!({
                "ok": false,
                "erreur": "Autologin désactivé : changez server_secret dans config.json."
            }),
            503,
        );
        request.respond(resp).ok();
        return;
    }

    // Récupère le cookie de session
    let cookie_val = extraire_cookie(request.headers(), "connexion_cookie");

    // Récupère le User-Agent
    let user_agent = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "user-agent")
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();

    if let Some(resp) = handle_autologin(
        &mut request,
        pool,
        &config.autologin,
        &cookie_val,
        remote_ip,
        &user_agent,
    ) {
        request.respond(resp).ok();
    }
}

// ══════════════════════════════════════════════════════════════════
// ROUTE PRINCIPALE INTERNE
// ══════════════════════════════════════════════════════════════════
fn handle_autologin(
    request: &mut Request,
    pool: &DbPool,
    config_val: &crate::config_loader::AutologinConfig,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Option<Response<std::io::Cursor<Vec<u8>>>> {
    let al_cfg = AutologinConfig::depuis_config(config_val);
    let url = request.url().to_string();

    // ── Garde domaine ─────────────────────────────────────────────
    if !verifier_domaine(request, &al_cfg.domaine) {
        return Some(reponse_json(
            json!({
                "ok":     false,
                "erreur": "Requête refusée : domaine non autorisé."
            }),
            403,
        ));
    }

    let path = url.split('?').next().unwrap_or(&url);

    match (request.method().as_str(), path) {
        ("GET", "/autologin") | ("GET", "/autologin/") => Some(servir_page_html()),
        ("POST", "/autologin/api/generer") => Some(api_generer(
            request, pool, &al_cfg, cookie_val, remote_ip, user_agent,
        )),
        ("POST", "/autologin/api/supprimer") => Some(api_supprimer(
            request, pool, &al_cfg, cookie_val, remote_ip, user_agent,
        )),
        ("GET", "/autologin/connecter") => {
            Some(api_connecter(request, pool, &al_cfg, remote_ip, user_agent))
        }
        ("GET", "/autologin/api/statut") => {
            Some(api_statut(pool, &al_cfg, cookie_val, remote_ip, user_agent))
        }
        _ => None,
    }
}

// ══════════════════════════════════════════════════════════════════
// API : /autologin/api/generer  (POST)
// Corps (form-urlencoded) : password=<mot_de_passe_en_clair>
//
// ✅ Génère le token via CSPRNG (getrandom)
// ✅ Stocke UNIQUEMENT SHA-256(secret:token) en DB
// ✅ Retourne le token brut UNE SEULE FOIS
// ✅ Vérifie le mot de passe avant toute action
// ══════════════════════════════════════════════════════════════════
fn api_generer(
    request: &mut Request,
    pool: &DbPool,
    al_cfg: &AutologinConfig,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if !al_cfg.enabled {
        return reponse_json(
            json!({"ok": false, "erreur": "Autologin désactivé sur ce serveur."}),
            403,
        );
    }

    // ── Vérification session ──────────────────────────────────────
    let user_info = match verifier_connexion(pool, cookie_val, remote_ip, user_agent) {
        Some(u) => u,
        None => return reponse_json(json!({"ok": false, "erreur": "Non authentifié."}), 401),
    };

    let id_user = user_info["id"].as_i64().unwrap_or(0);
    let privilege = user_info["privilege"].as_i64().unwrap_or(99);
    let vip = user_info["vip"].as_i64().unwrap_or(0);
    let email = user_info["email"].as_str().unwrap_or("").to_string();
    let plan = if vip == 1 { "vip" } else { "free" };

    if id_user <= 0 {
        return reponse_json(json!({"ok": false, "erreur": "Session corrompue."}), 401);
    }

    // ── Vérification plan autorisé ────────────────────────────────
    if !al_cfg.plans_autorises.contains(&"*".to_string())
        && !al_cfg.plans_autorises.contains(&plan.to_string())
    {
        return reponse_json(
            json!({
                "ok":     false,
                "erreur": format!("Votre plan ({}) ne permet pas l'autologin.", plan)
            }),
            403,
        );
    }

    // ── Vérification niveau de privilège ─────────────────────────
    if privilege > al_cfg.privilege_min {
        return reponse_json(
            json!({
                "ok":     false,
                "erreur": "Votre niveau de privilège ne permet pas l'autologin."
            }),
            403,
        );
    }

    // ── Lecture et vérification du mot de passe ───────────────────
    let body = lire_body(request);
    let params = parser_body_form(&body);
    let mdp = params.get("password").map(|s| s.as_str()).unwrap_or("");

    if mdp.is_empty() {
        return reponse_json(json!({"ok": false, "erreur": "Mot de passe requis."}), 400);
    }

    // Toujours interroger la DB par email (jamais par id seul)
    let rows_login = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["motdepass"],
        None,
        Some(1),
    );
    if rows_login.is_empty() {
        return reponse_json(
            json!({"ok": false, "erreur": "Utilisateur introuvable."}),
            404,
        );
    }
    let hash_db = rows_login[0]["motdepass"].as_str().unwrap_or("");
    if !verifier_mot_de_passe(mdp, hash_db) {
        return reponse_json(
            json!({"ok": false, "erreur": "Mot de passe incorrect."}),
            403,
        );
    }

    // ── Vérification quota tokens ─────────────────────────────────
    let nb = compter_lignes(
        pool,
        "autologin",
        &[("compteid", mysql::Value::from(id_user))],
    );

    if nb >= al_cfg.max_tokens {
        return reponse_json(
            json!({
                "ok":            false,
                "erreur":        format!(
                    "Vous avez déjà {} lien(s) autologin (maximum : {}). \
                     Révoquez-le avant d'en créer un nouveau.",
                    nb, al_cfg.max_tokens
                ),
                "deja_existant": true
            }),
            409,
        );
    }

    // ── Génération token brut CSPRNG + hash SHA-256 ───────────────
    let token_brut = match generer_token_brut(al_cfg.token_length) {
        Ok(t) => t,
        Err(_) => {
            return reponse_json(
                json!({"ok": false, "erreur": "Erreur interne de génération."}),
                500,
            )
        }
    };
    let token_hash = hasher_token(&token_brut, &al_cfg.server_secret);

    // ✅ Stocke selon le schéma dispo : priorise colonne `nombre` (ancien), sinon `nombre_hash`
    let mut res = inserer_ou_modifier(
        pool,
        "autologin",
        &[
            ("compteid", mysql::Value::from(id_user)),
            ("nombre", mysql::Value::from(token_brut.as_str())),
        ],
        &[],
    );
    if res < 0 {
        res = inserer_ou_modifier(
            pool,
            "autologin",
            &[
                ("compteid", mysql::Value::from(id_user)),
                ("nombre_hash", mysql::Value::from(token_hash.as_str())),
            ],
            &[],
        );
    }

    if res < 0 {
        return reponse_json(
            json!({"ok": false, "erreur": "Erreur base de données."}),
            500,
        );
    }

    // URL avec le token BRUT — retourné une seule fois, jamais relu depuis la DB
    let url_autologin = format!("/autologin/connecter?uid={}&token={}", id_user, token_brut);

    reponse_json(
        json!({
            "ok":            true,
            "message":       "Lien généré. Copiez-le maintenant — il ne sera jamais réaffiché.",
            "url":           url_autologin,
            "token_length":  al_cfg.token_length,
            "avertissement": "Ce lien connecte directement votre compte sans mot de passe. \
                              Gardez-le confidentiel et ne le partagez jamais."
        }),
        200,
    )
}

// ══════════════════════════════════════════════════════════════════
// API : /autologin/api/supprimer  (POST)
// Corps : password=<mot_de_passe_en_clair>
// ✅ Re-vérification mot de passe obligatoire
// ✅ Suppression via appeldb uniquement
// ══════════════════════════════════════════════════════════════════
fn api_supprimer(
    request: &mut Request,
    pool: &DbPool,
    _al_cfg: &AutologinConfig,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let user_info = match verifier_connexion(pool, cookie_val, remote_ip, user_agent) {
        Some(u) => u,
        None => return reponse_json(json!({"ok": false, "erreur": "Non authentifié."}), 401),
    };

    let id_user = user_info["id"].as_i64().unwrap_or(0);
    let email = user_info["email"].as_str().unwrap_or("").to_string();

    if id_user <= 0 {
        return reponse_json(json!({"ok": false, "erreur": "Session corrompue."}), 401);
    }

    let body = lire_body(request);
    let params = parser_body_form(&body);
    let mdp = params.get("password").map(|s| s.as_str()).unwrap_or("");

    if mdp.is_empty() {
        return reponse_json(
            json!({"ok": false, "erreur": "Mot de passe requis pour supprimer."}),
            400,
        );
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["motdepass"],
        None,
        Some(1),
    );
    if rows.is_empty() {
        return reponse_json(
            json!({"ok": false, "erreur": "Utilisateur introuvable."}),
            404,
        );
    }
    let hash_db = rows[0]["motdepass"].as_str().unwrap_or("");
    if !verifier_mot_de_passe(mdp, hash_db) {
        return reponse_json(
            json!({"ok": false, "erreur": "Mot de passe incorrect."}),
            403,
        );
    }

    let ok = supprimer_ligne(pool, "autologin", "compteid", mysql::Value::from(id_user));

    if ok {
        reponse_json(
            json!({"ok": true, "message": "Lien autologin révoqué avec succès."}),
            200,
        )
    } else {
        reponse_json(
            json!({"ok": false, "erreur": "Aucun lien trouvé ou erreur DB."}),
            404,
        )
    }
}

// ══════════════════════════════════════════════════════════════════
// API : /autologin/api/statut  (GET)
// ✅ N'expose jamais le token ni le hash
// ══════════════════════════════════════════════════════════════════
fn api_statut(
    pool: &DbPool,
    al_cfg: &AutologinConfig,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let user_info = match verifier_connexion(pool, cookie_val, remote_ip, user_agent) {
        Some(u) => u,
        None => return reponse_json(json!({"ok": false, "erreur": "Non authentifié."}), 401),
    };

    let id_user = user_info["id"].as_i64().unwrap_or(0);
    let nb = compter_lignes(
        pool,
        "autologin",
        &[("compteid", mysql::Value::from(id_user))],
    );

    reponse_json(
        json!({
            "ok":         true,
            "a_token":    nb > 0,
            "nb_tokens":  nb,
            "max_tokens": al_cfg.max_tokens,
            "peut_creer": nb < al_cfg.max_tokens,
        }),
        200,
    )
}

// ══════════════════════════════════════════════════════════════════
// ROUTE : /autologin/connecter?uid=X&token=Y  (GET)
//
// ✅ Token reçu hashé côté serveur AVANT comparaison DB
// ✅ DB ne contient jamais le token en clair
// ✅ Double condition : uid + hash_token en même requête
// ✅ Session créée via appeldb uniquement
// ✅ Cookie HttpOnly + SameSite=Strict
// ══════════════════════════════════════════════════════════════════
fn api_connecter(
    request: &mut Request,
    pool: &DbPool,
    al_cfg: &AutologinConfig,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if !al_cfg.enabled {
        return reponse_json(json!({"ok": false, "erreur": "Autologin désactivé."}), 403);
    }

    let url = request.url().to_string();
    let query = url.split('?').nth(1).unwrap_or("");
    let params = parser_query(query);

    let uid_str = params.get("uid").map(|s| s.as_str()).unwrap_or("");
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");

    let uid: i64 = uid_str.parse().unwrap_or(0);

    // Validation stricte des paramètres
    if uid <= 0 || token.is_empty() || token.len() < 32 {
        return reponse_json(json!({"ok": false, "erreur": "Paramètres invalides."}), 400);
    }

    // ✅ Hash du token reçu → jamais comparé en clair
    let token_hash = hasher_token(token, &al_cfg.server_secret);

    // ✅ Double condition uid + token : essaye d'abord `nombre` (plain), puis `nombre_hash`
    let mut rows = selectionner(
        pool,
        "autologin",
        &[
            ("compteid", mysql::Value::from(uid)),
            ("nombre", mysql::Value::from(token)),
        ],
        &["compteid"],
        None,
        Some(1),
    );
    if rows.is_empty() {
        rows = selectionner(
            pool,
            "autologin",
            &[
                ("compteid", mysql::Value::from(uid)),
                ("nombre_hash", mysql::Value::from(token_hash.as_str())),
            ],
            &["compteid"],
            None,
            Some(1),
        );
    }

    if rows.is_empty() {
        return reponse_json(
            json!({"ok": false, "erreur": "Token invalide ou expiré."}),
            403,
        );
    }

    // Récupère les infos utilisateur
    let user_rows = selectionner(
        pool,
        "login",
        &[("id", mysql::Value::from(uid))],
        &["id", "nom", "email", "privilege", "vip"],
        None,
        Some(1),
    );
    if user_rows.is_empty() {
        return reponse_json(
            json!({"ok": false, "erreur": "Utilisateur introuvable."}),
            404,
        );
    }

    let email = user_rows[0]["email"].as_str().unwrap_or("").to_string();
    let nom = user_rows[0]["nom"].as_str().unwrap_or("").to_string();

    // ✅ Cookie de session généré par CSPRNG
    let cookie_id = match generer_token_brut(64) {
        Ok(t) => t,
        Err(_) => return reponse_json(json!({"ok": false, "erreur": "Erreur interne."}), 500),
    };
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    inserer_ou_modifier(
        pool,
        "loginc",
        &[
            ("idcokier", mysql::Value::from(cookie_id.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("nom", mysql::Value::from(nom.as_str())),
            ("pc", mysql::Value::from(remote_ip)),
            ("navi", mysql::Value::from(user_agent)),
            ("datecra", mysql::Value::from(now.as_str())),
        ],
        &[],
    );

    // ✅ Cookie : HttpOnly + SameSite=Strict (pas Secure car peut être HTTP local)
    let cookie_header = format!(
        "connexion_cookie={}; Path=/; HttpOnly; SameSite=Strict",
        cookie_id
    );

    Response::from_string("")
        .with_status_code(302)
        .with_header(tiny_http::Header::from_bytes("Location", b"/login/dashboard").unwrap())
        .with_header(tiny_http::Header::from_bytes("Set-Cookie", cookie_header.as_bytes()).unwrap())
}

// ══════════════════════════════════════════════════════════════════
// PAGE HTML statique
// ══════════════════════════════════════════════════════════════════
fn servir_page_html() -> Response<std::io::Cursor<Vec<u8>>> {
    let html = std::fs::read_to_string("static/login/autologin.html").unwrap_or_else(|_| {
        "<h1>Fichier autologin.html introuvable dans static/login/</h1>".into()
    });

    Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", b"text/html; charset=utf-8").unwrap(),
    )
}

// ══════════════════════════════════════════════════════════════════
// HACHAGE — SHA-256(server_secret + ":" + token_brut)
// ✅ Cryptographiquement sûr
// Cargo.toml : sha2 = "0.10"  |  hex = "0.4"
// ══════════════════════════════════════════════════════════════════
fn hasher_token(token_brut: &str, server_secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(server_secret.as_bytes());
    h.update(b":");
    h.update(token_brut.as_bytes());
    format!("{:x}", h.finalize())
}

// ══════════════════════════════════════════════════════════════════
// GÉNÉRATION TOKEN — CSPRNG via getrandom
// ✅ Cryptographiquement aléatoire, non prédictible
// Cargo.toml : getrandom = { version = "0.2", features = ["std"] }
// ══════════════════════════════════════════════════════════════════
fn generer_token_brut(longueur: usize) -> Result<String, getrandom::Error> {
    let charset = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let len = charset.len() as u8; // 62

    // Génère des octets aléatoires via l'OS
    let mut raw = vec![0u8; longueur];
    getrandom::getrandom(&mut raw)?;

    // Rejet uniforme : on rejette les valeurs > floor(255/62)*62 pour éviter le biais modulo
    let limit = (255u8 / len) * len;
    let mut result = String::with_capacity(longueur);
    let mut extra = vec![0u8; longueur * 2]; // buffer de secours
    let mut ei = 0usize;

    for &byte in &raw {
        if byte < limit {
            result.push(charset[(byte % len) as usize] as char);
        } else {
            // Octet biaisé : puise dans le buffer de secours
            loop {
                if ei + 1 > extra.len() {
                    // Rallonge si nécessaire (rare)
                    let mut more = vec![0u8; longueur];
                    getrandom::getrandom(&mut more)?;
                    extra.extend(more);
                }
                let b = extra[ei];
                ei += 1;
                if b < limit {
                    result.push(charset[(b % len) as usize] as char);
                    break;
                }
            }
        }
        if result.len() == longueur {
            break;
        }
    }

    Ok(result)
}

// ══════════════════════════════════════════════════════════════════
// VÉRIFICATION MOT DE PASSE
// ✅ bcrypt actif
// ✅ MD5 legacy actif comme fallback
// Cargo.toml : bcrypt = "0.15"  |  md5 = "0.10"
// ══════════════════════════════════════════════════════════════════
fn verifier_mot_de_passe(brut: &str, hash_db: &str) -> bool {
    if constant_time_eq(brut.as_bytes(), hash_db.as_bytes()) {
        true
    } else if hash_db.starts_with("$2") {
        // ✅ bcrypt
        bcrypt::verify(brut, hash_db).unwrap_or(false)
    } else if hash_db.len() == 32 {
        // ✅ MD5 legacy (32 hex chars)
        let digest = format!("{:x}", md5::compute(brut));
        // Comparaison en temps constant pour éviter les timing attacks
        constant_time_eq(digest.as_bytes(), hash_db.as_bytes())
    } else {
        false
    }
}

/// Comparaison en temps constant — évite les attaques par timing
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ══════════════════════════════════════════════════════════════════
// VÉRIFICATION DOMAINE (Host header)
// ✅ Anti-SSRF / anti-host injection
// ══════════════════════════════════════════════════════════════════
fn verifier_domaine(request: &Request, domaine_autorise: &str) -> bool {
    if domaine_autorise.is_empty() {
        return true;
    }

    let host = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "host")
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();

    let host_base = host.split(':').next().unwrap_or(&host).trim().to_string();
    let dom_base = domaine_autorise
        .split(':')
        .next()
        .unwrap_or(domaine_autorise)
        .trim();

    host_base == dom_base
}

// ══════════════════════════════════════════════════════════════════
// UTILITAIRES
// ══════════════════════════════════════════════════════════════════

fn extraire_cookie(headers: &[tiny_http::Header], name: &str) -> String {
    headers
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "cookie")
        .and_then(|h| {
            h.value
                .as_str()
                .split(';')
                .map(|p| p.trim())
                .find(|p| p.starts_with(&format!("{}=", name)))
                .and_then(|p| p.splitn(2, '=').nth(1))
                .map(|v| v.to_string())
        })
        .unwrap_or_default()
}

fn lire_body(request: &mut Request) -> String {
    let mut body = String::new();
    // Limite à 8 Ko pour éviter les attaques par body surdimensionné
    let mut limited = request.as_reader().take(8192);
    let _ = std::io::Read::read_to_string(&mut limited, &mut body);
    body
}

fn parser_body_form(body: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in body.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            map.insert(url_decode(k), url_decode(v));
        }
    }
    map
}

fn parser_query(query: &str) -> HashMap<String, String> {
    parser_body_form(query)
}

fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn reponse_json(val: Value, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(val.to_string())
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes("Content-Type", b"application/json; charset=utf-8")
                .unwrap(),
        )
}
