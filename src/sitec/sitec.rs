// ══════════════════════════════════════════════════════════════════
// src/sitec/sitec.rs — Éditeur de pages web VEX (Sitec)
//   - /sitec/               → éditeur SPA (auth requise)
//   - /api/sitec/*          → API CRUD + partage (auth requise)
//   - /page/{id}            → rendu public de la page (id = 20 car.)
// Partage façon "fichiers" : champ `partage` CSV "uid:5,uid:12"
// ══════════════════════════════════════════════════════════════════

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::c::{verifier_session, SessionInfo};
use crate::function::{build_nav_html, html_escape, NavContext};
use crate::i18n::{t, Cle};
use crate::utils;
use serde_json::{json, Value};
use tiny_http::{Request, Response};

/// Dossier public (servi tel quel par serve_static, voir main.rs) où sont
/// copiees les images utilisees dans les blocs Sitec -- upload direct ou
/// import depuis "mes fichiers" (fchier). Doit rester public (pas dans
/// /api/*) car une page Sitec publique doit pouvoir etre vue sans session.
const MEDIA_DIR: &str = "./static/sitec_media";
const MAX_MEDIA_BYTES: usize = 5 * 1024 * 1024;
const MEDIA_MIME_EXT: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/jpg", "jpg"),
    ("image/png", "png"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
];

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
        "/api/sitec/media_upload" => {
            let body = read_body(request);
            handle_media_upload(&session, &body, &langue)
        }
        "/api/sitec/mes_images" => handle_mes_images(pool, &session),
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
// MÉDIAS — upload direct, import depuis "mes fichiers", vignettes
// ══════════════════════════════════════════════════════════════════

/// Enregistre `bytes` sous MEDIA_DIR/<owner_id>/<id_aleatoire>.<ext> et
/// renvoie l'URL publique (servie par serve_static, voir main.rs).
fn enregistrer_media(owner_id: i64, bytes: &[u8], mime_type: &str) -> Option<String> {
    let ext = MEDIA_MIME_EXT.iter().find(|(m, _)| *m == mime_type).map(|(_, e)| *e)?;
    let dir = format!("{}/{}", MEDIA_DIR, owner_id);
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("{}.{}", random_id(), ext);
    let path = format!("{}/{}", dir, name);
    std::fs::write(&path, bytes).ok()?;
    Some(format!("/static/sitec_media/{}/{}", owner_id, name))
}

fn handle_media_upload(session: &SessionInfo, body: &str, langue: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let data: Value = serde_json::from_str(body).unwrap_or_default();
    let mime_type = data["mime_type"].as_str().unwrap_or("").to_ascii_lowercase();
    let file_b64 = data["file_b64"].as_str().unwrap_or("");
    if !MEDIA_MIME_EXT.iter().any(|(m, _)| *m == mime_type) {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurTypeFichierInvalide)}), 400);
    }
    let bytes = match B64.decode(file_b64.as_bytes()) {
        Ok(b) => b,
        Err(_) => return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurTypeFichierInvalide)}), 400),
    };
    if bytes.is_empty() || bytes.len() > MAX_MEDIA_BYTES {
        return json_resp(json!({"success":false,"error":t(langue, Cle::SitecErreurFichierTropGros)}), 400);
    }
    match enregistrer_media(session.user_id, &bytes, &mime_type) {
        Some(url) => json_resp(json!({"success":true,"url":url}), 200),
        None => err500(),
    }
}

fn handle_mes_images(pool: &DbPool, session: &SessionInfo) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return err500(),
    };
    let rows: Vec<Value> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id,nom,taille FROM fichiers WHERE id_utilisateur=? AND type_fichier LIKE 'image/%' ORDER BY date DESC LIMIT 60",
        (session.user_id,),
        |(id, nom, taille): (i64, String, i64)| json!({"id":id,"nom":nom,"taille":taille}),
    )
    .unwrap_or_default();
    json_resp(json!({"success":true,"fichiers":rows}), 200)
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

    let (body_html, kf_css) = if page.mode == "brut" {
        (page.contenu_html.clone(), String::new())
    } else if page.mode == "blocs" {
        render_blocs_html(&page.contenu_blocs)
    } else {
        let titre_esc = html_escape(&page.contenu_titre);
        let corps_html = html_escape(&page.contenu_corps).replace('\n', "<br>");
        (
            format!(
                "<div class=\"sitec-view-wrap\"><h1>{}</h1><div class=\"sitec-view-corps\">{}</div></div>",
                titre_esc, corps_html
            ),
            String::new(),
        )
    };

    let (anim_css, anim_js) = if page.mode == "blocs" {
        (BLOCS_ANIM_CSS, format!("{}{}", BLOCS_ANIM_JS, BLOCS_EXTRA_JS))
    } else {
        ("", String::new())
    };

    let edit_fab = if is_owner {
        format!(
            "<a class=\"sitec-edit-fab\" href=\"/sitec?open={}\">{}</a>",
            html_escape(&page.id),
            html_escape(t(langue, Cle::SitecModifierPage))
        )
    } else {
        String::new()
    };

    // Largeur de contenu de la page (px) : reglable dans l'editeur de blocs
    // (rectangle repere redimensionnable) -- stockee dans contenu_corps au
    // format "LARGEURxFIN_FRISE" (la fin de frise est un reglage d'editeur
    // pur, ignore ici), inutilise en mode "blocs" (sert au texte en mode
    // "simple" seulement). Ne garde que la partie avant un eventuel "x"
    // pour rester compatible avec les pages enregistrees avant son ajout.
    let largeur_page: i64 = if page.mode == "blocs" {
        page.contenu_corps
            .trim()
            .split('x')
            .next()
            .unwrap_or("")
            .parse::<i64>()
            .unwrap_or(820)
            .clamp(320, 1600)
    } else {
        820
    };
    let doc = format!(
        "<!DOCTYPE html>\n<html lang=\"{langue}\"><head><meta charset=\"UTF-8\">\
        <meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\">\
        <title>{titre}</title>\
        <script src=\"/static/fa-local.js\" defer></script>\
        <style>body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
        max-width:{largeur_page}px;margin:40px auto;padding:0 20px;color:#1c1e21;line-height:1.6;\
        overflow-wrap:anywhere;word-wrap:break-word;word-break:break-word;overflow-x:hidden;}}\
        .sitec-bloc,.sitec-bloc *{{overflow-wrap:anywhere;word-wrap:break-word;word-break:break-word;min-width:0;}}\
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
        .sitec-edit-fab{{position:fixed;bottom:22px;right:22px;background:#2e7d32;color:#fff;\
        padding:12px 22px;border-radius:30px;text-decoration:none;font-weight:700;\
        box-shadow:0 4px 14px rgba(0,0,0,.25);z-index:9999;}}\
        .sitec-edit-fab:hover{{filter:brightness(1.08);}}\
        {extra_css}{anim_css}{kf_css}</style>\
        </head><body>{body}{edit_fab}{anim_js}</body></html>",
        langue = langue,
        titre = html_escape(&page.titre),
        body = body_html,
        edit_fab = edit_fab,
        extra_css = if page.mode == "blocs" { BLOCS_EXTRA_CSS } else { "" },
        anim_css = anim_css,
        anim_js = anim_js,
        kf_css = kf_css,
    );

    html_resp(&doc, 200)
}

