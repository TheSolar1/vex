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
use crate::i18n::{t, Cle};
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
                `contenu_blocs`  LONGTEXT,
                `public`         TINYINT      NOT NULL DEFAULT 0,
                `partage`        TEXT,
                `created_at`     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
                `updated_at`     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ) {
            eprintln!("[sitec] CREATE TABLE sitec_pages: {e}");
        }
        // Migration : table deja existante avant l'ajout du mode "blocs" --
        // ADD COLUMN echoue silencieusement (colonne deja presente) sur les
        // installations qui l'ont deja, comme les autres migrations du
        // projet (voir db_init.rs).
        let _ = mysql::prelude::Queryable::query_drop(
            &mut conn,
            "ALTER TABLE `sitec_pages` ADD COLUMN `contenu_blocs` LONGTEXT",
        );
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
    // ensure_schema() tourne deja une fois au demarrage (voir main.rs) --
    // pas besoin de la relancer a chaque requete.
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
    let accept_lang = crate::access_control::get_header(request, "Accept-Language");

    // ── Rendu public d'une page ────────────────────────────────────
    if let Some(id_raw) = path.strip_prefix("/page/") {
        let id = id_raw.trim_end_matches('/').to_string();
        let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
        let langue = crate::function::get_user_language(
            pool,
            if session.connecte { Some(session.user_id) } else { None },
            None,
            Some(&accept_lang),
        );
        return serve_page_view(pool, &id, &session, &langue);
    }

    // ── Éditeur (page HTML, auth requise) ──────────────────────────
    if path == "/sitec" || path == "/sitec/" {
        let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
        if !session.connecte {
            return redirect("/login/login");
        }
        let langue = crate::function::get_user_language(pool, Some(session.user_id), None, Some(&accept_lang));
        return serve_sitec_html(&langue);
    }

    // ── API (auth requise) ──────────────────────────────────────────
    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
    if !session.connecte {
        return json_resp(json!({"success":false,"error":"Non authentifié"}), 401);
    }
    let langue = crate::function::get_user_language(pool, Some(session.user_id), None, Some(&accept_lang));

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
        "/api/sitec/get" => handle_get(pool, &session, &url, &langue),
        "/api/sitec/create" => handle_create(pool, &session, &langue),
        "/api/sitec/save" => {
            let body = read_body(request);
            handle_save(pool, &session, &body, &langue)
        }
        "/api/sitec/delete" => {
            let body = read_body(request);
            handle_delete(pool, &session, &body, &langue)
        }
        "/api/sitec/share" => {
            let body = read_body(request);
            handle_share(pool, &session, &body, &langue)
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

fn handle_get(pool: &DbPool, session: &SessionInfo, url: &str, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = utils::parse_query(url);
    let id = match params.get("id") {
        Some(v) => v.clone(),
        None => return json_resp(json!({"success":false,"error":"id manquant"}), 400),
    };
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurPageIntrouvable)}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurAccesRefuse)}), 403);
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
                "contenu_blocs": serde_json::from_str::<Value>(&page.contenu_blocs).unwrap_or_else(|_| json!([])),
                "public": page.public,
                "shared_with": share_emails,
            }
        }),
        200,
    )
}

