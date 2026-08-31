// ══════════════════════════════════════════════════════════════════
// src/sitec/sitec.rs — Éditeur de pages web VEX (Sitec)
//   - /sitec/               → éditeur SPA (auth requise)
//   - /api/sitec/*          → API CRUD + partage (auth requise)
//   - /page/{id}            → rendu public de la page (id = 20 car.)
// Partage façon "fichiers" : champ `partage` CSV "uid:5,uid:12"
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::c::{verifier_session, SessionInfo};
use crate::function::{build_nav_html, html_escape, NavContext};
use crate::utils;
use serde_json::{json, Value};
use tiny_http::{Request, Response};

const ID_LEN: usize = 20;
const ID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

// ══════════════════════════════════════════════════════════════════
// SCHÉMA
// ══════════════════════════════════════════════════════════════════
pub fn ensure_schema(pool: &DbPool) {
    if let Ok(mut conn) = pool.get_conn() {
        if let Err(e) = mysql::prelude::Queryable::query_drop(
            &mut conn,
            "CREATE TABLE IF NOT EXISTS `sitec_pages` (
                `id`             VARCHAR(20)  PRIMARY KEY,
                `owner_id`       INT          NOT NULL,
                `titre`          VARCHAR(255) NOT NULL DEFAULT '',
                `mode`           VARCHAR(10)  NOT NULL DEFAULT 'simple',
                `contenu_html`   LONGTEXT,
                `contenu_titre`  VARCHAR(255),
                `contenu_corps`  LONGTEXT,
                `public`         TINYINT      NOT NULL DEFAULT 0,
                `partage`        TEXT,
                `created_at`     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
                `updated_at`     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ) {
            eprintln!("[sitec] CREATE TABLE sitec_pages: {e}");
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// ID ALÉATOIRE (20 car.)
// ══════════════════════════════════════════════════════════════════
fn random_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1) as u64;
    let mut state = seed ^ 0x9e3779b97f4a7c15 ^ (std::process::id() as u64);
    let mut out = String::with_capacity(ID_LEN);
    for _ in 0..ID_LEN {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(ID_CHARS[(state as usize) % ID_CHARS.len()] as char);
    }
    out
}

fn generate_page_id(pool: &DbPool) -> String {
    for _ in 0..10 {
        let candidate = random_id();
        let exists = !selectionner(
            pool,
            "sitec_pages",
            &[("id", mysql::Value::from(candidate.as_str()))],
            &["id"],
            None,
            Some(1),
        )
        .is_empty();
        if !exists {
            return candidate;
        }
    }
    random_id() // collision quasi impossible sur 20 car. (62^20)
}

// ══════════════════════════════════════════════════════════════════
// PARTAGE — helpers CSV "uid:5,uid:12"
// ══════════════════════════════════════════════════════════════════
fn partage_contains(partage: &str, uid: i64) -> bool {
    let needle = format!("uid:{}", uid);
    partage.split(',').any(|p| p.trim() == needle)
}

fn partage_add(partage: &str, uid: i64) -> String {
    if partage_contains(partage, uid) {
        return partage.to_string();
    }
    let entry = format!("uid:{}", uid);
    if partage.trim().is_empty() {
        entry
    } else {
        format!("{},{}", partage.trim_end_matches(','), entry)
    }
}

fn partage_remove(partage: &str, uid: i64) -> String {
    let needle = format!("uid:{}", uid);
    partage
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty() && *p != needle)
        .collect::<Vec<_>>()
        .join(",")
}

