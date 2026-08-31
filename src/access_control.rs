// ══════════════════════════════════════════════════════════════════
// access_control.rs — VEX contrôle d'accès
// 0 SQL direct — tout passe par appeldb:: et c::
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::c::{verifier_blocage, verifier_session, SessionInfo};
use crate::config_loader::VexConfig;
use crate::utils::strip_port;
use tiny_http::Request;

pub enum AccessResult {
    Ok(SessionInfo),
    NotConnected,
    Forbidden,
    Blocked,
}

pub fn check_access(
    pool: &DbPool,
    request: &Request,
    current_path: &str,
    privilege_max: i64,
) -> AccessResult {
    let cookie_val = get_cookie(request, "connexion_cookie");
    let remote_raw = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let remote_ip = strip_port(&remote_raw);
    let user_agent = get_header(request, "User-Agent");

    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);

    if !session.connecte {
        return AccessResult::NotConnected;
    }

    if privilege_max > 0 && session.user_privilege > privilege_max {
        return AccessResult::Forbidden;
    }

    if verifier_blocage(pool, session.user_id, session.user_privilege, current_path) {
        return AccessResult::Blocked;
    }

    AccessResult::Ok(session)
}

pub fn check_connected(pool: &DbPool, request: &Request) -> Option<SessionInfo> {
    let cookie_val = get_cookie(request, "connexion_cookie");
    let remote_raw = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let remote_ip = strip_port(&remote_raw);
    let user_agent = get_header(request, "User-Agent");
    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
    if session.connecte {
        Some(session)
    } else {
        None
    }
}

pub fn is_admin(session: &SessionInfo) -> bool {
    session.user_privilege <= 3
}
pub fn is_moderator(session: &SessionInfo) -> bool {
    session.user_privilege <= 6
}

pub fn redirect_to_login(request: tiny_http::Request) {
    let _ = request.respond(
        tiny_http::Response::empty(302)
            .with_header(tiny_http::Header::from_bytes("Location", "/login").unwrap()),
    );
}

pub fn respond_403(request: tiny_http::Request, message: &str) {
    let html = format!(
        r#"<!DOCTYPE html><html lang="fr"><head><meta charset="UTF-8"><title>403 — VEX</title></head>
<body style="font-family:monospace;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#0a0a0a;color:#4caf50;">
<div style="text-align:center">
<div style="font-size:6rem;font-weight:900">403</div>
<p style="opacity:.6;margin-top:1rem;">{}</p>
<a href="/" style="color:#4caf50;margin-top:2rem;display:block;">← Retour</a>
</div></body></html>"#,
        message
    );
    let _ = request.respond(
        tiny_http::Response::from_string(html)
            .with_status_code(403)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ),
    );
}

pub fn handle_access_result_or_continue(
    result: AccessResult,
    request: tiny_http::Request,
) -> Option<SessionInfo> {
    match result {
        AccessResult::Ok(session) => Some(session),
        AccessResult::NotConnected => {
            redirect_to_login(request);
            None
        }
        AccessResult::Forbidden => {
            respond_403(request, "Privilege insuffisant");
            None
        }
        AccessResult::Blocked => {
            respond_403(request, "Accès refusé pour ce compte");
            None
        }
    }
}

// ── Utilitaires HTTP ───────────────────────────────────────────────

pub fn get_cookie(request: &Request, name: &str) -> String {
    for h in request.headers() {
        if h.field.to_string().to_lowercase() == "cookie" {
            for part in h.value.as_str().split(';') {
                let mut kv = part.trim().splitn(2, '=');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    if k.trim() == name {
                        return v.trim().to_string();
                    }
                }
            }
        }
    }
    String::new()
}

pub fn get_header(request: &Request, name: &str) -> String {
    let nl = name.to_lowercase();
    for h in request.headers() {
        if h.field.to_string().to_lowercase() == nl {
            return h.value.as_str().to_string();
        }
    }
    String::new()
}

// ══════════════════════════════════════════════════════════════════
// EXTENSIONS — passerelle de permissions
// Sert /ext/<id> et /api/ext/<id>.
// L'extension n'est appelée qu'après validation :
//   1. extensions.enabled global
//   2. l'id existe dans extensions.extension_params
//   3. entrée .enabled
//   4. privilège utilisateur <= privilege_min de l'extension
//   5. plan utilisateur dans plans_autorises
//   6. aucun blocage bloqpage sur le chemin
// ══════════════════════════════════════════════════════════════════