/// Rend le tableau JSON de blocs (mode "blocs") en HTML. Chaque bloc est
/// deja passe par `sanitiser_blocs` a l'enregistrement (type verifie,
/// URLs limitees a http/https) -- ici on echappe en plus tout texte
/// affiche, en defense en profondeur.
fn render_blocs_html(contenu_blocs: &str) -> (String, String) {
    let blocs: Vec<Value> = serde_json::from_str(contenu_blocs).unwrap_or_default();
    // (ligne, largeur%, espace px, decalage gauche %, html du bloc) -- groupe
    // en second temps les blocs consecutifs partageant la meme "ligne" en
    // rangee flex ; un bloc seul (ligne vide) mais retreci (largeur<100) est
    // enveloppe individuellement avec sa largeur + son decalage.
    let mut items: Vec<(String, i64, i64, i64, String)> = Vec::new();
    // CSS des @keyframes de translation (mode Actions -> repere de position),
    // generee au fil de la boucle et renvoyee a part pour etre injectee dans
    // le <style> de la page (voir serve_page_view).
    let mut kf_css = String::new();
    let mut kf_counter: usize = 0;
    // Blocs en "position libre" (voir plus bas) : sortis du flux normal,
    // positionnes en absolu n'importe ou sur le canevas -- pas contraints a
    // etre avant/apres/a cote d'un autre bloc.
    let mut free_out = String::new();
    let mut free_bottom: i64 = 0;
    for b in &blocs {
        let kind = b["type"].as_str().unwrap_or("");
        let anim = safe_animation(b["animation"].as_str().unwrap_or(""));
        let inner = match kind {
            "titre" => {
                let n = san_choice(b, "niveau", &["h1", "h2", "h3", "h4"], "h2");
                format!("<{n} class=\"sitec-bloc sitec-bloc-titre\">{}</{n}>", html_escape(b["texte"].as_str().unwrap_or("")), n = n)
            }
            "texte" => {
                let texte = html_escape(b["texte"].as_str().unwrap_or(""));
                format!("<div class=\"sitec-bloc sitec-bloc-texte\">{}</div>", texte.replace('\n', "<br>"))
            }
            "image" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() { continue; }
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
            "galerie" => {
                let cols = san_int(b, "colonnes", 2, 4, 3);
                let imgs: String = b["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|it| {
                    let url = safe_url(it["url"].as_str().unwrap_or(""));
                    if url.is_empty() { return None; }
                    Some(format!("<img src=\"{}\" alt=\"{}\" loading=\"lazy\">", html_escape(&url), html_escape(it["alt"].as_str().unwrap_or(""))))
                }).collect();
                if imgs.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-galerie sitec-cols-{}\">{}</div>", cols, imgs)
            }
            "carousel" => {
                let slides: String = b["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|it| {
                    let url = safe_url(it["url"].as_str().unwrap_or(""));
                    if url.is_empty() { return None; }
                    Some(format!("<img src=\"{}\" alt=\"{}\" loading=\"lazy\">", html_escape(&url), html_escape(it["alt"].as_str().unwrap_or(""))))
                }).collect();
                if slides.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-carousel\"><div class=\"sitec-carousel-track\">{}</div></div>", slides)
            }
            "video" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-video\">{}</div>", video_embed_html(&url))
            }
            "audio" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-audio\"><audio controls src=\"{}\"></audio></div>", html_escape(&url))
            }
            "bouton" => {
                let url = safe_url(b["url"].as_str().unwrap_or(""));
                if url.is_empty() { continue; }
                let texte = html_escape(b["texte"].as_str().unwrap_or(""));
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-bouton\"><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">{}</a></div>",
                    html_escape(&url), texte
                )
            }
            "groupe_boutons" => {
                let btns: String = b["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|it| {
                    let url = safe_url(it["url"].as_str().unwrap_or(""));
                    if url.is_empty() { return None; }
                    Some(format!("<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">{}</a>", html_escape(&url), html_escape(it["texte"].as_str().unwrap_or(""))))
                }).collect();
                if btns.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-groupe-boutons\">{}</div>", btns)
            }
            "separateur" => "<hr class=\"sitec-bloc sitec-bloc-separateur\">".to_string(),
            "espace" => format!("<div class=\"sitec-bloc sitec-bloc-espace sitec-espace-{}\"></div>", san_choice(b, "hauteur", &["s", "m", "l", "xl"], "m")),
            "citation" => {
                let auteur = b["auteur"].as_str().unwrap_or("");
                let auteur_html = if auteur.is_empty() { String::new() } else { format!("<cite>— {}</cite>", html_escape(auteur)) };
                format!("<blockquote class=\"sitec-bloc sitec-bloc-citation\"><p>{}</p>{}</blockquote>", html_escape(b["texte"].as_str().unwrap_or("")), auteur_html)
            }
            "liste" => {
                let tag = if san_choice(b, "style", &["puce", "numero"], "puce") == "numero" { "ol" } else { "ul" };
                let lis: String = b["items"].as_array().cloned().unwrap_or_default().iter()
                    .map(|it| format!("<li>{}</li>", html_escape(it["texte"].as_str().unwrap_or(""))))
                    .collect();
                format!("<{t} class=\"sitec-bloc sitec-bloc-liste\">{}</{t}>", lis, t = tag)
            }
            "icone_texte" => format!(
                "<div class=\"sitec-bloc sitec-bloc-icone-texte\"><div class=\"sitec-icone\"><i class=\"fas fa-{}\"></i></div><div><h4>{}</h4><p>{}</p></div></div>",
                safe_icon(b["icone"].as_str().unwrap_or("")), html_escape(b["titre"].as_str().unwrap_or("")), html_escape(b["texte"].as_str().unwrap_or(""))
            ),
            "carte" => {
                let img = safe_url(b["url_image"].as_str().unwrap_or(""));
                let img_html = if img.is_empty() { String::new() } else { format!("<img src=\"{}\" alt=\"\" loading=\"lazy\">", html_escape(&img)) };
                let btn_html = bouton_optionnel(b, "sitec-carte-btn");
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-carte\">{}<div class=\"sitec-carte-corps\"><h4>{}</h4><p>{}</p>{}</div></div>",
                    img_html, html_escape(b["titre"].as_str().unwrap_or("")), html_escape(b["texte"].as_str().unwrap_or("")), btn_html
                )
            }
            "cartes_grille" => {
                let cols = san_int(b, "colonnes", 2, 4, 3);
                let cards: String = b["items"].as_array().cloned().unwrap_or_default().iter().map(|it| {
                    let img = safe_url(it["url_image"].as_str().unwrap_or(""));
                    let img_html = if img.is_empty() { String::new() } else { format!("<img src=\"{}\" alt=\"\" loading=\"lazy\">", html_escape(&img)) };
                    format!(
                        "<div class=\"sitec-carte-item\">{}<div class=\"sitec-carte-corps\"><h4>{}</h4><p>{}</p></div></div>",
                        img_html, html_escape(it["titre"].as_str().unwrap_or("")), html_escape(it["texte"].as_str().unwrap_or(""))
                    )
                }).collect();
                if cards.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-cartes-grille sitec-cols-{}\">{}</div>", cols, cards)
            }
            "accordeon" => {
                let acc: String = b["items"].as_array().cloned().unwrap_or_default().iter().map(|it| format!(
                    "<details class=\"sitec-accordeon-item\"><summary>{}</summary><div>{}</div></details>",
                    html_escape(it["question"].as_str().unwrap_or("")), html_escape(it["reponse"].as_str().unwrap_or(""))
                )).collect();
                if acc.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-accordeon\">{}</div>", acc)
            }
            "onglets" => {
                let items = b["items"].as_array().cloned().unwrap_or_default();
                if items.is_empty() { continue; }
                let btns: String = items.iter().enumerate().map(|(i, it)| format!(
                    "<button type=\"button\" class=\"sitec-tab-btn{}\" data-tab-idx=\"{}\">{}</button>",
                    if i == 0 { " active" } else { "" }, i, html_escape(it["titre"].as_str().unwrap_or(""))
                )).collect();
                let panels: String = items.iter().enumerate().map(|(i, it)| format!(
                    "<div class=\"sitec-tab-panel{}\" data-tab-idx=\"{}\">{}</div>",
                    if i == 0 { " active" } else { "" }, i, html_escape(it["contenu"].as_str().unwrap_or(""))
                )).collect();
                format!("<div class=\"sitec-bloc sitec-bloc-onglets\" data-tabs><div class=\"sitec-tab-btns\">{}</div><div class=\"sitec-tab-panels\">{}</div></div>", btns, panels)
            }
            "temoignage" => {
                let avatar = safe_url(b["avatar_url"].as_str().unwrap_or(""));
                let avatar_html = if avatar.is_empty() { String::new() } else { format!("<img class=\"sitec-avatar\" src=\"{}\" alt=\"\" loading=\"lazy\">", html_escape(&avatar)) };
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-temoignage\">{}<p class=\"sitec-temoignage-texte\">« {} »</p><div class=\"sitec-temoignage-auteur\"><strong>{}</strong><span>{}</span></div></div>",
                    avatar_html, html_escape(b["texte"].as_str().unwrap_or("")), html_escape(b["nom"].as_str().unwrap_or("")), html_escape(b["role"].as_str().unwrap_or(""))
                )
            }
            "barre_progression" => {
                let pct = san_int(b, "pourcentage", 0, 100, 50);
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-barre\"><div class=\"sitec-barre-label\">{} — {}%</div><div class=\"sitec-barre-fond\"><div class=\"sitec-barre-remplie\" style=\"width:{}%\"></div></div></div>",
                    html_escape(b["label"].as_str().unwrap_or("")), pct, pct
                )
            }
            "compteur" => format!(
                "<div class=\"sitec-bloc sitec-compteur\" data-cible=\"{}\"><span class=\"sitec-compteur-val\">0</span><span class=\"sitec-compteur-suffixe\">{}</span><div class=\"sitec-compteur-label\">{}</div></div>",
                san_int(b, "valeur", 0, 999_999_999, 0), html_escape(b["suffixe"].as_str().unwrap_or("")), html_escape(b["label"].as_str().unwrap_or(""))
            ),
            "stats_grille" => {
                let cards: String = b["items"].as_array().cloned().unwrap_or_default().iter().map(|it| {
                    let valeur: i64 = it["valeur"].as_str().unwrap_or("0").parse().unwrap_or(0);
                    format!(
                        "<div class=\"sitec-compteur\" data-cible=\"{}\"><span class=\"sitec-compteur-val\">0</span><span class=\"sitec-compteur-suffixe\">{}</span><div class=\"sitec-compteur-label\">{}</div></div>",
                        valeur, html_escape(it["suffixe"].as_str().unwrap_or("")), html_escape(it["label"].as_str().unwrap_or(""))
                    )
                }).collect();
                if cards.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-stats-grille\">{}</div>", cards)
            }
            "compte_rebours" => {
                let date = b["date_cible"].as_str().unwrap_or("");
                if date.is_empty() { continue; }
                format!(
                    "<div class=\"sitec-bloc sitec-countdown\" data-cible=\"{}\"><div class=\"sitec-countdown-label\">{}</div><div class=\"sitec-countdown-chiffres\"></div></div>",
                    html_escape(date), html_escape(b["label"].as_str().unwrap_or(""))
                )
            }
            "appel_action" => {
                let btn_html = bouton_optionnel(b, "sitec-cta-btn");
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-cta\"><h3>{}</h3><p>{}</p>{}</div>",
                    html_escape(b["titre"].as_str().unwrap_or("")), html_escape(b["texte"].as_str().unwrap_or("")), btn_html
                )
            }
            "reseaux_sociaux" => {
                // Pas d'icônes de marque disponibles en local (voir fa-local.js,
                // qui ne sert que /static/img/solid/) -- on affiche un sigle
                // texte plutôt qu'une icône manquante.
                const PLATEFORMES: &[(&str, &str)] = &[
                    ("facebook", "FB"), ("twitter", "X"), ("instagram", "IG"),
                    ("youtube", "YT"), ("linkedin", "in"), ("github", "GH"), ("tiktok", "TT"),
                ];
                let links: String = PLATEFORMES.iter().filter_map(|(key, sigle)| {
                    let url = safe_url(b[*key].as_str().unwrap_or(""));
                    if url.is_empty() { return None; }
                    Some(format!(
                        "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\" aria-label=\"{}\">{}</a>",
                        html_escape(&url), key, sigle
                    ))
                }).collect();
                if links.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-reseaux\">{}</div>", links)
            }
            "carte_google_maps" => {
                let adresse = b["adresse"].as_str().unwrap_or("");
                if adresse.is_empty() { continue; }
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-maps\"><iframe src=\"https://www.google.com/maps?q={}&output=embed\" loading=\"lazy\" referrerpolicy=\"no-referrer-when-downgrade\"></iframe></div>",
                    url_encode(adresse)
                )
            }
            "banniere_hero" => {
                let img = safe_url(b["url_image"].as_str().unwrap_or(""));
                let style = if img.is_empty() { String::new() } else { format!(" style=\"background-image:url('{}')\"", html_escape(&img)) };
                let btn_html = bouton_optionnel(b, "sitec-hero-btn");
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-hero\"{}><div class=\"sitec-hero-overlay\"><h1>{}</h1><p>{}</p>{}</div></div>",
                    style, html_escape(b["titre"].as_str().unwrap_or("")), html_escape(b["soustitre"].as_str().unwrap_or("")), btn_html
                )
            }
            "chronologie" => {
                let ev: String = b["items"].as_array().cloned().unwrap_or_default().iter().map(|it| format!(
                    "<div class=\"sitec-chrono-item\"><div class=\"sitec-chrono-date\">{}</div><div class=\"sitec-chrono-corps\"><h4>{}</h4><p>{}</p></div></div>",
                    html_escape(it["date"].as_str().unwrap_or("")), html_escape(it["titre"].as_str().unwrap_or("")), html_escape(it["texte"].as_str().unwrap_or(""))
                )).collect();
                if ev.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-chronologie\">{}</div>", ev)
            }
            "tarifs" => {
                let feats: String = b["items"].as_array().cloned().unwrap_or_default().iter()
                    .map(|it| format!("<li>{}</li>", html_escape(it["texte"].as_str().unwrap_or(""))))
                    .collect();
                let btn_html = bouton_optionnel(b, "sitec-tarif-btn");
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-tarif\"><h4>{}</h4><div class=\"sitec-tarif-prix\">{}<span>{}</span></div><ul>{}</ul>{}</div>",
                    html_escape(b["titre"].as_str().unwrap_or("")), html_escape(b["prix"].as_str().unwrap_or("")),
                    html_escape(b["periode"].as_str().unwrap_or("")), feats, btn_html
                )
            }
            "partage_social" => {
                const WHITELIST: &[&str] = &["facebook", "twitter", "linkedin", "whatsapp", "email"];
                let btns: String = b["reseaux"].as_array().cloned().unwrap_or_default().iter()
                    .filter_map(|r| r.as_str())
                    .filter(|r| WHITELIST.contains(r))
                    .map(|r| format!("<button type=\"button\" class=\"sitec-partage-btn\" data-reseau=\"{}\"><i class=\"fas fa-share-nodes\"></i> {}</button>", html_escape(r), html_escape(r)))
                    .collect();
                if btns.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-partage\">{}</div>", btns)
            }
            "membre_equipe" => {
                let avatar = safe_url(b["avatar_url"].as_str().unwrap_or(""));
                let avatar_html = if avatar.is_empty() { String::new() } else { format!("<img class=\"sitec-avatar\" src=\"{}\" alt=\"\" loading=\"lazy\">", html_escape(&avatar)) };
                format!(
                    "<div class=\"sitec-bloc sitec-bloc-membre\">{}<h4>{}</h4><div class=\"sitec-membre-role\">{}</div><p>{}</p></div>",
                    avatar_html, html_escape(b["nom"].as_str().unwrap_or("")), html_escape(b["role"].as_str().unwrap_or("")), html_escape(b["bio"].as_str().unwrap_or(""))
                )
            }
            "logos" => {
                let imgs: String = b["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|it| {
                    let url = safe_url(it["url"].as_str().unwrap_or(""));
                    if url.is_empty() { return None; }
                    Some(format!("<img src=\"{}\" alt=\"\" loading=\"lazy\">", html_escape(&url)))
                }).collect();
                if imgs.is_empty() { continue; }
                format!("<div class=\"sitec-bloc sitec-bloc-logos\">{}</div>", imgs)
            }
            "badge" => format!(
                "<div class=\"sitec-bloc\"><span class=\"sitec-badge sitec-badge-{}\">{}</span></div>",
                san_choice(b, "couleur", &["vert", "bleu", "rouge", "jaune", "gris", "violet"], "vert"),
                html_escape(b["texte"].as_str().unwrap_or(""))
            ),
            "retour_haut" => "<div class=\"sitec-bloc\"><button type=\"button\" class=\"sitec-retour-haut\" onclick=\"window.scrollTo({top:0,behavior:'smooth'})\">&uarr;</button></div>".to_string(),
            _ => continue,
        };
        // Repères de position (mode Actions -> "translation") : au moins 2
        // repères = interpolation continue entre eux via un @keyframes
        // dédié, appliqué sur un wrapper interne (pas le même élément que
        // l'animation d'entrée : les deux `animation` CSS ne se marchent
        // pas dessus). Position figée sur le premier repère avant lecture.
        let keyframes = sanitize_keyframes(&b["keyframes"]);
        let inner = if keyframes.len() >= 2 {
            let n = kf_counter;
            kf_counter += 1;
            let t0 = keyframes[0]["t"].as_i64().unwrap_or(0);
            let t_last = keyframes[keyframes.len() - 1]["t"].as_i64().unwrap_or(0);
            let dur = (t_last - t0).max(100);
            // Largeur/hauteur "trouées" (null = non fixée à ce repère)
            // comblées avant/apres : si aucun repere ne fixe la dimension,
            // tout reste a None -> pas de style largeur/hauteur genere,
            // comportement translation seule inchange. Une valeur explicite
            // de 0 (largeur/hauteur réellement nulle, ex: "disparition")
            // n'est en revanche jamais écrasée par ce comblement.
            let ws = fill_dimension(keyframes.iter().map(|k| k["w"].as_i64()).collect());
            let hs = fill_dimension(keyframes.iter().map(|k| k["h"].as_i64()).collect());
            let has_size = ws.iter().any(|w| w.is_some()) || hs.iter().any(|h| h.is_some());
            let x0 = keyframes[0]["x"].as_i64().unwrap_or(0);
            let y0 = keyframes[0]["y"].as_i64().unwrap_or(0);
            let w0 = ws[0].unwrap_or(0);
            let h0 = hs[0].unwrap_or(0);
            let stops: String = keyframes
                .iter()
                .enumerate()
                .map(|(idx, k)| {
                    let t = k["t"].as_i64().unwrap_or(0);
                    let x = k["x"].as_i64().unwrap_or(0);
                    let y = k["y"].as_i64().unwrap_or(0);
                    let pct = (t - t0) as f64 / dur as f64 * 100.0;
                    let size_style = if has_size {
                        format!("width:{}px;height:{}px;", ws[idx].unwrap_or(0), hs[idx].unwrap_or(0))
                    } else {
                        String::new()
                    };
                    format!("{:.2}%{{transform:translate({}px,{}px);{}}}", pct, x, y, size_style)
                })
                .collect();
            let boucle = b["boucle"].as_bool().unwrap_or(false);
            let iter = if boucle { "infinite " } else { "" };
            kf_css.push_str(&format!(
                "@keyframes sitecKf{n}{{{stops}}}[data-kf=\"{n}\"].sitec-kf-play{{animation:sitecKf{n} {dur}ms ease {iter}forwards;}}",
                n = n, stops = stops, dur = dur, iter = iter
            ));
            let size0 = if has_size { format!("width:{}px;height:{}px;box-sizing:border-box;", w0, h0) } else { String::new() };
            format!(
                "<div class=\"sitec-kf\" data-kf=\"{}\" style=\"transform:translate({}px,{}px);{}\">{}</div>",
                n, x0, y0, size0, inner
            )
        } else {
            inner
        };
        let delai = san_int(b, "anim_delai", 0, 120_000, 0);
        let hauteur = san_int(b, "hauteur", 0, 2000, 0);
        let extra_style = match (delai > 0, hauteur > 0) {
            (true, true) => format!(" style=\"animation-delay:{}ms;min-height:{}px;\"", delai, hauteur),
            (true, false) => format!(" style=\"animation-delay:{}ms\"", delai),
            (false, true) => format!(" style=\"min-height:{}px;\"", hauteur),
            (false, false) => String::new(),
        };
        let html = format!(
            "<div class=\"sitec-anim\" data-anim=\"{}\"{}>{}</div>",
            html_escape(&anim), extra_style, inner
        );
        let largeur = san_int(b, "largeur", 10, 100, 100);
        if b["libre"].as_bool().unwrap_or(false) {
            let pos_x = san_int(b, "pos_x", -2000, 8000, 0);
            let pos_y = san_int(b, "pos_y", -2000, 20000, 0);
            let rotation = san_int(b, "rotation", 0, 359, 0);
            let est_h = if hauteur > 0 { hauteur } else { 80 };
            free_bottom = free_bottom.max(pos_y + est_h);
            let rotate_style = if rotation > 0 { format!("transform:rotate({}deg);", rotation) } else { String::new() };
            free_out.push_str(&format!(
                "<div style=\"position:absolute;left:{}px;top:{}px;width:{}%;{}\">{}</div>",
                pos_x, pos_y, largeur, rotate_style, html
            ));
            continue;
        }
        let ligne = b["ligne"].as_str().unwrap_or("").to_string();
        let espace = san_int(b, "espace", 0, 1000, 16);
        let decalage = san_int(b, "decalage", 0, 90, 0);
        items.push((ligne, largeur, espace, decalage, html));
    }

    let mut out = String::new();
    let mut i = 0;
    while i < items.len() {
        let ligne = items[i].0.clone();
        if ligne.is_empty() {
            let (_, largeur, _, decalage, html) = &items[i];
            if *largeur < 100 || *decalage > 0 {
                out.push_str(&format!(
                    "<div style=\"width:{}%;margin-left:{}%;\">{}</div>",
                    largeur, decalage, html
                ));
            } else {
                out.push_str(html);
            }
            i += 1;
            continue;
        }
        let espace = items[i].2;
        let mut row_html = String::new();
        let mut j = i;
        while j < items.len() && items[j].0 == ligne {
            row_html.push_str(&format!(
                "<div class=\"sitec-row-item\" style=\"flex:0 0 {l}%;max-width:{l}%;\">{}</div>",
                items[j].4, l = items[j].1
            ));
            j += 1;
        }
        out.push_str(&format!(
            "<div class=\"sitec-row\" style=\"display:flex;flex-wrap:wrap;gap:{}px;\">{}</div>",
            espace, row_html
        ));
        i = j;
    }
    if !free_out.is_empty() {
        // overflow:hidden -- un bloc libre pos_x/pos_y hors de la largeur de
        // page (repere visuel seulement dans l'editeur) ne doit pas deborder
        // sur la page publiee. Sans danger verticalement : min-height vaut
        // deja free_bottom, la hauteur la plus basse parmi les blocs libres.
        out = format!(
            "<div style=\"position:relative;min-height:{}px;overflow:hidden;\">{}{}</div>",
            free_bottom, out, free_out
        );
    }
    (out, kf_css)
}

