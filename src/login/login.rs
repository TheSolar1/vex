// ══════════════════════════════════════════════════════════════════
// login/login.rs — VEX Login + Inscription — SRP-6a (RFC 5054, SHA-256)
//
// - Le mot de passe NE QUITTE JAMAIS le navigateur, sous AUCUNE forme,
//   même hashée : ni en clair, ni en hash réutilisable pour se connecter.
// - À l'inscription, le client calcule un `salt` + un `verifier`
//   mathématiques (voir crate::srp) et n'envoie que ça — le serveur ne
//   peut pas en retrouver le mot de passe, et le verifier seul ne
//   permet pas de se connecter (pas de "pass-the-hash").
// - À la connexion, un échange en 2 étapes (SRP step1/step2) prouve
//   que le client connaît le mot de passe sans jamais le transmettre
//   ni transmettre une valeur replayable.
// - Le mdp en clair reste en mémoire JS côté client, pour dériver la clé
//   de chiffrement des fichiers (inchangé, géré par vex-crypto.js).
//
// SCHÉMA DB REQUIS (migration depuis l'ancienne colonne `motdepass`) :
//
//   ALTER TABLE `login`
//     DROP COLUMN `motdepass`,
//     ADD COLUMN `srp_salt`     VARCHAR(64)  NOT NULL,   -- hex, 32 octets
//     ADD COLUMN `srp_verifier` VARCHAR(512) NOT NULL;   -- hex, 256 octets
//
//   CREATE TABLE `srp_sessions` (
//     `token`      VARCHAR(64)  NOT NULL PRIMARY KEY,   -- corrèle step1 → step2
//     `email`      VARCHAR(255) NOT NULL,
//     `b_hex`      VARCHAR(64)  NOT NULL,                -- exposant privé serveur (éphémère)
//     `created_at` DATETIME     NOT NULL DEFAULT NOW()
//   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
//
//   Nettoyage périodique recommandé (cron, comme meet_signaling) :
//     DELETE FROM srp_sessions WHERE created_at < NOW() - INTERVAL 5 MINUTE;
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{compter_lignes, inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::config_loader::VexConfig;
use crate::srp::{
    self, bigint_from_hex, compute_b_public, compute_k, compute_m1, compute_m2, compute_s_server,
    compute_u, constant_time_eq, generate_b, group, hex_decode, hex_encode, is_safe_public_value,
};
use crate::utils::{strip_port, url_decode};
use serde_json::json;
use std::collections::HashMap;
use tiny_http::{Request, Response};

/// Durée de vie max d'une session SRP éphémère (step1 → step2).
const SRP_SESSION_MAX_AGE_SECONDS: i64 = 300; // 5 minutes

pub fn handle_request(mut request: Request, pool: &DbPool, config: &VexConfig, remote_full: &str) {
    let remote_ip = strip_port(remote_full);
    let method = request.method().to_string();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    // ── Config publique ──────────────────────────────────────────
    if path == "/api/login/config" {
        let referer = get_header(&request, "Referer");
        let host = get_header(&request, "Host");
        if !referer.is_empty() && !referer.contains(&host) {
            respond_json(request, json!({"error":"Forbidden"}), 403);
            return;
        }
        respond_json(
            request,
            json!({
                "registration_mode":       config.users.registration_mode,
                "activation_key_required": config.users.activation_key_required,
                "password_min_length":     config.security.password_min_length,
                // FIX (page de paiement, maquette) : le prix des plans doit
                // être visible sans être connecté -- seul le prix/nom/id est
                // nécessaire ici, contrairement à /api/admin/config qui
                // expose toute la config (réservé aux admins).
                "plans": {
                    "available_plans": config.plans.available_plans,
                    "discount_codes": config.plans.extra.get("discount_codes").cloned().unwrap_or(json!([])),
                },
            }),
            200,
        );
        return;
    }

    // ── Déjà connecté → redirige ─────────────────────────────────
    if method == "GET" && !cookie_val.is_empty() {
        if crate::c::verifier_session(pool, &cookie_val, &remote_ip, &user_agent).connecte {
            redirect(request, "/login/dashboard");
            return;
        }
    }

    // ── Premier lancement → redirige vers first_setup ────────────
    if method == "GET" && compter_lignes(pool, "login", &[]) == 0 {
        redirect(request, "/login/first_setup");
        return;
    }

    // ── GET → sert le HTML statique ──────────────────────────────
    if method == "GET" {
        let accept_lang = get_header(&request, "Accept-Language");
        serve_login_html(request, pool, &accept_lang);
        return;
    }

    // ── IP blacklist / whitelist ──────────────────────────────────
    if config.security.ip_blacklist_enabled {
        if config.security.ip_blacklist.iter().any(|ip| ip == &remote_ip) {
            respond_json(
                request,
                json!({"success":false,"error":"Accès refusé : IP bloquée."}),
                403,
            );
            return;
        }
    }
    if config.security.ip_whitelist_enabled && !config.security.ip_whitelist.is_empty() {
        if !config.security.ip_whitelist.iter().any(|ip| ip == &remote_ip) {
            respond_json(
                request,
                json!({"success":false,"error":"Accès refusé : IP non autorisée."}),
                403,
            );
            return;
        }
    }

    // Pas de session avant connexion : langue deduite du seul en-tete
    // Accept-Language (meme mecanisme que serve_login_html).
    let accept_lang = get_header(&request, "Accept-Language");
    let langue = crate::function::get_user_language(pool, None, None, Some(&accept_lang));

    let body = read_body(&mut request);
    let action = body.get("action").map(|s| s.as_str()).unwrap_or("");

    match action {
        // ── Nouveau flux SRP-6a en 2 étapes ────────────────────────
        "srp_step1" => handle_srp_step1(request, pool, &body, &langue),
        "srp_step2" => handle_srp_step2(request, pool, &body, &remote_ip, &user_agent, &langue),
        "signup" => handle_signup(request, pool, config, &body, &langue),
        "enregistrer_recuperation" => handle_enregistrer_recuperation(request, pool, &body, &cookie_val, &remote_ip, &user_agent),
        "recuperation_info" => handle_recuperation_info(request, pool, &body),
        "recuperation_confirmer" => handle_recuperation_confirmer(request, pool, &body),
        "envoyer_code_recuperation" => handle_envoyer_code_recuperation(request, pool, &body),
        _ => respond_json(
            request,
            json!({"success":false,"error":"Action inconnue"}),
            400,
        ),
    }
}

// ══════════════════════════════════════════════════════════════════
// SRP — Étape 1 : le client envoie son email, le serveur répond avec
// le salt, sa valeur publique B, et un token pour corréler l'étape 2.
// ══════════════════════════════════════════════════════════════════
fn handle_srp_step1(request: Request, pool: &DbPool, body: &HashMap<String, String>, langue: &str) {
    use crate::i18n::{t, Cle};
    // FIX (connexion par pseudo) : le champ envoyé par le client s'appelle
    // toujours "email" pour compat, mais peut contenir soit l'email, soit
    // le pseudo du compte — on résout vers l'email réel ci-dessous, car
    // tout le calcul SRP et la dérivation des clés de fichiers (crypto.js)
    // sont ancrés sur l'email, jamais sur le pseudo.
    let identifiant = body.get("email").cloned().unwrap_or_default();

    // FIX (défense en profondeur, même logique que step2) : borne la
    // taille de l'identifiant avant tout hash/lookup — évite qu'une entrée
    // anormalement longue serve à faire travailler inutilement le
    // hachage SHA-256 ou les fonctions de la table `login`.
    if identifiant.is_empty() || identifiant.len() > 255 {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurEmailInvalide)}), 400);
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(identifiant.as_str()))],
        &["email", "srp_salt", "srp_verifier"],
        None,
        Some(1),
    );
    let rows = if rows.is_empty() {
        selectionner(
            pool,
            "login",
            &[("pseudo", mysql::Value::from(identifiant.as_str()))],
            &["email", "srp_salt", "srp_verifier"],
            None,
            Some(1),
        )
    } else {
        rows
    };

    // ── Anti-énumération de comptes ───────────────────────────────
    // Si le compte n'existe pas, on NE DOIT PAS répondre différemment
    // (sinon on révèle l'existence de l'email/pseudo). On génère un
    // salt/verifier factices mais déterministes-par-identifiant (donc
    // stables si l'attaquant retente), pour que le comportement soit
    // indistinguable d'un vrai compte du point de vue du timing/format
    // de réponse.
    let (email, salt_hex, verifier_hex) = if let Some(row) = rows.into_iter().next() {
        let e = row.get("email").and_then(|v| v.as_str()).unwrap_or(identifiant.as_str()).to_string();
        let s = row.get("srp_salt").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let v = row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
        (e, s, v)
    } else {
        let (s, v) = fake_salt_and_verifier(&identifiant);
        (identifiant.clone(), s, v)
    };

    let grp = group();
    let Some(v_big) = bigint_from_hex(&verifier_hex) else {
        respond_json(request, json!({"success":false,"error":"Erreur interne (verifier)."}), 500);
        return;
    };

    let b = generate_b();
    let b_pub = compute_b_public(&grp, &v_big, &b);

    let token = hex_encode(&srp::random_bytes(24));
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    inserer_ou_modifier(
        pool,
        "srp_sessions",
        &[
            ("token", mysql::Value::from(token.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("b_hex", mysql::Value::from(hex_encode(&b.to_bytes_be()).as_str())),
            ("created_at", mysql::Value::from(now.as_str())),
        ],
        &[],
    );

    respond_json(
        request,
        json!({
            "success": true,
            "email":   email,
            "salt":    salt_hex,
            "B":       hex_encode(&b_pub.to_bytes_be()),
            "token":   token,
        }),
        200,
    );
}

/// Génère un salt/verifier factices mais stables pour un email donné,
/// pour que /srp_step1 sur un compte inexistant se comporte comme un
/// vrai compte (anti-énumération). Dérivé de SHA-256(email) — jamais
/// utilisé pour un vrai calcul de mot de passe, juste pour la forme.
fn fake_salt_and_verifier(email: &str) -> (String, String) {
    let h = srp::sha256(email.to_lowercase().as_bytes());
    let salt_hex = hex_encode(&h[..16]);
    // "Verifier" factice = hash étendu, jamais un vrai g^x mod N, mais de
    // la bonne forme hex pour ne pas planter bigint_from_hex côté step1.
    let mut extended = h.clone();
    extended.extend_from_slice(&srp::sha256(&h));
    extended.extend_from_slice(&srp::sha256(&extended[..32]));
    extended.truncate(srp::N_LEN_BYTES);
    (salt_hex, hex_encode(&extended))
}

// ══════════════════════════════════════════════════════════════════
// SRP — Étape 2 : le client prouve qu'il connaît le mot de passe (M1),
// le serveur vérifie et renvoie sa propre preuve (M2) + ouvre la session.
// ══════════════════════════════════════════════════════════════════
fn handle_srp_step2(
    request: Request,
    pool: &DbPool,
    body: &HashMap<String, String>,
    remote_ip: &str,
    user_agent: &str,
    langue: &str,
) {
    use crate::i18n::{t, Cle};
    let token = body.get("token").cloned().unwrap_or_default();
    let email = body.get("email").cloned().unwrap_or_default();
    let a_hex = body.get("A").cloned().unwrap_or_default();
    let m1_hex = body.get("M1").cloned().unwrap_or_default();

    // ── FIX SÉCURITÉ (DoS) ──────────────────────────────────────────
    // Avant TOUT parsing en BigUint, on borne strictement la taille de
    // A et M1. Sans ce contrôle, un client pouvait envoyer un `A` de
    // plusieurs Mo de texte hex : ça se transforme en un BigUint géant,
    // et le modpow + hash qui suivent deviennent arbitrairement coûteux
    // (CPU/mémoire) — un déni de service trivial en une seule requête,
    // qui ne se voit jamais en usage normal (le login continue de
    // fonctionner), donc casse la sécurité "en silence".
    //   - A : au plus 512 car. hex (256 octets = taille de N)
    //   - M1 : exactement 64 car. hex (32 octets = SHA-256)
    if a_hex.is_empty() || a_hex.len() > 512 || !a_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Valeur d'authentification invalide."}), 400);
        return;
    }
    if m1_hex.len() != 64 || !m1_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Preuve d'authentification invalide."}), 400);
        return;
    }
    if token.len() != 48 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Session d'authentification invalide."}), 400);
        return;
    }

    // ── Récupère la session éphémère (b, corrélée au token) ───────
    let sess_rows = selectionner(
        pool,
        "srp_sessions",
        &[
            ("token", mysql::Value::from(token.as_str())),
            ("email", mysql::Value::from(email.as_str())),
        ],
        &["b_hex", "created_at"],
        None,
        Some(1),
    );
    let Some(sess) = sess_rows.into_iter().next() else {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurSessionAuthExpiree)}), 200);
        return;
    };
    // Session à usage unique — on la supprime immédiatement, qu'elle
    // réussisse ou échoue, pour empêcher tout replay de step2.
    supprimer_ligne(pool, "srp_sessions", "token", mysql::Value::from(token.as_str()));

    let created_at = sess.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
    if !crate::c::is_recent_local(created_at, SRP_SESSION_MAX_AGE_SECONDS) {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurSessionAuthExpiree)}), 200);
        return;
    }
    let b_hex = sess.get("b_hex").and_then(|v| v.as_str()).unwrap_or("");

    // ── Récupère le compte (peut ne pas exister si fake step1) ────
    let user_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id", "nom", "email", "srp_verifier", "vip", "privilege", "file_key_wrapped_pwd"],
        None,
        Some(1),
    );
    let Some(user_row) = user_rows.into_iter().next() else {
        // Compte inexistant : réponse volontairement identique à un
        // mauvais mot de passe, pour ne pas révéler l'absence du compte.
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurIdentifiants)}), 200);
        return;
    };

    let verifier_hex = user_row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("");
    let (Some(v_big), Some(b_bytes), Some(a_bytes), Some(m1_client)) = (
        bigint_from_hex(verifier_hex),
        hex_decode(b_hex),
        hex_decode(&a_hex),
        hex_decode(&m1_hex),
    ) else {
        respond_json(request, json!({"success":false,"error":"Format d'authentification invalide."}), 400);
        return;
    };

    let grp = group();
    let b = num_bigint::BigUint::from_bytes_be(&b_bytes);
    let a_pub = num_bigint::BigUint::from_bytes_be(&a_bytes);

    // ── Vérification anti-fuite : A ne doit jamais être 0 mod N ───
    if !is_safe_public_value(&a_pub, &grp.n) {
        respond_json(request, json!({"success":false,"error":"Valeur d'authentification invalide."}), 400);
        return;
    }

    let b_pub = compute_b_public(&grp, &v_big, &b);
    let u = compute_u(&a_pub, &b_pub);
    let s_server = compute_s_server(&grp, &a_pub, &v_big, &u, &b);
    let k_bytes = compute_k(&s_server);

    // Le salt réel n'est pas re-stocké ici (déjà envoyé en step1) —
    // on doit néanmoins le relire pour calculer M1 côté serveur.
    let salt_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["srp_salt"],
        None,
        Some(1),
    );
    let salt_hex = salt_rows
        .into_iter()
        .next()
        .and_then(|r| r.get("srp_salt").and_then(|v| v.as_str().map(|s| s.to_string())))
        .unwrap_or_default();
    let Some(salt_bytes) = hex_decode(&salt_hex) else {
        respond_json(request, json!({"success":false,"error":"Erreur interne (salt)."}), 500);
        return;
    };

    let m1_expected = compute_m1(&grp, &email, &salt_bytes, &a_pub, &b_pub, &k_bytes);

    if !constant_time_eq(&m1_client, &m1_expected) {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurIdentifiants)}), 200);
        return;
    }

    // ── Authentification réussie : preuve serveur + ouverture session ──
    let m2 = compute_m2(&a_pub, &m1_expected, &k_bytes);

    let user_id = user_row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let nom = user_row.get("nom").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let email_db = user_row.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let file_key_wrapped_pwd = user_row.get("file_key_wrapped_pwd").and_then(|v| v.as_str()).map(|s| s.to_string());
    let _ = user_id; // conservé pour lisibilité / usage futur (logs, etc.)

    let cookie_value = generate_session_token(32);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    inserer_ou_modifier(
        pool,
        "loginc",
        &[
            ("idcokier", mysql::Value::from(cookie_value.as_str())),
            ("datecra", mysql::Value::from(now.as_str())),
            ("pc", mysql::Value::from(remote_ip)),
            ("navi", mysql::Value::from(user_agent)),
            ("email", mysql::Value::from(email_db.as_str())),
            ("nom", mysql::Value::from(nom.as_str())),
        ],
        &[],
    );

    let expires = chrono::Local::now() + chrono::Duration::days(30);
    let cookie_str = format!(
        "connexion_cookie={}; Path=/; HttpOnly; SameSite=Strict; Expires={}",
        cookie_value,
        expires.format("%a, %d %b %Y %H:%M:%S GMT")
    );

    let body_json = serde_json::to_string(&json!({
        "success":  true,
        "M2":       hex_encode(&m2),
        "redirect": "/login/dashboard",
        "email":    email_db,
        // FIX (récupération sans email) : si le compte n'a pas encore de
        // masterKey enveloppée (compte créé avant cette fonctionnalité),
        // le client doit générer/enregistrer le matériel de récupération
        // maintenant qu'il a le mot de passe en clair — voir handle_request,
        // action "enregistrer_recuperation".
        "needs_recovery_setup": file_key_wrapped_pwd.is_none(),
        "file_key_wrapped_pwd": file_key_wrapped_pwd,
    }))
    .unwrap_or_default();

    let _ = request.respond(
        Response::from_string(body_json)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Set-Cookie", cookie_str.as_str()).unwrap()),
    );
}

