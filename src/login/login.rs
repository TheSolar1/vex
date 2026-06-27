// ══════════════════════════════════════════════════════════════════
// login/login.rs — VEX Login + Inscription
// - Le mot de passe N'EST JAMAIS reçu en clair côté serveur
// - Le client envoie un hash PBKDF2-SHA256(mdp + salt_minute)
// - Le serveur stocke ce même hash en DB (inscription) et compare (login)
// - Le mdp en clair reste en mémoire JS côté client pour chiffrer les fichiers
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{compter_lignes, inserer_ou_modifier, selectionner, DbPool};
use crate::config_loader::VexConfig;
use crate::utils::{strip_port, url_decode};
use serde_json::json;
use std::collections::HashMap;
use tiny_http::{Request, Response};

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
        if config
            .security
            .ip_blacklist
            .iter()
            .any(|ip| ip == &remote_ip)
        {
            respond_json(
                request,
                json!({"success":false,"error":"Accès refusé : IP bloquée."}),
                403,
            );
            return;
        }
    }
    if config.security.ip_whitelist_enabled && !config.security.ip_whitelist.is_empty() {
        if !config
            .security
            .ip_whitelist
            .iter()
            .any(|ip| ip == &remote_ip)
        {
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
        "login" => handle_login(request, pool, &body, &remote_ip, &user_agent),
        "signup" => handle_signup(request, pool, config, &body),
        _ => respond_json(
            request,
            json!({"success":false,"error":"Action inconnue"}),
            400,
        ),
    }
}

// ── Connexion ────────────────────────────────────────────────────
// Le champ "motdepass" reçu ici est un hash PBKDF2-SHA256 hex (64 car.)
// produit par le client — jamais le mot de passe en clair.
fn handle_login(
    request: Request,
    pool: &DbPool,
    body: &HashMap<String, String>,
    remote_ip: &str,
    user_agent: &str,
) {
    let email = body.get("email").cloned().unwrap_or_default();
    let hash_recu = body.get("motdepass").cloned().unwrap_or_default();

    // Validation basique : le hash PBKDF2 hex fait toujours 64 caractères
    if hash_recu.len() < 32 || hash_recu.len() > 128 || !hash_recu.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(
            request,
            json!({"success":false,"error":"Format d'authentification invalide."}),
            400,
        );
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id", "nom", "email", "motdepass", "vip", "privilege"],
        None,
        Some(1),
    );

    if rows.is_empty() {
        // Réponse délibérément vague pour ne pas leaker l'existence du compte
        respond_json(
            request,
            json!({"success":false,"error":"Email ou mot de passe incorrect."}),
            200,
        );
        return;
    }

    let row = &rows[0];
    let hash_db = row.get("motdepass").and_then(|v| v.as_str()).unwrap_or("");
    let nom = row
        .get("nom")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let email_db = row
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // ── Comparaison hash DB vs hash reçu ─────────────────────────
    // Le hash en DB est stocké tel quel (hex 64 car.) depuis l'inscription.
    // Pas de bcrypt serveur — la sécurité est assurée par PBKDF2 côté client.
    // Comparaison en temps constant pour éviter les timing attacks.
    if !constant_time_eq(hash_recu.as_bytes(), hash_db.as_bytes()) {
        respond_json(
            request,
            json!({"success":false,"error":"Email ou mot de passe incorrect."}),
            200,
        );
        return;
    }

    let cookie_value = generate_token(32);
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
        "connexion_cookie={}; Path=/; HttpOnly; Expires={}",
        cookie_value,
        expires.format("%a, %d %b %Y %H:%M:%S GMT")
    );

    let body_json = serde_json::to_string(&json!({
        "success":  true,
        "redirect": "/login/dashboard",
    }))
    .unwrap_or_default();

    let _ = request.respond(
        Response::from_string(body_json)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Set-Cookie", cookie_str.as_str()).unwrap()),
    );
}

// ── Inscription ──────────────────────────────────────────────────
// Le champ "motdepass" reçu ici est un hash PBKDF2-SHA256 hex (64 car.)
// On le stocke directement en DB — pas de double hachage serveur.
fn handle_signup(
    request: Request,
    pool: &DbPool,
    config: &VexConfig,
    body: &HashMap<String, String>,
) {
    let reg_mode = config.users.registration_mode.as_str();
    let activation_req = config.users.activation_key_required;
    let activation_key = config.users.activation_key.as_str();
    let max_users = config.users.max_users;

    if reg_mode == "closed" {
        respond_json(
            request,
            json!({"success":false,"error":"Les inscriptions sont fermées."}),
            200,
        );
        return;
    }
    if reg_mode == "invitation" && activation_req {
        let key = body.get("activation_key").map(|s| s.as_str()).unwrap_or("");
        if key != activation_key {
            respond_json(
                request,
                json!({"success":false,"error":"Clé d'activation invalide."}),
                200,
            );
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
    let hash_mdp = body.get("motdepass").cloned().unwrap_or_default();

    // Validation du hash PBKDF2 reçu
    if hash_mdp.len() != 64 || !hash_mdp.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(
            request,
            json!({"success":false,"error":"Format d'authentification invalide."}),
            400,
        );
        return;
    }

    // Note : la longueur minimale du mot de passe est vérifiée CÔTÉ CLIENT
    // avant le hachage. Le serveur ne peut pas vérifier la longueur du mdp
    // original à partir du hash — c'est intentionnel.

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
        respond_json(
            request,
            json!({"success":false,"error":"Ce nom ou cet email existe déjà."}),
            200,
        );
        return;
    }

    // On stocke le hash PBKDF2 tel quel en DB (hex 64 car.)
    let result = inserer_ou_modifier(
        pool,
        "login",
        &[
            ("nom", mysql::Value::from(nom.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("motdepass", mysql::Value::from(hash_mdp.as_str())),
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
        respond_json(
            request,
            json!({"success":false,"error":"Erreur lors de l'inscription."}),
            200,
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Comparaison en temps constant pour éviter les timing attacks.
/// Retourne true si les deux slices sont identiques.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn serve_static_html(request: Request, path: &str) {
    match std::fs::read_to_string(path) {
        Ok(html) => {
            let _ = request.respond(Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(
                Response::from_string(format!("Fichier introuvable : {}", path))
                    .with_status_code(500),
            );
        }
    }
}

fn respond_json(request: Request, body: serde_json::Value, status: u16) {
    let _ = request.respond(
        Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
    );
}

fn redirect(request: Request, location: &str) {
    let _ = request.respond(
        Response::empty(302)
            .with_header(tiny_http::Header::from_bytes("Location", location).unwrap()),
    );
}

fn generate_token(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(42);
    let mut state = seed ^ 0x9e3779b97f4a7c15;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            format!("{:02x}", (state & 0xFF) as u8)
        })
        .collect()
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
