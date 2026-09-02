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
    // NOTE : ne PAS utiliser X-Forwarded-For ici. L'app tourne derrière
    // Apache (reverse proxy), et le reste du code (main.rs, login.rs,
    // dashboard.rs) identifie/stocke la session via request.remote_addr()
    // brut (donc "127.0.0.1" côté serveur, pas l'IP réelle du client).
    // Préférer X-Forwarded-For ici cassait la comparaison IP de
    // verifier_connexion() : la session semblait toujours invalide sur
    // /fchier alors qu'elle était valide partout ailleurs.
    req.remote_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default()
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
    let ip = crate::utils::strip_port(&remote_ip(req));
    crate::appeldb::verifier_connexion(pool, &cookie, &ip, &user_agent(req))
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

    // OnlyOffice (le conteneur Document Server) appelle ces deux routes
    // directement, serveur-a-serveur -- il n'a pas notre cookie de
    // session. Protegees par un token opaque non devinable (voir
    // fchier/onlyoffice.rs), pas par l'authentification VEX normale.
    if url.starts_with("/api/fchier/onlyoffice/doc") {
        return super::onlyoffice::servir_doc(req);
    }
    if url.starts_with("/api/fchier/onlyoffice/callback") {
        return super::onlyoffice::callback(req);
    }

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
            "prefs" => api_prefs(pool, uid),     
            "upload" => api_upload(pool, req, uid),
            "create_folder" => api_create_folder(pool, req, uid),
            "create_page" => api_create_page(pool, req, uid),
            "share" => api_share(pool, req, uid),
            "change_visibility" => api_change_visibility(pool, req, uid),
            "rename" => api_rename(pool, req, uid),
            "delete" => api_delete(pool, req, uid),
            "move" => api_move(pool, req, uid),
            "download" => api_download(pool, req, uid),
            "edit_content" => api_edit_content(pool, req, uid),
            "onlyoffice/prepare" => super::onlyoffice::prepare(pool, req, uid),
            "onlyoffice/finish" => super::onlyoffice::finish(pool, req, uid),
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

    // ── Tous dossiers (déplacement + arborescence latérale) + index
    // fichier→dossier ET page→dossier. Ni un fichier ni une page n'a de
    // colonne "dossier" à lui : leur appartenance est encodée dans le
    // champ idpage DU DOSSIER qui les contient, sous forme "fich:<id>"
    // / "page:<id>" (posé par api_upload/api_create_page/api_move). On
    // construit ces index ICI, avant "Fichiers"/"Pages", pour pouvoir
    // filtrer par dossier courant juste après.
    let all_folders_raw = selectionner(
        pool,
        "sitecdos",
        &[],
        &["iddosier", "doisernom", "userid", "addpageuserid", "idpage"],
        Some("doisernom ASC"),
        None,
    );
    let mut all_folders: Vec<Value> = Vec::new();
    let mut file_parent: HashMap<i64, i64> = HashMap::new();
    let mut page_parent: HashMap<String, i64> = HashMap::new();
    for row in &all_folders_raw {
        let owner = row.get("userid").and_then(|v| v.as_i64()).unwrap_or(-1);
        let addpage = row
            .get("addpageuserid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let folder_id = row.get("iddosier").and_then(|v| v.as_i64()).unwrap_or(0);
        let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();

        // L'appartenance réelle d'un fichier/page à ce dossier ne dépend
        // pas de qui a le droit de VOIR le dossier dans l'arbre — on
        // indexe donc tous les dossiers, visibles ou non par l'utilisateur.
        for tok in idpage.split(',') {
            let tok = tok.trim();
            if let Some(fid_str) = tok.strip_prefix("fich:") {
                if let Ok(fid) = fid_str.parse::<i64>() {
                    file_parent.insert(fid, folder_id);
                }
            } else if let Some(pid_str) = tok.strip_prefix("page:") {
                page_parent.insert(pid_str.to_string(), folder_id);
            }
        }

        if owner != uid && !is_shared_with(&addpage, uid) {
            continue;
        }
        let parent_id = dos_parent(&idpage).unwrap_or(0);
        all_folders.push(json!({
            "id": folder_id,
            "nom": row.get("doisernom").cloned().unwrap_or(json!("")),
            "parent_id": parent_id,
        }));
    }

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
            let file_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
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

            // Filtrage par dossier courant (même principe que pour les
            // sous-dossiers juste au-dessus : un fichier sans entrée dans
            // file_parent est "à la racine").
            let file_folder = file_parent.get(&file_id).copied().unwrap_or(0);
            if file_folder != dossier {
                return None;
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

    // ── Pages (table sitec_pages — c'est celle que l'éditeur Sitec,
    // src/sitec/sitec.rs, lit et écrit réellement. La table "sitec" est
    // un vestige d'un ancien schéma et n'est plus utilisée par l'éditeur)
    let pages_raw = selectionner(
        pool,
        "sitec_pages",
        &[("owner_id", mysql::Value::from(uid))],
        &["id", "titre", "public", "partage"],
        Some("titre ASC"),
        None,
    );
    let pages_web: Vec<Value> = pages_raw
        .into_iter()
        .filter_map(|row| {
            let page_id = row.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let page_folder = page_parent.get(&page_id).copied().unwrap_or(0);
            if page_folder != dossier {
                return None;
            }
            let partage = row
                .get("partage")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            // sitec_pages.public : 1 = publique. La convention utilisée ailleurs
            // dans fchier (champs "visble"/"prob" : 1 = privé, 0 = public) est
            // inversée — on convertit pour que la modale Visibilité reste cohérente.
            let is_public = row.get("public").and_then(|v| v.as_i64()).unwrap_or(0) == 1;
            Some(json!({
                "id":      json!(page_id),
                "nom":     row.get("titre").cloned().unwrap_or(json!("")),
                "partage": partage,
                "prob":    if is_public { 0 } else { 1 },
            }))
        })
        .collect();

    // ── Breadcrumb
    let chemin = if dossier != 0 {
        build_chemin(pool, dossier)
    } else {
        vec![json!({"id":0,"nom":"Mes fichiers"})]
    };


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
// `sitecdos.iddosier` n'a pas AUTO_INCREMENT côté DB : on doit calculer
// nous-mêmes le prochain id (MAX(iddosier)+1).
fn next_sitecdos_id(pool: &DbPool) -> i64 {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return 1,
    };
    let max: Option<i64> = mysql::prelude::Queryable::query_first(
        &mut conn,
        "SELECT MAX(iddosier) FROM sitecdos",
    )
    .unwrap_or(None)
    .flatten();
    max.unwrap_or(0) + 1
}

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
    // `iddosier` n'est PAS auto_increment côté DB (confirmé via
    // SHOW COLUMNS FROM sitecdos : int NOT NULL, sans AUTO_INCREMENT ni
    // valeur par défaut) — il faut le fournir nous-mêmes, sinon MySQL
    // rejette l'insertion avec "Field 'iddosier' doesn't have a default
    // value". D'où le 500 systématique sur toute création de dossier.
    // Limite connue : MAX()+1 a une petite fenêtre de collision en cas
    // de deux créations strictement simultanées ; acceptable ici vu
    // l'absence d'AUTO_INCREMENT réel côté schéma.
    let new_id = next_sitecdos_id(pool);
    // addpageuserid = vide à la création (sera rempli par api_share)
    let id = inserer_ou_modifier(
        pool,
        "sitecdos",
        &[
            ("iddosier", mysql::Value::from(new_id)),
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
// POST /api/fchier/create_page
//   Crée une page dans `sitec_pages` (même table que l'éditeur Sitec)
//   et renvoie son id (20 caractères) pour que le front puisse ouvrir
//   /sitec?open=<id> immédiatement après création.
// ══════════════════════════════════════════════════════════════════
const PAGE_ID_LEN: usize = 20;
const PAGE_ID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

fn random_page_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1) as u64;
    let mut state = seed ^ 0x9e3779b97f4a7c15 ^ (std::process::id() as u64);
    let mut out = String::with_capacity(PAGE_ID_LEN);
    for _ in 0..PAGE_ID_LEN {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(PAGE_ID_CHARS[(state as usize) % PAGE_ID_CHARS.len()] as char);
    }
    out
}

fn generate_unique_page_id(pool: &DbPool) -> String {
    for _ in 0..10 {
        let candidate = random_page_id();
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
    random_page_id() // collision quasi impossible sur 20 car.
}

// Filet de sécurité : au cas où l'éditeur Sitec n'a encore jamais été
// ouvert et que la table n'existe pas encore.
fn ensure_sitec_pages_table(pool: &DbPool) {
    if let Ok(mut conn) = pool.get_conn() {
        let _ = mysql::prelude::Queryable::query_drop(
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
        );
    }
}

fn api_create_page(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    // Les pages sont rangées dans un dossier via le même mécanisme que
    // les fichiers : le champ idpage DU DOSSIER liste ses pages sous
    // forme "page:<id>" (id = 20 caractères), en plus de ses éventuels
    // "fich:<id>" et sous-dossiers "dos:<id>". sitec_pages elle-même n'a
    // toujours pas de colonne d'emplacement, et n'en a pas besoin.
    let body = parse_json_body(req);
    let parent_id = body
        .as_ref()
        .and_then(|b| b.get("parent_id"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    ensure_sitec_pages_table(pool);

    let id = generate_unique_page_id(pool);
    let ok = inserer_ou_modifier(
        pool,
        "sitec_pages",
        &[
            ("id", mysql::Value::from(id.as_str())),
            ("owner_id", mysql::Value::from(uid)),
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
    if ok < 0 {
        return json_response(500, json!({"error":"Erreur création page"}));
    }

    if parent_id > 0 {
        let rows = selectionner(
            pool,
            "sitecdos",
            &[("iddosier", mysql::Value::from(parent_id))],
            &["idpage"],
            None,
            Some(1),
        );
        if let Some(row) = rows.into_iter().next() {
            let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let new_idpage = if idpage.is_empty() {
                format!("page:{}", id)
            } else {
                format!("{},page:{}", idpage, id)
            };
            inserer_ou_modifier(
                pool,
                "sitecdos",
                &[("idpage", mysql::Value::from(new_idpage.as_str()))],
                &[("iddosier", mysql::Value::from(parent_id))],
            );
        }
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

    // ── Pages : id string (20 car.), table sitec_pages, format de partage
    // "uid:12,uid:45" attendu par src/sitec/sitec.rs (partage_contains).
    if item_type == "page" {
        let item_id = body
            .get("item_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if item_id.is_empty() {
            return json_response(400, json!({"error":"item_id manquant"}));
        }
        let rows = selectionner(
            pool,
            "sitec_pages",
            &[
                ("id", mysql::Value::from(item_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
            &["id"],
            None,
            Some(1),
        );
        if rows.is_empty() {
            return json_response(403, json!({"error":"Non autorisé"}));
        }
        let share_normalized: String = share_data
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i64>().ok())
            .map(|n| format!("uid:{}", n))
            .collect::<Vec<_>>()
            .join(",");
        let res = inserer_ou_modifier(
            pool,
            "sitec_pages",
            &[("partage", mysql::Value::from(share_normalized.as_str()))],
            &[
                ("id", mysql::Value::from(item_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
        );
        if res < 0 {
            return json_response(500, json!({"error":"Erreur partage"}));
        }
        return json_response(200, json!({"success":true}));
    }

    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
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

    if item_type == "page" {
        let page_id = body
            .get("page_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if page_id.is_empty() {
            return json_response(400, json!({"error":"page_id manquant"}));
        }
        // Convention fchier : "1"=privé,"0"=public. sitec_pages.public : 1=public.
        let public_val: i64 = if new_visibility == "0" { 1 } else { 0 };
        let res = inserer_ou_modifier(
            pool,
            "sitec_pages",
            &[("public", mysql::Value::from(public_val))],
            &[
                ("id", mysql::Value::from(page_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
        );
        if res < 0 {
            return json_response(500, json!({"error":"Erreur visibilité"}));
        }
        return json_response(200, json!({"success":true}));
    }

    let page_id = body.get("page_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let res = inserer_ou_modifier(
        pool,
        "fichiers",
        &[("visble", mysql::Value::from(new_visibility.as_str()))],
        &[
            ("id", mysql::Value::from(page_id)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
    );
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

    if item_type == "page" {
        let item_id = body
            .get("item_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if item_id.is_empty() {
            return json_response(400, json!({"error":"item_id manquant"}));
        }
        let res = inserer_ou_modifier(
            pool,
            "sitec_pages",
            &[("titre", mysql::Value::from(new_name.as_str()))],
            &[
                ("id", mysql::Value::from(item_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
        );
        if res < 0 {
            return json_response(500, json!({"error":"Erreur renommage"}));
        }
        return json_response(200, json!({"success":true}));
    }

    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
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
// POST /api/fchier/edit_content — enregistre le contenu edite d'un
// fichier texte depuis l'editeur en ligne. Reserve au proprietaire
// (la clause id_utilisateur=uid scope la mise a jour, meme pattern que
// api_rename : 0 ligne affectee si l'appelant n'est pas proprietaire).
// ══════════════════════════════════════════════════════════════════
fn api_edit_content(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let contenu_b64 = body.get("contenu_b64").and_then(|v| v.as_str()).unwrap_or("");
    if item_id == 0 {
        return json_response(400, json!({"error":"id manquant"}));
    }
    // Limite volontairement basse : l'editeur en ligne vise les petits
    // fichiers texte, pas un remplacement de l'upload pour gros fichiers.
    const MAX_EDIT_B64: usize = 4_000_000;
    if contenu_b64.len() > MAX_EDIT_B64 {
        return json_response(400, json!({"error":"Fichier trop volumineux pour l'editeur en ligne (max ~3 Mo)."}));
    }
    if B64.decode(contenu_b64).is_err() {
        return json_response(400, json!({"error":"Contenu invalide (base64 attendu)."}));
    }
    let taille_reelle = contenu_b64.len() as i64 * 3 / 4;
    let res = inserer_ou_modifier(
        pool,
        "fichiers",
        &[
            ("fichier", mysql::Value::from(contenu_b64)),
            ("taille", mysql::Value::from(taille_reelle)),
        ],
        &[
            ("id", mysql::Value::from(item_id)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
    );
    if res < 0 {
        return json_response(500, json!({"error":"Erreur d'enregistrement"}));
    }
    json_response(200, json!({"success":true}))
}

// ══════════════════════════════════════════════════════════════════
// POST /api/fchier/delete
// ── CORRIGÉ : vérification de propriété ajoutée pour "folder" et "page"
//    (auparavant, n'importe quel utilisateur connecté pouvait supprimer
//    le dossier ou la page de n'importe qui — faille IDOR).
// ══════════════════════════════════════════════════════════════════
fn api_delete(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"error":"Corps invalide"})),
    };
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();

    if item_type == "page" {
        let item_id = body
            .get("item_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if item_id.is_empty() {
            return json_response(400, json!({"error":"item_id manquant"}));
        }
        let rows = selectionner(
            pool,
            "sitec_pages",
            &[
                ("id", mysql::Value::from(item_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
            &["id"],
            None,
            Some(1),
        );
        if rows.is_empty() {
            return json_response(403, json!({"error":"Non autorisé"}));
        }
        let needle = format!("page:{}", item_id);
        let mes_dossiers = selectionner(
            pool,
            "sitecdos",
            &[("userid", mysql::Value::from(uid))],
            &["iddosier", "idpage"],
            None,
            None,
        );
        for row in mes_dossiers {
            let fid = row.get("iddosier").and_then(|v| v.as_i64()).unwrap_or(0);
            let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if idpage.split(',').any(|t| t.trim() == needle) {
                let cleaned: String = idpage
                    .split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty() && *t != needle)
                    .collect::<Vec<_>>()
                    .join(",");
                inserer_ou_modifier(
                    pool,
                    "sitecdos",
                    &[("idpage", mysql::Value::from(cleaned.as_str()))],
                    &[("iddosier", mysql::Value::from(fid))],
                );
            }
        }
        supprimer_ligne(pool, "sitec_pages", "id", mysql::Value::from(item_id.as_str()));
        return json_response(200, json!({"success":true}));
    }

    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    if item_id == 0 {
        return json_response(400, json!({"error":"item_id manquant"}));
    }
    match item_type.as_str() {
        "folder" => {
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
            supprimer_ligne(pool, "sitecdos", "iddosier", mysql::Value::from(item_id));
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
    let item_type = body
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("file")
        .to_string();

    if item_type == "page" {
        // Même mécanisme que pour les fichiers : la page n'a pas de
        // colonne d'emplacement propre, son appartenance est encodée
        // dans le champ idpage DU DOSSIER, sous forme "page:<id>".
        let item_id = body
            .get("item_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let target_folder = body
            .get("target_folder")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if item_id.is_empty() {
            return json_response(400, json!({"error":"item_id manquant"}));
        }
        let owned = selectionner(
            pool,
            "sitec_pages",
            &[
                ("id", mysql::Value::from(item_id.as_str())),
                ("owner_id", mysql::Value::from(uid)),
            ],
            &["id"],
            None,
            Some(1),
        );
        if owned.is_empty() {
            return json_response(403, json!({"error":"Non autorisé"}));
        }

        let needle = format!("page:{}", item_id);
        let mes_dossiers = selectionner(
            pool,
            "sitecdos",
            &[("userid", mysql::Value::from(uid))],
            &["iddosier", "idpage"],
            None,
            None,
        );
        for row in mes_dossiers {
            let fid = row.get("iddosier").and_then(|v| v.as_i64()).unwrap_or(0);
            let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if idpage.split(',').any(|t| t.trim() == needle) {
                let cleaned: String = idpage
                    .split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty() && *t != needle)
                    .collect::<Vec<_>>()
                    .join(",");
                inserer_ou_modifier(
                    pool,
                    "sitecdos",
                    &[("idpage", mysql::Value::from(cleaned.as_str()))],
                    &[("iddosier", mysql::Value::from(fid))],
                );
            }
        }

        if target_folder > 0 {
            let rows = selectionner(
                pool,
                "sitecdos",
                &[("iddosier", mysql::Value::from(target_folder))],
                &["idpage"],
                None,
                Some(1),
            );
            if let Some(row) = rows.into_iter().next() {
                let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let new_idpage = if idpage.is_empty() {
                    format!("page:{}", item_id)
                } else {
                    format!("{},page:{}", idpage, item_id)
                };
                inserer_ou_modifier(
                    pool,
                    "sitecdos",
                    &[("idpage", mysql::Value::from(new_idpage.as_str()))],
                    &[("iddosier", mysql::Value::from(target_folder))],
                );
            } else {
                return json_response(404, json!({"error":"Dossier cible introuvable"}));
            }
        }
        return json_response(200, json!({"success":true}));
    }

    let item_id = body.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let target_folder = body
        .get("target_folder")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if item_type == "file" {
        // Vérifie que le fichier appartient à l'utilisateur.
        let owned = selectionner(
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
        if owned.is_empty() {
            return json_response(403, json!({"error":"Non autorisé"}));
        }

        // Un fichier n'a pas de colonne "dossier" à lui : son appartenance
        // est encodée dans le champ idpage DU DOSSIER qui le contient,
        // sous forme "fich:<id>" (même convention que api_upload). On
        // retire donc d'abord la référence de tous les dossiers de
        // l'utilisateur qui la listent...
        let needle = format!("fich:{}", item_id);
        let mes_dossiers = selectionner(
            pool,
            "sitecdos",
            &[("userid", mysql::Value::from(uid))],
            &["iddosier", "idpage"],
            None,
            None,
        );
        for row in mes_dossiers {
            let fid = row.get("iddosier").and_then(|v| v.as_i64()).unwrap_or(0);
            let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if idpage.split(',').any(|t| t.trim() == needle) {
                let cleaned: String = idpage
                    .split(',')
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty() && *t != needle)
                    .collect::<Vec<_>>()
                    .join(",");
                inserer_ou_modifier(
                    pool,
                    "sitecdos",
                    &[("idpage", mysql::Value::from(cleaned.as_str()))],
                    &[("iddosier", mysql::Value::from(fid))],
                );
            }
        }

        // ...puis on l'ajoute au dossier cible (si ce n'est pas la racine —
        // la racine n'a pas de ligne sitecdos, "ne plus être listé nulle
        // part" suffit à représenter "à la racine").
        if target_folder > 0 {
            let rows = selectionner(
                pool,
                "sitecdos",
                &[("iddosier", mysql::Value::from(target_folder))],
                &["idpage"],
                None,
                Some(1),
            );
            if let Some(row) = rows.into_iter().next() {
                let idpage = row.get("idpage").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let new_idpage = if idpage.is_empty() {
                    format!("fich:{}", item_id)
                } else {
                    format!("{},fich:{}", idpage, item_id)
                };
                inserer_ou_modifier(
                    pool,
                    "sitecdos",
                    &[("idpage", mysql::Value::from(new_idpage.as_str()))],
                    &[("iddosier", mysql::Value::from(target_folder))],
                );
            } else {
                return json_response(404, json!({"error":"Dossier cible introuvable"}));
            }
        }
        return json_response(200, json!({"success":true}));
    }

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

// ── Envoi P2P d'un fichier à un utilisateur (d'un autre nœud) ──
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
// ══════════════════════════════════════════════════════════════════
// api_prefs
//   Table réelle : `pref` (pas "users")
//   Colonne id   : `id-user` (avec des backticks, contient un tiret)
//   Lecture par requête SQL brute via mysql::prelude::Queryable::query_first
// ══════════════════════════════════════════════════════════════════
fn api_prefs(pool: &DbPool, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(e) => {
            return json_response(
                500,
                json!({"success": false, "error": format!("DB: {}", e)}),
            )
        }
    };

    let teme: i64 = mysql::prelude::Queryable::query_first(
        &mut conn,
        format!("SELECT COALESCE(teme,0) FROM pref WHERE `id-user`={}", uid),
    )
    .unwrap_or(Some(0))
    .unwrap_or(0);

    json_response(200, json!({ "success": true, "teme": teme }))
}