// ══════════════════════════════════════════════════════════════════
// Inscription — le client envoie salt + verifier (calculés côté JS),
// jamais le mot de passe, jamais un hash équivalent au mot de passe.
// ══════════════════════════════════════════════════════════════════
fn handle_signup(request: Request, pool: &DbPool, config: &VexConfig, body: &HashMap<String, String>, langue: &str) {
    use crate::i18n::{t, Cle};
    let reg_mode = config.users.registration_mode.as_str();
    let activation_req = config.users.activation_key_required;
    let activation_key = config.users.activation_key.as_str();
    let max_users = config.users.max_users;

    if reg_mode == "closed" {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginInscriptionsFermees)}), 200);
        return;
    }
    if reg_mode == "invitation" && activation_req {
        let key = body.get("activation_key").map(|s| s.as_str()).unwrap_or("");
        if key != activation_key {
            respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurCleActivationInvalide)}), 200);
            return;
        }
    }
    if body.get("scales").is_none() {
        respond_json(
            request,
            json!({"success":false,"error":t(langue, Cle::LoginErreurAccepterPolitique)}),
            200,
        );
        return;
    }
    if compter_lignes(pool, "login", &[]) >= max_users {
        respond_json(
            request,
            json!({"success":false,"error":t(langue, Cle::LoginErreurMaxUtilisateurs).replace("{n}", &max_users.to_string())}),
            200,
        );
        return;
    }

    let nom = html_escape(body.get("nom").cloned().unwrap_or_default().trim());
    let email = html_escape(body.get("email").cloned().unwrap_or_default().trim());
    let salt_hex = body.get("srp_salt").cloned().unwrap_or_default();
    let verifier_hex = body.get("srp_verifier").cloned().unwrap_or_default();
    // FIX (récupération de compte) : matériel de récupération généré côté
    // client à l'inscription (voir static/crypto.js + login.html) — des
    // blobs chiffrés et un hash de preuve, jamais de secret en clair.
    let file_key_wrapped_pwd = body.get("file_key_wrapped_pwd").cloned().unwrap_or_default();
    let file_key_wrapped_recovery = body.get("file_key_wrapped_recovery").cloned().unwrap_or_default();
    let recovery_salt = body.get("recovery_salt").cloned().unwrap_or_default();
    let recovery_proof_hash = body.get("recovery_proof_hash").cloned().unwrap_or_default();

    // Validation de forme : salt = 16 octets hex (32 car.), verifier = 256 octets hex (512 car.)
    if salt_hex.len() != 32 || !salt_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Format de salt invalide."}), 400);
        return;
    }
    if verifier_hex.len() > 512 || verifier_hex.is_empty() || !verifier_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        respond_json(request, json!({"success":false,"error":"Format de verifier invalide."}), 400);
        return;
    }

    let existing = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id"],
        None,
        Some(1),
    );
    let existing_nom = selectionner(
        pool,
        "login",
        &[("nom", mysql::Value::from(nom.as_str()))],
        &["id"],
        None,
        Some(1),
    );

    if !existing.is_empty() || !existing_nom.is_empty() {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurNomOuEmailExistant)}), 200);
        return;
    }

    let result = inserer_ou_modifier(
        pool,
        "login",
        &[
            ("nom", mysql::Value::from(nom.as_str())),
            ("email", mysql::Value::from(email.as_str())),
            ("srp_salt", mysql::Value::from(salt_hex.as_str())),
            ("srp_verifier", mysql::Value::from(verifier_hex.as_str())),
            ("vip", mysql::Value::from(0i64)),
            ("file_key_wrapped_pwd", mysql::Value::from(file_key_wrapped_pwd.as_str())),
            ("file_key_wrapped_recovery", mysql::Value::from(file_key_wrapped_recovery.as_str())),
            ("recovery_salt", mysql::Value::from(recovery_salt.as_str())),
            ("recovery_proof_hash", mysql::Value::from(recovery_proof_hash.as_str())),
        ],
        &[],
    );

    if result > 0 {
        respond_json(
            request,
            json!({"success":true,"message":t(langue, Cle::LoginInscriptionReussie)}),
            200,
        );
    } else {
        respond_json(request, json!({"success":false,"error":t(langue, Cle::LoginErreurInscription)}), 200);
    }
}

