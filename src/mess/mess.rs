// ══════════════════════════════════════════════════════════════════
// src/mess/mess.rs — Messagerie chiffrée VEX
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{selectionner, inserer_ou_modifier, DbPool};
use crate::c::verifier_session;
use crate::function::{build_nav_html, NavContext};
use crate::utils;
use serde_json::{json, Value};
use tiny_http::{Request, Response};

/// Migrations idempotentes au démarrage.
pub fn ensure_schema(pool: &DbPool) {
    // Vérifie si mess_pub_key existe dans information_schema via selectionner
    let col = selectionner(
        pool,
        "information_schema.COLUMNS",
        &[
            ("TABLE_SCHEMA", mysql::Value::from("DATABASE()")),
            ("TABLE_NAME",   mysql::Value::from("login")),
            ("COLUMN_NAME",  mysql::Value::from("mess_pub_key")),
        ],
        &["COLUMN_NAME"],
        None,
        Some(1),
    );

    if col.is_empty() {
        // La colonne n'existe pas → on l'ajoute via une connexion directe
        if let Ok(mut conn) = pool.get_conn() {
            if let Err(e) = mysql::prelude::Queryable::query_drop(
                &mut conn,
                "ALTER TABLE `login` ADD COLUMN `mess_pub_key` LONGTEXT DEFAULT NULL",
            ) {
                eprintln!("[mess] ALTER TABLE login: {e}");
            }
        }
    }

    // p2p_messages — aucune migration, on utilise metadata+status existants
}

pub fn handle(pool: &DbPool, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let url    = request.url().to_string();
    let method = request.method().to_string();
    let path   = url.split('?').next().unwrap_or(&url).to_string();

    let remote_full = request.remote_addr().map(|a| a.to_string()).unwrap_or_default();
    let remote_ip   = utils::strip_port(&remote_full);
    let user_agent  = request.headers().iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("user-agent"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();

    // ── Route HTML publique (pas d'auth)
    if path == "/mess" || path == "/mess/" {
        return serve_html();
    }

    // ── Auth via verifier_session (même logique que tous les autres modules)
    let cookie_val = get_cookie(request, "connexion_cookie");
    let session    = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);

    if !session.connecte {
        return json_resp(json!({"success":false,"error":"Non authentifié"}), 401);
    }

    // ── Routing
    match path.as_str() {
        "/api/mess/navbar" => {
            let ctx = NavContext {
                pool,
                user_id:    Some(session.user_id),
                page_key:   "mess",
                cookie_val: &cookie_val,
                remote_ip:  &remote_ip,
                user_agent: &user_agent,
                query_id:   None,
                apps:       Vec::new(),
                admin_apps: Vec::new(),
            };
            html_resp(&build_nav_html(&ctx), 200)
        }

        "/api/mess/prefs" => {
            with_conn(pool, |c| handle_prefs(c, pool, &session))
        }

        "/api/mess/list" => {
            with_conn(pool, |c| handle_list(c, &session, &url))
        }

        "/api/mess/users" => {
            with_conn(pool, |c| handle_users(c, &session))
        }

        "/api/mess/pubkey" | "/api/mess/pubkey/" if method == "GET" => {
            handle_get_my_pubkey(pool, &session)
        }

        "/api/mess/pubkey" | "/api/mess/pubkey/" => {
            let body = read_body(request);
            handle_set_pubkey(pool, &session, &body)
        }

        "/api/mess/send" => {
            let body = read_body(request);
            handle_send(pool, &session, &body)
        }

        "/api/mess/read" => {
            let body = read_body(request);
            with_conn(pool, |c| handle_read(c, &session, &body))
        }

        "/api/mess/delete" => {
            let body = read_body(request);
            with_conn(pool, |c| handle_delete(c, &session, &body))
        }

        p if p.starts_with("/api/mess/pubkey/") => {
            let raw   = p.trim_start_matches("/api/mess/pubkey/");
            let email = url_decode(raw);
            handle_get_pubkey_for(pool, &email)
        }

        _ => json_resp(json!({"success":false,"error":"Route inconnue"}), 404),
    }
}

// ══════════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════════

fn serve_html() -> Response<std::io::Cursor<Vec<u8>>> {
    match std::fs::read("./static/mess/mess.html") {
        Ok(d) => Response::from_data(d).with_header(
            tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()),
        Err(_) => html_resp("<h1>mess.html introuvable</h1>", 404),
    }
}