fn handle_create(pool: &DbPool, session: &SessionInfo, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    ensure_schema(pool);

    let id = generate_page_id(pool);
    let nouvelle_page = t(langue, Cle::SitecNouvellePage);
    let ok = inserer_ou_modifier(
        pool,
        "sitec_pages",
        &[
            ("id", mysql::Value::from(id.as_str())),
            ("owner_id", mysql::Value::from(session.user_id)),
            ("titre", mysql::Value::from(nouvelle_page)),
            ("mode", mysql::Value::from("simple")),
            ("contenu_titre", mysql::Value::from(nouvelle_page)),
            ("contenu_corps", mysql::Value::from("")),
            ("contenu_html", mysql::Value::from("")),
            ("contenu_blocs", mysql::Value::from("[]")),
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

fn handle_save(pool: &DbPool, session: &SessionInfo, body: &str, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return json_resp(json!({"success":false,"error":"id manquant"}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurPageIntrouvable)}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurAccesRefuse)}), 403);
    }

    let titre = data["titre"].as_str().unwrap_or_else(|| t(langue, Cle::SitecSansTitre)).to_string();
    let mode = match data["mode"].as_str() {
        Some("brut") => "brut",
        Some("blocs") => "blocs",
        _ => "simple",
    };
    let contenu_html = data["contenu_html"].as_str().unwrap_or("").to_string();
    let contenu_titre = data["contenu_titre"].as_str().unwrap_or("").to_string();
    let contenu_corps = data["contenu_corps"].as_str().unwrap_or("").to_string();
    let contenu_blocs = sanitiser_blocs(&data["contenu_blocs"]);
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
            ("contenu_blocs", mysql::Value::from(contenu_blocs.as_str())),
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

fn handle_delete(pool: &DbPool, session: &SessionInfo, body: &str, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return json_resp(json!({"success":false,"error":"id manquant"}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurPageIntrouvable)}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurAccesRefuse)}), 403);
    }
    let ok = supprimer_ligne(pool, "sitec_pages", "id", mysql::Value::from(id.as_str()));
    if ok {
        json_resp(json!({"success":true}), 200)
    } else {
        err500()
    }
}

