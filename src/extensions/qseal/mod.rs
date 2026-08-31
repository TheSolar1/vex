// ══════════════════════════════════════════════════════════════════
// extensions/qseal — QSeal porte dans VEX
//
// Chiffrement et signature post-quantiques (ML-KEM-512, Falcon-512,
// AES-GCM) au meme format que l'extension navigateur QSeal : les
// blocs -----BEGIN QSEAL ...----- passent de l'un a l'autre.
//
// Toute la cryptographie se fait dans le navigateur (qseal-core.js).
// Le serveur ne voit jamais une cle secrete en clair : il ne fait que
// ranger les fichiers de cles dans l'explorateur de fichiers VEX
// (table `fichiers`), comme n'importe quel autre document de
// l'utilisateur, avec visble=1 (prive).
//
// Servi sur /ext/qseal, API sur /api/ext/qseal/...
// Le privilege et le plan sont deja verifies par access_control.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::c::SessionInfo;
use serde_json::{json, Value};
use std::io::Cursor;
use tiny_http::{Header, Request, Response};

/// Type MIME maison : c'est lui qui distingue une cle QSeal des
/// autres fichiers de l'explorateur.
const MIME_CLE: &str = "application/x-qseal-key";
const EXTENSION_CLE: &str = ".qsealkey";
const TAILLE_MAX_CLE: usize = 256 * 1024;

pub fn handle(
    pool: &DbPool,
    session: &SessionInfo,
    req: &mut Request,
) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let chemin = url.split('?').next().unwrap_or(&url).to_string();

    if let Some(sous) = chemin.strip_prefix("/api/ext/qseal") {
        let sous = sous.trim_end_matches('/');
        let corps = lire_corps(req);
        let reponse = match sous {
            "/keys" | "" => lister_cles(pool, session),
            "/keys/get" => lire_cle(pool, session, &url),
            "/keys/save" => enregistrer_cle(pool, session, &corps),
            "/keys/delete" => supprimer_cle(pool, session, &corps),
            _ => json!({"success": false, "error": "Route QSeal inconnue."}),
        };
        return json_response(reponse);
    }

    // ── Page ─────────────────────────────────────────────────────
    // Les fichiers statiques (js, css) sont servis par /static/.
    match std::fs::read_to_string("static/extensions/qseal/index.html") {
        Ok(html) => {
            // Barre de navigation VEX + theme de l'utilisateur, comme
            // sur les pages integrees.
            let prefs = crate::function::get_user_preferences(pool, session.user_id);
            let theme = if prefs.teme == 1 { "dark" } else { "light" };
            let nav = crate::access_control::nav_extension(pool, session, req, "qseal");
            let html = html
                .replace("__NAV_HTML__", &nav)
                .replace("__THEME__", theme)
                .replace("__LANG__", &prefs.langue)
                .replace("__USER_NOM__", &echapper(&session.user_nom))
                .replace("__USER_EMAIL__", &echapper(&session.user_email))
                .replace("__USER_ID__", &session.user_id.to_string());
            Response::from_string(html).with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            )
        }
        Err(e) => Response::from_string(format!(
            "<h1>QSeal</h1><p>Page introuvable : static/extensions/qseal/index.html ({})</p>",
            e
        ))
        .with_status_code(500)
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()),
    }
}

// ══════════════════════════════════════════════════════════════════
// Cles rangees dans l'explorateur de fichiers
// ══════════════════════════════════════════════════════════════════

/// Toutes les cles QSeal de l'utilisateur connecte.
fn lister_cles(pool: &DbPool, session: &SessionInfo) -> Value {
    let lignes = selectionner(
        pool,
        "fichiers",
        &[
            ("id_utilisateur", mysql::Value::from(session.user_id.to_string())),
            ("type_fichier", mysql::Value::from(MIME_CLE)),
        ],
        &["id", "nom", "taille", "date"],
        Some("id DESC"),
        None,
    );
    json!({"success": true, "data": lignes.iter().map(|r| json!({
        "id":     r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        "nom":    r.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
        "taille": r.get("taille").and_then(|v| v.as_i64()).unwrap_or(0),
        "date":   r.get("date").and_then(|v| v.as_str()).unwrap_or(""),
    })).collect::<Vec<_>>()})
}