// ══════════════════════════════════════════════════════════════════
// Récupération de compte — enveloppe de la masterKey fichiers sous un
// code de récupération à 20 caractères (voir static/crypto.js). Le
// serveur ne voit jamais la masterKey, le mot de passe, ni le code —
// uniquement des blobs chiffrés et un hash de preuve.
// ══════════════════════════════════════════════════════════════════

/// Enregistre (ou renouvelle) le matériel de récupération d'un compte
/// déjà connecté — utilisé pour la migration douce des comptes créés
/// avant cette fonctionnalité, et pour une régénération volontaire du
/// code depuis les paramètres du compte.
fn handle_enregistrer_recuperation(
    request: Request,
    pool: &DbPool,
    body: &HashMap<String, String>,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) {
    let session = crate::c::verifier_session(pool, cookie_val, remote_ip, user_agent);
    if !session.connecte {
        respond_json(request, json!({"success":false,"error":"Non connecté."}), 401);
        return;
    }

    let wrapped_pwd = body.get("file_key_wrapped_pwd").cloned().unwrap_or_default();
    let wrapped_recovery = body.get("file_key_wrapped_recovery").cloned().unwrap_or_default();
    let recovery_salt = body.get("recovery_salt").cloned().unwrap_or_default();
    let proof_hash = body.get("recovery_proof_hash").cloned().unwrap_or_default();

    if wrapped_pwd.is_empty()
        || wrapped_recovery.is_empty()
        || recovery_salt.len() != 64
        || !recovery_salt.chars().all(|c| c.is_ascii_hexdigit())
        || proof_hash.len() != 64
        || !proof_hash.chars().all(|c| c.is_ascii_hexdigit())
    {
        respond_json(request, json!({"success":false,"error":"Champs invalides."}), 400);
        return;
    }

    let result = inserer_ou_modifier(
        pool,
        "login",
        &[
            ("file_key_wrapped_pwd", mysql::Value::from(wrapped_pwd.as_str())),
            ("file_key_wrapped_recovery", mysql::Value::from(wrapped_recovery.as_str())),
            ("recovery_salt", mysql::Value::from(recovery_salt.as_str())),
            ("recovery_proof_hash", mysql::Value::from(proof_hash.as_str())),
        ],
        &[("email", mysql::Value::from(session.user_email.as_str()))],
    );

    respond_json(request, json!({"success": result >= 0}), 200);
}