/// Bouton optionnel partagé par plusieurs types de blocs (carte, hero, CTA,
/// tarif…) : rendu seulement si texte ET URL sont renseignés.
fn bouton_optionnel(b: &Value, classe: &str) -> String {
    let url = safe_url(b["bouton_url"].as_str().unwrap_or(""));
    let texte = html_escape(b["bouton_texte"].as_str().unwrap_or(""));
    if url.is_empty() || texte.is_empty() {
        return String::new();
    }
    format!(
        "<a class=\"{}\" href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">{}</a>",
        classe, html_escape(&url), texte
    )
}

/// Encodage pourcentage minimal (RFC 3986) -- utilisé pour la saisie libre
/// "adresse" injectée dans l'URL d'intégration Google Maps.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

/// Effets d'entree proposes dans l'editeur -- toute valeur hors de cette
/// liste (jamais cense arriver, `sanitiser_blocs` filtre deja a
/// l'enregistrement) retombe sur "aucune animation".
const ANIMATIONS: &[&str] = &[
    "", "fade", "fade-down", "slide-up", "slide-down", "slide-left", "slide-right",
    "zoom-in", "zoom-out", "bounce", "rotate", "rotate-in", "flip-x", "flip-y",
    "roll-in", "blur-in", "pulse", "shake", "swing", "tada", "jello", "flash",
    "rubber-band", "wobble", "heartbeat",
];
fn safe_animation(a: &str) -> String {
    if ANIMATIONS.contains(&a) { a.to_string() } else { String::new() }
}

