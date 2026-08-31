// ══════════════════════════════════════════════════════════════════
// login/account.rs — VEX Page de réglages du compte
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::c::verifier_session;
use crate::config_loader::VexConfig;
use crate::function::{
    build_nav_html, get_privilege_details_json, get_theme_attr, get_user_preferences,
    update_user_preference, NavContext,
};
use crate::utils::{strip_port, url_decode};
use serde_json::{json, Value};
use std::collections::HashMap;
use tiny_http::{Request, Response};

pub fn handle_request(mut request: Request, pool: &DbPool, config: &VexConfig, remote_full: &str) {
    let remote_ip = strip_port(remote_full);
    let method = request.method().to_string();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    // ── GET /login/account → sert le HTML avec navbar + thème injectés ───
    if method == "GET" && (path == "/login/account" || path == "/login/account/") {
        let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
        if !session.connecte {
            redirect(request, "/login");
            return;
        }

        // FIX (cohérence thème) : auparavant cette page codait en dur
        // data-theme="light" dans le HTML et gérait le mode sombre
        // uniquement via une classe JS posée depuis localStorage
        // (body.dark-mode). Résultat : flash visible en clair avant que
        // le JS s'exécute, et thème potentiellement désynchronisé de la
        // préférence réellement stockée en base (table `pref`). On
        // injecte désormais le thème serveur, comme sur toutes les
        // autres pages VEX (viso, admin, etc.), via get_theme_attr().
        let theme = get_theme_attr(pool, session.user_id);

        // Construit la navbar de façon autonome (juste cookie + ip + ua)
        let nav_ctx = NavContext {
            pool,
            user_id: None, // résolu automatiquement depuis le cookie
            page_key: "account",
            cookie_val: &cookie_val,
            remote_ip: &remote_ip,
            user_agent: &user_agent,
            query_id: None,
            apps: vec![],
            admin_apps: vec![],
        };
        let nav_html = build_nav_html(&nav_ctx);
        serve_html_with_nav(request, "static/login/account.html", &nav_html, theme);
        return;
    }

    // ── Auth requise pour toutes les routes /api/account ─────────
    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
    if !session.connecte {
        respond_json(
            request,
            json!({"success":false,"error":"Non connecté"}),
            401,
        );
        return;
    }

    let user_id = session.user_id;
    let user_email = session.user_email.clone();

    match (method.as_str(), path.as_str()) {
        // ── Données du compte ─────────────────────────────────────
        ("GET", "/api/account/data") => {
            let data = build_account_data(pool, config, user_id, &user_email);
            respond_json(request, data, 200);
        }

        // ── Affichage : tuiles, evenements, apps ──────────────────
        ("GET", "/api/account/affichage") => {
            let prefs = crate::function::get_user_preferences(pool, user_id);
            let etat = |m: &std::collections::HashMap<String, serde_json::Value>, k: &str| {
                m.get(k).map(|v| v.as_i64().unwrap_or(1) != 0).unwrap_or(true)
            };

            // Tuiles integrees + tuiles publiees par les extensions
            let mut tuiles = vec![
                json!({"id":"admin",    "label":"Administration"}),
                json!({"id":"fichiers", "label":"Fichiers"}),
                json!({"id":"vexmail",  "label":"VexMail"}),
                json!({"id":"sitec",    "label":"Sitec"}),
                json!({"id":"editeur",  "label":"Éditeur de fichiers"}),
                json!({"id":"videos",   "label":"Vidéos"}),
            ];
            let mut apps = vec![
                json!({"id":"login_dashboard","label":"Accueil","url":"/login/dashboard"}),
                json!({"id":"mess",           "label":"Mail","url":"/mess/"}),
                json!({"id":"fchier",         "label":"Fichiers","url":"/fchier/"}),
                json!({"id":"viso",           "label":"Vidéos","url":"/viso/"}),
                json!({"id":"sitec",          "label":"Sitec","url":"/sitec/"}),
                json!({"id":"admin",          "label":"Administration","url":"/admin"}),
            ];
            for (id, e) in crate::function::extensions_actives("config.json") {
                if let Some(t) = e.get("dashboard_tile") {
                    tuiles.push(json!({
                        "id": format!("ext_{}", id),
                        "label": t.get("titre").and_then(|v| v.as_str()).unwrap_or(id.as_str()),
                        "extension": true,
                    }));
                }
                if let Some(a) = e.get("nav_app") {
                    let url = a.get("url").and_then(|v| v.as_str())
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| format!("/ext/{}", id));
                    apps.push(json!({
                        "id": crate::function::cle_app(&url),
                        "label": a.get("label").and_then(|v| v.as_str()).unwrap_or(id.as_str()),
                        "url": url,
                        "extension": true,
                    }));
                }
            }

            let evenements = vec![
                json!({"id":"stats",      "label":"Chiffres cles du service"}),
                json!({"id":"admins",     "label":"Comptes administrateurs"}),
                json!({"id":"etat",       "label":"Etat du serveur (maintenance, debug)"}),
                json!({"id":"extensions", "label":"Infos publiees par les extensions"}),
                json!({"id":"connexions", "label":"Connexions recentes"}),
            ];

            let marque = |liste: &Vec<serde_json::Value>,
                          m: &std::collections::HashMap<String, serde_json::Value>| {
                liste.iter().map(|x| {
                    let id = x.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let mut o = x.clone();
                    o["actif"] = json!(etat(m, id));
                    o
                }).collect::<Vec<_>>()
            };

            respond_json(request, json!({"success":true,"data":{
                "tuiles":      marque(&tuiles, &prefs.dashboard_tiles),
                "evenements":  marque(&evenements, &prefs.dashboard_events),
                "apps":        marque(&apps, &prefs.nav_apps),
            }}), 200);
        }

        ("POST", "/api/account/affichage") => {
            let body = read_body(&mut request);
            let mut ok = true;
            for champ in ["dashboard_tiles", "dashboard_events", "nav_apps"] {
                if let Some(v) = body.get(champ) {
                    // On valide que c'est bien un objet JSON avant d'ecrire
                    match serde_json::from_str::<serde_json::Value>(v) {
                        Ok(j) if j.is_object() => {
                            ok &= update_user_preference(pool, user_id, champ, v);
                        }
                        _ => {
                            return respond_json(
                                request,
                                json!({"success":false,"error":format!("{} invalide", champ)}),
                                200,
                            )
                        }
                    }
                }
            }
            respond_json(
                request,
                json!({"success":ok,"message":"Affichage enregistré."}),
                200,
            );
        }

        // ── Changer le thème ──────────────────────────────────────
        ("POST", "/api/account/theme") => {
            let body = read_body(&mut request);
            let theme = body
                .get("theme")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let ok = update_user_preference(pool, user_id, "teme", &theme.to_string());
            if ok {
                respond_json(request, json!({"success":true,"theme":theme}), 200);
            } else {
                respond_json(
                    request,
                    json!({"success":false,"error":"Erreur mise à jour thème"}),
                    200,
                );
            }
        }

        // ── Changer le mot de passe ───────────────────────────────
        ("POST", "/api/account/password") => {
            let body = read_body(&mut request);
            let old_mdp = body.get("enmotdepass").cloned().unwrap_or_default();
            let new_mdp = body.get("modifier_motdepasse").cloned().unwrap_or_default();
            let pass_min = config.security.password_min_length as usize;

            if old_mdp.is_empty() || new_mdp.is_empty() {
                respond_json(
                    request,
                    json!({"success":false,"error":"Champs obligatoires manquants"}),
                    200,
                );
                return;
            }
            if new_mdp.len() < pass_min {
                respond_json(
                    request,
                    json!({"success":false,
                    "error":format!("Mot de passe trop court (min. {} car.)", pass_min)}),
                    200,
                );
                return;
            }

            let rows = selectionner(
                pool,
                "login",
                &[("email", mysql::Value::from(user_email.as_str()))],
                &["motdepass"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                respond_json(
                    request,
                    json!({"success":false,"error":"Compte introuvable"}),
                    200,
                );
                return;
            }
            let current_hash = rows[0]
                .get("motdepass")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !verify_password(&old_mdp, &current_hash) {
                respond_json(
                    request,
                    json!({"success":false,"error":"Mot de passe actuel incorrect"}),
                    200,
                );
                return;
            }

            let new_hash = hash_password(&new_mdp);
            inserer_ou_modifier(
                pool,
                "login",
                &[("motdepass", mysql::Value::from(new_hash.as_str()))],
                &[("email", mysql::Value::from(user_email.as_str()))],
            );

            respond_json(
                request,
                json!({"success":true,"message":"Mot de passe mis à jour"}),
                200,
            );
        }

        // ── Créer un token autologin ──────────────────────────────
        ("POST", "/api/account/autologin/create") => {
            let autologin_cfg = &config.autologin;
            let enabled = autologin_cfg.enabled;
            let token_length = autologin_cfg.token_length as usize;
            let max_tokens = autologin_cfg.max_tokens_per_user;
            let privilege_min = autologin_cfg.privilege_min as i64;
            let plans_ok = &autologin_cfg.plans_autorises;

            let user_plan = if session.user_vip == 1 { "vip" } else { "free" };
            let plan_ok = plans_ok.iter().any(|p| p == "*" || p == user_plan);
            let allowed = enabled && plan_ok && session.user_privilege <= privilege_min;

            if !allowed {
                respond_json(
                    request,
                    json!({"success":false,
                    "error":"Autologin non disponible pour ce compte"}),
                    200,
                );
                return;
            }

            let existing = selectionner(
                pool,
                "autologin",
                &[("compteid", mysql::Value::from(user_id))],
                &["nombre"],
                None,
                None,
            );

            if existing.len() >= max_tokens as usize {
                respond_json(
                    request,
                    json!({"success":false,
                    "error":"Nombre maximum de liens autologin atteint"}),
                    200,
                );
                return;
            }

            let server_secret = autologin_cfg.server_secret.trim();
            if server_secret.is_empty() || server_secret == "vex_changeme_secret" {
                respond_json(
                    request,
                    json!({"success":false,
                    "error":"server_secret manquant ou invalide dans config.json"}),
                    200,
                );
                return;
            }

            let token = match generate_token(token_length) {
                Ok(token) => token,
                Err(_) => {
                    respond_json(
                        request,
                        json!({"success":false,"error":"Erreur création token"}),
                        200,
                    );
                    return;
                }
            };
            // Compat schémas : d'abord colonne `nombre` (ancienne), puis `nombre_hash`
            let mut result = inserer_ou_modifier(
                pool,
                "autologin",
                &[
                    ("compteid", mysql::Value::from(user_id)),
                    ("nombre", mysql::Value::from(token.as_str())),
                ],
                &[],
            );
            if result < 0 {
                let token_hash = hash_autologin_token(&token, server_secret);
                result = inserer_ou_modifier(
                    pool,
                    "autologin",
                    &[
                        ("compteid", mysql::Value::from(user_id)),
                        ("nombre_hash", mysql::Value::from(token_hash.as_str())),
                    ],
                    &[],
                );
            }

            if result >= 0 {
                let url = format!("/autologin/connecter?uid={}&token={}", user_id, token);
                respond_json(request, json!({"success":true,"url":url}), 200);
            } else {
                respond_json(
                    request,
                    json!({"success":false,"error":"Erreur création token"}),
                    200,
                );
            }
        }

        // ── Supprimer un token autologin ──────────────────────────
        ("POST", "/api/account/autologin/delete") => {
            let rows = selectionner(
                pool,
                "autologin",
                &[("compteid", mysql::Value::from(user_id))],
                &["compteid"],
                None,
                Some(1),
            );
            if rows.is_empty() {
                respond_json(
                    request,
                    json!({"success":false,"error":"Aucun lien autologin actif"}),
                    200,
                );
                return;
            }
            supprimer_ligne(pool, "autologin", "compteid", mysql::Value::from(user_id));
            respond_json(
                request,
                json!({"success":true,"message":"Lien autologin supprimé"}),
                200,
            );
        }

        _ => respond_json(
            request,
            json!({"success":false,"error":"Route inconnue"}),
            404,
        ),
    }
}

