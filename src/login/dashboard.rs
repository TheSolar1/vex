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

/// Un evenement est suivi sauf si l'utilisateur l'a explicitement coupe.
fn evt_actif(prefs: &crate::function::UserPrefs, cle: &str) -> bool {
    prefs
        .dashboard_events
        .get(cle)
        .map(|v| v.as_i64().unwrap_or(1) != 0)
        .unwrap_or(true)
}

/// Etiquette de la tuile Administration.
/// Reprend exactement le gabarit de "Actif" (.item-badge + .badge-*),
/// seule la teinte change : pas de couleur en dur cote Rust.
fn etiquette(texte: &str, niveau: &str) -> String {
    let classe = match niveau {
        "ok" => "badge-success",
        "attention" => "badge-warn",
        "alerte" => "badge-danger",
        "neutre" => "badge-neutral",
        _ => "badge-info",
    };
    format!(
        r#"<span class="item-badge {c}">{t}</span>"#,
        c = classe,
        t = he(texte)
    )
}

/// Horodatage relatif affiche a droite d'une ligne, recalcule cote
/// navigateur toutes les 10 s grace a data-ts.
fn horodatage(ts: i64) -> String {
    format!(
        r#"<span class="item-time" data-ts="{ts}">{txt}</span>"#,
        ts = ts,
        txt = he(&depuis(ts))
    )
}

/// "il y a 3 min" a partir d'un timestamp Unix.
fn depuis(ts: i64) -> String {
    if ts <= 0 {
        return "date inconnue".into();
    }
    let d = chrono::Utc::now().timestamp() - ts;
    if d < 0 {
        return "a l'instant".into();
    }
    if d < 60 {
        return "a l'instant".into();
    }
    if d < 3600 {
        return format!("il y a {} min", d / 60);
    }
    if d < 86400 {
        return format!("il y a {} h", d / 3600);
    }
    format!("il y a {} j", d / 86400)
}

/// Timestamp Unix d'une date "YYYY-MM-DD HH:MM:SS".
fn ts_de(datetime: &str) -> i64 {
    chrono::NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d %H:%M:%S")
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0)
}

