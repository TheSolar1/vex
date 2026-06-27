// ══════════════════════════════════════════════════════════════════
// admin/Admin.rs — VEX Admin Panel
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{
    compter_lignes, compter_sessions_actives, decrire_table, executer_sql_admin, get_taille_db,
    get_tailles_tables, inserer_ou_modifier, lire_lignes_table, lister_tables, selectionner,
    supprimer_ligne, verifier_connexion_avec_expiration, DbPool,
};
use crate::config_loader::{load_config, VexConfig};
use crate::function::{build_nav_html, get_user_language, get_user_preferences, NavContext};
use crate::p2p::p2p::{admin_handle_api as p2p_admin_handle_api, NodeState, P2pConfig};
use crate::utils::{parse_query, strip_port, url_decode};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock};
use tiny_http::{Request, Response};

const HTML_PATH: &str = "static/admin/admin.html";
const PRIVILEGE_MAX: i64 = 3; // accès panel admin
const PRIVILEGE_SUPER: i64 = 2; // superadmin — DB edit + SQL runner + P2P
const PRIVILEGE_MIN_SET: i64 = 2; // on ne peut pas mettre privilege < 2 via l'admin

fn vex_pages() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/login/login", "Login"),
        ("/login/dashboard", "Dashboard"),
        ("/login/account", "Mon compte"),
        ("/tel/", "ExoDrive — liste"),
        ("/edite1/", "Éditeur — liste"),
        ("/sitec", "SiteC (tout)"),
        ("/meet", "Meet (tout)"),
        ("/mess", "Messagerie (tout)"),
        ("/admin/admin", "Admin Panel"),
        ("/node", "Node (tout)"),
        ("/onlyoffice", "OnlyOffice (tout)"),
    ]
}

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("onlyoffice-callback.log")
}

pub fn handle_request(
    request: Request,
    pool: &DbPool,
    config: &VexConfig,
    config_path: &str,
    remote_full: &str,
) {
    let remote_ip = strip_port(remote_full);
    let url = request.url().to_string();
    let method = request.method().to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    let session_minutes = config.users.session_expiration_minutes as u32;
    let user_info = verifier_connexion_avec_expiration(
        pool,
        &cookie_val,
        &remote_ip,
        &user_agent,
        session_minutes,
    );
    let user = match &user_info {
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
    let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);

    if privilege > PRIVILEGE_MAX {
        let _ = request.respond(Response::from_string(
            r#"<!DOCTYPE html><html><body style="font-family:monospace;display:flex;align-items:center;justify-content:center;height:100vh;background:#0a0a0a;color:#4caf50">
<div style="text-align:center"><div style="font-size:6rem;font-weight:900">403</div>
<p style="opacity:.6">Accès réservé aux administrateurs</p>
<a href="/" style="color:#4caf50;display:block;margin-top:1rem">← Retour</a></div>
</body></html>"#)
            .with_status_code(403)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()));
        return;
    }

    let query = parse_query(&url);
    let prefs = get_user_preferences(pool, user_id);
    let theme = if prefs.teme == 1 { "dark" } else { "light" };
    let lang = get_user_language(pool, Some(user_id), None, None);

    let path = url.split('?').next().unwrap_or(&url).to_string();
    if path.starts_with("/api/admin") {
        handle_api(
            request,
            pool,
            config_path,
            &query,
            &method,
            user_id,
            privilege,
            &cookie_val,
            &remote_ip,
            &user_agent,
        );
        return;
    }

    let nav_ctx = NavContext {
        pool,
        user_id: Some(user_id),
        page_key: "admin",
        cookie_val: &cookie_val,
        remote_ip: &remote_ip,
        user_agent: &user_agent,
        query_id: None,
        apps: vec![],
        admin_apps: vec![],
    };
    let nav_html = build_nav_html(&nav_ctx);

    let html = match std::fs::read_to_string(HTML_PATH) {
        Ok(s) => s
            .replace("__NAV_HTML__", &nav_html)
            .replace("__LANG__", &lang)
            .replace("__THEME__", theme),
        Err(e) => {
            eprintln!("[admin] Impossible de lire {} : {}", HTML_PATH, e);
            format!("<h1>Erreur</h1><p>Fichier introuvable : {}</p>", HTML_PATH)
        }
    };

    let _ = request.respond(Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
    ));
}