fn partage_user_ids(partage: &str) -> Vec<i64> {
    partage
        .split(',')
        .filter_map(|p| p.trim().strip_prefix("uid:"))
        .filter_map(|s| s.parse::<i64>().ok())
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// ROUTING
// ══════════════════════════════════════════════════════════════════
pub fn handle(pool: &DbPool, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();

    let remote_full = request.remote_addr().map(|a| a.to_string()).unwrap_or_default();
    let remote_ip = utils::strip_port(&remote_full);
    let user_agent = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("user-agent"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    let cookie_val = get_cookie(request, "connexion_cookie");

    // ── Rendu public d'une page ────────────────────────────────────
    if let Some(id_raw) = path.strip_prefix("/page/") {
        let id = id_raw.trim_end_matches('/').to_string();
        let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
        return serve_page_view(pool, &id, &session);
    }

    // ── Éditeur (page HTML, auth requise) ──────────────────────────
    if path == "/sitec" || path == "/sitec/" {
        let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
        if !session.connecte {
            return redirect("/login/login");
        }
        return serve_sitec_html();
    }

    // ── API (auth requise) ──────────────────────────────────────────
    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
    if !session.connecte {
        return json_resp(json!({"success":false,"error":"Non authentifié"}), 401);
    }

    match path.as_str() {
        "/api/sitec/navbar" => {
            let ctx = NavContext {
                pool,
                user_id: Some(session.user_id),
                page_key: "sitec",
                cookie_val: &cookie_val,
                remote_ip: &remote_ip,
                user_agent: &user_agent,
                query_id: None,
                apps: Vec::new(),
                admin_apps: Vec::new(),
            };
            html_resp(&build_nav_html(&ctx), 200)
        }
        "/api/sitec/list" => handle_list(pool, &session),
        "/api/sitec/get" => handle_get(pool, &session, &url),
        "/api/sitec/create" => handle_create(pool, &session),
        "/api/sitec/save" => {
            let body = read_body(request);
            handle_save(pool, &session, &body)
        }
        "/api/sitec/delete" => {
            let body = read_body(request);
            handle_delete(pool, &session, &body)
        }
        "/api/sitec/share" => {
            let body = read_body(request);
            handle_share(pool, &session, &body)
        }
        "/api/sitec/users" => handle_users(pool, &session),
        _ => json_resp(json!({"success":false,"error":"Route inconnue"}), 404),
    }
}

// ══════════════════════════════════════════════════════════════════
// HANDLERS — CRUD
// ══════════════════════════════════════════════════════════════════

fn handle_list(pool: &DbPool, session: &SessionInfo) -> Response<std::io::Cursor<Vec<u8>>> {
    let rows = selectionner(
        pool,
        "sitec_pages",
        &[("owner_id", mysql::Value::from(session.user_id))],
        &["id", "titre", "mode", "public", "partage", "created_at", "updated_at"],
        Some("updated_at DESC"),
        None,
    );
    let pages: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let partage = r.get("partage").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "id":          r.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "titre":       r.get("titre").and_then(|v| v.as_str()).unwrap_or(""),
                "mode":        r.get("mode").and_then(|v| v.as_str()).unwrap_or("simple"),
                "public":      r.get("public").and_then(|v| v.as_i64()).unwrap_or(0),
                "share_count": partage_user_ids(partage).len(),
                "updated_at":  r.get("updated_at").and_then(|v| v.as_str()).unwrap_or(""),
            })
        })
        .collect();
    json_resp(json!({"success":true,"pages":pages}), 200)
}

fn handle_get(pool: &DbPool, session: &SessionInfo, url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = utils::parse_query(url);
    let id = match params.get("id") {
        Some(v) => v.clone(),
        None => return json_resp(json!({"success":false,"error":"id manquant"}), 400),
    };
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":"Page introuvable"}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":"Accès refusé"}), 403);
    }

    let share_ids = partage_user_ids(&page.partage);
    let share_emails: Vec<Value> = if share_ids.is_empty() {
        vec![]
    } else {
        let mut out = vec![];
        for uid in share_ids {
            if let Some(u) = selectionner(
                pool,
                "login",
                &[("id", mysql::Value::from(uid))],
                &["email", "nom"],
                None,
                Some(1),
            )
            .into_iter()
            .next()
            {
                out.push(json!({
                    "id": uid,
                    "email": u.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                    "nom": u.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
                }));
            }
        }
        out
    };

    json_resp(
        json!({
            "success": true,
            "page": {
                "id": page.id,
                "titre": page.titre,
                "mode": page.mode,
                "contenu_html": page.contenu_html,
                "contenu_titre": page.contenu_titre,
                "contenu_corps": page.contenu_corps,
                "public": page.public,
                "shared_with": share_emails,
            }
        }),
        200,
    )
}

