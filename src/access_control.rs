// ══════════════════════════════════════════════════════════════════
// access_control.rs — VEX contrôle d'accès
// 0 SQL direct — tout passe par appeldb:: et c::
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::c::{verifier_blocage, verifier_session, SessionInfo};
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
