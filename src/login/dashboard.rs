// ══════════════════════════════════════════════════════════════════
// login/dashboard.rs — Handler dashboard VEX
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{compter_lignes, selectionner, verifier_connexion, DbPool};
use crate::config_loader::VexConfig;
use crate::function::{
    build_nav_html, get_privilege_details_json, get_user_preferences, NavContext,
};
use crate::utils::strip_port;
use std::fs;
use tiny_http::{Request, Response};

// ── Utilitaires ───────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.2} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} octets", bytes)
    }
}

fn time_ago(datetime: &str) -> String {
    let ts = chrono::NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0);
    let diff = chrono::Utc::now().timestamp() - ts;
    if diff < 60 {
        "à l'instant".to_string()
    } else if diff < 3600 {
        format!("{} min", diff / 60)
    } else if diff < 86400 {
        format!("{} h", diff / 3600)
    } else if diff < 604800 {
        format!("{} j", diff / 86400)
    } else {
        chrono::NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M:%S")
            .map(|dt| dt.format("%d/%m/%Y").to_string())
            .unwrap_or_else(|_| datetime.to_string())
    }
}

fn file_icon(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" | "png" | "gif" => "fa-file-image",
        "mp4" | "webm" => "fa-file-video",
        "pdf" => "fa-file-pdf",
        "doc" | "docx" => "fa-file-word",
        "xls" | "xlsx" => "fa-file-excel",
        "zip" | "rar" => "fa-file-zipper",
        "php" | "js" | "html" => "fa-file-code",
        _ => "fa-file",
    }
}

fn he(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn v_str<'a>(map: &'a std::collections::HashMap<String, serde_json::Value>, k: &str) -> &'a str {
    map.get(k).and_then(|v| v.as_str()).unwrap_or("")
}
fn v_u64(map: &std::collections::HashMap<String, serde_json::Value>, k: &str) -> u64 {
    map.get(k).and_then(|v| v.as_u64()).unwrap_or(0)
}
fn v_i64(map: &std::collections::HashMap<String, serde_json::Value>, k: &str) -> i64 {
    map.get(k).and_then(|v| v.as_i64()).unwrap_or(0)
}

// ── Blocs HTML ────────────────────────────────────────────────────