/// Slug d'icône (Font Awesome solid, servi localement via fa-local.js) --
/// utilisé à la place d'un émoji libre pour le bloc "icone_texte". Filtré à
/// [a-z0-9-] pour atterrir sans risque dans un attribut class="fas fa-...".
fn safe_icon(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .take(40)
        .collect();
    if cleaned.is_empty() { "star".to_string() } else { cleaned }
}

/// CSS des animations d'entree + script d'activation au scroll (mode
/// "blocs" uniquement). Degrade proprement sans JS : le contenu reste
/// visible (`sitec-anim-hidden` n'est ajoutee que par le script lui-meme),
/// juste sans l'effet d'entree.
const BLOCS_ANIM_CSS: &str = "\
.sitec-anim-hidden{opacity:0!important;}\
.sitec-anim-play[data-anim=\"fade\"]{animation:sitecFade .8s ease forwards;}\
.sitec-anim-play[data-anim=\"fade-down\"]{animation:sitecFadeDown .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-up\"]{animation:sitecSlideUp .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-down\"]{animation:sitecSlideDown .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-left\"]{animation:sitecSlideLeft .8s ease forwards;}\
.sitec-anim-play[data-anim=\"slide-right\"]{animation:sitecSlideRight .8s ease forwards;}\
.sitec-anim-play[data-anim=\"zoom-in\"]{animation:sitecZoomIn .8s ease forwards;}\
.sitec-anim-play[data-anim=\"zoom-out\"]{animation:sitecZoomOut .8s ease forwards;}\
.sitec-anim-play[data-anim=\"bounce\"]{animation:sitecBounce .9s ease forwards;}\
.sitec-anim-play[data-anim=\"rotate\"]{animation:sitecRotate .8s ease forwards;}\
.sitec-anim-play[data-anim=\"rotate-in\"]{animation:sitecRotateIn .9s ease forwards;}\
.sitec-anim-play[data-anim=\"flip-x\"]{animation:sitecFlipX .9s ease forwards;}\
.sitec-anim-play[data-anim=\"flip-y\"]{animation:sitecFlipY .9s ease forwards;}\
.sitec-anim-play[data-anim=\"roll-in\"]{animation:sitecRollIn 1s ease forwards;}\
.sitec-anim-play[data-anim=\"blur-in\"]{animation:sitecBlurIn .9s ease forwards;}\
.sitec-anim-play[data-anim=\"pulse\"]{animation:sitecPulse 1.6s ease-in-out infinite;}\
.sitec-anim-play[data-anim=\"shake\"]{animation:sitecShake 1s ease forwards;}\
.sitec-anim-play[data-anim=\"swing\"]{animation:sitecSwing 1s ease forwards;}\
.sitec-anim-play[data-anim=\"tada\"]{animation:sitecTada 1.1s ease forwards;}\
.sitec-anim-play[data-anim=\"jello\"]{animation:sitecJello 1.1s ease forwards;}\
.sitec-anim-play[data-anim=\"flash\"]{animation:sitecFlash 1.2s ease forwards;}\
.sitec-anim-play[data-anim=\"rubber-band\"]{animation:sitecRubberBand 1.1s ease forwards;}\
.sitec-anim-play[data-anim=\"wobble\"]{animation:sitecWobble 1.1s ease forwards;}\
.sitec-anim-play[data-anim=\"heartbeat\"]{animation:sitecHeartbeat 1.2s ease forwards;}\
@keyframes sitecFade{from{opacity:0}to{opacity:1}}\
@keyframes sitecFadeDown{from{opacity:0;transform:translateY(-40px)}to{opacity:1;transform:translateY(0)}}\
@keyframes sitecSlideUp{from{opacity:0;transform:translateY(40px)}to{opacity:1;transform:translateY(0)}}\
@keyframes sitecSlideDown{from{opacity:0;transform:translateY(-70px)}to{opacity:1;transform:translateY(0)}}\
@keyframes sitecSlideLeft{from{opacity:0;transform:translateX(60px)}to{opacity:1;transform:translateX(0)}}\
@keyframes sitecSlideRight{from{opacity:0;transform:translateX(-60px)}to{opacity:1;transform:translateX(0)}}\
@keyframes sitecZoomIn{from{opacity:0;transform:scale(.7)}to{opacity:1;transform:scale(1)}}\
@keyframes sitecZoomOut{from{opacity:0;transform:scale(1.3)}to{opacity:1;transform:scale(1)}}\
@keyframes sitecBounce{0%{opacity:0;transform:translateY(-30px)}50%{opacity:1;transform:translateY(8px)}70%{transform:translateY(-6px)}100%{opacity:1;transform:translateY(0)}}\
@keyframes sitecRotate{from{opacity:0;transform:rotate(-15deg) scale(.9)}to{opacity:1;transform:rotate(0) scale(1)}}\
@keyframes sitecRotateIn{from{opacity:0;transform:rotate(-200deg)}to{opacity:1;transform:rotate(0)}}\
@keyframes sitecFlipX{from{opacity:0;transform:perspective(400px) rotateX(90deg)}to{opacity:1;transform:perspective(400px) rotateX(0)}}\
@keyframes sitecFlipY{from{opacity:0;transform:perspective(400px) rotateY(90deg)}to{opacity:1;transform:perspective(400px) rotateY(0)}}\
@keyframes sitecRollIn{from{opacity:0;transform:translateX(-100%) rotate(-120deg)}to{opacity:1;transform:translateX(0) rotate(0)}}\
@keyframes sitecBlurIn{from{opacity:0;filter:blur(12px)}to{opacity:1;filter:blur(0)}}\
@keyframes sitecPulse{0%,100%{transform:scale(1)}50%{transform:scale(1.04)}}\
@keyframes sitecShake{0%{opacity:0;transform:translateX(0)}10%{opacity:1;transform:translateX(-10px)}20%{transform:translateX(10px)}30%{transform:translateX(-8px)}40%{transform:translateX(8px)}50%{transform:translateX(-5px)}60%{transform:translateX(5px)}70%{transform:translateX(-2px)}80%{transform:translateX(2px)}100%{opacity:1;transform:translateX(0)}}\
@keyframes sitecSwing{0%{opacity:0;transform:rotate(0)}20%{opacity:1;transform:rotate(12deg)}40%{transform:rotate(-8deg)}60%{transform:rotate(5deg)}80%{transform:rotate(-3deg)}100%{transform:rotate(0)}}\
@keyframes sitecTada{0%{opacity:0;transform:scale(1)}10%{opacity:1;transform:scale(.9) rotate(-3deg)}20%{transform:scale(.9) rotate(-3deg)}30%,50%,70%,90%{transform:scale(1.05) rotate(3deg)}40%,60%,80%{transform:scale(1.05) rotate(-3deg)}100%{transform:scale(1) rotate(0)}}\
@keyframes sitecJello{0%{opacity:0}11%{opacity:1;transform:skewX(-12.5deg) skewY(-12.5deg)}22%{transform:skewX(6.25deg) skewY(6.25deg)}33%{transform:skewX(-3.125deg) skewY(-3.125deg)}44%{transform:skewX(1.5deg) skewY(1.5deg)}55%{transform:skewX(-.7deg) skewY(-.7deg)}100%{opacity:1;transform:skewX(0) skewY(0)}}\
@keyframes sitecFlash{0%{opacity:0}25%{opacity:1}50%{opacity:.3}75%{opacity:1}100%{opacity:1}}\
@keyframes sitecRubberBand{0%{opacity:0;transform:scale(1)}30%{opacity:1;transform:scaleX(1.25) scaleY(.75)}40%{transform:scaleX(.75) scaleY(1.25)}50%{transform:scaleX(1.15) scaleY(.85)}65%{transform:scaleX(.95) scaleY(1.05)}75%{transform:scaleX(1.05) scaleY(.95)}100%{opacity:1;transform:scale(1)}}\
@keyframes sitecWobble{0%{opacity:0;transform:translateX(0) rotate(0)}15%{opacity:1;transform:translateX(-25%) rotate(-5deg)}30%{transform:translateX(20%) rotate(3deg)}45%{transform:translateX(-15%) rotate(-3deg)}60%{transform:translateX(10%) rotate(2deg)}75%{transform:translateX(-5%) rotate(-1deg)}100%{opacity:1;transform:translateX(0) rotate(0)}}\
@keyframes sitecHeartbeat{0%{opacity:0;transform:scale(1)}14%{opacity:1;transform:scale(1.3)}28%{transform:scale(1)}42%{transform:scale(1.3)}70%{transform:scale(1)}100%{opacity:1;transform:scale(1)}}";