/// Envoie le code de récupération (en clair, tel que tapé/généré côté
/// client) par email au compte concerné. Le serveur ne "redécouvre"
/// jamais le code seul — le client le fournit ici explicitement, une
/// seule fois, pour ce cas d'usage précis (le code n'est autrement
/// jamais transmis). Deux garde-fous empêchent d'en faire un relais de
/// spam :
///   1. `proof` doit correspondre à `recovery_proof_hash` déjà stocké
///      pour ce compte (même mécanisme que `recuperation_confirmer`) —
///      donc `code` doit être le vrai code de ce compte.
///   2. Le destinataire est TOUJOURS l'email déjà enregistré en base
///      pour ce compte, jamais une adresse fournie par le client.
fn handle_envoyer_code_recuperation(request: Request, pool: &DbPool, body: &HashMap<String, String>) {
    let email = body.get("email").cloned().unwrap_or_default();
    let code = body.get("code").cloned().unwrap_or_default();
    let proof = body.get("proof").cloned().unwrap_or_default();

    let code_valide = code.len() == 24
        && code.as_bytes().chunks(5).enumerate().all(|(i, chunk)| {
            if i < 4 {
                chunk.len() == 5
                    && chunk[4] == b'-'
                    && chunk[..4].iter().all(|c| c.is_ascii_alphanumeric())
            } else {
                chunk.len() == 4 && chunk.iter().all(|c| c.is_ascii_alphanumeric())
            }
        });

    if email.is_empty() || email.len() > 255
        || !code_valide
        || proof.len() != 64 || !proof.chars().all(|c| c.is_ascii_hexdigit())
    {
        respond_json(request, json!({"success":false,"error":"Champs invalides."}), 400);
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["recovery_proof_hash"],
        None,
        Some(1),
    );
    let Some(row) = rows.into_iter().next() else {
        respond_json(request, json!({"success":false,"error":"Code de récupération invalide."}), 200);
        return;
    };
    let stored_proof = row.get("recovery_proof_hash").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if stored_proof.is_empty() || !constant_time_eq(stored_proof.as_bytes(), proof.as_bytes()) {
        respond_json(request, json!({"success":false,"error":"Code de récupération invalide."}), 200);
        return;
    }

    let html = format!(
        "<p>Voici ton code de récupération de compte VEX :</p>\
         <p style=\"font-family:monospace;font-size:18px;letter-spacing:1px\"><strong>{}</strong></p>\
         <p>Ce code permet de reprendre l'accès à ton compte et à tes fichiers déjà chiffrés \
         si tu perds ton mot de passe. Garde-le en lieu sûr — ne le transmets à personne.</p>",
        code
    );
    let envoye = crate::function::vex_send_mail(&email, "Ton code de récupération VEX", &html);

    if envoye {
        respond_json(request, json!({"success": true}), 200);
    } else {
        respond_json(request, json!({"success":false,"error":"Échec de l'envoi de l'email."}), 502);
    }
}