fn handle_create(pool: &DbPool, session: &SessionInfo) -> Response<std::io::Cursor<Vec<u8>>> {
    ensure_schema(pool);

    let id = generate_page_id(pool);
    let ok = inserer_ou_modifier(
        pool,
        "sitec_pages",
        &[
            ("id", mysql::Value::from(id.as_str())),
            ("owner_id", mysql::Value::from(session.user_id)),
            ("titre", mysql::Value::from("Nouvelle page")),
            ("mode", mysql::Value::from("simple")),
            ("contenu_titre", mysql::Value::from("Nouvelle page")),
            ("contenu_corps", mysql::Value::from("")),
            ("contenu_html", mysql::Value::from("")),
            ("public", mysql::Value::from(0i64)),
            ("partage", mysql::Value::from("")),
        ],
        &[],
    );
    if ok >= 0 {
        json_resp(json!({"success":true,"id":id}), 200)
    } else {
        err500()
    }
}

fn handle_save(pool: &DbPool, session: &SessionInfo, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return json_resp(json!({"success":false,"error":"id manquant"}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":"Page introuvable"}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":"Accès refusé"}), 403);
    }

    let titre = data["titre"].as_str().unwrap_or("Sans titre").to_string();
    let mode = match data["mode"].as_str() {
        Some("brut") => "brut",
        _ => "simple",
    };
    let contenu_html = data["contenu_html"].as_str().unwrap_or("").to_string();
    let contenu_titre = data["contenu_titre"].as_str().unwrap_or("").to_string();
    let contenu_corps = data["contenu_corps"].as_str().unwrap_or("").to_string();
    let public = if data["public"].as_bool().unwrap_or(false) { 1i64 } else { 0i64 };

    let ok = inserer_ou_modifier(
        pool,
        "sitec_pages",
        &[
            ("titre", mysql::Value::from(titre.as_str())),
            ("mode", mysql::Value::from(mode)),
            ("contenu_html", mysql::Value::from(contenu_html.as_str())),
            ("contenu_titre", mysql::Value::from(contenu_titre.as_str())),
            ("contenu_corps", mysql::Value::from(contenu_corps.as_str())),
            ("public", mysql::Value::from(public)),
        ],
        &[("id", mysql::Value::from(id.as_str()))],
    );
    if ok >= 0 {
        json_resp(json!({"success":true}), 200)
    } else {
        err500()
    }
}

fn handle_delete(pool: &DbPool, session: &SessionInfo, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return json_resp(json!({"success":false,"error":"id manquant"}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":"Page introuvable"}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":"Accès refusé"}), 403);
    }
    let ok = supprimer_ligne(pool, "sitec_pages", "id", mysql::Value::from(id.as_str()));
    if ok {
        json_resp(json!({"success":true}), 200)
    } else {
        err500()
    }
}

/// action: "add" | "remove", email: destinataire du partage
fn handle_share(pool: &DbPool, session: &SessionInfo, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    let action = data["action"].as_str().unwrap_or("add").to_string();
    let email = data["email"].as_str().unwrap_or("").trim().to_string();
    if id.is_empty() || email.is_empty() {
        return json_resp(json!({"success":false,"error":"Champs manquants"}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":"Page introuvable"}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":"Accès refusé"}), 403);
    }
    let target = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id"],
        None,
        Some(1),
    );
    let target_id = match target.into_iter().next().and_then(|r| r.get("id").and_then(|v| v.as_i64())) {
        Some(uid) => uid,
        None => return json_resp(json!({"success":false,"error":"Utilisateur introuvable"}), 404),
    };

    let new_partage = if action == "remove" {
        partage_remove(&page.partage, target_id)
    } else {
        partage_add(&page.partage, target_id)
    };

    let ok = inserer_ou_modifier(
        pool,
        "sitec_pages",
        &[("partage", mysql::Value::from(new_partage.as_str()))],
        &[("id", mysql::Value::from(id.as_str()))],
    );
    if ok >= 0 {
        json_resp(json!({"success":true}), 200)
    } else {
        err500()
    }
}

fn handle_users(pool: &DbPool, session: &SessionInfo) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return err500(),
    };
    let rows: Vec<Value> = mysql::prelude::Queryable::query_map(
        &mut conn,
        format!(
            "SELECT nom,email FROM login WHERE email!='{}' ORDER BY nom LIMIT 100",
            esc(&session.user_email)
        ),
        |(nom, email): (String, String)| json!({"nom":nom,"email":email}),
    )
    .unwrap_or_default();
    json_resp(json!({"success":true,"users":rows}), 200)
}

