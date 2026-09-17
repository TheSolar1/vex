// ══════════════════════════════════════════════════════════════════
// login/first_setup.rs — sert /static/login/first_setup.html
// GET  → HTML avec __PASS_MIN_LEN__ injecté
// POST → JSON {success, error}
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{compter_lignes, inserer_avec_erreur, inserer_ou_modifier, DbPool};
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
        // Pas de compte, donc pas de session/preference : langue deduite du
        // seul en-tete Accept-Language (meme principe que login.rs).
        let accept_lang = crate::access_control::get_header(&request, "Accept-Language");
        let langue = crate::function::get_user_language(pool, None, None, Some(&accept_lang));
        let body = read_body(&mut request);
        let resp = handle_post(pool, &body, &langue);
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

fn handle_post(pool: &DbPool, body: &HashMap<String, String>, langue: &str) -> String {
    use crate::i18n::{t, Cle};
    if body.get("setup").is_none() {
        return jerr(t(langue, Cle::SetupErreurRequeteInvalide));
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
    let salt_hex = body.get("srp_salt").cloned().unwrap_or_default();
    let verifier_hex = body.get("srp_verifier").cloned().unwrap_or_default();
    // FIX (code de recuperation des la creation) : le tout premier compte
    // (fondateur/superadmin) passait par un chemin distinct du signup
    // normal et n'envoyait jamais ce materiel -- needs_recovery_setup
    // rattrapait ca a la CONNEXION suivante seulement. Meme logique que
    // handle_signup dans login.rs, generee ici directement.
    let file_key_wrapped_pwd = body.get("file_key_wrapped_pwd").cloned().unwrap_or_default();
    let file_key_wrapped_recovery = body.get("file_key_wrapped_recovery").cloned().unwrap_or_default();
    let recovery_salt = body.get("recovery_salt").cloned().unwrap_or_default();
    let recovery_proof_hash = body.get("recovery_proof_hash").cloned().unwrap_or_default();
    // Pseudo optionnel choisi des la creation du tout premier compte --
    // memes regles que /api/account/pseudo et handle_signup (login.rs).
    let pseudo = html_escape(body.get("pseudo").cloned().unwrap_or_default().trim());

    if nom.is_empty() || email.is_empty() {
        return jerr(t(langue, Cle::SetupErreurChampsObligatoires));
    }
    if !email.contains('@') || !email.contains('.') {
        return jerr(t(langue, Cle::LoginErreurEmailInvalide));
    }
    if !pseudo.is_empty() && (pseudo.len() > 64 || pseudo.contains('@') || pseudo.chars().any(|c| c.is_whitespace())) {
        return jerr("Pseudo invalide.");
    }
    // Validation de forme : salt = 16 octets hex (32 car.), verifier = 256 octets hex (512 car. max)
    if salt_hex.len() != 32 || !salt_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return jerr("Format de salt invalide.");
    }
    if verifier_hex.is_empty()
        || verifier_hex.len() > 512
        || !verifier_hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        return jerr("Format de verifier invalide.");
    }
    if compter_lignes(pool, "login", &[]) > 0 {
        return jerr(t(langue, Cle::SetupErreurCompteExisteDeja));
    }

    // FIX : un echec ici renvoyait un message generique ("Erreur lors de
    // l'inscription.") sans aucun detail -- particulierement genant pour
    // le tout premier compte, qui n'a pas encore de panel admin/Logs pour
    // aller consulter les eprintln du serveur. On affiche desormais le
    // vrai message SQL (meme esprit que le detail d'erreur deja affiche
    // sur la page de connexion normale).
    let mut donnees_insert: Vec<(&str, mysql::Value)> = vec![
        ("nom", mysql::Value::from(html_escape(&nom).as_str())),
        ("email", mysql::Value::from(html_escape(&email).as_str())),
        // FIX (INSERT login echoue silencieusement) : voir le meme fix
        // dans login.rs handle_signup -- `motdepass` est NOT NULL a la
        // creation de la table, rendu nullable seulement par une
        // migration qui peut avoir echoue en silence. On ne depend plus
        // de son succes.
        ("motdepass", mysql::Value::from("")),
        ("srp_salt", mysql::Value::from(salt_hex.as_str())),
        ("srp_verifier", mysql::Value::from(verifier_hex.as_str())),
        ("file_key_wrapped_pwd", mysql::Value::from(file_key_wrapped_pwd.as_str())),
        ("file_key_wrapped_recovery", mysql::Value::from(file_key_wrapped_recovery.as_str())),
        ("recovery_salt", mysql::Value::from(recovery_salt.as_str())),
        ("recovery_proof_hash", mysql::Value::from(recovery_proof_hash.as_str())),
        // Superadmin (2), pas fondateur (1) : le fondateur est un role
        // protege/permanent qui ne devrait pas etre attribue automatiquement
        // au premier compte cree, meme legitime.
        ("privilege", mysql::Value::from(2i64)),
        ("vip", mysql::Value::from(1i64)),
    ];
    if !pseudo.is_empty() {
        donnees_insert.push(("pseudo", mysql::Value::from(pseudo.as_str())));
    }
    let id = match inserer_avec_erreur(pool, "login", &donnees_insert) {
        Ok(id) if id > 0 => id,
        Ok(_) => return jerr(t(langue, Cle::LoginErreurInscription)),
        Err(e) => return jerr(&format!("{} ({e})", t(langue, Cle::LoginErreurInscription))),
    };

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