/// Génère un sel/blob factices mais stables pour un email donné — pour
/// que `recuperation_info` sur un compte inexistant (ou sans matériel
/// de récupération encore configuré) se comporte comme un vrai compte
/// (anti-énumération). Jamais un vrai déchiffrement possible.
fn fake_recovery_info(email: &str) -> (String, String) {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    let h = srp::sha256(format!("vex-recovery-salt:{}", email.to_lowercase()).as_bytes());
    let salt_hex = hex_encode(&h);
    let mut fake_blob = h.clone();
    fake_blob.extend_from_slice(&srp::sha256(&h));
    fake_blob.truncate(60); // taille plausible : IV(12) + masterKey(32) + tag(16)
    (salt_hex, B64.encode(&fake_blob))
}

/// Étape 1 de la récupération : le client envoie l'email, le serveur
/// renvoie le sel + l'enveloppe chiffrée de la masterKey sous le code de
/// récupération (données chiffrées, sans risque à exposer — comme le
/// salt/verifier SRP).
fn handle_recuperation_info(request: Request, pool: &DbPool, body: &HashMap<String, String>) {
    let email = body.get("email").cloned().unwrap_or_default();
    if email.is_empty() || email.len() > 255 {
        respond_json(request, json!({"success":false,"error":"Email invalide."}), 400);
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["recovery_salt", "file_key_wrapped_recovery"],
        None,
        Some(1),
    );

    let (recovery_salt, wrapped_recovery) = match rows.into_iter().next() {
        Some(row) => {
            let salt = row.get("recovery_salt").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let wrapped = row.get("file_key_wrapped_recovery").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if salt.is_empty() || wrapped.is_empty() {
                fake_recovery_info(&email)
            } else {
                (salt, wrapped)
            }
        }
        None => fake_recovery_info(&email),
    };

    respond_json(
        request,
        json!({
            "success":       true,
            "recovery_salt": recovery_salt,
            "file_key_wrapped_recovery": wrapped_recovery,
        }),
        200,
    );
}