/// CSS de mise en forme des types de blocs ajoutés au-delà des 4 historiques
/// (texte/image/bouton/video) -- toujours injecté en mode "blocs" (voir
/// serve_page_view), même si un type donné n'est pas utilisé sur la page.
const BLOCS_EXTRA_CSS: &str = "\
.sitec-row{align-items:flex-start;}\
.sitec-row-item{box-sizing:border-box;min-width:0;}\
.sitec-bloc-titre{margin:0 0 4px;}\
.sitec-bloc-galerie{display:grid;gap:10px;}\
.sitec-cols-2{grid-template-columns:repeat(2,1fr);}\
.sitec-cols-3{grid-template-columns:repeat(3,1fr);}\
.sitec-cols-4{grid-template-columns:repeat(4,1fr);}\
.sitec-bloc-galerie img{width:100%;height:100%;object-fit:cover;border-radius:8px;display:block;}\
.sitec-bloc-carousel{overflow-x:auto;}\
.sitec-carousel-track{display:flex;gap:12px;}\
.sitec-carousel-track img{height:260px;border-radius:8px;flex-shrink:0;}\
.sitec-bloc-audio audio{width:100%;}\
.sitec-bloc-groupe-boutons{display:flex;gap:10px;flex-wrap:wrap;}\
.sitec-bloc-groupe-boutons a{display:inline-block;padding:12px 24px;background:#2e7d32;color:#fff;text-decoration:none;border-radius:8px;font-weight:600;}\
.sitec-bloc-separateur{border:none;border-top:1px solid #dcdfe3;margin:10px 0;}\
.sitec-espace-s{height:20px;}.sitec-espace-m{height:40px;}.sitec-espace-l{height:80px;}.sitec-espace-xl{height:140px;}\
.sitec-bloc-citation{border-left:4px solid #2e7d32;padding:10px 20px;font-style:italic;color:#444;}\
.sitec-bloc-citation cite{display:block;margin-top:8px;font-style:normal;font-weight:600;color:#65676b;}\
.sitec-bloc-liste{padding-left:22px;}\
.sitec-bloc-icone-texte{display:flex;gap:14px;align-items:flex-start;}\
.sitec-bloc-icone-texte>div{min-width:0;}\
.sitec-icone{font-size:28px;line-height:1;}\
.sitec-bloc-carte{border:1px solid #e4e6eb;border-radius:10px;overflow:hidden;}\
.sitec-bloc-carte img{width:100%;display:block;}\
.sitec-carte-corps{padding:16px;}\
.sitec-carte-btn,.sitec-hero-btn,.sitec-cta-btn,.sitec-tarif-btn{display:inline-block;margin-top:10px;padding:10px 20px;background:#2e7d32;color:#fff;text-decoration:none;border-radius:8px;font-weight:600;}\
.sitec-bloc-cartes-grille{display:grid;gap:16px;}\
.sitec-carte-item{border:1px solid #e4e6eb;border-radius:10px;overflow:hidden;}\
.sitec-carte-item img{width:100%;display:block;}\
.sitec-accordeon-item{border:1px solid #e4e6eb;border-radius:8px;padding:12px 16px;margin-bottom:8px;}\
.sitec-accordeon-item summary{cursor:pointer;font-weight:600;}\
.sitec-tab-btns{display:flex;gap:6px;flex-wrap:wrap;margin-bottom:14px;}\
.sitec-tab-btn{padding:9px 16px;border:1px solid #e4e6eb;background:#f0f2f5;border-radius:8px;cursor:pointer;font-weight:600;}\
.sitec-tab-btn.active{background:#2e7d32;color:#fff;border-color:#2e7d32;}\
.sitec-tab-panel{display:none;}.sitec-tab-panel.active{display:block;}\
.sitec-bloc-temoignage{text-align:center;padding:20px;}\
.sitec-avatar{width:64px;height:64px;border-radius:50%;object-fit:cover;margin:0 auto 10px;display:block;}\
.sitec-temoignage-texte{font-style:italic;font-size:17px;}\
.sitec-temoignage-auteur{margin-top:8px;display:flex;flex-direction:column;color:#65676b;}\
.sitec-bloc-barre{margin-bottom:6px;}\
.sitec-barre-label{font-size:13px;font-weight:600;margin-bottom:6px;}\
.sitec-barre-fond{background:#e4e6eb;border-radius:20px;overflow:hidden;height:14px;}\
.sitec-barre-remplie{background:#2e7d32;height:100%;border-radius:20px;transition:width 1s ease;}\
.sitec-bloc-stats-grille{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:16px;text-align:center;}\
.sitec-compteur{text-align:center;}\
.sitec-compteur-val,.sitec-compteur-suffixe{font-size:34px;font-weight:800;color:#2e7d32;}\
.sitec-compteur-label{font-size:13px;color:#65676b;margin-top:4px;}\
.sitec-countdown{text-align:center;}\
.sitec-countdown-label{font-weight:600;margin-bottom:8px;}\
.sitec-countdown-chiffres{display:flex;gap:14px;justify-content:center;font-size:26px;font-weight:800;}\
.sitec-countdown-chiffres span{display:block;font-size:12px;font-weight:600;color:#65676b;}\
.sitec-bloc-cta{text-align:center;background:#f0f2f5;padding:36px 24px;border-radius:12px;}\
.sitec-bloc-reseaux{display:flex;gap:12px;}\
.sitec-bloc-reseaux a{width:40px;height:40px;border-radius:50%;background:#f0f2f5;display:flex;align-items:center;justify-content:center;color:#1c1e21;text-decoration:none;font-weight:800;font-size:12px;}\
.sitec-bloc-maps iframe{width:100%;height:340px;border:0;border-radius:10px;}\
.sitec-bloc-hero{background-size:cover;background-position:center;border-radius:12px;overflow:hidden;}\
.sitec-hero-overlay{background:rgba(0,0,0,.45);color:#fff;padding:60px 30px;text-align:center;border-radius:12px;}\
.sitec-hero-overlay h1{font-size:32px;margin-bottom:10px;}\
.sitec-bloc-chronologie{border-left:3px solid #2e7d32;padding-left:20px;}\
.sitec-chrono-item{margin-bottom:20px;}\
.sitec-chrono-date{font-size:12px;font-weight:700;color:#2e7d32;}\
.sitec-bloc-tarif{border:1px solid #e4e6eb;border-radius:12px;padding:24px;text-align:center;max-width:320px;}\
.sitec-tarif-prix{font-size:32px;font-weight:800;margin:10px 0;}\
.sitec-tarif-prix span{font-size:14px;font-weight:400;color:#65676b;}\
.sitec-bloc-tarif ul{list-style:none;margin:14px 0;padding:0;}\
.sitec-bloc-tarif li{padding:6px 0;border-top:1px solid #f0f2f5;}\
.sitec-bloc-partage{display:flex;gap:10px;flex-wrap:wrap;}\
.sitec-partage-btn{padding:9px 16px;border:1px solid #e4e6eb;border-radius:8px;background:#f0f2f5;cursor:pointer;font-weight:600;text-transform:capitalize;}\
.sitec-bloc-membre{text-align:center;}\
.sitec-membre-role{color:#65676b;font-size:13px;margin-bottom:6px;}\
.sitec-bloc-logos{display:flex;gap:24px;flex-wrap:wrap;align-items:center;justify-content:center;}\
.sitec-bloc-logos img{max-height:48px;filter:grayscale(1);opacity:.7;}\
.sitec-badge{display:inline-block;padding:5px 14px;border-radius:20px;font-size:12px;font-weight:700;}\
.sitec-badge-vert{background:#e3f2e6;color:#2e7d32;}\
.sitec-badge-bleu{background:#e3f0fd;color:#1565c0;}\
.sitec-badge-rouge{background:#fde3e3;color:#c62828;}\
.sitec-badge-jaune{background:#fdf3d8;color:#996f00;}\
.sitec-badge-gris{background:#eceff1;color:#546e7a;}\
.sitec-badge-violet{background:#eee3fd;color:#6a1b9a;}\
.sitec-retour-haut{width:44px;height:44px;border-radius:50%;border:none;background:#2e7d32;color:#fff;font-size:18px;cursor:pointer;}";