// ══════════════════════════════════════════════════════════════════
// API JSON /api/admin/*
// ══════════════════════════════════════════════════════════════════
fn handle_api(
    mut request: Request,
    pool: &DbPool,
    config_path: &str,
    query: &HashMap<String, String>,
    method: &str,
    user_id: i64,
    privilege: i64,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) {
    let full_url = request.url().to_string();
    let path = full_url.split('?').next().unwrap_or(&full_url).to_string();
    let sub = path.trim_start_matches("/api/admin");

    let body = if method == "POST" {
        read_body(&mut request)
    } else {
        HashMap::new()
    };

    // ── Routes superadmin uniquement ─────────────────────────────
    let superadmin_routes = [
        "/db/cell/edit",
        "/db/row/delete",
        "/db/row/add",
        "/db/sql",
        "/p2p",
        "/p2p/peers",
        "/p2p/kick",
        "/p2p/users",
        "/p2p/transfers",
        "/p2p/annuaire",
        "/p2p/sync_now",
        "/onlyoffice/start",
        "/onlyoffice/stop",
    ];
    let needs_super =
        sub.starts_with("/p2p") || superadmin_routes.iter().any(|r| sub.starts_with(r));
    if needs_super && privilege > PRIVILEGE_SUPER {
        return respond_json(
            request,
            json!({"success":false,"error":"Réservé aux superadmins."}),
        );
    }

    let resp: Value = match sub {
        // P2P admin routes (delegated to p2p module)
        s if s.starts_with("/p2p") => {
            let cfg = load_config(config_path);
            let vex_url = cfg
                .extra
                .get("server")
                .and_then(|s| s.get("public_url"))
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080")
                .to_string();
            let p2p_cfg = P2pConfig::from_vex_config(&cfg);
            let node_state = Arc::new(RwLock::new(NodeState::init(&vex_url, p2p_cfg.clone())));
            p2p_admin_handle_api(pool, s, &body, method, &node_state)
        }

        "/dashboard" => {
            let vex_cfg = read_config(config_path);
            let sess_min = vex_cfg["users"]["session_expiration_minutes"]
                .as_u64()
                .unwrap_or(60) as u32;
            let (oo_ok, oo_ms) = check_oo(
                vex_cfg["extensions"]["extension_params"]["onlyoffice"]["params"]["server_url"]
                    .as_str()
                    .unwrap_or("http://localhost:8084"),
            );
            let last = selectionner(
                pool,
                "login",
                &[],
                &["id", "nom", "email", "privilege", "vip"],
                Some("id DESC"),
                Some(5),
            );
            json!({ "success": true, "data": {
                "nb_users":    compter_lignes(pool, "login",    &[]),
                "nb_fichiers": compter_lignes(pool, "fichiers", &[]),
                "nb_sessions": compter_sessions_actives(pool, sess_min),
                "nb_pages":    compter_lignes(pool, "sitec",    &[]),
                "db_size_mb":  get_taille_db(pool),
                "onlyoffice":  { "online": oo_ok, "ms": oo_ms },
                "is_super":    privilege <= PRIVILEGE_SUPER,
                "last_users":  last.iter().map(|u| json!({
                    "id":        u.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    "nom":       u.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
                    "email":     u.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                    "privilege": u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(0),
                    "vip":       u.get("vip").and_then(|v| v.as_i64()).unwrap_or(0),
                })).collect::<Vec<_>>(),
            }})
        }

        "/users" => {
            let users = selectionner(
                pool,
                "login",
                &[],
                &["id", "nom", "email", "privilege", "vip"],
                Some("privilege ASC, nom ASC"),
                None,
            );
            json!({ "success": true, "data": users.iter().map(|u| json!({
                "id":        u.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "nom":       u.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
                "email":     u.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                "privilege": u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(0),
                "vip":       u.get("vip").and_then(|v| v.as_i64()).unwrap_or(0),
            })).collect::<Vec<_>>() })
        }

        "/users/privilege" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let priv_val = body
                .get("privilege")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);

            if priv_val < PRIVILEGE_MIN_SET {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Le privilege 1 ne peut pas être attribué via le panel."}),
                );
            }
            if priv_val > 12 || tid == user_id {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Action non autorisée."}),
                );
            }
            if privilege > PRIVILEGE_SUPER && priv_val < privilege {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Vous ne pouvez pas donner un privilege supérieur au vôtre."}),
                );
            }
            inserer_ou_modifier(
                pool,
                "login",
                &[("privilege", mysql::Value::from(priv_val))],
                &[("id", mysql::Value::from(tid))],
            );
            json!({"success":true,"message":"Privilège mis à jour."})
        }

        "/users/vip" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let vip = body
                .get("vip")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            inserer_ou_modifier(
                pool,
                "login",
                &[("vip", mysql::Value::from(vip))],
                &[("id", mysql::Value::from(tid))],
            );
            json!({"success":true,"message":"Statut VIP modifié."})
        }

        "/users/delete" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            if tid == user_id {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Impossible de supprimer votre propre compte."}),
                );
            }
            let target = selectionner(
                pool,
                "login",
                &[("id", mysql::Value::from(tid))],
                &["privilege"],
                None,
                Some(1),
            );
            let target_priv = target
                .first()
                .and_then(|r| r.get("privilege"))
                .and_then(|v| v.as_i64())
                .unwrap_or(99);
            if target_priv <= PRIVILEGE_SUPER && privilege > PRIVILEGE_SUPER {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Impossible de supprimer un superadmin."}),
                );
            }
            supprimer_ligne(pool, "login", "id", mysql::Value::from(tid));
            json!({"success":true,"message":"Utilisateur supprimé."})
        }

        "/blocks" => {
            let blocks = selectionner(
                pool,
                "bloqpage",
                &[],
                &["id", "iduserb", "priviautro", "iduserquiab", "pageb"],
                Some("id DESC"),
                None,
            );
            json!({ "success": true, "data": blocks.iter().map(|b| json!({
                "id":          b.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "iduserb":     b.get("iduserb").and_then(|v| v.as_str()).unwrap_or("all"),
                "priviautro":  b.get("priviautro").and_then(|v| v.as_i64()).unwrap_or(0),
                "iduserquiab": b.get("iduserquiab").and_then(|v| v.as_i64()).unwrap_or(0),
                "pageb":       b.get("pageb").and_then(|v| v.as_str()).unwrap_or(""),
            })).collect::<Vec<_>>() })
        }

        "/blocks/add" => {
            let iduserb = body.get("iduserb").cloned().unwrap_or_else(|| "all".into());
            let priviautro = body
                .get("priviautro")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(10);
            let pageb = body.get("pageb").cloned().unwrap_or_default();
            if pageb.is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Sélectionne au moins une page."}),
                );
            }
            inserer_ou_modifier(
                pool,
                "bloqpage",
                &[
                    ("iduserb", mysql::Value::from(iduserb.as_str())),
                    ("priviautro", mysql::Value::from(priviautro)),
                    ("iduserquiab", mysql::Value::from(user_id)),
                    ("pageb", mysql::Value::from(pageb.as_str())),
                ],
                &[],
            );
            json!({"success":true,"message":"Blocage ajouté."})
        }

        "/blocks/delete" => {
            let bid = body
                .get("bid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            supprimer_ligne(pool, "bloqpage", "id", mysql::Value::from(bid));
            json!({"success":true,"message":"Blocage supprimé."})
        }

        "/pages" => {
            json!({"success":true,"data": vex_pages().iter().map(|(p,l)| json!([p,l])).collect::<Vec<_>>()})
        }

        "/db/tables" => {
            json!({"success":true,"data":{"tables":lister_tables(pool),"sizes":get_tailles_tables(pool)}})
        }

        // ── Schéma d'une table : /db/table/<nom>/schema ───────────
        s if s.starts_with("/db/table/") && s.ends_with("/schema") => {
            let table_raw = s
                .trim_start_matches("/db/table/")
                .trim_end_matches("/schema");
            let table = url_decode(table_raw);
            let schema = decrire_table(pool, &table);
            if schema.is_empty() {
                json!({"success":false,"error":"Table introuvable ou inaccessible."})
            } else {
                json!({"success":true,"data":{"table":table,"schema":schema}})
            }
        }

        // ── Contenu d'une table : /db/table/<nom> ─────────────────
        s if s.starts_with("/db/table/") && !s.contains("/cell") && !s.contains("/row") => {
            let table_raw = s.trim_start_matches("/db/table/");
            let table = url_decode(table_raw);
            let page = query
                .get("page")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1)
                .max(1);
            let per_page = 30u64;
            let total = compter_lignes(pool, &table, &[]);
            let offset = (page - 1) * per_page;

            match lire_lignes_table(pool, &table, per_page, offset) {
                Some(data) => json!({"success":true,"data":{
                    "table": table, "total": total,
                    "page": page,
                    "total_pages": if total == 0 { 1u64 } else { ((total as f64) / (per_page as f64)).ceil() as u64 },
                    "cols": data["cols"], "rows": data["rows"],
                    "is_super": privilege <= PRIVILEGE_SUPER,
                }}),
                None => json!({"success":false,"error":"Table introuvable."}),
            }
        }

        "/db/cell/edit" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let col = body.get("col").cloned().unwrap_or_default();
            let val = body.get("val").cloned().unwrap_or_default();
            let pk_col = body.get("pk_col").cloned().unwrap_or_else(|| "id".into());
            let pk_val = body.get("pk_val").cloned().unwrap_or_default();

            if table == "login" && col == "privilege" {
                let new_priv = val.parse::<i64>().unwrap_or(99);
                if new_priv < PRIVILEGE_MIN_SET {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Le privilege 1 est interdit, même en édition directe."}),
                    );
                }
            }

            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":"Table inconnue."}));
            }
            inserer_ou_modifier(
                pool,
                &table,
                &[(&col, mysql::Value::from(val.as_str()))],
                &[(&pk_col, mysql::Value::from(pk_val.as_str()))],
            );
            json!({"success":true,"message":"Cellule modifiée."})
        }

        "/db/row/delete" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let pk_col = body.get("pk_col").cloned().unwrap_or_else(|| "id".into());
            let pk_val = body.get("pk_val").cloned().unwrap_or_default();

            if table == "login" && pk_col == "id" {
                let target = selectionner(
                    pool,
                    "login",
                    &[("id", mysql::Value::from(pk_val.as_str()))],
                    &["privilege"],
                    None,
                    Some(1),
                );
                let tp = target
                    .first()
                    .and_then(|r| r.get("privilege"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(99);
                if tp <= PRIVILEGE_SUPER {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Impossible de supprimer un superadmin depuis l'explorateur DB."}),
                    );
                }
            }

            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":"Table inconnue."}));
            }
            supprimer_ligne(pool, &table, &pk_col, mysql::Value::from(pk_val.as_str()));
            json!({"success":true,"message":"Ligne supprimée."})
        }

        "/db/row/add" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":"Table inconnue."}));
            }
            let cols_raw = body.get("cols").cloned().unwrap_or_default();
            let vals_raw = body.get("vals").cloned().unwrap_or_default();
            let cols: Vec<String> = serde_json::from_str(&cols_raw).unwrap_or_default();
            let vals: Vec<String> = serde_json::from_str(&vals_raw).unwrap_or_default();
            if cols.is_empty() || cols.len() != vals.len() {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Colonnes/valeurs invalides."}),
                );
            }
            if table == "login" {
                for (c, v) in cols.iter().zip(vals.iter()) {
                    if c == "privilege" {
                        if v.parse::<i64>().unwrap_or(99) < PRIVILEGE_MIN_SET {
                            return respond_json(
                                request,
                                json!({"success":false,"error":"Le privilege 1 est interdit."}),
                            );
                        }
                    }
                }
            }
            let pairs: Vec<(&str, mysql::Value)> = cols
                .iter()
                .zip(vals.iter())
                .map(|(c, v)| (c.as_str(), mysql::Value::from(v.as_str())))
                .collect();
            let new_id = inserer_ou_modifier(pool, &table, &pairs, &[]);
            json!({"success":true,"message":"Ligne ajoutée.","id":new_id})
        }

        // ── SQL Runner : /db/sql ───────────────────────────────────
        // Toutes les requêtes sont autorisées SAUF :
        //   - Commandes système dangereuses (LOAD_FILE, INTO OUTFILE, SLEEP…)
        //   - Toute tentative de mettre privilege=1 dans la table login
        // La vérification de connexion est re-validée dans appeldb::executer_sql_admin.
        "/db/sql" => {
            let raw_sql = body.get("sql").cloned().unwrap_or_default();
            executer_sql_admin(pool, cookie_val, remote_ip, user_agent, &raw_sql)
        }

        "/p2p/peers" => {
            let peers = selectionner(
                pool,
                "p2p_peers",
                &[],
                &["id", "node_id", "ip", "port", "last_seen", "status"],
                Some("last_seen DESC"),
                None,
            );
            let total = compter_lignes(pool, "p2p_peers", &[]);
            let online = compter_lignes(
                pool,
                "p2p_peers",
                &[("status", mysql::Value::from("online"))],
            );
            json!({"success":true,"data":{
                "peers": peers.iter().map(|p| json!({
                    "id":        p.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    "node_id":   p.get("node_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "ip":        p.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                    "port":      p.get("port").and_then(|v| v.as_i64()).unwrap_or(0),
                    "last_seen": p.get("last_seen").and_then(|v| v.as_str()).unwrap_or(""),
                    "status":    p.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                })).collect::<Vec<_>>(),
                "total":  total,
                "online": online,
            }})
        }

        "/p2p/kick" => {
            let peer_id = body
                .get("peer_id")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            supprimer_ligne(pool, "p2p_peers", "id", mysql::Value::from(peer_id));
            json!({"success":true,"message":"Peer supprimé."})
        }

        "/config" => {
            if method == "POST" {
                let new_cfg_str = body.get("config").cloned().unwrap_or_default();
                match serde_json::from_str::<Value>(&new_cfg_str) {
                    Ok(v) => {
                        let _ = std::fs::write(
                            config_path,
                            serde_json::to_string_pretty(&v).unwrap_or_default(),
                        );
                        json!({"success":true,"message":"Configuration sauvegardée."})
                    }
                    Err(e) => json!({"success":false,"error":format!("JSON invalide : {}", e)}),
                }
            } else {
                json!({"success":true,"data": read_config(config_path)})
            }
        }

        "/server" => {
            let (free, total, used, pct) = disk_info();
            json!({"success":true,"data":{
                "vex_version": env!("CARGO_PKG_VERSION"),
                "os":          std::env::consts::OS,
                "arch":        std::env::consts::ARCH,
                "uptime_sec":  uptime_sec(),
                "disk": {"free_gb":free,"total_gb":total,"used_gb":used,"used_pct":pct},
            }})
        }

        "/logs" => {
            let content = std::fs::read_to_string(log_path()).unwrap_or_default();
            if content.trim().is_empty() {
                json!({"success":true,"data":{"empty":true}})
            } else {
                let lines: Vec<&str> = content.lines().collect();
                let trimmed = lines
                    .iter()
                    .rev()
                    .take(200)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                json!({"success":true,"data":{"empty":false,"content":trimmed}})
            }
        }

        "/logs/clear" => {
            let _ = std::fs::write(log_path(), "");
            json!({"success":true,"message":"Log vidé."})
        }

        "/onlyoffice" => {
            let cfg = read_config(config_path);
            let oo = &cfg["extensions"]["extension_params"]["onlyoffice"];
            let srv = cfg.get("onlyoffice_server").cloned().unwrap_or_default();
            let url = srv
                .get("server_url")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    oo["params"]
                        .get("server_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("http://localhost:8084")
                });
            let health_path = srv
                .get("healthcheck_path")
                .and_then(|v| v.as_str())
                .unwrap_or("/healthcheck");
            let (online, ms) = check_oo_with_path(url, health_path);
            json!({"success":true,"data":{
                "online":  online,
                "ms":      ms,
                "url":     url,
                "enabled": oo["enabled"].as_bool().unwrap_or(false),
                "version": oo["version"].as_str().unwrap_or("?"),
                "params":  oo["params"].clone(),
                "server":  srv,
            }})
        }

        "/onlyoffice/start" => {
            let cfg = read_config(config_path);
            let srv = cfg.get("onlyoffice_server").cloned().unwrap_or_default();
            let enabled = srv
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if !enabled {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Serveur OnlyOffice désactivé dans la configuration."}),
                );
            }
            let start_cmd = srv
                .get("start_cmd")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if start_cmd.trim().is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":"start_cmd non défini dans onlyoffice_server."}),
                );
            }
            let wait_ms = srv
                .get("wait_boot_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(8000);
            let server_url = srv
                .get("server_url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://127.0.0.1:8080");
            let health_path = srv
                .get("healthcheck_path")
                .and_then(|v| v.as_str())
                .unwrap_or("/healthcheck");

            let cmd_res = run_shell_command(start_cmd);
            let mut online = false;
            let mut ms = 0u64;
            if cmd_res.0 {
                std::thread::sleep(std::time::Duration::from_millis(wait_ms.min(20_000)));
                let (ok, ping_ms) = check_oo_with_path(server_url, health_path);
                online = ok;
                ms = ping_ms;
            }
            json!({"success":online, "cmd_ok":cmd_res.0, "output":cmd_res.1, "ping_ms":ms, "online":online})
        }

        "/onlyoffice/stop" => {
            let cfg = read_config(config_path);
            let srv = cfg.get("onlyoffice_server").cloned().unwrap_or_default();
            let stop_cmd = srv
                .get("stop_cmd")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if stop_cmd.trim().is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":"stop_cmd non défini dans onlyoffice_server."}),
                );
            }
            let cmd_res = run_shell_command(stop_cmd);
            json!({"success":cmd_res.0,"output":cmd_res.1})
        }

        _ => json!({"success":false,"error":"Route inconnue."}),
    };

    respond_json(request, resp);
}