fn handle_prefs(
    conn: &mut mysql::PooledConn,
    _pool: &DbPool,
    session: &crate::c::SessionInfo,
) -> Response<std::io::Cursor<Vec<u8>>> {
    // Récupère le thème depuis la table pref
    let teme: i64 = mysql::prelude::Queryable::query_first(conn,
        format!("SELECT COALESCE(teme,0) FROM pref WHERE `id-user`={}", session.user_id)
    ).unwrap_or(Some(0)).unwrap_or(0);

    let langue: String = mysql::prelude::Queryable::query_first(conn,
        format!("SELECT COALESCE(langue,'fr') FROM pref WHERE `id-user`={}", session.user_id)
    ).unwrap_or(Some("fr".into())).unwrap_or_else(|| "fr".into());

    json_resp(json!({
        "success": true,
        "teme":    teme,
        "langue":  langue,
        "user": {
            "id":    session.user_id,
            "nom":   session.user_nom,
            "email": session.user_email,
        }
    }), 200)
}

fn handle_get_my_pubkey(
    pool: &DbPool,
    session: &crate::c::SessionInfo,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(session.user_email.as_str()))],
        &["mess_pub_key"],
        None,
        Some(1),
    );
    let pub_key = rows.first()
        .and_then(|r| r.get("mess_pub_key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    json_resp(json!({"success":true,"pub_key":pub_key}), 200)
}

fn handle_set_pubkey(
    pool: &DbPool,
    session: &crate::c::SessionInfo,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let pub_key = data["pub_key"].as_str().unwrap_or("").to_string();
    if pub_key.is_empty() {
        return json_resp(json!({"success":false,"error":"pub_key manquante"}), 400);
    }
    let ok = inserer_ou_modifier(
        pool,
        "login",
        &[("mess_pub_key", mysql::Value::from(pub_key.as_str()))],
        &[("email",        mysql::Value::from(session.user_email.as_str()))],
    );
    if ok >= 0 { json_resp(json!({"success":true}), 200) }
    else       { err500() }
}

fn handle_get_pubkey_for(
    pool: &DbPool,
    email: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email))],
        &["mess_pub_key"],
        None,
        Some(1),
    );
    let pub_key = rows.first()
        .and_then(|r| r.get("mess_pub_key"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    match pub_key {
        Some(k) => json_resp(json!({"success":true,"pub_key":k}), 200),
        None    => json_resp(json!({"success":false,
            "error":"Destinataire sans clé — doit ouvrir la messagerie une fois"}), 404),
    }
}

fn handle_list(
    conn: &mut mysql::PooledConn,
    session: &crate::c::SessionInfo,
    url: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = utils::parse_query(url);
    let folder = params.get("folder").cloned().unwrap_or_else(|| "inbox".to_string());
    let email  = esc(&session.user_email);

    let where_clause = match folder.as_str() {
        "sent"  => format!("message_type='mess' AND JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.from'))='{}' AND status!='delivered'", email),
        "trash" => format!("message_type='mess' AND status='delivered' AND (JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.from'))='{0}' OR JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.to'))='{0}')", email),
        _       => format!("message_type='mess' AND JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.to'))='{}' AND status!='delivered'", email),
    };

    let sql = format!(
        "SELECT id,metadata,content,UNIX_TIMESTAMP(created_at),status \
         FROM p2p_messages WHERE {} ORDER BY created_at DESC LIMIT 200",
        where_clause
    );

    let rows: Vec<Value> = mysql::prelude::Queryable::query_map(conn, sql,
        |(id, metadata, content, created_at, status): (i64, String, String, u64, String)| {
            let meta: Value = serde_json::from_str(&metadata).unwrap_or_default();
            json!({
                "id":         id,
                "from_email": meta["from"].as_str().unwrap_or(""),
                "to_email":   meta["to"].as_str().unwrap_or(""),
                "subj_enc":   meta["subj_enc"].as_str().unwrap_or(""),
                "body_enc":   content,
                "created_at": created_at,
                "lu":         status == "read",
            })
        }
    ).unwrap_or_default();

    let unread = rows.iter().filter(|m| !m["lu"].as_bool().unwrap_or(true)).count();
    json_resp(json!({"success":true,"messages":rows,"unread":unread}), 200)
}

fn handle_send(
    pool: &DbPool,
    session: &crate::c::SessionInfo,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let to_email = data["to_email"].as_str().unwrap_or("").trim().to_string();
    let subj_enc = data["subj_enc"].as_str().unwrap_or("").to_string();
    let body_enc = data["body_enc"].as_str().unwrap_or("").to_string();

    if to_email.is_empty() || subj_enc.is_empty() || body_enc.is_empty() {
        return json_resp(json!({"success":false,"error":"Champs manquants"}), 400);
    }

    // Vérifier que le destinataire existe
    let exists = !selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(to_email.as_str()))],
        &["email"],
        None,
        Some(1),
    ).is_empty();

    if !exists {
        return json_resp(json!({"success":false,"error":"Destinataire introuvable"}), 404);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    match pool.get_conn() {
        Ok(mut conn) => match mysql::prelude::Queryable::query_drop(&mut conn, format!(
            "INSERT INTO p2p_messages (id,from_user_id,to_user_id,message_type,metadata,content,status) \
             VALUES (NULL,0,0,'mess','{}','{}','sent')",
            esc(&json!({"from":session.user_email,"to":to_email,"subj_enc":subj_enc}).to_string()), esc(&body_enc)
        )) {
            Ok(_) => json_resp(json!({"success":true}), 200),
            Err(e) => { eprintln!("[mess/send] {e}"); err500() }
        },
        Err(_) => err500(),
    }
}

