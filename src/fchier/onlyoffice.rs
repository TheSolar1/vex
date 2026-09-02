// ══════════════════════════════════════════════════════════════════
// fchier/onlyoffice.rs — pont entre le gestionnaire de fichiers (fchier,
// chiffre de bout en bout cote navigateur) et OnlyOffice Document Server
// (qui a besoin de recuperer le document lui-meme, en clair, via une URL
// qu'il fetch server-a-server).
//
// COMPROMIS DE SECURITE ASSUME (valide avec l'utilisateur) : pendant une
// session d'edition, le contenu DECHIFFRE du document existe brievement
// sur le disque du serveur (le navigateur l'a deja dechiffre pour
// l'envoyer ici — le serveur ne connait jamais la cle elle-meme). Ce
// fichier temporaire est supprime des la fin de l'edition (`finish`).
// Tous les autres fichiers fchier restent chiffres de bout en bout.
//
// Flux :
//   1. prepare()  : le client envoie le contenu dechiffre -> fichier
//                   temporaire + config OnlyOffice signee (JWT).
//   2. servir_doc(): OnlyOffice (le conteneur) recupere ce fichier via
//                   une URL Rust dediee (PAS sous /onlyoffice/, qui est
//                   intercepte par le reverse-proxy Apache vers le
//                   conteneur -- voir note plus bas).
//   3. callback()  : OnlyOffice notifie les sauvegardes ; on ecrase le
//                   fichier temporaire avec le nouveau contenu (clair).
//   4. finish()    : le client recupere le contenu clair final, le
//                   rechiffre lui-meme, l'enregistre via edit_content,
//                   puis le fichier temporaire est supprime.
//
// Note routage : /onlyoffice/* est entierement redirige par Apache vers
// le conteneur Document Server -- on ne peut donc PAS y servir de
// fichiers statiques. Le stockage physique reste dans
// /var/www/html/onlyoffice/documents/ (deja present, bonnes permissions)
// mais c'est VEX (Rust) qui sert son contenu, pas Apache.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{selectionner, DbPool};
use base64::{engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD}, Engine as _};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tiny_http::{Request, Response};

const DOCUMENTS_DIR: &str = "/var/www/html/onlyoffice/documents";
const EDIT_TTL_SECS: u64 = 3600; // 1h -- une session d'edition ne doit pas trainer indefiniment

struct PendingEdit {
    user_id: i64,
    file_id: i64,
    ext: String,
    path: String,
    created_at: Instant,
}

static PENDING: OnceLock<Mutex<HashMap<String, PendingEdit>>> = OnceLock::new();

fn store() -> &'static Mutex<HashMap<String, PendingEdit>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn purger_expires(map: &mut HashMap<String, PendingEdit>) {
    let now = Instant::now();
    map.retain(|_, p| {
        let vivant = now.duration_since(p.created_at).as_secs() < EDIT_TTL_SECS;
        if !vivant {
            let _ = std::fs::remove_file(&p.path);
        }
        vivant
    });
}

fn json_response(code: u16, v: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(v.to_string())
        .with_status_code(code)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
}

fn lire_body(req: &mut Request) -> Option<Value> {
    let mut s = String::new();
    std::io::Read::read_to_string(req.as_reader(), &mut s).ok()?;
    serde_json::from_str(&s).ok()
}

/// Type de document OnlyOffice ("word"/"cell"/"slide") d'apres l'extension.
fn document_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "xlsx" | "xls" | "ods" | "csv" => "cell",
        "pptx" | "ppt" | "odp" => "slide",
        _ => "word",
    }
}

fn mime_pour_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        _ => "application/octet-stream",
    }
}

/// JWT HS256 minimal (header+payload+signature, base64url sans padding) --
/// suffisant pour signer la config envoyee a DocsAPI.DocEditor, sans
/// ajouter une dependance JWT complete pour ce seul usage.
fn jwt_hs256(payload: &Value, secret: &str) -> String {
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let header_b64 = URL_SAFE_NO_PAD.encode(header.to_string());
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload.to_string());
    let signing_input = format!("{}.{}", header_b64, payload_b64);

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepte une cle de longueur quelconque");
    mac.update(signing_input.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

    format!("{}.{}", signing_input, signature)
}