// ══════════════════════════════════════════════════════════════════
// Construction des données du compte
// ══════════════════════════════════════════════════════════════════
fn build_account_data(pool: &DbPool, config: &VexConfig, user_id: i64, user_email: &str) -> Value {
    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(user_email))],
        &["id", "nom", "email", "privilege", "vip"],
        None,
        Some(1),
    );

    if rows.is_empty() {
        return json!({"success":false,"error":"Utilisateur introuvable"});
    }

    let row = &rows[0];
    let nom = row
        .get("nom")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let email = row
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let privilege = row.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);
    let vip = row.get("vip").and_then(|v| v.as_i64()).unwrap_or(0);
    let pd = get_privilege_details_json(privilege);

    let prefs = get_user_preferences(pool, user_id);
    let theme = prefs.teme;

    let autologin_cfg = &config.autologin;
    let enabled = autologin_cfg.enabled;
    let token_length = autologin_cfg.token_length;
    let max_tokens = autologin_cfg.max_tokens_per_user;
    let privilege_min = autologin_cfg.privilege_min as i64;
    let plans_ok = &autologin_cfg.plans_autorises;

    let user_plan = if vip == 1 { "vip" } else { "free" };
    let plan_ok = plans_ok.iter().any(|p| p == "*" || p == user_plan);
    let autologin_allowed = enabled && plan_ok && privilege <= privilege_min;

    let tokens_rows = selectionner(
        pool,
        "autologin",
        &[("compteid", mysql::Value::from(user_id))],
        &["compteid"],
        None,
        None,
    );
    let has_token = !tokens_rows.is_empty();

    json!({
        "success": true,
        "data": {
            "user": {
                "id":        user_id,
                "nom":       nom,
                "email":     email,
                "privilege": privilege,
                "vip":       vip,
                "privilege_details": pd,
            },
            "theme": theme,
            "autologin": {
                "allowed":      autologin_allowed,
                "enabled":      enabled,
                "token_length": token_length,
                "max_tokens":   max_tokens,
                "has_token":    has_token,
            }
        }
    })
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Sert un fichier HTML en remplaçant __NAV_HTML__ par la navbar et
/// {{THEME}} par le thème résolu côté serveur ("light" | "dark").
/// FIX : theme désormais passé en paramètre au lieu d'être codé en dur
/// dans le fichier statique — cohérent avec build_nav_html/viso/admin.
fn serve_html_with_nav(request: Request, path: &str, nav_html: &str, theme: &str) {
    match std::fs::read_to_string(path) {
        Ok(html) => {
            let html = html
                .replace("__NAV_HTML__", nav_html)
                .replace("{{THEME}}", theme);
            let _ = request.respond(Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(
                Response::from_string(format!("Fichier introuvable : {}", path))
                    .with_status_code(500),
            );
        }
    }
}

fn respond_json(request: Request, body: Value, status: u16) {
    let _ = request.respond(
        Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
    );
}

fn hash_password(password: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(password.as_bytes());
    format!("{:x}", h.finalize())
}

fn verify_password(password: &str, hash: &str) -> bool {
    hash_password(password) == hash
}

fn generate_token(len: usize) -> Result<String, getrandom::Error> {
    let charset = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let max = charset.len() as u8;
    let limit = (255u8 / max) * max;
    let mut bytes = vec![0u8; len * 2];
    getrandom::getrandom(&mut bytes)?;

    let mut token = String::with_capacity(len);
    for byte in bytes {
        if byte < limit {
            token.push(charset[(byte % max) as usize] as char);
            if token.len() == len {
                return Ok(token);
            }
        }
    }

    while token.len() < len {
        let mut extra = [0u8; 32];
        getrandom::getrandom(&mut extra)?;
        for byte in extra {
            if byte < limit {
                token.push(charset[(byte % max) as usize] as char);
                if token.len() == len {
                    break;
                }
            }
        }
    }

    Ok(token)
}

fn hash_autologin_token(token: &str, server_secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(server_secret.as_bytes());
    h.update(b":");
    h.update(token.as_bytes());
    format!("{:x}", h.finalize())
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

fn redirect(request: Request, location: &str) {
    let _ = request.respond(
        Response::empty(302)
            .with_header(tiny_http::Header::from_bytes("Location", location).unwrap()),
    );
}