/// Extrait l'id d'extension depuis /ext/<id>/... ou /api/ext/<id>/...
pub fn extension_id_depuis_path(path: &str) -> String {
    let reste = if let Some(r) = path.strip_prefix("/api/ext/") {
        r
    } else if let Some(r) = path.strip_prefix("/ext/") {
        r
    } else {
        return String::new();
    };
    reste
        .split('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Le plan de l'utilisateur est-il dans la liste autorisée ?
/// La colonne `vip` est tantôt numérique (0 = free), tantôt un id de plan
/// ("free", "vip") selon l'endroit du code : les deux formes sont acceptées.
/// Liste vide ou contenant "*" = tous les plans.
pub fn plan_autorise(plans_autorises: &[String], user_vip: i64) -> bool {
    if plans_autorises.is_empty() || plans_autorises.iter().any(|p| p == "*") {
        return true;
    }
    let plan_effectif = if user_vip == 0 { "free" } else { "vip" };
    plans_autorises.iter().any(|p| {
        let p = p.trim();
        p.eq_ignore_ascii_case(plan_effectif) || p == user_vip.to_string()
    })
}

/// Réponse d'erreur JSON ou HTML selon que le chemin est une API ou une page.
fn ext_refus(
    path: &str,
    code: u16,
    message: &str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if path.starts_with("/api/") {
        let body = serde_json::json!({ "success": false, "error": message }).to_string();
        tiny_http::Response::from_string(body)
            .with_status_code(code)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            )
    } else {
        let html = format!(
            r#"<!DOCTYPE html><html lang="fr"><head><meta charset="UTF-8"><title>{} — VEX</title></head>
<body style="font-family:monospace;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#0a0a0a;color:#4caf50;">
<div style="text-align:center">
<div style="font-size:6rem;font-weight:900">{}</div>
<p style="opacity:.6;margin-top:1rem;">{}</p>
<a href="/login/dashboard" style="color:#4caf50;margin-top:2rem;display:block;">← Retour</a>
</div></body></html>"#,
            code, code, message
        );
        tiny_http::Response::from_string(html)
            .with_status_code(code)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            )
    }
}

/// Point d'entrée unique des extensions uploadées depuis le panel admin.
pub fn servir_extension(
    pool: &DbPool,
    config: &VexConfig,
    mut request: tiny_http::Request,
    path: &str,
) {
    let id = extension_id_depuis_path(path);

    if !config.extensions.enabled {
        let _ = request.respond(ext_refus(path, 503, "Les extensions sont désactivées."));
        return;
    }
    if id.is_empty() {
        let _ = request.respond(ext_refus(path, 404, "Extension non spécifiée."));
        return;
    }

    let entree = match config.extensions.extension_params.get(&id) {
        Some(e) => e.clone(),
        None => {
            let _ = request.respond(ext_refus(
                path,
                404,
                &format!("Extension « {} » inconnue.", id),
            ));
            return;
        }
    };

    if !entree
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let _ = request.respond(ext_refus(
            path,
            503,
            &format!("Extension « {} » désactivée.", id),
        ));
        return;
    }

    // ── Session ───────────────────────────────────────────────────
    let session = match check_connected(pool, &request) {
        Some(s) => s,
        None => {
            if path.starts_with("/api/") {
                let _ = request.respond(ext_refus(path, 401, "Non connecté."));
            } else {
                redirect_to_login(request);
            }
            return;
        }
    };

    // ── Privilège ─────────────────────────────────────────────────
    let privilege_min = entree
        .get("privilege_min")
        .and_then(|v| v.as_i64())
        .unwrap_or(10);
    if session.user_privilege > privilege_min {
        let _ = request.respond(ext_refus(
            path,
            403,
            "Privilège insuffisant pour cette extension.",
        ));
        return;
    }

    // ── Plan ──────────────────────────────────────────────────────
    let plans: Vec<String> = entree
        .get("plans_autorises")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if config.plans.enforce_plan_restrictions && !plan_autorise(&plans, session.user_vip) {
        let _ = request.respond(ext_refus(
            path,
            403,
            "Votre plan ne donne pas accès à cette extension.",
        ));
        return;
    }

    // ── Blocage bloqpage ──────────────────────────────────────────
    if verifier_blocage(pool, session.user_id, session.user_privilege, path) {
        let _ = request.respond(ext_refus(path, 403, "Accès refusé pour ce compte."));
        return;
    }

    // ── Dispatch vers le code compilé ─────────────────────────────
    if let Some(resp) = crate::extensions::dispatch(&id, pool, &session, &mut request) {
        let _ = request.respond(resp);
        return;
    }

    // ── Repli : l'extension n'est pas (encore) compilée dans le
    // binaire en cours. On sert directement son dossier statique
    // static/extensions/<id>/ pour qu'elle soit utilisable tout de
    // suite, sans attendre une recompilation.
    if let Some(resp) = servir_statique_extension_habillee(pool, &session, &request, &id, path) {
        let _ = request.respond(resp);
        return;
    }

    let _ = request.respond(ext_refus(
        path,
        501,
        &format!(
            "Extension « {} » enregistrée mais ni compilée ni pourvue de \
             static/extensions/{}/index.html.",
            id, id
        ),
    ));
}