/// JS partagé pour les blocs interactifs : compteurs animés, compte à
/// rebours, onglets, partage social. Ajouté à la suite de BLOCS_ANIM_JS
/// (voir serve_page_view), toujours inoffensif si aucun bloc concerné n'est
/// présent sur la page (les querySelectorAll renvoient alors une liste vide).
const BLOCS_EXTRA_JS: &str = "\
<script>(function(){\
var counters=document.querySelectorAll('.sitec-compteur[data-cible]');\
if(counters.length&&'IntersectionObserver' in window){\
var cobs=new IntersectionObserver(function(entries){\
entries.forEach(function(entry){\
if(!entry.isIntersecting)return;\
var el=entry.target,target=parseInt(el.dataset.cible,10)||0;\
var span=el.querySelector('.sitec-compteur-val');\
var start=null,dur=1400;\
function step(ts){\
if(!start)start=ts;\
var p=Math.min((ts-start)/dur,1);\
span.textContent=Math.floor(p*target).toLocaleString();\
if(p<1)requestAnimationFrame(step);else span.textContent=target.toLocaleString();\
}\
requestAnimationFrame(step);\
cobs.unobserve(el);\
});\
},{threshold:0.3});\
counters.forEach(function(el){cobs.observe(el);});\
}else{\
counters.forEach(function(el){\
var span=el.querySelector('.sitec-compteur-val');\
if(span)span.textContent=(parseInt(el.dataset.cible,10)||0).toLocaleString();\
});\
}\
document.querySelectorAll('.sitec-countdown[data-cible]').forEach(function(el){\
var target=new Date(el.dataset.cible).getTime();\
var box=el.querySelector('.sitec-countdown-chiffres');\
if(!box||isNaN(target))return;\
function tick(){\
var diff=target-Date.now();if(diff<0)diff=0;\
var j=Math.floor(diff/86400000),h=Math.floor(diff/3600000)%24,m=Math.floor(diff/60000)%60,s=Math.floor(diff/1000)%60;\
box.innerHTML='<div>'+j+'<span>j</span></div><div>'+h+'<span>h</span></div><div>'+m+'<span>min</span></div><div>'+s+'<span>s</span></div>';\
}\
tick();setInterval(tick,1000);\
});\
document.querySelectorAll('[data-tabs]').forEach(function(wrap){\
wrap.querySelectorAll('.sitec-tab-btn').forEach(function(btn){\
btn.addEventListener('click',function(){\
var idx=btn.dataset.tabIdx;\
wrap.querySelectorAll('.sitec-tab-btn').forEach(function(b){b.classList.toggle('active',b===btn);});\
wrap.querySelectorAll('.sitec-tab-panel').forEach(function(p){p.classList.toggle('active',p.dataset.tabIdx===idx);});\
});\
});\
});\
document.querySelectorAll('.sitec-partage-btn[data-reseau]').forEach(function(btn){\
btn.addEventListener('click',function(){\
var url=encodeURIComponent(window.location.href),titre=encodeURIComponent(document.title);\
var map={\
facebook:'https://www.facebook.com/sharer/sharer.php?u='+url,\
twitter:'https://twitter.com/intent/tweet?url='+url+'&text='+titre,\
linkedin:'https://www.linkedin.com/sharing/share-offsite/?url='+url,\
whatsapp:'https://wa.me/?text='+titre+'%20'+url,\
email:'mailto:?subject='+titre+'&body='+url,\
};\
var t=map[btn.dataset.reseau];if(!t)return;\
if(btn.dataset.reseau==='email'){window.location.href=t;}else{window.open(t,'_blank','noopener,noreferrer,width=600,height=500');}\
});\
});\
})();</script>";