/// action: "add" | "remove", email: destinataire du partage
fn handle_share(pool: &DbPool, session: &SessionInfo, body: &str, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let id = data["id"].as_str().unwrap_or("").to_string();
    let action = data["action"].as_str().unwrap_or("add").to_string();
    let email = data["email"].as_str().unwrap_or("").trim().to_string();
    if id.is_empty() || email.is_empty() {
        return json_resp(json!({"success":false,"error":t(langue, Cle::MessErreurChampsManquants)}), 400);
    }
    let page = match get_page(pool, &id) {
        Some(p) => p,
        None => return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurPageIntrouvable)}), 404),
    };
    if page.owner_id != session.user_id && session.user_privilege > 6 {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurAccesRefuse)}), 403);
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
        None => return json_resp(json!({"success":false,"error":t(langue, Cle::MessErreurDestinataireIntrouvable)}), 404),
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
fn serve_page_view(pool: &DbPool, id: &str, session: &SessionInfo, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let page = match get_page(pool, id) {
        Some(p) => p,
        None => return html_resp(&error_page_html(t(langue, Cle::SitecErreurPageIntrouvable), langue), 404),
    };

    let is_owner = session.connecte && session.user_id == page.owner_id;
    let is_shared = session.connecte && partage_contains(&page.partage, session.user_id);
    let can_view = page.public == 1 || is_owner || is_shared;

    if !can_view {
        return html_resp(&error_page_html(t(langue, Cle::SitecErreurAccesNonAutorisePage), langue), 403);
    }

    let body_html = if page.mode == "brut" {
        page.contenu_html.clone()
    } else if page.mode == "blocs" {
        render_blocs_html(&page.contenu_blocs)
    } else {
        let titre_esc = html_escape(&page.contenu_titre);
        let corps_html = html_escape(&page.contenu_corps).replace('\n', "<br>");
        format!(
            "<div class=\"sitec-view-wrap\"><h1>{}</h1><div class=\"sitec-view-corps\">{}</div></div>",
            titre_esc, corps_html
        )
    };

    let (anim_css, anim_js) = if page.mode == "blocs" {
        (BLOCS_ANIM_CSS, BLOCS_ANIM_JS)
    } else {
        ("", "")
    };

    let doc = format!(
        "<!DOCTYPE html>\n<html lang=\"{langue}\"><head><meta charset=\"UTF-8\">\
        <meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\">\
        <title>{titre}</title>\
        <style>body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
        max-width:820px;margin:40px auto;padding:0 20px;color:#1c1e21;line-height:1.6;}}\
        .sitec-view-wrap h1{{margin-bottom:16px;}}\
        .sitec-view-corps{{font-size:16px;white-space:pre-wrap;}}\
        .sitec-anim{{opacity:1;}}\
        .sitec-bloc{{margin-bottom:22px;}}\
        .sitec-bloc-texte{{font-size:16px;white-space:pre-wrap;}}\
        .sitec-bloc-image img{{max-width:100%;height:auto;border-radius:8px;display:block;}}\
        .sitec-bloc-image-legende{{font-size:13px;color:#65676b;margin-top:6px;}}\
        .sitec-bloc-bouton a{{display:inline-block;padding:12px 24px;background:#2e7d32;color:#fff;\
        text-decoration:none;border-radius:8px;font-weight:600;}}\
        .sitec-bloc-bouton a:hover{{filter:brightness(0.92);}}\
        .sitec-bloc-video{{position:relative;padding-bottom:56.25%;height:0;overflow:hidden;\
        border-radius:8px;background:#000;}}\
        .sitec-bloc-video iframe,.sitec-bloc-video video{{position:absolute;top:0;left:0;\
        width:100%;height:100%;border:0;}}\
        {anim_css}</style>\
        </head><body>{body}{anim_js}</body></html>",
        langue = langue,
        titre = html_escape(&page.titre),
        body = body_html,
        anim_css = anim_css,
        anim_js = anim_js,
    );

    html_resp(&doc, 200)
}

/// Rend le tableau JSON de blocs (mode "blocs") en HTML. Chaque bloc est
/// deja passe par `sanitiser_blocs` a l'enregistrement (type verifie,
/// URLs limitees a http/https) -- ici on echappe en plus tout texte
/// affiche, en defense en profondeur.
fn render_blocs_html(contenu_blocs: &str) -> String {
    let blocs: Vec<Value> = serde_json::from_str(contenu_blocs).unwrap_or_default();
    let mut out = String::new();
    for b in &blocs {
        let kind = b["type"].as_str().unwrap_or("");
        let anim = safe_animation(b["animation"].as_str().unwrap_or(""));
        let inner = match kind {
            "texte" => {
                let texte = html_escape(b["texte"].as_str().unwrap_or(""));
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-texte\">{}</div>",
                    texte.replace('\n', "<br>")
                )
            }
            "image" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() {
                    continue;
                }
                let alt = html_escape(b["alt"].as_str().unwrap_or(""));
                let legende = b["legende"].as_str().unwrap_or("");
                let legende_html = if legende.is_empty() {
                    String::new()
                } else {
                    format!("<div class=\"sitec-bloc-image-legende\">{}</div>", html_escape(legende))
                };
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-image\"><img src=\"{}\" alt=\"{}\" loading=\"lazy\">{}</div>",
                    html_escape(&url), alt, legende_html
                )
            }
            "bouton" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() {
                    continue;
                }
                let texte = html_escape(b["texte"].as_str().unwrap_or(""));
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-bouton\"><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">{}</a></div>",
                    html_escape(&url), texte
                )
            }
            "video" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() {
                    continue;
                }
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-video\">{}</div>",
                    video_embed_html(&url)
                )
            }
            // "code" : contenu HTML/CSS/JS libre, insere TEL QUEL (non
            // echappe, non filtre) -- meme modele de confiance que le mode
            // de page "brut" deja existant : uniquement le proprietaire de
            // la page peut y ecrire, lui seul choisit de la rendre publique.
            "code" => {
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-code\">{}</div>",
                    b["code"].as_str().unwrap_or("")
                )
            }
            _ => continue,
        };
        out.push_str(&format!(
            "<div class=\"sitec-anim\" data-anim=\"{}\">{}</div>",
            html_escape(&anim), inner
        ));
    }
    out
}

