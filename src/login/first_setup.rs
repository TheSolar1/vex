// ══════════════════════════════════════════════════════════════════
// login/first_setup.rs — sert /static/login/first_setup.html
// GET  → HTML avec __PASS_MIN_LEN__ injecté
// POST → JSON {success, error}
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{compter_lignes, inserer_ou_modifier, DbPool};
use crate::config_loader::VexConfig;
use crate::function::html_escape;
use crate::utils::url_decode;
use std::collections::HashMap;
use tiny_http::{Request, Response};

const HTML_PATH: &str = "static/login/first_setup.html";

pub fn handle_request(mut request: Request, pool: &DbPool, config: &VexConfig, _remote: &str) {
    if compter_lignes(pool, "login", &[]) > 0 {
        let _ = request.respond(
            Response::empty(302)
                .with_header(tiny_http::Header::from_bytes("Location", "/login").unwrap()),
        );
        return;
    }

    if request.method().to_string() == "POST" {
        let body = read_body(&mut request);
        let resp = handle_post(pool, config.security.password_min_length as usize, &body);
        let _ = request.respond(
            Response::from_string(resp).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
        );
        return;
    }

    // GET — sert le HTML statique
    let html = match std::fs::read_to_string(HTML_PATH) {
        Ok(s) => s.replace(
            "__PASS_MIN_LEN__",
            &config.security.password_min_length.to_string(),
        ),
        Err(e) => {
            eprintln!("[first_setup] {}: {}", HTML_PATH, e);
            format!("<h1>Erreur</h1><p>Fichier introuvable : {}</p>", HTML_PATH)
        }
    };
    let _ = request.respond(Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
    ));
}

fn handle_post(pool: &DbPool, pass_min: usize, body: &HashMap<String, String>) -> String {
    if body.get("setup").is_none() {
        return jerr("Requête invalide.");
    }

    let nom = body
        .get("nom")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let email = body
        .get("email")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let mdp = body.get("motdepass").cloned().unwrap_or_default();
    let mdp2 = body.get("motdepass2").cloned().unwrap_or_default();

    if nom.is_empty() || email.is_empty() || mdp.is_empty() {
        return jerr("Tous les champs sont obligatoires.");
    }
    if !email.contains('@') || !email.contains('.') {
        return jerr("Adresse email invalide.");
    }
    if mdp.len() < pass_min {
        return jerr(&format!("Mot de passe : {} caractères minimum.", pass_min));
    }
    if mdp != mdp2 {
        return jerr("Les mots de passe ne correspondent pas.");
    }
    if compter_lignes(pool, "login", &[]) > 0 {
        return jerr("Un compte existe déjà.");
    }

    let id = inserer_ou_modifier(
        pool,
        "login",
        &[
            ("nom", mysql::Value::from(html_escape(&nom).as_str())),
            ("email", mysql::Value::from(html_escape(&email).as_str())),
            ("motdepass", mysql::Value::from(hash_pw(&mdp).as_str())),
            ("privilege", mysql::Value::from(2i64)),
            ("vip", mysql::Value::from(1i64)),
        ],
        &[],
    );

    if id <= 0 {
        return jerr("Erreur lors de la création du compte.");
    }

    inserer_ou_modifier(
        pool,
        "pref",
        &[
            ("id-user", mysql::Value::from(id)),
            ("teme", mysql::Value::from(0i64)),
            ("langue", mysql::Value::from("fr")),
            ("profile_icon_type", mysql::Value::from("initials")),
            ("nav_button_style", mysql::Value::from("{\"dashboard\":1}")),
            ("logo_pages", mysql::Value::from("{\"dashboard\":1}")),
        ],
        &[],
    );

    r#"{"success":true}"#.to_string()
}

fn jerr(msg: &str) -> String {
    format!(
        r#"{{"success":false,"error":"{}"}}"#,
        msg.replace('"', "\\\"")
    )
}

fn hash_pw(pw: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(pw.as_bytes());
    format!("{:x}", h.finalize())
}

fn read_body(req: &mut Request) -> HashMap<String, String> {
    let mut s = String::new();
    let _ = std::io::Read::read_to_string(req.as_reader(), &mut s);
    let mut m = HashMap::new();
    for pair in s.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            m.insert(url_decode(k), url_decode(v));
        }
    }
    m
}