// ══════════════════════════════════════════════════════════════════
// RENDU PUBLIC — /page/{id}
// ══════════════════════════════════════════════════════════════════
fn serve_page_view(pool: &DbPool, id: &str, session: &SessionInfo) -> Response<std::io::Cursor<Vec<u8>>> {
    let page = match get_page(pool, id) {
        Some(p) => p,
        None => return html_resp(&error_page_html("Page introuvable"), 404),
    };

    let is_owner = session.connecte && session.user_id == page.owner_id;
    let is_shared = session.connecte && partage_contains(&page.partage, session.user_id);
    let can_view = page.public == 1 || is_owner || is_shared;

    if !can_view {
        return html_resp(&error_page_html("Accès non autorisé à cette page"), 403);
    }

    let body_html = if page.mode == "brut" {
        page.contenu_html.clone()
    } else {
        let titre_esc = html_escape(&page.contenu_titre);
        let corps_html = html_escape(&page.contenu_corps).replace('\n', "<br>");
        format!(
            "<div class=\"sitec-view-wrap\"><h1>{}</h1><div class=\"sitec-view-corps\">{}</div></div>",
            titre_esc, corps_html
        )
    };

    let doc = format!(
        "<!DOCTYPE html>\n<html lang=\"fr\"><head><meta charset=\"UTF-8\">\
        <meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\">\
        <title>{titre}</title>\
        <style>body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
        max-width:820px;margin:40px auto;padding:0 20px;color:#1c1e21;line-height:1.6;}}\
        .sitec-view-wrap h1{{margin-bottom:16px;}}\
        .sitec-view-corps{{font-size:16px;white-space:pre-wrap;}}</style>\
        </head><body>{body}</body></html>",
        titre = html_escape(&page.titre),
        body = body_html
    );

    html_resp(&doc, 200)
}

fn error_page_html(msg: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"fr\"><head><meta charset=\"UTF-8\">\
        <title>Sitec</title><style>body{{font-family:sans-serif;display:flex;\
        align-items:center;justify-content:center;height:100vh;margin:0;\
        background:#f0f2f5;color:#65676b;}}</style></head>\
        <body><p>{}</p></body></html>",
        html_escape(msg)
    )
}

// ══════════════════════════════════════════════════════════════════
// STRUCT / ACCÈS DB
// ══════════════════════════════════════════════════════════════════
struct SitecPage {
    id: String,
    owner_id: i64,
    titre: String,
    mode: String,
    contenu_html: String,
    contenu_titre: String,
    contenu_corps: String,
    public: i64,
    partage: String,
}

fn get_page(pool: &DbPool, id: &str) -> Option<SitecPage> {
    let row = selectionner(
        pool,
        "sitec_pages",
        &[("id", mysql::Value::from(id))],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()?;

    Some(SitecPage {
        id: row.get("id")?.as_str()?.to_string(),
        owner_id: row.get("owner_id")?.as_i64()?,
        titre: row.get("titre").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        mode: row.get("mode").and_then(|v| v.as_str()).unwrap_or("simple").to_string(),
        contenu_html: row.get("contenu_html").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        contenu_titre: row.get("contenu_titre").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        contenu_corps: row.get("contenu_corps").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        public: row.get("public").and_then(|v| v.as_i64()).unwrap_or(0),
        partage: row.get("partage").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

// ══════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════
fn serve_sitec_html() -> Response<std::io::Cursor<Vec<u8>>> {
    match std::fs::read("./static/sitec/sitec.html") {
        Ok(d) => Response::from_data(d).with_header(
            tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
        ),
        Err(_) => html_resp("<h1>sitec.html introuvable</h1>", 404),
    }
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

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn redirect(location: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("")
        .with_status_code(302)
        .with_header(tiny_http::Header::from_bytes("Location", location).unwrap())
}

fn json_resp(body: Value, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(code)
        .with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
        )
}

fn html_resp(body: &str, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(code)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}

fn err500() -> Response<std::io::Cursor<Vec<u8>>> {
    json_resp(json!({"success":false,"error":"Erreur serveur"}), 500)
}