/// Date de derniere modification d'un fichier, en timestamp Unix.
fn ts_fichier(chemin: &str) -> i64 {
    std::fs::metadata(chemin)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Une ligne d'information de la tuile Administration.
fn ligne_info(icone: &str, titre: &str, sous_titre: &str, droite: &str) -> String {
    ligne_info_coloree(icone, titre, sous_titre, droite, "", "")
}

/// Variante ou le pseudo porte lui-meme la couleur du privilege,
/// plutot qu'une etiquette posee a cote.
fn ligne_info_coloree(
    icone: &str,
    titre: &str,
    sous_titre: &str,
    droite: &str,
    couleur: &str,
    infobulle: &str,
) -> String {
    // Le privilege 1 utilise la classe animee "fona" et non une couleur.
    let fona = couleur == "fona";
    let classe_titre = if fona { " item-title fona" } else { " item-title" };
    let style_titre = if couleur.is_empty() || fona {
        String::new()
    } else {
        format!(" style=\"color:{}\"", he(couleur))
    };
    let style_icone = if couleur.is_empty() || fona {
        String::new()
    } else {
        format!(" style=\"color:{}\"", he(couleur))
    };
    let bulle = if infobulle.is_empty() {
        String::new()
    } else {
        format!(" title=\"{}\"", he(infobulle))
    };
    format!(
        r#"<div class="log-item"{bulle}>
            <div class="item-left">
                <i class="fas fa-{ico} item-icon"{sico}></i>
                <div class="item-info">
                    <div class="{ctit}"{stit}>{t}</div>
                    <div class="item-subtitle">{st}</div>
                </div>
            </div>
            <div class="item-right">{d}</div>
        </div>"#,
        ico = he(icone),
        t = he(titre),
        st = he(sous_titre),
        d = droite,
        bulle = bulle,
        sico = style_icone,
        stit = style_titre,
        ctit = classe_titre.trim(),
    )
}

fn build_admin_card(
    pool: &DbPool,
    prefs: &crate::function::UserPrefs,
    user_privilege: i64,
) -> String {
    // Seul le fondateur voit le fondateur : ni son compte dans la liste
    // du staff, ni ses connexions dans le journal.
    let voit_fondateur = user_privilege <= 1;
    let emails_fondateur: Vec<String> = if voit_fondateur {
        Vec::new()
    } else {
        selectionner(
            pool,
            "login",
            &[("privilege", mysql::Value::from(1i64))],
            &["email"],
            None,
            None,
        )
        .iter()
        .map(|u| v_str(u, "email").to_lowercase())
        .collect()
    };

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
        .filter(|row| {
            voit_fondateur
                || !emails_fondateur.contains(&v_str(row, "email").to_lowercase())
        })
        .take(5)
        .collect();

    let maintenant = chrono::Utc::now().timestamp();
    let mut content = String::new();

    // ── Chiffres cles ─────────────────────────────────────────────
    if evt_actif(prefs, "stats") {
        let nb_users = compter_lignes(pool, "login", &[]);
        let nb_fichiers = compter_lignes(pool, "fichiers", &[]);
        let nb_pages = compter_lignes(pool, "sitec", &[]);
        content.push_str(&ligne_info(
            "chart-simple",
            "Vue d'ensemble",
            &format!(
                "{} comptes — {} fichiers — {} pages",
                nb_users, nb_fichiers, nb_pages
            ),
            &format!(
                "{}{}",
                etiquette(
                    &format!("{} session{}", logs.len(), if logs.len() > 1 { "s" } else { "" }),
                    "info"
                ),
                horodatage(maintenant)
            ),
        ));
    }

    // ── Comptes a privilege eleve ─────────────────────────────────
    if evt_actif(prefs, "admins") {
        let staff = selectionner(
            pool,
            "login",
            &[],
            &["nom", "email", "privilege"],
            Some("privilege ASC"),
            Some(50),
        );
        let staff: Vec<_> = staff
            .into_iter()
            .filter(|u| {
                let p = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
                p <= 3 && (p != 1 || voit_fondateur)
            })
            .collect();
        if staff.is_empty() {
            content.push_str(&ligne_info(
                "user-shield",
                "Aucun administrateur",
                "Aucun compte de privilege 1 a 3",
                &etiquette("Attention", "#f59e0b"),
            ));
        } else {
            for u in staff.iter().take(4) {
                let p = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
                // Couleur et libelle : source unique = get_privilege_details.
                let pd = crate::function::get_privilege_details(p);
                let (lib, coul) = (pd.nom, pd.couleur);
                // Le role se lit a la couleur du pseudo. A droite on met
                // la derniere fois que ce compte s'est connecte.
                let derniere = selectionner(
                    pool,
                    "loginc",
                    &[("email", mysql::Value::from(v_str(u, "email")))],
                    &["datecra"],
                    Some("datecra DESC"),
                    Some(1),
                );
                let ts = derniere
                    .first()
                    .map(|r| ts_de(v_str(r, "datecra")))
                    .unwrap_or(0);
                let droite = if ts > 0 {
                    horodatage(ts)
                } else {
                    r#"<span class="item-time">jamais connecte</span>"#.to_string()
                };
                content.push_str(&ligne_info_coloree(
                    "user-shield",
                    v_str(u, "nom"),
                    v_str(u, "email"),
                    &droite,
                    coul,
                    lib,
                ));
            }
        }
    }

    // ── Etat du serveur (maintenance / debug) ─────────────────────
    if evt_actif(prefs, "etat") {
        let cfg: serde_json::Value = std::fs::read_to_string("config.json")
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let maintenance = cfg["app"]["maintenance_mode"].as_bool().unwrap_or(false);
        let debug = cfg["app"]["debug_mode"].as_bool().unwrap_or(false);
        let mut badges = String::new();
        if maintenance {
            badges.push_str(&etiquette("Maintenance", "alerte"));
        }
        if debug {
            badges.push_str(&etiquette("Debug", "attention"));
        }
        if badges.is_empty() {
            badges = etiquette("Normal", "ok");
        }
        // Horodatage = derniere modification de la configuration.
        badges.push_str(&horodatage(ts_fichier("config.json")));
        content.push_str(&ligne_info(
            "server",
            "Etat du serveur",
            cfg["app"]["version"].as_str().unwrap_or("VEX"),
            &badges,
        ));
    }

    // ── Infos publiees par les extensions ─────────────────────────
    if evt_actif(prefs, "extensions") {
        let ts_config = ts_fichier("config.json");
        for (id, e) in crate::function::extensions_actives("config.json") {
            let infos = match e.get("admin_infos").and_then(|v| v.as_array()) {
                Some(a) => a.clone(),
                None => continue,
            };
            for info in infos.iter().take(3) {
                let niveau = info.get("niveau").and_then(|v| v.as_str()).unwrap_or("info");
                let valeur = info.get("valeur").and_then(|v| v.as_str()).unwrap_or("");
                // "maj" (timestamp Unix) publie par l'extension ;
                // a defaut, date du fichier de configuration.
                let ts = info
                    .get("maj")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(ts_config);
                content.push_str(&ligne_info(
                    info.get("icone").and_then(|v| v.as_str()).unwrap_or("puzzle-piece"),
                    info.get("label").and_then(|v| v.as_str()).unwrap_or(id.as_str()),
                    info.get("detail").and_then(|v| v.as_str()).unwrap_or(id.as_str()),
                    &format!("{}{}", etiquette(valeur, niveau), horodatage(ts)),
                ));
            }
        }
    }

    // ── Connexions recentes ───────────────────────────────────────
    let connexions = if !evt_actif(prefs, "connexions") {
        String::new()
    } else if logs.is_empty() {
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
                        <span class="item-time" data-ts="{ts}">{time}</span>
                    </div>
                </div>"#,
                nom = he(v_str(log, "nom")),
                email = he(v_str(log, "email")),
                pc = he(v_str(log, "pc")),
                time = time_ago(v_str(log, "datecra")),
                ts = ts_de(v_str(log, "datecra")),
            ));
        }
        rows
    };
    content.push_str(&connexions);
    if content.trim().is_empty() {
        content = r#"<div class="empty-state"><i class="fas fa-eye-slash"></i><p>Aucun evenement selectionne — reglez l'affichage dans Mon compte.</p></div>"#.to_string();
    }
    content.push_str(&format!(
        r#"<div class="show-more" style="display:flex;align-items:center;justify-content:space-between;gap:10px">
            <a href="/admin/"><i class="fas fa-chevron-right"></i> Tous les logs</a>
            <span class="item-time" data-ts="{ts}" title="Rafraichissement automatique toutes les 30 s">Mis a jour {txt}</span>
        </div>"#,
        ts = maintenant,
        txt = he(&depuis(maintenant)),
    ));

    format!(
        r#"<div class="card" data-tuile="admin" id="carte-admin">
            <div class="card-header">
                <div class="card-icon" style="background:rgba(211,47,47,0.1);color:#d32f2f;">
                    <i class="fas fa-shield-alt"></i>
                </div>
                <div class="card-header-text">
                    <div class="card-title">Administration</div>
                    <div class="card-subtitle">Etat du service</div>
                </div>
                <a href="/admin/" class="btn-action"><i class="fas fa-cog"></i> Gérer</a>
            </div>
            <div class="card-content">{content}</div>
        </div>"#,
        content = content,
    )
}

