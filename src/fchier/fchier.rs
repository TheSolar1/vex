// ══════════════════════════════════════════════════════════════════
// fchier.rs — Module Gestionnaire de Fichiers VEX
// Colonnes réelles : fichiers(nom, fichier, type_fichier, taille,
//   visble, id_utilisateur, date, partage)
//   sitecdos(iddosier, doisernom, userid, idpage, addpageuserid)
//   sitec(idpage, nompage, urlpage, user_id, prob)
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, p2p_get_peer, selectionner, supprimer_ligne, DbPool};
use crate::config_loader::load_config;
use crate::function::{build_nav_html, NavContext};
use crate::p2p::p2p::{NodeState, P2pConfig};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use hex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read as IoRead;
use tiny_http::{Header, Request, Response};
use ureq;
use uuid::Uuid;

fn get_cookie(req: &Request, name: &str) -> String {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Cookie"))
        .and_then(|h| {
            h.value.as_str().split(';').find_map(|part| {
                let part = part.trim();
                if part.starts_with(name) && part[name.len()..].starts_with('=') {
                    Some(part[name.len() + 1..].to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_default()
}

fn remote_ip(req: &Request) -> String {
    req.headers()
        .iter()
        .find(|h| {
            h.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case("X-Forwarded-For")
        })
        .and_then(|h| {
            h.value
                .as_str()
                .split(',')
                .next()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| {
            req.remote_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_default()
        })
}

fn user_agent(req: &Request) -> String {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("User-Agent"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default()
}

fn verifier_session(pool: &DbPool, req: &Request) -> Option<HashMap<String, Value>> {
    let cookie = get_cookie(req, "connexion_cookie");
    crate::appeldb::verifier_connexion(pool, &cookie, &remote_ip(req), &user_agent(req))
}

fn json_response(status: u16, body: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(body.to_string().into_bytes())
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn html_response(html: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(html.into_bytes())
        .with_status_code(200)
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}

fn redirect_response(location: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(vec![])
        .with_status_code(302)
        .with_header(Header::from_bytes("Location", location).unwrap())
}

fn parse_query(url: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(q) = url.split('?').nth(1) {
        for part in q.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                map.insert(urlencoding_decode(k), urlencoding_decode(v));
            }
        }
    }
    map
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.replace('+', " ");
    let mut chars = bytes.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(b) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                out.push(b as char);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn read_body(req: &mut Request) -> String {
    let mut body = String::new();
    let _ = std::io::Read::read_to_string(req.as_reader(), &mut body);
    body
}

fn parse_json_body(req: &mut Request) -> Option<Value> {
    serde_json::from_str(&read_body(req)).ok()
}

fn urlenc_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn load_p2p_state() -> NodeState {
    let cfg = load_config("config.json");
    let vex_url = cfg
        .extra
        .get("server")
        .and_then(|s| s.get("public_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8080")
        .to_string();
    let p2p_cfg = P2pConfig::from_vex_config(&cfg);
    NodeState::init(&vex_url, p2p_cfg)
}

fn send_file_via_p2p(pool: &DbPool, file_path: &str, file_name: &str, to_user: i64) -> Value {
    // Trouver le node du destinataire
    let dest_node = selectionner(
        pool,
        "p2p_users",
        &[("user_id", mysql::Value::from(to_user))],
        &["node_id"],
        Some("updated_at DESC"),
        Some(1),
    )
    .into_iter()
    .next();
    let dest_node_id = match dest_node.and_then(|m| {
        m.get("node_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }) {
        Some(id) => id,
        None => return json!({"success":false,"error":"Destinataire introuvable dans p2p_users"}),
    };

    // Infos sur le pair
    let peer = match p2p_get_peer(pool, &dest_node_id) {
        Some(p) => p,
        None => return json!({"success":false,"error":"Pair introuvable dans p2p_peers"}),
    };
    let url = peer
        .get("tor_addr")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| peer.get("vex_url").and_then(|v| v.as_str()).unwrap_or(""));
    if url.is_empty() {
        return json!({"success":false,"error":"URL pair vide"});
    }

    // Node local + config
    let ns = load_p2p_state();
    let chunk_size = ns.config.chunk_size_bytes as usize;

    // Fichier à envoyer
    let mut f = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => return json!({"success":false,"error":format!("Ouverture fichier: {}", e)}),
    };
    let file_size = match f.metadata() {
        Ok(m) => m.len(),
        Err(e) => return json!({"success":false,"error":e.to_string()}),
    };
    let chunks_total = ((file_size + chunk_size as u64 - 1) / chunk_size as u64) as usize;
    let transfer_id = Uuid::new_v4().to_string();

    // Init transfert
    let sig_init = ns.signer(transfer_id.as_bytes());
    let init_body = format!(
        "transfer_id={}&from_node={}&to_user={}&file_name={}&file_size={}&chunk_size={}&chunks_total={}&sig={}",
        urlenc_simple(&transfer_id),
        urlenc_simple(&ns.node_id),
        to_user,
        urlenc_simple(file_name),
        file_size,
        chunk_size,
        chunks_total,
        urlenc_simple(&sig_init),
    );
    let init_resp = ureq::post(&format!("{}/p2p/transfer/init", url.trim_end_matches('/')))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(std::time::Duration::from_secs(15))
        .send_string(&init_body);
    if let Err(e) = init_resp {
        return json!({"success":false,"error":format!("Init: {}", e)});
    }
    let init_ok = init_resp
        .unwrap()
        .into_string()
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("success").and_then(|b| b.as_bool()));
    if init_ok != Some(true) {
        return json!({"success":false,"error":"Init transfert refusée"});
    }

    // Envoi des chunks
    let mut buf = vec![0u8; chunk_size];
    for idx in 0..chunks_total {
        let n = match f.read(&mut buf) {
            Ok(0) => 0,
            Ok(n) => n,
            Err(e) => {
                return json!({"success":false,"error":format!("Lecture chunk {}: {}", idx, e)})
            }
        };
        let data = &buf[..n];
        let hash = hex::encode(Sha256::digest(data));
        let data_b64 = B64.encode(data);
        let sig_chunk = ns.signer(format!("{}:{}", transfer_id, idx).as_bytes());

        let body = format!(
            "transfer_id={}&chunk_idx={}&data={}&hash={}&sig={}",
            urlenc_simple(&transfer_id),
            idx,
            urlenc_simple(&data_b64),
            urlenc_simple(&hash),
            urlenc_simple(&sig_chunk),
        );
        let resp = ureq::post(&format!("{}/p2p/transfer/chunk", url.trim_end_matches('/')))
            .set("Content-Type", "application/x-www-form-urlencoded")
            .timeout(std::time::Duration::from_secs(30))
            .send_string(&body);
        if let Err(e) = resp {
            return json!({"success":false,"error":format!("Chunk {}: {}", idx, e)});
        }
        let ok = resp
            .unwrap()
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.get("success").and_then(|b| b.as_bool()))
            .unwrap_or(false);
        if !ok {
            return json!({"success":false,"error":format!("Chunk {} refusé", idx)});
        }
    }

    json!({"success":true,"transfer_id":transfer_id,"chunks":chunks_total})
}
/// Renvoie true si uid est dans la liste de partage ET que la liste n'est pas vide.
/// Format : "id:permission,id2:permission2" ou "id,id2"
fn is_shared_with(partage: &str, uid: i64) -> bool {
    let p = partage.trim();
    if p.is_empty() {
        return false;
    }
    p.split(',').any(|s| {
        s.trim()
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .parse::<i64>()
            .ok()
            == Some(uid)
    })
}

/// Indique si un champ partage est non-vide (pour le badge côté JSON)
fn has_shares(partage: &str) -> bool {
    !partage.trim().is_empty()
}

fn dos_parent(idpage: &str) -> Option<i64> {
    let prefix = "dos:";
    if let Some(pos) = idpage.find(prefix) {
        let rest = &idpage[pos + prefix.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse::<i64>().ok()
    } else {
        None
    }
}

// ══════════════════════════════════════════════════════════════════
// POINT D'ENTRÉE
// ══════════════════════════════════════════════════════════════════
pub fn handle(pool: &DbPool, req: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let url = req.url().to_string();

    if url == "/fchier" || url == "/fchier/" {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return redirect_response("/login"),
        };
        let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let cookie = get_cookie(req, "connexion_cookie");
        let ip = remote_ip(req);
        let ua = user_agent(req);
        let nav = build_nav_html(&NavContext {
            pool,
            user_id: Some(uid),
            page_key: "fchier",
            cookie_val: &cookie,
            remote_ip: &ip,
            user_agent: &ua,
            query_id: None,
            apps: vec![],
            admin_apps: vec![],
        });
        return html_response(serve_html(&nav));
    }

    if url.starts_with("/api/fchier/") {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"error":"Non connecté"})),
        };
        let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let path = url.split('?').next().unwrap_or("");
        let action = path.trim_start_matches("/api/fchier/");
        return match action {
            "data" => api_data(pool, req, uid),
            "upload" => api_upload(pool, req, uid),
            "create_folder" => api_create_folder(pool, req, uid),
            "share" => api_share(pool, req, uid),
            "change_visibility" => api_change_visibility(pool, req, uid),
            "rename" => api_rename(pool, req, uid),
            "delete" => api_delete(pool, req, uid),
            "move" => api_move(pool, req, uid),
            "download" => api_download(pool, req, uid),
            "send_p2p" => api_send_p2p(pool, req, uid),
            _ => json_response(404, json!({"error":"Endpoint inconnu"})),
        };
    }

    json_response(404, json!({"error":"Route inconnue"}))
}

// ══════════════════════════════════════════════════════════════════
// GET /api/fchier/data
// ══════════════════════════════════════════════════════════════════
fn api_data(pool: &DbPool, req: &Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = parse_query(req.url());
    let dossier = params
        .get("dossier")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let shared = params
        .get("shared")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);

    // ── Dossiers
    let uid_val_dos = mysql::Value::from(uid);
    let tuple_dos = ("userid", uid_val_dos);
    let filter_dos: &[(&str, mysql::Value)] = if shared == 0 {
        std::slice::from_ref(&tuple_dos)
    } else {
        &[]
    };
    let all_dos = selectionner(
        pool,
        "sitecdos",
        filter_dos,
        &["iddosier", "doisernom", "userid", "idpage", "addpageuserid"],
        Some("doisernom ASC"),
        None,
    );
    let mes_dossiers: Vec<Value> = all_dos
        .into_iter()
        .filter_map(|row| {
            let owner = row.get("userid").and_then(|v| v.as_i64()).unwrap_or(-1);
            let addpage = row
                .get("addpageuserid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let idpage = row
                .get("idpage")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if shared == 1 {
                if owner == uid {
                    return None;
                }
                if !is_shared_with(&addpage, uid) {
                    return None;
                }
            }
            // Filtrage par niveau
            if dossier == 0 {
                if idpage.contains("dos:") {
                    return None;
                }
            } else {
                if !idpage.contains(&format!("dos:{}", dossier)) {
                    return None;
                }
            }
            // Badge partage : la colonne addpageuserid contient des IDs d'autres utilisateurs
            // On ne montre le badge que si des utilisateurs AUTRES que le propriétaire sont dans la liste
            let partage_affiche = {
                let ids: Vec<&str> = addpage
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .filter(|s| {
                        let id_part = s.split(':').next().unwrap_or("").trim();
                        id_part.parse::<i64>().ok() != Some(owner)
                    })
                    .collect();
                if ids.is_empty() {
                    String::new()
                } else {
                    ids.join(",")
                }
            };

            Some(json!({
                "id":        row.get("iddosier").cloned().unwrap_or(json!(0)),
                "nom":       row.get("doisernom").cloned().unwrap_or(json!("")),
                "partage":   partage_affiche,
                "owner_id":  owner,
                "is_owner":  owner == uid,
            }))
        })
        .collect();

    // ── Fichiers
    let uid_val_fich = mysql::Value::from(uid);
    let tuple_fich = ("id_utilisateur", uid_val_fich);
    let filter_fich: &[(&str, mysql::Value)] = if shared == 0 {
        std::slice::from_ref(&tuple_fich)
    } else {
        &[]
    };
    let all_fich = selectionner(
        pool,
        "fichiers",
        filter_fich,
        &[
            "id",
            "nom",
            "taille",
            "type_fichier",
            "date",
            "visble",
            "partage",
            "id_utilisateur",
        ],
        Some("date DESC"),
        Some(100),
    );
    let mes_fichiers: Vec<Value> = all_fich
        .into_iter()
        .filter_map(|row| {
            let owner = row
                .get("id_utilisateur")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            let partage_raw = row
                .get("partage")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if shared == 1 {
                if owner == uid || !is_shared_with(&partage_raw, uid) {
                    return None;
                }
            }

            // N'affiche le badge partage que si des ids différents du propriétaire existent
            let partage_affiche = {
                let ids: Vec<&str> = partage_raw
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .filter(|s| {
                        let id_part = s.split(':').next().unwrap_or("").trim();
                        id_part.parse::<i64>().ok() != Some(owner)
                    })
                    .collect();
                if ids.is_empty() {
                    String::new()
                } else {
                    ids.join(",")
                }
            };

            Some(json!({
                "id":           row.get("id").cloned().unwrap_or(json!(0)),
                "nom":          row.get("nom").cloned().unwrap_or(json!("")),
                "taille":       row.get("taille").cloned().unwrap_or(json!(0)),
                "type_fichier": row.get("type_fichier").cloned().unwrap_or(json!("")),
                "date":         row.get("date").cloned().unwrap_or(json!("")),
                "visble":       row.get("visble").cloned().unwrap_or(json!("")),
                "partage":      partage_affiche,
                "owner_id":     owner,
                "is_owner":     owner == uid,
            }))
        })
        .collect();

    // ── Pages
    let pages_raw = selectionner(
        pool,
        "sitec",
        &[("user_id", mysql::Value::from(uid))],
        &["idpage", "nompage", "urlpage", "prob"],
        Some("nompage ASC"),
        None,
    );
    let pages_web: Vec<Value> = pages_raw
        .into_iter()
        .map(|row| {
            json!({
                "id":      row.get("idpage").cloned().unwrap_or(json!(0)),
                "nom":     row.get("nompage").cloned().unwrap_or(json!("")),
                "urlpage": row.get("urlpage").cloned().unwrap_or(json!("")),
                "prob":    row.get("prob").cloned().unwrap_or(json!(1)),
            })
        })
        .collect();

    // ── Breadcrumb
    let chemin = if dossier != 0 {
        build_chemin(pool, dossier)
    } else {
        vec![json!({"id":0,"nom":"Mes fichiers"})]
    };

    // ── Tous dossiers (déplacement)
    let all_folders_raw = selectionner(
        pool,
        "sitecdos",
        &[],
        &["iddosier", "doisernom", "userid", "addpageuserid"],
        Some("doisernom ASC"),
        None,
    );
    let all_folders: Vec<Value> = all_folders_raw.into_iter().filter_map(|row| {
        let owner   = row.get("userid").and_then(|v| v.as_i64()).unwrap_or(-1);
        let addpage = row.get("addpageuserid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if owner != uid && !is_shared_with(&addpage, uid) { return None; }
        Some(json!({"id": row.get("iddosier").cloned().unwrap_or(json!(0)), "nom": row.get("doisernom").cloned().unwrap_or(json!(""))}))
    }).collect();

    // ── Quota
    let quota: i64 = selectionner(
        pool,
        "fichiers",
        &[("id_utilisateur", mysql::Value::from(uid))],
        &["taille"],
        None,
        None,
    )
    .iter()
    .filter_map(|r| r.get("taille").and_then(|v| v.as_i64()))
    .sum();

    json_response(
        200,
        json!({
            "dossiers":    mes_dossiers,
            "fichiers":    mes_fichiers,
            "pages":       pages_web,
            "chemin":      chemin,
            "dossier_courant": dossier,
            "shared":      shared,
            "all_folders": all_folders,
            "quota":       {"utilise": quota, "max": 5_368_709_120i64},
        }),
    )
}

fn build_chemin(pool: &DbPool, mut id: i64) -> Vec<Value> {
    let mut chemin = vec![];
    let mut seen = std::collections::HashSet::new();
    loop {
        if seen.contains(&id) || id == 0 {
            break;
        }
        seen.insert(id);
        let rows = selectionner(
            pool,
            "sitecdos",
            &[("iddosier", mysql::Value::from(id))],
            &["iddosier", "doisernom", "idpage"],
            None,
            Some(1),
        );
        match rows.into_iter().next() {
            Some(row) => {
                chemin.push(json!({"id": row.get("iddosier").cloned().unwrap_or(json!(0)), "nom": row.get("doisernom").cloned().unwrap_or(json!(""))}));
                let idpage = row
                    .get("idpage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                id = dos_parent(&idpage).unwrap_or(0);
            }
            None => break,
        }
    }
    chemin.push(json!({"id":0,"nom":"Mes fichiers"}));
    chemin.reverse();
    chemin
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/upload
// ══════════════════════════════════════════════════════════════════
fn api_upload(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let content_type = req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Content-Type"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();

    // Support multipart/form-data ET application/json
    let (nom, file_b64, mime_type, taille, visble, current_folder) =
        if content_type.contains("application/json") {
            let body = match parse_json_body(req) {
                Some(b) => b,
                None => return json_response(400, json!({"error":"Corps JSON invalide"})),
            };
            (
                body.get("file_name").and_then(|v| v.as_str()).unwrap_or("fichier").to_string(),
                body.get("file_b64").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                body.get("mime_type").and_then(|v| v.as_str()).unwrap_or("application/octet-stream").to_string(),
                body.get("taille").and_then(|v| v.as_i64()).unwrap_or(0),
                body.get("visble").and_then(|v| v.as_str()).unwrap_or("1").to_string(),
                body.get("current_folder").and_then(|v| v.as_i64()).unwrap_or(0),
            )
        } else {
            // multipart: lit le body brut et parse manuellement
            let raw = read_body(req);
            let get_field = |name: &str| -> String {
                // cherche name=...valeur... dans le body urlencoded ou multipart simplifié
                raw.split('&').find_map(|part| {
                    let mut kv = part.splitn(2, '=');
                    let k = kv.next().unwrap_or("").trim();
                    let v = kv.next().unwrap_or("").trim();
                    if k == name { Some(urlencoding_decode(v)) } else { None }
                }).unwrap_or_default()
            };
            let taille_str = get_field("taille");
            let folder_str = get_field("current_folder");
            (
                get_field("file_name"),
                get_field("file_b64"),
                get_field("mime_type"),
                taille_str.parse::<i64>().unwrap_or(0),
                get_field("visble"),
                folder_str.parse::<i64>().unwrap_or(0),
            )
        };

    if file_b64.is_empty() {
        return json_response(400, json!({"error":"Fichier vide"}));
    }
    if nom.is_empty() || nom.len() > 255 {
        return json_response(400, json!({"error":"Nom invalide"}));
    }

    // Vérifie l'extension — liste bloquée minimale (exécutables dangereux seulement)
    let ext = nom.rsplit('.').next().unwrap_or("").to_lowercase();
    let blocked = ["exe", "bat", "cmd", "com", "msi", "scr", "vbs", "ps1"];
    if blocked.contains(&ext.as_str()) {
        return json_response(400, json!({"error": format!("Extension .{} non autorisée", ext)}));
    }

    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    // Taille réelle depuis le base64 si non fournie
    let taille_reelle = if taille > 0 {
        taille
    } else {
        // base64: 4 chars = 3 bytes
        (file_b64.len() as i64 * 3 / 4)
    };

    let id = inserer_ou_modifier(
        pool,
        "fichiers",
        &[
            ("id_utilisateur", mysql::Value::from(uid)),
            ("nom", mysql::Value::from(nom.as_str())),
            ("taille", mysql::Value::from(taille_reelle)),
            ("type_fichier", mysql::Value::from(mime_type.as_str())),
            ("visble", mysql::Value::from(visble.as_str())),
            ("fichier", mysql::Value::from(file_b64.as_str())),
            ("partage", mysql::Value::from("")),
            ("date", mysql::Value::from(now.as_str())),
        ],
        &[],
    );
    if id < 0 {
        return json_response(500, json!({"error":"Erreur insertion DB"}));
    }

    if current_folder > 0 {
        let rows = selectionner(
            pool,
            "sitecdos",
            &[("iddosier", mysql::Value::from(current_folder))],
            &["idpage"],
            None,
            Some(1),
        );
        if let Some(row) = rows.into_iter().next() {
            let idpage = row
                .get("idpage")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let new_idpage = if idpage.is_empty() {
                format!("fich:{}", id)
            } else {
                format!("{},fich:{}", idpage, id)
            };
            inserer_ou_modifier(
                pool,
                "sitecdos",
                &[("idpage", mysql::Value::from(new_idpage.as_str()))],
                &[("iddosier", mysql::Value::from(current_folder))],
            );
        }
    }
    json_response(200, json!({"success":true,"id":id}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/create_folder
// ══════════════════════════════════════════════════════════════════
fn api_create_folder(
    pool: &DbPool,
    req: &mut Request,
    uid: i64,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let folder_name = body
        .get("folder_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let parent_id = body.get("parent_id").and_then(|v| v.as_i64()).unwrap_or(0);
    if folder_name.is_empty() || folder_name.len() > 100 {
        return json_response(400, json!({"error":"Nom invalide"}));
    }
    let idpage_val = if parent_id > 0 {
        format!("dos:{}", parent_id)
    } else {
        String::new()
    };
    // addpageuserid = vide à la création (sera rempli par api_share)
    let id = inserer_ou_modifier(
        pool,
        "sitecdos",
        &[
            ("doisernom", mysql::Value::from(folder_name.as_str())),
            ("userid", mysql::Value::from(uid)),
            ("idpage", mysql::Value::from(idpage_val.as_str())),
            ("addpageuserid", mysql::Value::from("")), // vide = pas de partage
        ],
        &[],
    );
    if id < 0 {
        return json_response(500, json!({"error":"Erreur création"}));
    }
    json_response(200, json!({"success":true,"id":id}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/share
// ══════════════════════════════════════════════════════════════════
fn api_share(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();
    let share_data = body
        .get("share_data")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if item_id == 0 {
        return json_response(400, json!({"error":"item_id manquant"}));
    }

    // Valider le format de share_data : doit être vide ou une liste d'ids numériques
    // "12, 45, 78" → on normalise en "12,45,78"
    let share_normalized: String = if share_data.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = share_data
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.parse::<i64>().is_ok())
            .collect();
        parts.join(",")
    };

    let res = match item_type.as_str() {
        "folder" => {
            // Pour les dossiers : on cherche d'abord par iddosier seul,
            // puis on vérifie que userid == uid
            let rows = selectionner(
                pool,
                "sitecdos",
                &[
                    ("iddosier", mysql::Value::from(item_id)),
                    ("userid", mysql::Value::from(uid)),
                ],
                &["iddosier"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                return json_response(403, json!({"error":"Non autorisé"}));
            }
            inserer_ou_modifier(
                pool,
                "sitecdos",
                &[(
                    "addpageuserid",
                    mysql::Value::from(share_normalized.as_str()),
                )],
                &[
                    ("iddosier", mysql::Value::from(item_id)),
                    ("userid", mysql::Value::from(uid)),
                ],
            )
        }
        "page" => {
            let rows = selectionner(
                pool,
                "sitec",
                &[
                    ("idpage", mysql::Value::from(item_id)),
                    ("user_id", mysql::Value::from(uid)),
                ],
                &["idpage"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                return json_response(403, json!({"error":"Non autorisé"}));
            }
            inserer_ou_modifier(
                pool,
                "sitec",
                &[(
                    "addpageuserid",
                    mysql::Value::from(share_normalized.as_str()),
                )],
                &[
                    ("idpage", mysql::Value::from(item_id)),
                    ("user_id", mysql::Value::from(uid)),
                ],
            )
        }
        _ => {
            // Fichiers : vérifier ownership avant update
            let rows = selectionner(
                pool,
                "fichiers",
                &[
                    ("id", mysql::Value::from(item_id)),
                    ("id_utilisateur", mysql::Value::from(uid)),
                ],
                &["id"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                return json_response(403, json!({"error":"Non autorisé ou fichier introuvable"}));
            }
            inserer_ou_modifier(
                pool,
                "fichiers",
                &[("partage", mysql::Value::from(share_normalized.as_str()))],
                &[
                    ("id", mysql::Value::from(item_id)),
                    ("id_utilisateur", mysql::Value::from(uid)),
                ],
            )
        }
    };

    if res < 0 {
        return json_response(500, json!({"error":"Erreur partage"}));
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/change_visibility
// ══════════════════════════════════════════════════════════════════
fn api_change_visibility(
    pool: &DbPool,
    req: &mut Request,
    uid: i64,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let page_id = body.get("page_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();
    let new_visibility = body
        .get("new_visibility")
        .and_then(|v| v.as_str())
        .unwrap_or("1")
        .to_string();
    let res = match item_type.as_str() {
        "page" => inserer_ou_modifier(
            pool,
            "sitec",
            &[("prob", mysql::Value::from(new_visibility.as_str()))],
            &[
                ("idpage", mysql::Value::from(page_id)),
                ("user_id", mysql::Value::from(uid)),
            ],
        ),
        _ => inserer_ou_modifier(
            pool,
            "fichiers",
            &[("visble", mysql::Value::from(new_visibility.as_str()))],
            &[
                ("id", mysql::Value::from(page_id)),
                ("id_utilisateur", mysql::Value::from(uid)),
            ],
        ),
    };
    if res < 0 {
        return json_response(500, json!({"error":"Erreur visibilité"}));
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/rename
// ══════════════════════════════════════════════════════════════════
fn api_rename(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();
    let new_name = body
        .get("new_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if new_name.is_empty() || new_name.len() > 255 {
        return json_response(400, json!({"error":"Nom invalide"}));
    }
    let res = match item_type.as_str() {
        "folder" => inserer_ou_modifier(
            pool,
            "sitecdos",
            &[("doisernom", mysql::Value::from(new_name.as_str()))],
            &[
                ("iddosier", mysql::Value::from(item_id)),
                ("userid", mysql::Value::from(uid)),
            ],
        ),
        "page" => inserer_ou_modifier(
            pool,
            "sitec",
            &[("nompage", mysql::Value::from(new_name.as_str()))],
            &[
                ("idpage", mysql::Value::from(item_id)),
                ("user_id", mysql::Value::from(uid)),
            ],
        ),
        _ => inserer_ou_modifier(
            pool,
            "fichiers",
            &[("nom", mysql::Value::from(new_name.as_str()))],
            &[
                ("id", mysql::Value::from(item_id)),
                ("id_utilisateur", mysql::Value::from(uid)),
            ],
        ),
    };
    if res < 0 {
        return json_response(500, json!({"error":"Erreur renommage"}));
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/delete
// ══════════════════════════════════════════════════════════════════
fn api_delete(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();
    if item_id == 0 {
        return json_response(400, json!({"error":"item_id manquant"}));
    }
    match item_type.as_str() {
        "folder" => {
            supprimer_ligne(pool, "sitecdos", "iddosier", mysql::Value::from(item_id));
        }
        "page" => {
            supprimer_ligne(pool, "sitec", "idpage", mysql::Value::from(item_id));
        }
        _ => {
            let rows = selectionner(
                pool,
                "fichiers",
                &[
                    ("id", mysql::Value::from(item_id)),
                    ("id_utilisateur", mysql::Value::from(uid)),
                ],
                &["id"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                return json_response(403, json!({"error":"Non autorisé"}));
            }
            supprimer_ligne(pool, "fichiers", "id", mysql::Value::from(item_id));
        }
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/move
// ══════════════════════════════════════════════════════════════════
fn api_move(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let target_folder = body
        .get("target_folder")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();
    let idpage_val = if target_folder > 0 {
        format!("dos:{}", target_folder)
    } else {
        String::new()
    };
    let res = match item_type.as_str() {
        "folder" => inserer_ou_modifier(
            pool,
            "sitecdos",
            &[("idpage", mysql::Value::from(idpage_val.as_str()))],
            &[
                ("iddosier", mysql::Value::from(item_id)),
                ("userid", mysql::Value::from(uid)),
            ],
        ),
        "page" => inserer_ou_modifier(
            pool,
            "sitec",
            &[(
                "idpage",
                mysql::Value::from(target_folder.to_string().as_str()),
            )],
            &[
                ("idpage", mysql::Value::from(item_id)),
                ("user_id", mysql::Value::from(uid)),
            ],
        ),
        _ => return json_response(400, json!({"error":"Type non déplaçable"})),
    };
    if res < 0 {
        return json_response(500, json!({"error":"Erreur déplacement"}));
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// GET /api/fchier/download
// ══════════════════════════════════════════════════════════════════
fn api_download(pool: &DbPool, req: &Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = parse_query(req.url());
    let file_id = match params.get("id").and_then(|v| v.parse::<i64>().ok()) {
        Some(id) => id,
        None => return json_response(400, json!({"error":"id manquant"})),
    };
    let rows = selectionner(
        pool,
        "fichiers",
        &[("id", mysql::Value::from(file_id))],
        &[
            "id_utilisateur",
            "nom",
            "taille",
            "type_fichier",
            "visble",
            "partage",
            "fichier",
        ],
        None,
        Some(1),
    );
    let row = match rows.into_iter().next() {
        Some(r) => r,
        None => return json_response(404, json!({"error":"Fichier introuvable"})),
    };
    let owner = row
    .get("id_utilisateur")
    .and_then(|v| v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())))
    .unwrap_or(-1);
    let visble = row.get("visble").and_then(|v| v.as_str()).unwrap_or("1");
    let partage = row.get("partage").and_then(|v| v.as_str()).unwrap_or("");
    // Accès autorisé si : propriétaire, OU fichier public (visble=0), OU partagé avec l'uid
    if owner != uid && visble != "0" && !is_shared_with(partage, uid) {
        return json_response(403, json!({"error":"Accès refusé"}));
    }
    json_response(
        200,
        json!({
            "success": true,
            "nom":     row.get("nom").cloned().unwrap_or(json!("")),
            "mime":    row.get("type_fichier").cloned().unwrap_or(json!("")),
            "taille":  row.get("taille").cloned().unwrap_or(json!(0)),
            "contenu": row.get("fichier").and_then(|v| v.as_str()).unwrap_or(""),
        }),
    )
}

// â•â• Envoi P2P d'un fichier Ã  un utilisateur (d'un autre nÅ“ud) â•â•
fn api_send_p2p(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"success":false,"error":"JSON invalide"})),
    };
    let fid = body.get("file_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let to_user = body.get("to_user").and_then(|v| v.as_i64()).unwrap_or(0);
    if fid == 0 || to_user == 0 {
        return json_response(400, json!({"success":false,"error":"Paramètres manquants"}));
    }
    // Vérifie que le fichier appartient à l'utilisateur courant
    let fichier = selectionner(
        pool,
        "fichiers",
        &[
            ("id", mysql::Value::from(fid)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
        &["nom", "fichier", "taille"],
        None,
        Some(1),
    )
    .into_iter()
    .next();
    let (nom, chemin) = match fichier {
        Some(f) => (
            f.get("nom")
                .and_then(|v| v.as_str())
                .unwrap_or("file")
                .to_string(),
            f.get("fichier")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        None => return json_response(404, json!({"success":false,"error":"Fichier introuvable"})),
    };
    if chemin.is_empty() {
        return json_response(404, json!({"success":false,"error":"Chemin vide"}));
    }

    let res = send_file_via_p2p(pool, &chemin, &nom, to_user);
    let status = if res.get("success").and_then(|v| v.as_bool()) == Some(true) {
        200
    } else {
        400
    };
    json_response(status, res)
}

fn serve_html(nav_html: &str) -> String {
    include_str!("../../static/fchier/fchier.html").replace("__NAV_HTML__", nav_html)
}