fn handle_read(
    conn: &mut mysql::PooledConn,
    session: &crate::c::SessionInfo,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_i64().unwrap_or(0);
    if id == 0 { return json_resp(json!({"success":false,"error":"id manquant"}), 400); }
    mysql::prelude::Queryable::query_drop(conn, format!(
        "UPDATE p2p_messages SET status='read' WHERE id={} AND message_type='mess' AND JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.to'))='{}'",
        id, esc(&session.user_email)
    )).ok();
    json_resp(json!({"success":true}), 200)
}

fn handle_delete(
    conn: &mut mysql::PooledConn,
    session: &crate::c::SessionInfo,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_i64().unwrap_or(0);
    if id == 0 { return json_resp(json!({"success":false,"error":"id manquant"}), 400); }
    mysql::prelude::Queryable::query_drop(conn, format!(
        "UPDATE p2p_messages SET status='delivered' \
         WHERE id={} AND message_type='mess' AND (JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.to'))='{}' OR JSON_UNQUOTE(JSON_EXTRACT(metadata,'$.from'))='{}')",
        id, esc(&session.user_email), esc(&session.user_email)
    )).ok();
    json_resp(json!({"success":true}), 200)
}

fn handle_users(
    conn: &mut mysql::PooledConn,
    session: &crate::c::SessionInfo,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let rows: Vec<Value> = mysql::prelude::Queryable::query_map(conn,
        format!("SELECT nom,email FROM login WHERE email!='{}' ORDER BY nom LIMIT 100",
            esc(&session.user_email)),
        |(nom, email): (String, String)| json!({"nom":nom,"email":email})
    ).unwrap_or_default();
    json_resp(json!({"success":true,"users":rows}), 200)
}

// ══════════════════════════════════════════════════════════════════
// Helpers internes
// ══════════════════════════════════════════════════════════════════

fn with_conn<F>(pool: &DbPool, f: F) -> Response<std::io::Cursor<Vec<u8>>>
where F: FnOnce(&mut mysql::PooledConn) -> Response<std::io::Cursor<Vec<u8>>>
{
    match pool.get_conn() { Ok(mut c) => f(&mut c), Err(_) => err500() }
}

fn get_cookie(request: &Request, name: &str) -> String {
    for h in request.headers() {
        if h.field.as_str().as_str().eq_ignore_ascii_case("cookie") {
            for part in h.value.as_str().split(';') {
                let part = part.trim();
                if let Some(rest) = part.strip_prefix(name) {
                    if let Some(val) = rest.strip_prefix('=') {
                        return val.trim().to_string();
                    }
                }
            }
        }
    }
    String::new()
}

fn read_body(request: &mut Request) -> String {
    use std::io::Read;
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

/// Décode %XX dans une URL (ex: %40 → @)
fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(b) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                out.push(b as char);
            } else {
                out.push('%'); out.push(h1); out.push(h2);
            }
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('\'', "\\'") }

fn json_resp(body: Value, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(code)
        .with_header(tiny_http::Header::from_bytes(
            "Content-Type", "application/json; charset=utf-8").unwrap())
}
fn html_resp(body: &str, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(code)
        .with_header(tiny_http::Header::from_bytes(
            "Content-Type", "text/html; charset=utf-8").unwrap())
}
fn err500() -> Response<std::io::Cursor<Vec<u8>>> {
    json_resp(json!({"success":false,"error":"Erreur serveur"}), 500)
}