/// Effets d'entree proposes dans l'editeur -- toute valeur hors de cette
/// liste (jamais cense arriver, `sanitiser_blocs` filtre deja a
/// l'enregistrement) retombe sur "aucune animation".
const ANIMATIONS: &[&str] = &[
    "", "fade", "slide-up", "slide-left", "slide-right", "zoom-in", "bounce", "rotate", "pulse",
];
fn safe_animation(a: &str) -> String {
    if ANIMATIONS.contains(&a) { a.to_string() } else { String::new() }
}

/// CSS des animations d'entree + script d'activation au scroll (mode
/// "blocs" uniquement). Degrade proprement sans JS : le contenu reste
/// visible (`sitec-anim-hidden` n'est ajoutee que par le script lui-meme),
/// juste sans l'effet d'entree.
const BLOCS_ANIM_CSS: &str = "\
.sitec-anim-hidden{opacity:0!important;}\
.sitec-anim-play[data-anim=\"fade\"]{animation:sitecFade .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-up\"]{animation:sitecSlideUp .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-left\"]{animation:sitecSlideLeft .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-right\"]{animation:sitecSlideRight .8s ease forwards;}\
.sitec-anim-play[data-anim=\"zoom-in\"]{animation:sitecZoomIn .8s ease forwards;}\
.sitec-anim-play[data-anim=\"bounce\"]{animation:sitecBounce .9s ease forwards;}\
.sitec-anim-play[data-anim=\"rotate\"]{animation:sitecRotate .8s ease forwards;}\
.sitec-anim-play[data-anim=\"pulse\"]{animation:sitecPulse 1.6s ease-in-out infinite;}\
@keyframes sitecFade{from{opacity:0}to{opacity:1}}\
@keyframes sitecSlideUp{from{opacity:0;transform:translateY(40px)}to{opacity:1;transform:translateY(0)}}\
@keyframes sitecSlideLeft{from{opacity:0;transform:translateX(60px)}to{opacity:1;transform:translateX(0)}}\
@keyframes sitecSlideRight{from{opacity:0;transform:translateX(-60px)}to{opacity:1;transform:translateX(0)}}\
@keyframes sitecZoomIn{from{opacity:0;transform:scale(.7)}to{opacity:1;transform:scale(1)}}\
@keyframes sitecBounce{0%{opacity:0;transform:translateY(-30px)}50%{opacity:1;transform:translateY(8px)}70%{transform:translateY(-6px)}100%{opacity:1;transform:translateY(0)}}\
@keyframes sitecRotate{from{opacity:0;transform:rotate(-15deg) scale(.9)}to{opacity:1;transform:rotate(0) scale(1)}}\
@keyframes sitecPulse{0%,100%{transform:scale(1)}50%{transform:scale(1.04)}}";

const BLOCS_ANIM_JS: &str = "\
<script>(function(){\
var els=document.querySelectorAll('.sitec-anim[data-anim]:not([data-anim=\"\"])');\
if(!('IntersectionObserver' in window)||!els.length)return;\
els.forEach(function(el){el.classList.add('sitec-anim-hidden');});\
var obs=new IntersectionObserver(function(entries){\
entries.forEach(function(entry){\
if(entry.isIntersecting){\
entry.target.classList.remove('sitec-anim-hidden');\
entry.target.classList.add('sitec-anim-play');\
obs.unobserve(entry.target);\
}\
});\
},{threshold:0.15});\
els.forEach(function(el){obs.observe(el);});\
})();</script>";

/// N'autorise que http(s) -- bloque `javascript:`, `data:`, etc. dans les
/// URLs saisies par l'utilisateur (image/bouton/video) avant qu'elles
/// n'atterrissent dans un attribut href/src.
fn safe_url(u: &str) -> String {
    let u = u.trim();
    let lower = u.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        u.chars().take(2000).collect()
    } else {
        String::new()
    }
}