/// Étape 2 de la récupération : le client a déchiffré la masterKey en
/// local avec son code, et soumet le nouveau matériel (nouveau mot de
/// passe → nouveau salt/verifier SRP + nouvelles enveloppes + nouveau
/// code de récupération tourné) accompagné de la preuve de connaissance
/// du code. `proof` est la SEULE porte d'autorisation côté serveur.
fn handle_recuperation_confirmer(request: Request, pool: &DbPool, body: &HashMap<String, String>) {
    let email = body.get("email").cloned().unwrap_or_default();
    let proof = body.get("proof").cloned().unwrap_or_default();
    let new_srp_salt = body.get("new_srp_salt").cloned().unwrap_or_default();
    let new_srp_verifier = body.get("new_srp_verifier").cloned().unwrap_or_default();
    let new_wrapped_pwd = body.get("new_file_key_wrapped_pwd").cloned().unwrap_or_default();
    let new_recovery_salt = body.get("new_recovery_salt").cloned().unwrap_or_default();
    let new_wrapped_recovery = body.get("new_file_key_wrapped_recovery").cloned().unwrap_or_default();
    let new_proof_hash = body.get("new_recovery_proof_hash").cloned().unwrap_or_default();

    if email.is_empty()
        || proof.len() != 64 || !proof.chars().all(|c| c.is_ascii_hexdigit())
        || new_srp_salt.len() != 32 || !new_srp_salt.chars().all(|c| c.is_ascii_hexdigit())
        || new_srp_verifier.is_empty() || new_srp_verifier.len() > 512 || !new_srp_verifier.chars().all(|c| c.is_ascii_hexdigit())
        || new_wrapped_pwd.is_empty() || new_wrapped_recovery.is_empty()
        || new_recovery_salt.len() != 64 || !new_recovery_salt.chars().all(|c| c.is_ascii_hexdigit())
        || new_proof_hash.len() != 64 || !new_proof_hash.chars().all(|c| c.is_ascii_hexdigit())
    {
        respond_json(request, json!({"success":false,"error":"Champs invalides."}), 400);
        return;
    }

    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["recovery_proof_hash"],
        None,
        Some(1),
    );
    let Some(row) = rows.into_iter().next() else {
        // Compte inexistant : réponse identique à une mauvaise preuve.
        respond_json(request, json!({"success":false,"error":"Code de récupération invalide."}), 200);
        return;
    };
    let stored_proof = row.get("recovery_proof_hash").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if stored_proof.is_empty() || !constant_time_eq(stored_proof.as_bytes(), proof.as_bytes()) {
        respond_json(request, json!({"success":false,"error":"Code de récupération invalide."}), 200);
        return;
    }

    inserer_ou_modifier(
        pool,
        "login",
        &[
            ("srp_salt", mysql::Value::from(new_srp_salt.as_str())),
            ("srp_verifier", mysql::Value::from(new_srp_verifier.as_str())),
            ("file_key_wrapped_pwd", mysql::Value::from(new_wrapped_pwd.as_str())),
            ("recovery_salt", mysql::Value::from(new_recovery_salt.as_str())),
            ("file_key_wrapped_recovery", mysql::Value::from(new_wrapped_recovery.as_str())),
            ("recovery_proof_hash", mysql::Value::from(new_proof_hash.as_str())),
        ],
        &[("email", mysql::Value::from(email.as_str()))],
    );

    // Invalide toutes les sessions existantes du compte (le mot de passe
    // a changé, on force une reconnexion partout).
    let mut conn_ok = true;
    if let Ok(mut conn) = pool.get_conn() {
        use mysql::prelude::Queryable;
        conn_ok = conn.exec_drop("DELETE FROM `loginc` WHERE `email` = ?", (email.as_str(),)).is_ok();
        let _ = conn.exec_drop("DELETE FROM `srp_sessions` WHERE `email` = ?", (email.as_str(),));
        let _ = conn.exec_drop("DELETE FROM `autologin` WHERE `compteid` IN (SELECT `id` FROM `login` WHERE `email` = ?)", (email.as_str(),));
        let _ = conn.exec_drop("DELETE FROM `appareil_jetons` WHERE `user_id` IN (SELECT `id` FROM `login` WHERE `email` = ?)", (email.as_str(),));
    }
    let _ = conn_ok;

    respond_json(request, json!({"success": true}), 200);
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Sert login.html traduit dans la langue detectee (voir
/// function::get_user_language) -- pas de session avant connexion, donc pas
/// de preference de compte : uniquement l'en-tete Accept-Language du
/// navigateur (cookie_lang=None, pas encore de cookie de langue dedie).
fn serve_login_html(request: Request, pool: &DbPool, accept_lang: &str) {
    use crate::i18n::{appliquer_traductions, objet_js, Cle};
    let path = "static/login/login.html";
    match std::fs::read_to_string(path) {
        Ok(html) => {
            let langue = crate::function::get_user_language(pool, None, None, Some(accept_lang));
            // Pas de session avant connexion → thème par défaut "light".
            // (Si tu veux respecter un thème mémorisé pré-connexion, il
            // faudrait un cookie non-HttpOnly dédié — hors scope ici.)
            let html = html.replace("{{THEME}}", "light");

            let html = appliquer_traductions(
                &html,
                &langue,
                &[
                    ("{{T_TITRE_ONGLET}}", Cle::LoginTitreOnglet),
                    ("{{T_SOUS_TITRE}}", Cle::LoginSousTitre),
                    ("{{T_LABEL_EMAIL}}", Cle::LoginLabelEmail),
                    ("{{T_LABEL_MDP}}", Cle::LoginLabelMdp),
                    ("{{T_BOUTON_CONNECTER}}", Cle::LoginBoutonConnecter),
                    ("{{T_NOTE_SECURITE}}", Cle::LoginNoteSecurite),
                    ("{{T_CREER_COMPTE_TITRE}}", Cle::LoginCreerCompteTitre),
                    ("{{T_LABEL_NOM}}", Cle::LoginLabelNom),
                    ("{{T_PLACEHOLDER_NOM}}", Cle::LoginPlaceholderNom),
                    ("{{T_ACCEPTE_LABEL}}", Cle::LoginAccepteLabel),
                    ("{{T_POLITIQUE_CONFIDENTIALITE}}", Cle::LoginPolitiqueConfidentialite),
                    ("{{T_BOUTON_ANNULER}}", Cle::LoginBoutonAnnuler),
                    ("{{T_BOUTON_CREER_COMPTE}}", Cle::LoginBoutonCreerCompte),
                ],
            );

            let i18n_js = objet_js(
                &langue,
                &[
                    ("BOUTON_CONNECTER", Cle::LoginBoutonConnecter),
                    ("BOUTON_CONNECTER_EN_COURS", Cle::LoginBoutonConnecterEnCours),
                    ("BOUTON_CREER_COMPTE", Cle::LoginBoutonCreerCompte),
                    ("BOUTON_CREATION_EN_COURS", Cle::LoginBoutonCreationEnCours),
                    ("BOUTON_NOUVEAU_COMPTE", Cle::LoginBoutonNouveauCompte),
                    ("OU", Cle::LoginOu),
                    ("INSCRIPTIONS_FERMEES", Cle::LoginInscriptionsFermees),
                    ("LABEL_CLE_ACTIVATION", Cle::LoginLabelCleActivation),
                    ("PLACEHOLDER_CLE_ACTIVATION", Cle::LoginPlaceholderCleActivation),
                    ("LABEL_MDP_INDICATION", Cle::LoginLabelMdpIndication),
                    ("ERREUR_CONNEXION", Cle::LoginErreurConnexion),
                    ("ERREUR_IDENTIFIANTS", Cle::LoginErreurIdentifiants),
                    ("ERREUR_SERVEUR_NON_PROUVE", Cle::LoginErreurServeurNonProuve),
                    ("ERREUR_RESEAU", Cle::LoginErreurReseau),
                    ("ERREUR_ACCEPTER_POLITIQUE", Cle::LoginErreurAccepterPolitique),
                    ("ERREUR_MDP_COURT", Cle::LoginErreurMdpCourt),
                    ("INSCRIPTION_REUSSIE", Cle::LoginInscriptionReussie),
                ],
            );
            let html = html.replace("{{I18N_JS}}", &i18n_js);

            let _ = request.respond(Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(
                Response::from_string(format!("Fichier introuvable : {}", path)).with_status_code(500),
            );
        }
    }
}

fn respond_json(request: Request, body: serde_json::Value, status: u16) {
    let _ = request.respond(
        Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
            ),
    );
}

fn redirect(request: Request, location: &str) {
    let _ = request.respond(
        Response::empty(302).with_header(tiny_http::Header::from_bytes("Location", location).unwrap()),
    );
}

/// Token de session (cookie) — utilise le générateur cryptographique
/// getrandom (srp::random_bytes), PAS le PRNG faible XorShift qui servait
/// avant ici. Un cookie de session prévisible = compromission totale.
fn generate_session_token(len_bytes: usize) -> String {
    hex_encode(&srp::random_bytes(len_bytes))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn read_body(request: &mut Request) -> HashMap<String, String> {
    let mut body = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);
    let mut map = HashMap::new();
    for pair in body.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            map.insert(url_decode(k), url_decode(v));
        }
    }
    map
}