/// Lit editor.providers.onlyoffice depuis config.json (Value brut : ces
/// champs sont dans `extra`, pas types dans EditorConfig).
fn onlyoffice_provider_cfg() -> Value {
    let cfg = crate::config_loader::load_config("config.json");
    cfg.editor
        .extra
        .get("providers")
        .and_then(|p| p.get("onlyoffice"))
        .cloned()
        .unwrap_or(json!({}))
}

/// URL publique de base (ex: https://vex.hopto.org). Deduite de
/// config.json (extra.server.public_url) si presente, sinon retombe sur
/// un placeholder -- a completer dans config.json le cas echeant.
fn base_url_publique() -> String {
    let cfg = crate::config_loader::load_config("config.json");
    cfg.extra
        .get("server")
        .and_then(|s| s.get("public_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://vex.hopto.org")
        .trim_end_matches('/')
        .to_string()
}

// ══════════════════════════════════════════════════════════════════
// 1. prepare — cree la session d'edition
// ══════════════════════════════════════════════════════════════════

pub fn prepare(pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(body) = lire_body(req) else {
        return json_response(400, json!({"success": false, "error": "Corps invalide"}));
    };
    let file_id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let ext = body.get("ext").and_then(|v| v.as_str()).unwrap_or("").trim_start_matches('.').to_lowercase();
    let contenu_b64 = body.get("contenu_plain_b64").and_then(|v| v.as_str()).unwrap_or("");
    if file_id == 0 || ext.is_empty() || contenu_b64.is_empty() {
        return json_response(400, json!({"success": false, "error": "Parametres manquants"}));
    }
    const MAX_DOC_B64: usize = 30_000_000; // ~22 Mo en clair
    if contenu_b64.len() > MAX_DOC_B64 {
        return json_response(400, json!({"success": false, "error": "Document trop volumineux pour l'edition en ligne."}));
    }
    let Ok(bytes) = B64.decode(contenu_b64) else {
        return json_response(400, json!({"success": false, "error": "Contenu invalide (base64 attendu)."}));
    };

    // Le fichier doit appartenir a l'utilisateur (pas d'edition sur le
    // fichier de quelqu'un d'autre, meme partage en lecture).
    let rows = selectionner(
        pool, "fichiers",
        &[("id", mysql::Value::from(file_id)), ("id_utilisateur", mysql::Value::from(uid))],
        &["nom"], None, Some(1),
    );
    let Some(row) = rows.into_iter().next() else {
        return json_response(403, json!({"success": false, "error": "Fichier introuvable ou acces refuse."}));
    };
    let nom = row.get("nom").and_then(|v| v.as_str()).unwrap_or("document").to_string();

    if std::fs::create_dir_all(DOCUMENTS_DIR).is_err() {
        return json_response(500, json!({"success": false, "error": "Dossier de travail OnlyOffice inaccessible."}));
    }
    let token = format!("{}{}", crate::c::random_hex_id(), crate::c::random_hex_id());
    let path = format!("{}/{}.{}", DOCUMENTS_DIR, token, ext);
    if std::fs::write(&path, &bytes).is_err() {
        return json_response(500, json!({"success": false, "error": "Ecriture du fichier temporaire impossible."}));
    }

    {
        let mut map = store().lock().unwrap();
        purger_expires(&mut map);
        map.insert(token.clone(), PendingEdit {
            user_id: uid, file_id, ext: ext.clone(), path: path.clone(), created_at: Instant::now(),
        });
    }

    let base = base_url_publique();
    let provider = onlyoffice_provider_cfg();
    let jwt_secret = provider.get("jwt_secret").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let jwt_enabled = provider.get("jwt_enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let editor_api_js = format!("{}/onlyoffice/web-apps/apps/api/documents/api.js", base);

    let document_url = format!("{}/api/fchier/onlyoffice/doc?token={}", base, token);
    let callback_url = format!("{}/api/fchier/onlyoffice/callback?token={}", base, token);
    let doc_key = format!("vex_{}_{}", file_id, &token[..16]);

    let mut config = json!({
        "document": {
            "fileType": ext,
            "key": doc_key,
            "title": nom,
            "url": document_url,
            "permissions": {"edit": true, "download": true, "print": true},
        },
        "documentType": document_type(&ext),
        "editorConfig": {
            "mode": "edit",
            "lang": "fr",
            "callbackUrl": callback_url,
            "user": {"id": uid.to_string(), "name": ""},
            "customization": {"forcesave": true, "autosave": true},
        },
    });

    if jwt_enabled && !jwt_secret.is_empty() {
        let token_jwt = jwt_hs256(&config, &jwt_secret);
        config["token"] = json!(token_jwt);
    }

    json_response(200, json!({
        "success": true,
        "token": token,
        "editor_api_js": editor_api_js,
        "config": config,
    }))
}

// ══════════════════════════════════════════════════════════════════
// 2. servir_doc — OnlyOffice recupere le fichier en clair (pas de
//    session : appel serveur-a-serveur depuis le conteneur, protege par
//    le token opaque, non devinable, present uniquement en memoire).
// ══════════════════════════════════════════════════════════════════

pub fn servir_doc(req: &Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = crate::utils::parse_query(req.url());
    let Some(token) = params.get("token") else {
        return Response::from_string("token manquant").with_status_code(400);
    };
    let (path, ext) = {
        let map = store().lock().unwrap();
        match map.get(token) {
            Some(p) => (p.path.clone(), p.ext.clone()),
            None => return Response::from_string("introuvable ou expire").with_status_code(404),
        }
    };
    match std::fs::read(&path) {
        Ok(bytes) => Response::from_data(bytes).with_header(
            tiny_http::Header::from_bytes("Content-Type", mime_pour_ext(&ext)).unwrap(),
        ),
        Err(_) => Response::from_string("fichier introuvable").with_status_code(404),
    }
}

// ══════════════════════════════════════════════════════════════════
// 3. callback — OnlyOffice notifie qu'une sauvegarde est disponible
// ══════════════════════════════════════════════════════════════════

pub fn callback(req: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let params = crate::utils::parse_query(req.url());
    let token = params.get("token").cloned().unwrap_or_default();
    let Some(body) = lire_body(req) else {
        return json_response(200, json!({"error": 0}));
    };
    let status = body.get("status").and_then(|v| v.as_i64()).unwrap_or(0);

    // status 2 = pret a sauver (edition terminee), 6 = forcesave en cours
    // d'edition. Les autres statuts (1=en edition, 4=ferme sans modif) ne
    // necessitent aucune action de notre part.
    if status == 2 || status == 6 {
        if let Some(url) = body.get("url").and_then(|v| v.as_str()) {
            let path = {
                let map = store().lock().unwrap();
                map.get(&token).map(|p| p.path.clone())
            };
            if let Some(path) = path {
                if let Ok(resp) = ureq::get(url).timeout(Duration::from_secs(30)).call() {
                    let mut bytes = Vec::new();
                    if std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes).is_ok() {
                        let _ = std::fs::write(&path, bytes);
                    }
                }
            }
        }
    }

    // OnlyOffice attend exactement {"error":0} pour considerer l'appel reussi.
    json_response(200, json!({"error": 0}))
}

// ══════════════════════════════════════════════════════════════════
// 4. finish — le client reprend la main pour rechiffrer et nettoyer
// ══════════════════════════════════════════════════════════════════

pub fn finish(_pool: &DbPool, req: &mut Request, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(body) = lire_body(req) else {
        return json_response(400, json!({"success": false, "error": "Corps invalide"}));
    };
    let token = body.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let pending = {
        let mut map = store().lock().unwrap();
        match map.get(&token) {
            Some(p) if p.user_id == uid => map.remove(&token),
            Some(_) => return json_response(403, json!({"success": false, "error": "Acces refuse."})),
            None => None,
        }
    };
    let Some(pending) = pending else {
        return json_response(404, json!({"success": false, "error": "Session d'edition introuvable ou deja terminee."}));
    };

    let contenu = std::fs::read(&pending.path).unwrap_or_default();
    let _ = std::fs::remove_file(&pending.path);

    json_response(200, json!({
        "success": true,
        "file_id": pending.file_id,
        "ext": pending.ext,
        "contenu_plain_b64": B64.encode(contenu),
    }))
}