/// YouTube/Vimeo -> iframe d'integration ; sinon on suppose un fichier
/// video direct (mp4/webm/ogg) et on utilise la balise <video> native.
fn video_embed_html(url: &str) -> String {
    if let Some(video_id) = youtube_id(url) {
        return format!(
            "<iframe src=\"https://www.youtube-nocookie.com/embed/{}\" allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture\" allowfullscreen></iframe>",
            html_escape(&video_id)
        );
    }
    if let Some(video_id) = vimeo_id(url) {
        return format!(
            "<iframe src=\"https://player.vimeo.com/video/{}\" allow=\"autoplay; fullscreen; picture-in-picture\" allowfullscreen></iframe>",
            html_escape(&video_id)
        );
    }
    format!(
        "<video controls src=\"{}\"></video>",
        html_escape(url)
    )
}

fn youtube_id(url: &str) -> Option<String> {
    let u = url.split(['?', '&']).next().unwrap_or(url);
    if let Some(rest) = url.split("youtu.be/").nth(1) {
        return Some(rest.split(['?', '&', '/']).next().unwrap_or(rest).to_string());
    }
    if u.contains("youtube.com/watch") {
        for part in url.split(['?', '&']) {
            if let Some(id) = part.strip_prefix("v=") {
                return Some(id.to_string());
            }
        }
    }
    if let Some(rest) = url.split("youtube.com/embed/").nth(1) {
        return Some(rest.split(['?', '&', '/']).next().unwrap_or(rest).to_string());
    }
    None
}

fn vimeo_id(url: &str) -> Option<String> {
    let rest = url.split("vimeo.com/").nth(1)?;
    let id = rest.split(['?', '&', '/']).next().unwrap_or(rest);
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id.to_string())
    } else {
        None
    }
}