fn build_welcome_name(nom: &str, couleur: &str) -> String {
    if couleur == "fona" {
        format!(r#"<div class="welcome-name fona">{}</div>"#, he(nom))
    } else {
        format!(
            r#"<div class="welcome-name" style="color:{};">{}</div>"#,
            he(couleur),
            he(nom)
        )
    }
}

fn build_admin_card(pool: &DbPool) -> String {
    let logs = selectionner(
        pool,
        "loginc",
        &[],
        &["email", "nom", "datecra", "pc"],
        Some("datecra DESC"),
        Some(20),
    );
    let now = chrono::Utc::now().timestamp();
    let logs: Vec<_> = logs
        .into_iter()
        .filter(|row| {
            let ts =
                chrono::NaiveDateTime::parse_from_str(v_str(row, "datecra"), "%Y-%m-%d %H:%M:%S")
                    .map(|dt| dt.and_utc().timestamp())
                    .unwrap_or(0);
            now - ts < 3600
        })
        .take(5)
        .collect();

    let content = if logs.is_empty() {
        r#"<div class="empty-state"><i class="fas fa-clipboard-list"></i><p>Aucune connexion active</p></div>"#.to_string()
    } else {
        let mut rows = String::new();
        for log in &logs {
            rows.push_str(&format!(
                r#"<div class="log-item">
                    <div class="item-left">
                        <i class="fas fa-user-shield item-icon"></i>
                        <div class="item-info">
                            <div class="item-title">{nom}</div>
                            <div class="item-subtitle">{email} — IP: {pc}</div>
                        </div>
                    </div>
                    <div class="item-right">
                        <span class="item-badge badge-success">Actif</span>
                        <span class="item-time">{time}</span>
                    </div>
                </div>"#,
                nom = he(v_str(log, "nom")),
                email = he(v_str(log, "email")),
                pc = he(v_str(log, "pc")),
                time = time_ago(v_str(log, "datecra")),
            ));
        }
        rows.push_str(r#"<div class="show-more"><a href="/admin/"><i class="fas fa-chevron-right"></i> Tous les logs</a></div>"#);
        rows
    };

    format!(
        r#"<div class="card">
            <div class="card-header">
                <div class="card-icon" style="background:rgba(211,47,47,0.1);color:#d32f2f;">
                    <i class="fas fa-shield-alt"></i>
                </div>
                <div class="card-header-text">
                    <div class="card-title">Administration</div>
                    <div class="card-subtitle">Dernières connexions</div>
                </div>
                <a href="/admin/" class="btn-action"><i class="fas fa-cog"></i> Gérer</a>
            </div>
            <div class="card-content">{content}</div>
        </div>"#,
        content = content,
    )
}

fn build_exodrive_stats(pool: &DbPool, user_id: i64) -> String {
    let total = compter_lignes(
        pool,
        "fichiers",
        &[("id_utilisateur", mysql::Value::from(user_id))],
    );
    let all = selectionner(
        pool,
        "fichiers",
        &[("id_utilisateur", mysql::Value::from(user_id))],
        &["taille"],
        None,
        None,
    );
    let size: u64 = all.iter().map(|f| v_u64(f, "taille")).sum();
    format!(
        "{} fichier{} — {}",
        total,
        if total > 1 { "s" } else { "" },
        format_size(size)
    )
}

fn build_exodrive_files(pool: &DbPool, user_id: i64) -> String {
    let mut files = selectionner(
        pool,
        "fichiers",
        &[("id_utilisateur", mysql::Value::from(user_id))],
        &["id", "nom_fichier", "taille", "date"],
        Some("date DESC"),
        Some(5),
    );
    if files.len() < 5 {
        let rem = (5 - files.len()) as u64;
        let pub_ = selectionner(
            pool,
            "fichiers",
            &[("visble", mysql::Value::from("public"))],
            &["id", "nom_fichier", "taille", "date"],
            Some("date DESC"),
            Some(rem),
        );
        files.extend(pub_);
    }
    if files.is_empty() {
        return r#"<div class="empty-state"><i class="fas fa-folder-open"></i><p>Aucun fichier</p></div>"#.to_string();
    }
    let mut html = String::new();
    for f in &files {
        let id = v_i64(f, "id");
        let nom = v_str(f, "nom_fichier");
        html.push_str(&format!(
            r#"<div class="file-item" onclick="window.location.href='/tel/?id={id}'">
                <div class="item-left">
                    <i class="fas {ico} item-icon"></i>
                    <div class="item-info">
                        <div class="item-title">{nom}</div>
                        <div class="item-subtitle">{size}</div>
                    </div>
                </div>
                <div class="item-right">
                    <span class="item-time">{time}</span>
                    <a href="/tel/?id={id}&amp;download=1" class="btn-action" onclick="event.stopPropagation();"><i class="fas fa-download"></i></a>
                    <a href="/tel/?id={id}" class="btn-action" onclick="event.stopPropagation();"><i class="fas fa-eye"></i></a>
                </div>
            </div>"#,
            id   = id,
            ico  = file_icon(nom),
            nom  = he(nom),
            size = format_size(v_u64(f, "taille")),
            time = time_ago(v_str(f, "date")),
        ));
    }
    html.push_str(r#"<div class="show-more"><a href="/tel/"><i class="fas fa-chevron-right"></i> Tous les fichiers</a></div>"#);
    html
}

fn build_mail_unread(pool: &DbPool, email: &str) -> String {
    compter_lignes(
        pool,
        "mail",
        &[
            ("a@", mysql::Value::from(email)),
            ("read", mysql::Value::from(0i64)),
            ("folder", mysql::Value::from("inbox")),
        ],
    )
    .to_string()
}

fn build_mail_messages(pool: &DbPool, email: &str) -> String {
    let msgs = selectionner(
        pool,
        "mail",
        &[
            ("a@", mysql::Value::from(email)),
            ("folder", mysql::Value::from("inbox")),
        ],
        &["cd@", "objet", "date"],
        Some("date DESC"),
        Some(5),
    );
    if msgs.is_empty() {
        return r#"<div class="empty-state"><i class="fas fa-inbox"></i><p>Aucun email récent</p></div>"#.to_string();
    }
    let mut html = String::new();
    for m in &msgs {
        let objet = v_str(m, "objet");
        let objet = if objet.trim().is_empty() {
            "(Sans objet)"
        } else {
            objet
        };
        html.push_str(&format!(
            r#"<div class="mail-item" onclick="window.location.href='/mess/vexmail'">
                <div class="item-left">
                    <i class="fas fa-envelope item-icon"></i>
                    <div class="item-info">
                        <div class="item-title">{objet}</div>
                        <div class="item-subtitle">De: {from}</div>
                    </div>
                </div>
                <div class="item-right">
                    <span class="item-time">{time}</span>
                    <a href="/mess/vexmail" class="btn-action" onclick="event.stopPropagation();"><i class="fas fa-eye"></i></a>
                </div>
            </div>"#,
            objet = he(objet),
            from  = he(v_str(m, "cd@")),
            time  = time_ago(v_str(m, "date")),
        ));
    }
    html.push_str(r#"<div class="show-more"><a href="/mess/vexmail"><i class="fas fa-chevron-right"></i> Tous les emails</a></div>"#);
    html
}

fn build_sitec_stats(pool: &DbPool, user_id: i64) -> String {
    let n = compter_lignes(pool, "sitec", &[("user_id", mysql::Value::from(user_id))]);
    format!("{} site{}", n, if n > 1 { "s" } else { "" })
}

fn build_sitec_sites(pool: &DbPool, user_id: i64) -> String {
    let mut sites = selectionner(
        pool,
        "sitec",
        &[("user_id", mysql::Value::from(user_id))],
        &["idpage", "nompage", "urlpage", "popular"],
        Some("popular DESC"),
        Some(5),
    );
    if sites.len() < 5 {
        let rem = (5 - sites.len()) as u64;
        let pub_ = selectionner(
            pool,
            "sitec",
            &[("porb", mysql::Value::from(1i64))],
            &["idpage", "nompage", "urlpage", "popular"],
            Some("popular DESC"),
            Some(rem),
        );
        sites.extend(pub_);
    }
    if sites.is_empty() {
        return r#"<div class="empty-state"><i class="fas fa-globe"></i><p>Aucun site créé</p></div>"#.to_string();
    }
    let mut html = String::new();
    for s in &sites {
        let url = format!("/sitec/pages/{}", v_str(s, "urlpage"));
        html.push_str(&format!(
            r#"<div class="file-item" onclick="window.open('{url}','_blank')">
                <div class="item-left">
                    <i class="fas fa-globe item-icon"></i>
                    <div class="item-info">
                        <div class="item-title">{nom}</div>
                        <div class="item-subtitle">{urlpage}</div>
                    </div>
                </div>
                <div class="item-right">
                    <span class="item-badge badge-info">{pop} vues</span>
                    <a href="{url}" target="_blank" class="btn-action" onclick="event.stopPropagation();"><i class="fas fa-external-link-alt"></i></a>
                </div>
            </div>"#,
            url     = he(&url),
            nom     = he(v_str(s, "nompage")),
            urlpage = he(v_str(s, "urlpage")),
            pop     = v_u64(s, "popular"),
        ));
    }
    html.push_str(r#"<div class="show-more"><a href="/sitec/"><i class="fas fa-chevron-right"></i> Tous les sites</a></div>"#);
    html
}

// ── Handler principal ─────────────────────────────────────────────

pub fn handle_request(request: Request, pool: &DbPool, _config: &VexConfig, remote: &str) {
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");
    let remote_ip = strip_port(remote);

    // ── Auth ─────────────────────────────────────────────────────
    let user = match verifier_connexion(pool, &cookie_val, &remote_ip, &user_agent) {
        Some(u) => u,
        None => {
            let _ = request.respond(
                Response::empty(302)
                    .with_header(tiny_http::Header::from_bytes("Location", "/login").unwrap()),
            );
            return;
        }
    };

    let user_id = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let user_privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
    let user_email = user
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let user_nom = user
        .get("nom")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let pd = get_privilege_details_json(user_privilege);
    let couleur = pd["couleur_privilege"]
        .as_str()
        .unwrap_or("#ffffff")
        .to_string();

    let prefs = get_user_preferences(pool, user_id);
    let theme = if prefs.teme == 1 { "dark" } else { "light" };

    // ── Navbar — autonome (user_id fourni = pas de 2e requête cookie) ──
    let nav_ctx = NavContext {
        pool,
        user_id: Some(user_id), // déjà connu, évite une requête
        page_key: "dashboard",
        cookie_val: &cookie_val,
        remote_ip: &remote_ip,
        user_agent: &user_agent,
        query_id: None,
        apps: vec![],
        admin_apps: vec![],
    };
    let nav_html = build_nav_html(&nav_ctx);

    // ── Template HTML ────────────────────────────────────────────
    let template = match fs::read_to_string("static/login/dashboard.html") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[dashboard] template introuvable: {}", e);
            let _ = request.respond(
                Response::from_string("Erreur interne : dashboard.html introuvable")
                    .with_status_code(500),
            );
            return;
        }
    };

    let admin_card = if user_privilege < 8 {
        build_admin_card(pool)
    } else {
        String::new()
    };

    // ── Remplace tous les placeholders + navbar ──────────────────
    let html = template
        .replace("__NAV_HTML__", &nav_html)
        .replace("{{THEME}}", theme)
        .replace(
            "{{WELCOME_NAME_BLOCK}}",
            &build_welcome_name(&user_nom, &couleur),
        )
        .replace("{{ADMIN_CARD}}", &admin_card)
        .replace("{{EXODRIVE_STATS}}", &build_exodrive_stats(pool, user_id))
        .replace("{{EXODRIVE_FILES}}", &build_exodrive_files(pool, user_id))
        .replace("{{MAIL_UNREAD}}", &build_mail_unread(pool, &user_email))
        .replace("{{MAIL_MESSAGES}}", &build_mail_messages(pool, &user_email))
        .replace("{{SITEC_STATS}}", &build_sitec_stats(pool, user_id))
        .replace("{{SITEC_SITES}}", &build_sitec_sites(pool, user_id));

    let _ = request.respond(
        Response::from_string(html)
            .with_status_code(200)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ),
    );
}