// ── Helpers ───────────────────────────────────────────────────────

fn respond_json(request: Request, body: Value) {
    let _ = request.respond(Response::from_string(body.to_string()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    ));
}

fn read_config(path: &str) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}

fn read_body(request: &mut Request) -> HashMap<String, String> {
    let mut s = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut s);
    let mut m = HashMap::new();
    for pair in s.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            m.insert(url_decode(k), url_decode(v));
        }
    }
    m
}

fn check_oo(base_url: &str) -> (bool, u64) {
    check_oo_with_path(base_url, "/healthcheck")
}

fn check_oo_with_path(base_url: &str, path: &str) -> (bool, u64) {
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'));
    let t0 = std::time::Instant::now();
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .call();
    let ms = t0.elapsed().as_millis() as u64;
    match resp {
        Ok(r) => (r.into_string().unwrap_or_default().trim() == "true", ms),
        Err(_) => (false, ms),
    }
}

fn run_shell_command(cmd: &str) -> (bool, String) {
    if cmd.trim().is_empty() {
        return (false, "Commande vide".into());
    }
    #[cfg(windows)]
    let mut c = Command::new("cmd");
    #[cfg(windows)]
    {
        c.arg("/C").arg(cmd);
    }
    #[cfg(not(windows))]
    let mut c = Command::new("sh");
    #[cfg(not(windows))]
    {
        c.arg("-c").arg(cmd);
    }
    match c.output() {
        Ok(out) => {
            let mut s = String::new();
            if !out.stdout.is_empty() {
                s.push_str(&String::from_utf8_lossy(&out.stdout));
            }
            if !out.stderr.is_empty() {
                s.push_str(&String::from_utf8_lossy(&out.stderr));
            }
            (out.status.success(), s.trim().to_string())
        }
        Err(e) => (false, format!("{}", e)),
    }
}