/// Tuiles declarees par les extensions actives (config.json -> dashboard_tile).
/// Aucune recompilation necessaire : le bloc est lu a chaque affichage.
fn build_ext_tiles() -> String {
    let mut out = String::new();
    for (id, e) in crate::function::extensions_actives("config.json") {
        let t = match e.get("dashboard_tile") {
            Some(t) if t.is_object() => t,
            _ => continue,
        };
        let titre = t.get("titre").and_then(|v| v.as_str()).unwrap_or(id.as_str());
        let sous = t.get("sous_titre").and_then(|v| v.as_str()).unwrap_or("");
        let icone = t.get("icone").and_then(|v| v.as_str()).unwrap_or("puzzle-piece");
        let couleur = t.get("couleur").and_then(|v| v.as_str()).unwrap_or("#3b82f6");
        let url = t
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("/ext/{}", id));

        let mut lignes = String::new();
        if let Some(arr) = t.get("lignes").and_then(|v| v.as_array()) {
            for l in arr.iter().take(5) {
                let badge = l.get("badge").and_then(|v| v.as_str()).unwrap_or("");
                let temps = l.get("temps").and_then(|v| v.as_str()).unwrap_or("");
                let mut droite = String::new();
                if !badge.is_empty() {
                    droite.push_str(&etiquette(badge, couleur));
                }
                if !temps.is_empty() {
                    droite.push_str(&format!(
                        r#"<span class="item-time">{}</span>"#,
                        he(temps)
                    ));
                }
                lignes.push_str(&ligne_info(
                    l.get("icone").and_then(|v| v.as_str()).unwrap_or(icone),
                    l.get("titre").and_then(|v| v.as_str()).unwrap_or(""),
                    l.get("sous_titre").and_then(|v| v.as_str()).unwrap_or(""),
                    &droite,
                ));
            }
        }
        if lignes.is_empty() {
            lignes = r#"<div class="empty-state"><i class="fas fa-puzzle-piece"></i><p>Rien a afficher</p></div>"#.to_string();
        }

        out.push_str(&format!(
            r#"<div class="card" data-tuile="ext_{id}">
                <div class="card-header">
                    <div class="card-icon" style="background:{c}1a;color:{c};">
                        <i class="fas fa-{ico}"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">{titre}</div>
                        <div class="card-subtitle">{sous}</div>
                    </div>
                    <a href="{url}" class="btn-action"><i class="fas fa-arrow-right"></i> Ouvrir</a>
                </div>
                <div class="card-content">{lignes}</div>
            </div>"#,
            id = he(&id),
            c = he(couleur),
            ico = he(icone),
            titre = he(titre),
            sous = he(sous),
            url = he(&url),
            lignes = lignes,
        ));
    }
    out
}