/// Valide/nettoie le tableau de blocs envoye par l'editeur avant stockage :
/// type reconnu uniquement, URLs http(s) uniquement, tailles bornees.
/// Toute entree invalide ou de type inconnu est silencieusement ignoree
/// plutot que de faire echouer tout l'enregistrement.
fn sanitiser_blocs(v: &Value) -> String {
    const MAX_BLOCS: usize = 200;
    const MAX_TEXTE: usize = 20_000;
    const MAX_COURT: usize = 300;
    // Le bloc "code" est volontairement plus permissif : il est destine a
    // du HTML/CSS/JS complet (voir render_blocs_html, insere non echappe).
    const MAX_CODE: usize = 50_000;

    let tronque = |s: &str, n: usize| -> String { s.chars().take(n).collect() };

    let Some(arr) = v.as_array() else {
        return "[]".to_string();
    };

    let mut out: Vec<Value> = Vec::new();
    for b in arr.iter().take(MAX_BLOCS) {
        let kind = b["type"].as_str().unwrap_or("");
        let anim = safe_animation(b["animation"].as_str().unwrap_or(""));
        let mut bloc = match kind {
            "texte" => json!({
                "type": "texte",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_TEXTE),
            }),
            "image" => json!({
                "type": "image",
                "url": tronque(b["url"].as_str().unwrap_or(""), 2000),
                "alt": tronque(b["alt"].as_str().unwrap_or(""), MAX_COURT),
                "legende": tronque(b["legende"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "bouton" => json!({
                "type": "bouton",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_COURT),
                "url": tronque(b["url"].as_str().unwrap_or(""), 2000),
            }),
            "video" => json!({
                "type": "video",
                "url": tronque(b["url"].as_str().unwrap_or(""), 2000),
            }),
            "code" => json!({
                "type": "code",
                "code": tronque(b["code"].as_str().unwrap_or(""), MAX_CODE),
            }),
            _ => continue,
        };
        bloc["animation"] = json!(anim);
        out.push(bloc);
    }
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".to_string())
}

fn error_page_html(msg: &str, langue: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"{langue}\"><head><meta charset=\"UTF-8\">\
        <title>Sitec</title><style>body{{font-family:sans-serif;display:flex;\
        align-items:center;justify-content:center;height:100vh;margin:0;\
        background:#f0f2f5;color:#65676b;}}</style></head>\
        <body><p>{msg}</p></body></html>",
        langue = langue,
        msg = html_escape(msg)
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
    contenu_blocs: String,
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
        contenu_blocs: row.get("contenu_blocs").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or("[]").to_string(),
        public: row.get("public").and_then(|v| v.as_i64()).unwrap_or(0),
        partage: row.get("partage").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

// ══════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════
fn serve_sitec_html(langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match std::fs::read_to_string("./static/sitec/sitec.html") {
        Ok(html) => {
            let html = html.replace("lang=\"fr\"", &format!("lang=\"{}\"", langue));
            let html = crate::i18n::appliquer_traductions(
                &html,
                langue,
                &[
                    ("{{T_TITRE_ONGLET}}", Cle::SitecTitreOnglet),
                    ("{{T_NOUVELLE_PAGE}}", Cle::SitecNouvellePage),
                    ("{{T_SELECTIONNEZ_OU_CREEZ}}", Cle::SitecSelectionnezOuCreez),
                ],
            );
            let i18n_js = crate::i18n::objet_js(
                langue,
                &[
                    ("ERREUR_CHARGEMENT", Cle::SitecErreurChargement),
                    ("AUCUNE_PAGE", Cle::SitecAucunePage),
                    ("SELECTIONNEZ_OU_CREEZ", Cle::SitecSelectionnezOuCreez),
                    ("SANS_TITRE", Cle::SitecSansTitre),
                    ("PUBLIC", Cle::SitecPublicBadge),
                    ("N_PARTAGES", Cle::SitecNPartages),
                    ("PRIVE", Cle::SitecPrive),
                    ("CONFIRM_QUITTER_SANS_SAUVEGARDER", Cle::SitecConfirmQuitterSansSauvegarder),
                    ("ERREUR", Cle::SitecErreur),
                    ("PAGE_CREEE", Cle::SitecPageCreee),
                    ("ERREUR_CREATION", Cle::SitecErreurCreation),
                    ("TITRE_DE_LA_PAGE", Cle::SitecTitreDeLaPage),
                    ("SUPPRIMER", Cle::SitecSupprimer),
                    ("ENREGISTRER", Cle::SitecEnregistrer),
                    ("OUVRIR", Cle::SitecOuvrir),
                    ("COPIER", Cle::SitecCopier),
                    ("PAGE_PUBLIQUE", Cle::SitecPagePublique),
                    ("VISIBLE_PAR_TOUT_LE_MONDE", Cle::SitecVisibleParTousMonde),
                    ("PARTAGER_AVEC_UTILISATEURS", Cle::SitecPartagerAvecUtilisateurs),
                    ("EMAIL_EXEMPLE_PLACEHOLDER", Cle::SitecEmailExemplePlaceholder),
                    ("AJOUTER", Cle::SitecAjouter),
                    ("RETIRER_ACCES", Cle::SitecRetirerAcces),
                    ("PERSONNE_ACCES", Cle::SitecPersonneAcces),
                    ("MODE_SIMPLE", Cle::SitecModeSimple),
                    ("HTML_BRUT", Cle::SitecHtmlBrut),
                    ("TITRE_AFFICHE", Cle::SitecTitreAffiche),
                    ("CONTENU", Cle::SitecContenu),
                    ("TEXTE_PLACEHOLDER", Cle::SitecTextePlaceholder),
                    ("HINT_TEXTE_SIMPLE", Cle::SitecHintTexteSimple),
                    ("HTML_DE_LA_PAGE", Cle::SitecHtmlDeLaPage),
                    ("PLACEHOLDER_HTML_EXEMPLE", Cle::SitecPlaceholderHtmlExemple),
                    ("HINT_HTML_BRUT", Cle::SitecHintHtmlBrut),
                    ("LIEN_COPIE", Cle::SitecLienCopie),
                    ("PAGE_ENREGISTREE", Cle::SitecPageEnregistree),
                    ("ERREUR_ENREGISTREMENT", Cle::SitecErreurEnregistrement),
                    ("CONFIRM_SUPPRIMER_PAGE", Cle::SitecConfirmSupprimerPage),
                    ("PAGE_SUPPRIMEE", Cle::SitecPageSupprimee),
                    ("ERREUR_SUPPRESSION", Cle::SitecErreurSuppression),
                    ("ACCES_ACCORDE", Cle::SitecAccesAccorde),
                    ("ACCES_RETIRE", Cle::SitecAccesRetire),
                    ("MODE_BLOCS", Cle::SitecModeBlocs),
                    ("BLOC_TEXTE", Cle::SitecBlocTexte),
                    ("BLOC_IMAGE", Cle::SitecBlocImage),
                    ("BLOC_BOUTON", Cle::SitecBlocBouton),
                    ("BLOC_VIDEO", Cle::SitecBlocVideo),
                    ("BLOC_TEXTE_PLACEHOLDER", Cle::SitecBlocTextePlaceholder),
                    ("BLOC_IMAGE_URL_PLACEHOLDER", Cle::SitecBlocImageUrlPlaceholder),
                    ("BLOC_IMAGE_ALT_PLACEHOLDER", Cle::SitecBlocImageAltPlaceholder),
                    ("BLOC_IMAGE_LEGENDE_PLACEHOLDER", Cle::SitecBlocImageLegendePlaceholder),
                    ("BLOC_BOUTON_TEXTE_PLACEHOLDER", Cle::SitecBlocBoutonTextePlaceholder),
                    ("BLOC_BOUTON_URL_PLACEHOLDER", Cle::SitecBlocBoutonUrlPlaceholder),
                    ("BLOC_VIDEO_URL_PLACEHOLDER", Cle::SitecBlocVideoUrlPlaceholder),
                    ("BLOC_MONTER_TITLE", Cle::SitecBlocMonterTitle),
                    ("BLOC_DESCENDRE_TITLE", Cle::SitecBlocDescendreTitle),
                    ("BLOC_SUPPRIMER_TITLE", Cle::SitecBlocSupprimerTitle),
                    ("BLOCS_VIDE", Cle::SitecBlocsVide),
                    ("BLOCS_HINT", Cle::SitecBlocsHint),
                    ("ANIMATION_LABEL", Cle::SitecAnimationLabel),
                    ("ANIMATION_AUCUNE", Cle::SitecAnimationAucune),
                    ("ANIMATION_FADE", Cle::SitecAnimationFade),
                    ("ANIMATION_SLIDE_UP", Cle::SitecAnimationSlideUp),
                    ("ANIMATION_SLIDE_LEFT", Cle::SitecAnimationSlideLeft),
                    ("ANIMATION_SLIDE_RIGHT", Cle::SitecAnimationSlideRight),
                    ("ANIMATION_ZOOM_IN", Cle::SitecAnimationZoomIn),
                    ("ANIMATION_BOUNCE", Cle::SitecAnimationBounce),
                    ("ANIMATION_ROTATE", Cle::SitecAnimationRotate),
                    ("ANIMATION_PULSE", Cle::SitecAnimationPulse),
                    ("BLOC_CODE", Cle::SitecBlocCode),
                    ("BLOC_CODE_PLACEHOLDER", Cle::SitecBlocCodePlaceholder),
                    ("BLOC_CODE_HINT", Cle::SitecBlocCodeHint),
                ],
            );
            let html = html.replacen("{{I18N_JS}}", &i18n_js, 1);
            Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            )
        }
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