fn disk_info() -> (f64, f64, f64, u64) {
    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("df").args(["-B1", "/"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            let p: Vec<&str> = s.lines().nth(1).unwrap_or("").split_whitespace().collect();
            if p.len() >= 4 {
                let t = p[1].parse::<u64>().unwrap_or(0);
                let u = p[2].parse::<u64>().unwrap_or(0);
                let f = p[3].parse::<u64>().unwrap_or(0);
                return (
                    f as f64 / 1e9,
                    t as f64 / 1e9,
                    u as f64 / 1e9,
                    if t > 0 { u * 100 / t } else { 0 },
                );
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(out) = std::process::Command::new("wmic")
            .args([
                "logicaldisk",
                "where",
                "DeviceID='C:'",
                "get",
                "Size,FreeSpace",
                "/format:csv",
            ])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines().skip(2) {
                let p: Vec<&str> = line.split(',').collect();
                if p.len() >= 3 {
                    let f = p[1].trim().parse::<u64>().unwrap_or(0);
                    let t = p[2].trim().parse::<u64>().unwrap_or(0);
                    let u = t.saturating_sub(f);
                    return (
                        f as f64 / 1e9,
                        t as f64 / 1e9,
                        u as f64 / 1e9,
                        if t > 0 { u * 100 / t } else { 0 },
                    );
                }
            }
        }
    }
    (0.0, 0.0, 0.0, 0)
}

fn uptime_sec() -> u64 {
    #[cfg(unix)]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/uptime") {
            if let Some(v) = s.split_whitespace().next() {
                return v.parse::<f64>().unwrap_or(0.0) as u64;
            }
        }
    }
    0
}