fn build_Fichiers_stats(pool: &DbPool, user_id: i64) -> String {
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

fn build_Fichiers_files(pool: &DbPool, user_id: i64) -> String {
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

    // ── Tuiles publiees par les extensions ────────────────────────
    let ext_tiles = build_ext_tiles();

    // ── Tuiles masquees par l'utilisateur ─────────────────────────
    let masquees: Vec<String> = prefs
        .dashboard_tiles
        .iter()
        .filter(|(_, v)| v.as_i64().unwrap_or(1) == 0)
        .map(|(k, _)| format!("[data-tuile=\"{}\"]", k.replace('"', "")))
        .collect();
    let tuiles_style = if masquees.is_empty() {
        String::new()
    } else {
        format!("<style>{}{{display:none!important}}</style>", masquees.join(","))
    };

    // ── API de rafraichissement : renvoie juste la tuile Administration.
    // Permet au navigateur de la reactualiser sans recharger la page.
    if request.url().starts_with("/api/dashboard/admin") {
        let corps = if user_privilege < 8 {
            serde_json::json!({
                "success": true,
                "html": build_admin_card(pool, &prefs, user_privilege)
            })
        } else {
            serde_json::json!({"success": false, "error": "Non autorise"})
        };
        let _ = request.respond(
            Response::from_string(corps.to_string()).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
        );
        return;
    }

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
        build_admin_card(pool, &prefs, user_privilege)
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
        .replace("{{TUILES_STYLE}}", &tuiles_style)
        .replace("{{EXT_TILES}}", &ext_tiles)
        .replace("{{ADMIN_CARD}}", &admin_card)
        .replace("{{Fichiers_STATS}}", &build_Fichiers_stats(pool, user_id))
        .replace("{{Fichiers_FILES}}", &build_Fichiers_files(pool, user_id))
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