const BLOCS_ANIM_JS: &str = "\
<script>(function(){\
var all=document.querySelectorAll('.sitec-anim');\
var els=Array.prototype.filter.call(all,function(el){\
return (el.dataset.anim&&el.dataset.anim!=='')||el.querySelector('.sitec-kf');\
});\
if(!('IntersectionObserver' in window)||!els.length)return;\
els.forEach(function(el){if(el.dataset.anim&&el.dataset.anim!=='')el.classList.add('sitec-anim-hidden');});\
var obs=new IntersectionObserver(function(entries){\
entries.forEach(function(entry){\
if(entry.isIntersecting){\
if(entry.target.dataset.anim&&entry.target.dataset.anim!==''){\
entry.target.classList.remove('sitec-anim-hidden');\
entry.target.classList.add('sitec-anim-play');\
}\
var kf=entry.target.querySelector('.sitec-kf');\
if(kf)kf.classList.add('sitec-kf-play');\
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
        return u.chars().take(2000).collect();
    }
    // Médias hébergés par VEX lui-même (upload direct ou import depuis
    // ExoDrive, voir `enregistrer_media`) : chemin relatif généré côté
    // serveur, jamais saisi tel quel par l'utilisateur -- sans danger à
    // autoriser en plus des URL absolues. Sans ce cas, un bloc image/
    // vidéo/audio utilisant un fichier importé depuis ses propres fichiers
    // (au lieu d'une URL externe) était silencieusement retiré de la page
    // publiée (voir `render_blocs_html`, `if url.is_empty() { continue; }`).
    if u.starts_with("/static/sitec_media/") {
        return u.chars().take(2000).collect();
    }
    String::new()
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
/// Sanitise un tableau de sous-éléments (galerie, liste, accordéon…) : ne
/// garde que des objets, tronque chaque champ autorisé à sa longueur max,
/// ignore tout champ non listé dans `fields`.
fn san_items(v: &Value, max_items: usize, fields: &[(&str, usize)]) -> Vec<Value> {
    let tronque = |s: &str, n: usize| -> String { s.chars().take(n).collect() };
    v.as_array()
        .map(|a| {
            a.iter()
                .filter(|it| it.is_object())
                .take(max_items)
                .map(|it| {
                    let mut o = serde_json::Map::new();
                    for (k, maxlen) in fields {
                        o.insert((*k).to_string(), json!(tronque(it[*k].as_str().unwrap_or(""), *maxlen)));
                    }
                    Value::Object(o)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Repères de position/déformation (mode Actions -> "translation") : liste
/// {t,x,y,w,h} triée par temps croissant. w/h (largeur/hauteur en px) sont
/// `null` quand ce repère ne fixe pas la dimension (taille naturelle du
/// bloc -- voir `fill_dimension`), distinct d'une largeur/hauteur
/// explicitement nulle (0, ex: un bloc qui se réduit à rien pour
/// "disparaître"). Permettent de faire suivre le texte lors d'une
/// deformation, contrairement a `transform: scale()` qui l'etirerait
/// visuellement sans reflow. Pas de plafond de nombre de reperes autre
/// qu'une limite large anti-abus (200).
fn sanitize_keyframes(v: &Value) -> Vec<Value> {
    let mut out: Vec<(i64, i64, i64, Option<i64>, Option<i64>)> = v
        .as_array()
        .map(|a| {
            a.iter()
                .take(200)
                .map(|k| {
                    (
                        san_int(k, "t", 0, 120_000, 0),
                        san_int(k, "x", -4000, 4000, 0),
                        san_int(k, "y", -4000, 4000, 0),
                        san_int_opt(k, "w", 0, 3000),
                        san_int_opt(k, "h", 0, 3000),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(t, _, _, _, _)| *t);
    out.into_iter()
        .map(|(t, x, y, w, h)| json!({"t": t, "x": x, "y": y, "w": w, "h": h}))
        .collect()
}

/// Comble les repères "null" (dimension non fixée) avec la valeur définie
/// la plus proche (avant puis après) -- pour que l'animation CSS interpole
/// entre des nombres concrets partout au lieu de sauter vers/depuis une
/// valeur absente. Si aucun repère ne fixe la dimension, le résultat reste
/// tout à `None` (aucune animation de taille générée).
fn fill_dimension(mut vals: Vec<Option<i64>>) -> Vec<Option<i64>> {
    let mut last: Option<i64> = None;
    for v in vals.iter_mut() {
        if v.is_some() { last = *v; } else if last.is_some() { *v = last; }
    }
    let mut next: Option<i64> = None;
    for v in vals.iter_mut().rev() {
        if v.is_some() { next = *v; } else if next.is_some() { *v = next; }
    }
    vals
}

/// Les champs numériques de l'éditeur (select "colonnes", input[type=number])
/// arrivent en JSON sous forme de chaîne (valeur DOM native) -- on accepte
/// donc aussi bien un nombre JSON qu'une chaîne numérique.
fn san_int(v: &Value, key: &str, min: i64, max: i64, default: i64) -> i64 {
    v[key]
        .as_i64()
        .or_else(|| v[key].as_f64().map(|f| f as i64))
        .or_else(|| v[key].as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        .map(|n| n.clamp(min, max))
        .unwrap_or(default)
}

/// Comme `san_int`, mais un champ absent/null/chaîne vide reste `None`
/// (non fixé) plutôt que d'être remplacé par une valeur par défaut -- pour
/// les dimensions de repère où "non fixé" doit rester distinct de 0.
fn san_int_opt(v: &Value, key: &str, min: i64, max: i64) -> Option<i64> {
    if v[key].is_null() { return None; }
    v[key]
        .as_i64()
        .or_else(|| v[key].as_f64().map(|f| f as i64))
        .or_else(|| v[key].as_str().and_then(|s| { let s = s.trim(); if s.is_empty() { None } else { s.parse::<i64>().ok() } }))
        .map(|n| n.clamp(min, max))
}

fn san_choice(v: &Value, key: &str, choices: &[&str], default: &str) -> String {
    let s = v[key].as_str().unwrap_or("");
    if choices.contains(&s) { s.to_string() } else { default.to_string() }
}

/// Valide/nettoie le tableau de blocs envoye par l'editeur avant stockage :
/// type reconnu uniquement (34 types), champs bornes en longueur, valeurs
/// numeriques/enum verifiees. Les URLs sont stockees telles quelles (juste
/// tronquees) -- le filtrage http(s) (`safe_url`) se fait au RENDU, pas ici,
/// comme pour les blocs historiques texte/image/bouton/video.
/// Toute entree invalide ou de type inconnu est silencieusement ignoree
/// plutot que de faire echouer tout l'enregistrement.
fn sanitiser_blocs(v: &Value) -> String {
    const MAX_BLOCS: usize = 200;
    const MAX_TEXTE: usize = 20_000;
    const MAX_MOYEN: usize = 1_000;
    const MAX_COURT: usize = 300;
    const MAX_URL: usize = 2000;

    let tronque = |s: &str, n: usize| -> String { s.chars().take(n).collect() };

    let Some(arr) = v.as_array() else {
        return "[]".to_string();
    };

    let mut out: Vec<Value> = Vec::new();
    for b in arr.iter().take(MAX_BLOCS) {
        let kind = b["type"].as_str().unwrap_or("");
        let anim = safe_animation(b["animation"].as_str().unwrap_or(""));
        let mut bloc = match kind {
            "titre" => json!({
                "type": "titre",
                "niveau": san_choice(b, "niveau", &["h1", "h2", "h3", "h4"], "h2"),
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "texte" => json!({
                "type": "texte",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_TEXTE),
            }),
            "image" => json!({
                "type": "image",
                "url": tronque(b["url"].as_str().unwrap_or(""), MAX_URL),
                "alt": tronque(b["alt"].as_str().unwrap_or(""), MAX_COURT),
                "legende": tronque(b["legende"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "galerie" => json!({
                "type": "galerie",
                "colonnes": san_int(b, "colonnes", 2, 4, 3),
                "items": san_items(&b["items"], 24, &[("url", MAX_URL), ("alt", MAX_COURT)]),
            }),
            "carousel" => json!({
                "type": "carousel",
                "items": san_items(&b["items"], 20, &[("url", MAX_URL), ("alt", MAX_COURT)]),
            }),
            "video" => json!({
                "type": "video",
                "url": tronque(b["url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "audio" => json!({
                "type": "audio",
                "url": tronque(b["url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "bouton" => json!({
                "type": "bouton",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_COURT),
                "url": tronque(b["url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "groupe_boutons" => json!({
                "type": "groupe_boutons",
                "items": san_items(&b["items"], 4, &[("texte", MAX_COURT), ("url", MAX_URL)]),
            }),
            "separateur" => json!({ "type": "separateur" }),
            "espace" => json!({
                "type": "espace",
                "hauteur": san_choice(b, "hauteur", &["s", "m", "l", "xl"], "m"),
            }),
            "citation" => json!({
                "type": "citation",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_MOYEN),
                "auteur": tronque(b["auteur"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "liste" => json!({
                "type": "liste",
                "style": san_choice(b, "style", &["puce", "numero"], "puce"),
                "items": san_items(&b["items"], 30, &[("texte", MAX_COURT)]),
            }),
            "icone_texte" => json!({
                "type": "icone_texte",
                "icone": safe_icon(b["icone"].as_str().unwrap_or("")),
                "titre": tronque(b["titre"].as_str().unwrap_or(""), MAX_COURT),
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_MOYEN),
            }),
            "carte" => json!({
                "type": "carte",
                "url_image": tronque(b["url_image"].as_str().unwrap_or(""), MAX_URL),
                "titre": tronque(b["titre"].as_str().unwrap_or(""), MAX_COURT),
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_MOYEN),
                "bouton_texte": tronque(b["bouton_texte"].as_str().unwrap_or(""), MAX_COURT),
                "bouton_url": tronque(b["bouton_url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "cartes_grille" => json!({
                "type": "cartes_grille",
                "colonnes": san_int(b, "colonnes", 2, 4, 3),
                "items": san_items(&b["items"], 12, &[("url_image", MAX_URL), ("titre", MAX_COURT), ("texte", MAX_MOYEN)]),
            }),
            "accordeon" => json!({
                "type": "accordeon",
                "items": san_items(&b["items"], 20, &[("question", MAX_COURT), ("reponse", MAX_MOYEN)]),
            }),
            "onglets" => json!({
                "type": "onglets",
                "items": san_items(&b["items"], 10, &[("titre", MAX_COURT), ("contenu", MAX_MOYEN)]),
            }),
            "temoignage" => json!({
                "type": "temoignage",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_MOYEN),
                "nom": tronque(b["nom"].as_str().unwrap_or(""), MAX_COURT),
                "role": tronque(b["role"].as_str().unwrap_or(""), MAX_COURT),
                "avatar_url": tronque(b["avatar_url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "barre_progression" => json!({
                "type": "barre_progression",
                "label": tronque(b["label"].as_str().unwrap_or(""), MAX_COURT),
                "pourcentage": san_int(b, "pourcentage", 0, 100, 50),
            }),
            "compteur" => json!({
                "type": "compteur",
                "valeur": san_int(b, "valeur", 0, 999_999_999, 0),
                "label": tronque(b["label"].as_str().unwrap_or(""), MAX_COURT),
                "suffixe": tronque(b["suffixe"].as_str().unwrap_or(""), 10),
            }),
            "stats_grille" => json!({
                "type": "stats_grille",
                "items": san_items(&b["items"], 8, &[("valeur", 12), ("label", MAX_COURT), ("suffixe", 10)]),
            }),
            "compte_rebours" => json!({
                "type": "compte_rebours",
                "date_cible": tronque(b["date_cible"].as_str().unwrap_or(""), 20),
                "label": tronque(b["label"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "appel_action" => json!({
                "type": "appel_action",
                "titre": tronque(b["titre"].as_str().unwrap_or(""), MAX_COURT),
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_MOYEN),
                "bouton_texte": tronque(b["bouton_texte"].as_str().unwrap_or(""), MAX_COURT),
                "bouton_url": tronque(b["bouton_url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "reseaux_sociaux" => {
                let mut o = serde_json::Map::new();
                o.insert("type".to_string(), json!("reseaux_sociaux"));
                for r in ["facebook", "twitter", "instagram", "youtube", "linkedin", "github", "tiktok"] {
                    o.insert(r.to_string(), json!(tronque(b[r].as_str().unwrap_or(""), MAX_URL)));
                }
                Value::Object(o)
            }
            "carte_google_maps" => json!({
                "type": "carte_google_maps",
                "adresse": tronque(b["adresse"].as_str().unwrap_or(""), MAX_COURT),
            }),
            "banniere_hero" => json!({
                "type": "banniere_hero",
                "url_image": tronque(b["url_image"].as_str().unwrap_or(""), MAX_URL),
                "titre": tronque(b["titre"].as_str().unwrap_or(""), MAX_COURT),
                "soustitre": tronque(b["soustitre"].as_str().unwrap_or(""), MAX_MOYEN),
                "bouton_texte": tronque(b["bouton_texte"].as_str().unwrap_or(""), MAX_COURT),
                "bouton_url": tronque(b["bouton_url"].as_str().unwrap_or(""), MAX_URL),
            }),
            "chronologie" => json!({
                "type": "chronologie",
                "items": san_items(&b["items"], 20, &[("date", MAX_COURT), ("titre", MAX_COURT), ("texte", MAX_MOYEN)]),
            }),
            "tarifs" => json!({
                "type": "tarifs",
                "titre": tronque(b["titre"].as_str().unwrap_or(""), MAX_COURT),
                "prix": tronque(b["prix"].as_str().unwrap_or(""), MAX_COURT),
                "periode": tronque(b["periode"].as_str().unwrap_or(""), MAX_COURT),
                "bouton_texte": tronque(b["bouton_texte"].as_str().unwrap_or(""), MAX_COURT),
                "bouton_url": tronque(b["bouton_url"].as_str().unwrap_or(""), MAX_URL),
                "items": san_items(&b["items"], 12, &[("texte", MAX_COURT)]),
            }),
            "partage_social" => {
                const WHITELIST: &[&str] = &["facebook", "twitter", "linkedin", "whatsapp", "email"];
                let reseaux: Vec<Value> = b["reseaux"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .filter(|s| WHITELIST.contains(s))
                            .take(5)
                            .map(|s| json!(s))
                            .collect()
                    })
                    .unwrap_or_else(|| WHITELIST.iter().map(|s| json!(*s)).collect());
                json!({ "type": "partage_social", "reseaux": reseaux })
            }
            "membre_equipe" => json!({
                "type": "membre_equipe",
                "avatar_url": tronque(b["avatar_url"].as_str().unwrap_or(""), MAX_URL),
                "nom": tronque(b["nom"].as_str().unwrap_or(""), MAX_COURT),
                "role": tronque(b["role"].as_str().unwrap_or(""), MAX_COURT),
                "bio": tronque(b["bio"].as_str().unwrap_or(""), MAX_MOYEN),
            }),
            "logos" => json!({
                "type": "logos",
                "items": san_items(&b["items"], 20, &[("url", MAX_URL)]),
            }),
            "badge" => json!({
                "type": "badge",
                "texte": tronque(b["texte"].as_str().unwrap_or(""), MAX_COURT),
                "couleur": san_choice(b, "couleur", &["vert", "bleu", "rouge", "jaune", "gris", "violet"], "vert"),
            }),
            "retour_haut" => json!({ "type": "retour_haut" }),
            _ => continue,
        };
        bloc["animation"] = json!(anim);
        // Disposition en ligne (plusieurs blocs cote a cote) : "ligne" est un
        // identifiant partage par les blocs d'une meme rangee (chaine libre,
        // generee cote client), "largeur" leur part en % de la rangee,
        // "espace" l'ecart en px entre blocs de la rangee. "anim_delai"
        // decale le declenchement de l'animation d'entree (mode "actions").
        bloc["ligne"] = json!(tronque(b["ligne"].as_str().unwrap_or(""), 20));
        bloc["largeur"] = json!(san_int(b, "largeur", 10, 100, 100));
        // Decalage a gauche (%) quand le bloc est seul sur sa ligne mais
        // retreci (largeur<100) -- permet de le positionner horizontalement
        // au lieu de le laisser colle au bord gauche.
        bloc["decalage"] = json!(san_int(b, "decalage", 0, 90, 0));
        // Position libre : sort le bloc du flux normal (plus contraint a
        // etre avant/apres/a cote d'un autre), positionne en absolu.
        bloc["libre"] = json!(b["libre"].as_bool().unwrap_or(false));
        bloc["pos_x"] = json!(san_int(b, "pos_x", -2000, 8000, 0));
        bloc["pos_y"] = json!(san_int(b, "pos_y", -2000, 20000, 0));
        // Rotation (degrés, blocs libres uniquement).
        bloc["rotation"] = json!(san_int(b, "rotation", 0, 359, 0));
        bloc["espace"] = json!(san_int(b, "espace", 0, 1000, 16));
        bloc["anim_delai"] = json!(san_int(b, "anim_delai", 0, 120_000, 0));
        // Hauteur minimale forcee (px), 0 = automatique (contenu).
        bloc["hauteur"] = json!(san_int(b, "hauteur", 0, 2000, 0));
        bloc["keyframes"] = json!(sanitize_keyframes(&b["keyframes"]));
        // Boucle l'animation de repères (translation/déformation) au lieu de
        // s'arrêter une fois sur le dernier repère.
        bloc["boucle"] = json!(b["boucle"].as_bool().unwrap_or(false));
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
                ],
            );
            let html = html.replacen("{{I18N_JS}}", &i18n_js, 1);
            // Cache-Control manquant ici (present seulement sur le chemin
            // d'erreur juste en dessous, via html_resp) -- trouve comme la
            // vraie cause probable des "rien n'est applique" repetes tout
            // au long de la session malgre des deploiements verifies cote
            // serveur : le navigateur pouvait garder cette page (servie a
            // CHAQUE chargement normal de l'editeur) en cache indefiniment.
            Response::from_string(html)
                .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
                .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-cache, no-store, must-revalidate").unwrap())
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
    // no-cache : evite qu'un navigateur (ou Apache en reverse proxy) serve
    // une vieille version de l'editeur/d'une page publiee apres deploiement
    // d'un correctif -- l'app se met a jour souvent, ces pages ne doivent
    // jamais rester en cache silencieusement.
    Response::from_string(body)
        .with_status_code(code)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
        .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-cache, no-store, must-revalidate").unwrap())
}

fn err500() -> Response<std::io::Cursor<Vec<u8>>> {
    json_resp(json!({"success":false,"error":"Erreur serveur"}), 500)
}