/// Barre de navigation VEX pour une page d'extension.
/// Les extensions n'ont qu'a poser __NAV_HTML__ dans leur HTML :
/// le placeholder est rempli ici, comme pour les pages integrees.
pub fn nav_extension(
    pool: &DbPool,
    session: &SessionInfo,
    request: &Request,
    ext_id: &str,
) -> String {
    let remote_raw = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let ctx = crate::function::NavContext {
        pool,
        user_id: Some(session.user_id),
        page_key: "extension",
        cookie_val: &get_cookie(request, "connexion_cookie"),
        remote_ip: &strip_port(&remote_raw),
        user_agent: &get_header(request, "User-Agent"),
        query_id: None,
        apps: vec![],
        admin_apps: vec![],
    };
    let _ = ext_id;
    crate::function::build_nav_html(&ctx)
}

/// Remplit les placeholders communs d'une page d'extension.
fn habiller_page(html: &str, nav: &str, theme: &str, langue: &str) -> String {
    html.replace("__NAV_HTML__", nav)
        .replace("__THEME__", theme)
        .replace("__LANG__", langue)
}

/// Type MIME d'apres l'extension du fichier.
fn mime_de(nom: &str) -> &'static str {
    match nom.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Repli statique, avec la barre de navigation injectee dans le HTML.
fn servir_statique_extension_habillee(
    pool: &DbPool,
    session: &SessionInfo,
    request: &tiny_http::Request,
    id: &str,
    path: &str,
) -> Option<tiny_http::Response<std::io::Cursor<Vec<u8>>>> {
    let (donnees, mime) = lire_fichier_extension(id, path)?;
    if !mime.starts_with("text/html") {
        return Some(tiny_http::Response::from_data(donnees).with_header(
            tiny_http::Header::from_bytes("Content-Type", mime).unwrap(),
        ));
    }
    let brut = String::from_utf8_lossy(&donnees).to_string();
    let prefs = crate::function::get_user_preferences(pool, session.user_id);
    let theme = if prefs.teme == 1 { "dark" } else { "light" };
    let nav = nav_extension(pool, session, request, id);
    let html = habiller_page(&brut, &nav, theme, &prefs.langue);
    Some(
        tiny_http::Response::from_string(html).with_header(
            tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
        ),
    )
}

/// Sert un fichier de static/extensions/<id>/.
/// /ext/<id>            -> index.html
/// /ext/<id>/x/y.css    -> x/y.css
fn lire_fichier_extension(id: &str, path: &str) -> Option<(Vec<u8>, &'static str)> {
    let reste = path
        .trim_start_matches("/api/ext/")
        .trim_start_matches("/ext/")
        .trim_start_matches(id)
        .trim_start_matches('/');
    let sous = if reste.is_empty() { "index.html" } else { reste };

    // Pas de remontee de dossier.
    if sous.contains("..") || sous.contains('\\') || sous.starts_with('/') {
        return None;
    }

    let mut chemin = std::path::PathBuf::from("static/extensions");
    chemin.push(id);
    for seg in sous.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        chemin.push(seg);
    }

    let donnees = std::fs::read(&chemin).ok()?;
    Some((donnees, mime_de(sous)))
}

/// Autorisation interne a une extension.
/// L'extension declare ses propres actions dans config.json :
///   extension_params.<id>.permissions.<action> =
///       { "privilege_min": 6, "plans_autorises": ["vip"] }
/// et appelle ce helper depuis son code :
///   if !ext_permission_ok(config, "monchat", "moderer", session) { ... }
/// Une action absente est autorisee si l'utilisateur passe deja le
/// controle global de l'extension.
pub fn ext_permission_ok(
    config: &VexConfig,
    ext_id: &str,
    action: &str,
    session: &SessionInfo,
) -> bool {
    let entree = match config.extensions.extension_params.get(ext_id) {
        Some(e) => e,
        None => return false,
    };
    let regle = match entree.get("permissions").and_then(|p| p.get(action)) {
        Some(r) => r,
        None => return true,
    };
    let priv_min = regle
        .get("privilege_min")
        .and_then(|v| v.as_i64())
        .unwrap_or(12);
    if session.user_privilege > priv_min {
        return false;
    }
    let plans: Vec<String> = regle
        .get("plans_autorises")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    plan_autorise(&plans, session.user_vip)
}