/// Contenu d'une cle — restreint a son proprietaire.
fn lire_cle(pool: &DbPool, session: &SessionInfo, url: &str) -> Value {
    let id = crate::utils::parse_query(url)
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let lignes = selectionner(
        pool,
        "fichiers",
        &[
            ("id", mysql::Value::from(id)),
            // Le filtre proprietaire est dans la requete, pas apres :
            // impossible de lire la cle d'un autre compte.
            ("id_utilisateur", mysql::Value::from(session.user_id.to_string())),
            ("type_fichier", mysql::Value::from(MIME_CLE)),
        ],
        &["id", "nom", "fichier"],
        None,
        Some(1),
    );
    match lignes.first() {
        Some(r) => json!({"success": true, "data": {
            "id":      r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
            "nom":     r.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
            "contenu": r.get("fichier").and_then(|v| v.as_str()).unwrap_or(""),
        }}),
        None => json!({"success": false, "error": "Cle introuvable."}),
    }
}

/// Cree ou remplace une cle. `contenu` est deja encode en base64
/// par le navigateur ; le serveur ne l'interprete pas.
fn enregistrer_cle(
    pool: &DbPool,
    session: &SessionInfo,
    corps: &std::collections::HashMap<String, String>,
) -> Value {
    let nom_brut = corps.get("nom").cloned().unwrap_or_default();
    let nom = nettoyer_nom(&nom_brut);
    if nom.is_empty() {
        return json!({"success": false, "error": "Nom de cle invalide."});
    }
    let contenu = corps.get("contenu").cloned().unwrap_or_default();
    if contenu.trim().is_empty() {
        return json!({"success": false, "error": "Contenu vide."});
    }
    if contenu.len() > TAILLE_MAX_CLE {
        return json!({"success": false, "error": "Cle trop volumineuse."});
    }

    let fichier = format!("{}{}", nom, EXTENSION_CLE);
    let existant = selectionner(
        pool,
        "fichiers",
        &[
            ("nom", mysql::Value::from(fichier.as_str())),
            ("id_utilisateur", mysql::Value::from(session.user_id.to_string())),
            ("type_fichier", mysql::Value::from(MIME_CLE)),
        ],
        &["id"],
        None,
        Some(1),
    );

    let champs: Vec<(&str, mysql::Value)> = vec![
        ("nom", mysql::Value::from(fichier.as_str())),
        ("fichier", mysql::Value::from(contenu.as_str())),
        ("type_fichier", mysql::Value::from(MIME_CLE)),
        ("taille", mysql::Value::from(contenu.len() as i64)),
        // visble = 1 : prive, jamais expose dans les partages publics.
        ("visble", mysql::Value::from("1")),
        (
            "id_utilisateur",
            mysql::Value::from(session.user_id.to_string()),
        ),
    ];

    let id = match existant.first().and_then(|r| r.get("id")).and_then(|v| v.as_i64()) {
        Some(id) => {
            inserer_ou_modifier(pool, "fichiers", &champs, &[("id", mysql::Value::from(id))]);
            id
        }
        None => inserer_ou_modifier(pool, "fichiers", &champs, &[]),
    };

    json!({"success": true, "message": format!("Cle « {} » enregistree dans vos fichiers.", fichier),
           "data": {"id": id, "nom": fichier}})
}

/// Supprime une cle de l'explorateur — proprietaire uniquement.
fn supprimer_cle(
    pool: &DbPool,
    session: &SessionInfo,
    corps: &std::collections::HashMap<String, String>,
) -> Value {
    let id = corps
        .get("id")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let a_moi = selectionner(
        pool,
        "fichiers",
        &[
            ("id", mysql::Value::from(id)),
            ("id_utilisateur", mysql::Value::from(session.user_id.to_string())),
            ("type_fichier", mysql::Value::from(MIME_CLE)),
        ],
        &["id"],
        None,
        Some(1),
    );
    if a_moi.is_empty() {
        return json!({"success": false, "error": "Cle introuvable."});
    }
    supprimer_ligne(pool, "fichiers", "id", mysql::Value::from(id));
    json!({"success": true, "message": "Cle supprimee."})
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Nom de fichier sur : ni chemin, ni caractere de controle.
fn nettoyer_nom(nom: &str) -> String {
    nom.trim()
        .chars()
        .filter(|c| {
            c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ' || *c == '.'
        })
        .take(64)
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn lire_corps(req: &mut Request) -> std::collections::HashMap<String, String> {
    let mut brut = String::new();
    let _ = std::io::Read::read_to_string(req.as_reader(), &mut brut);
    let mut m = std::collections::HashMap::new();
    for paire in brut.split('&') {
        let mut kv = paire.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            m.insert(crate::utils::url_decode(k), crate::utils::url_decode(v));
        }
    }
    m
}

fn json_response(v: Value) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(v.to_string()).with_header(
        Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    )
}
