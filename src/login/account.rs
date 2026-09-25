// ══════════════════════════════════════════════════════════════════
// login/account.rs — VEX Page de réglages du compte
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use crate::c::verifier_session;
use crate::config_loader::VexConfig;
use crate::function::{
    build_nav_html, get_privilege_details_json, get_supported_languages, get_theme_attr,
    get_user_language, get_user_preferences, set_user_language, update_user_preference,
    NavContext,
};
use crate::i18n::{self, Cle};
use crate::utils::{strip_port, url_decode};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::HashMap;
use tiny_http::{Request, Response};

type HmacSha256 = Hmac<Sha256>;

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
        let langue = get_user_language(pool, Some(session.user_id), None, None);

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
        serve_html_with_nav(request, "static/login/account.html", &nav_html, theme, &langue);
        return;
    }

    // ── Auth requise pour toutes les routes /api/account ─────────
    let session = verifier_session(pool, &cookie_val, &remote_ip, &user_agent);
    if !session.connecte {
        // FIX (i18n) : la detection cote JS se basait sur un sous-texte
        // FRANCAIS de "error" ("non connect") pour rediriger vers /login --
        // casse pour toute langue non-fr des que ce message est traduit.
        // On ajoute un "code" stable independant de la langue et le JS
        // s'appuie desormais dessus.
        let langue_anon = get_user_language(pool, None, None, None);
        respond_json(
            request,
            json!({"success":false,"code":"non_connecte","error":i18n::t(&langue_anon, Cle::AccErreurNonConnecte)}),
            401,
        );
        return;
    }

    let user_id = session.user_id;
    let user_email = session.user_email.clone();
    let langue = get_user_language(pool, Some(user_id), None, None);

    match (method.as_str(), path.as_str()) {
        // ── Données du compte ─────────────────────────────────────
        ("GET", "/api/account/data") => {
            let data = build_account_data(pool, config, user_id, &user_email, &langue);
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
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurTheme)}),
                    200,
                );
            }
        }

        // ── Changer la langue de l'interface ──────────────────────
        ("POST", "/api/account/language") => {
            let body = read_body(&mut request);
            let lang = body.get("langue").cloned().unwrap_or_default();
            let ok = set_user_language(pool, user_id, &lang);
            if ok {
                respond_json(request, json!({"success":true,"langue":lang}), 200);
            } else {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurLangue)}),
                    200,
                );
            }
        }

        // ── Définir / modifier le pseudo (connexion par pseudo) ───
        ("POST", "/api/account/pseudo") => {
            let body = read_body(&mut request);
            let pseudo = body.get("pseudo").cloned().unwrap_or_default().trim().to_string();

            if pseudo.is_empty() {
                // Pseudo vidé : le compte redevient connectable uniquement par email.
                inserer_ou_modifier(
                    pool,
                    "login",
                    &[("pseudo", mysql::Value::NULL)],
                    &[("email", mysql::Value::from(user_email.as_str()))],
                );
                respond_json(request, json!({"success":true,"pseudo":""}), 200);
                return;
            }
            if pseudo.len() > 64 || pseudo.contains('@') || pseudo.chars().any(|c| c.is_whitespace()) {
                respond_json(request, json!({"success":false,"error":"Pseudo invalide."}), 200);
                return;
            }

            let existing = selectionner(
                pool,
                "login",
                &[("pseudo", mysql::Value::from(pseudo.as_str()))],
                &["email"],
                None,
                Some(1),
            );
            let deja_pris = existing
                .into_iter()
                .next()
                .map(|r| r.get("email").and_then(|v| v.as_str()).unwrap_or("") != user_email.as_str())
                .unwrap_or(false);
            if deja_pris {
                respond_json(request, json!({"success":false,"error":"Ce pseudo est déjà utilisé."}), 200);
                return;
            }

            inserer_ou_modifier(
                pool,
                "login",
                &[("pseudo", mysql::Value::from(pseudo.as_str()))],
                &[("email", mysql::Value::from(user_email.as_str()))],
            );
            respond_json(request, json!({"success":true,"pseudo":pseudo}), 200);
        }

        // ── Lien de paiement signe (evite de retaper son email sur le
        // service externe paiement-pi, qui n'a pas acces a notre session)
        // ───────────────────────────────────────────────────────────
        // FIX (demande utilisateur) : la premiere version de paiement-pi
        // demandait l'email sur SA propre page pour savoir a quel compte
        // rattacher le paiement, puisque ce service tourne separement de
        // VEX (voir CLAUDE.md) et n'a jamais accès à notre cookie de
        // session. On genere ici un lien signe (uid + expiration + HMAC)
        // que paiement-pi peut verifier lui-meme SANS jamais avoir besoin
        // de lire notre base ou notre session -- juste un secret partage,
        // configure independamment des deux cotes (jamais de session/
        // cookie partages, toujours deux services separes).
        ("POST", "/api/account/lien_paiement") => {
            let body = read_body(&mut request);
            let plan_id = body.get("plan_id").cloned().unwrap_or_default();
            let periode = body.get("periode").cloned().unwrap_or_else(|| "mois".to_string());
            if plan_id.is_empty() {
                respond_json(request, json!({"success":false,"error":"plan_id manquant."}), 400);
                return;
            }
            let url_base = config.plans.extra.get("external_payment_url").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let secret = config.plans.extra.get("paiement_secret").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if url_base.is_empty() || secret.is_empty() {
                respond_json(request, json!({"success":false,"error":"Paiement non configuré côté serveur."}), 200);
                return;
            }
            let exp = maintenant_epoch() + 600; // lien valable 10 minutes
            let message = format!("{}.{}", user_id, exp);
            let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
                Ok(m) => m,
                Err(_) => {
                    respond_json(request, json!({"success":false,"error":"Secret de paiement invalide."}), 500);
                    return;
                }
            };
            mac.update(message.as_bytes());
            let sig = hex::encode(mac.finalize().into_bytes());
            let sep = if url_base.contains('?') { "&" } else { "?" };
            let url = format!(
                "{url_base}{sep}uid={user_id}&exp={exp}&sig={sig}&plan={plan}&periode={periode}",
                plan = url_encode_simple(&plan_id),
                periode = url_encode_simple(&periode),
            );
            respond_json(request, json!({"success":true,"url":url}), 200);
        }

        // ── Changer le mot de passe (SRP-6a) : etape 1 ────────────
        // Le mot de passe (ancien ou nouveau) ne quitte jamais le
        // navigateur -- meme mecanisme de preuve que la connexion
        // (voir login.rs). L'email vient de la session serveur, jamais
        // du client, pour ne jamais pouvoir demarrer une preuve sur un
        // autre compte que le sien.
        ("POST", "/api/account/password/step1") => {
            let rows = selectionner(
                pool,
                "login",
                &[("email", mysql::Value::from(user_email.as_str()))],
                &["srp_salt", "srp_verifier", "file_key_wrapped_pwd"],
                None,
                Some(1),
            );
            let Some(row) = rows.into_iter().next() else {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurCompteIntrouvable)}),
                    200,
                );
                return;
            };
            let salt_hex = row.get("srp_salt").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let verifier_hex = row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let wrapped_pwd = row.get("file_key_wrapped_pwd").and_then(|v| v.as_str()).map(|s| s.to_string());

            let Some(v_big) = crate::srp::bigint_from_hex(&verifier_hex) else {
                respond_json(request, json!({"success":false,"error":"Erreur interne (verifier)."}), 500);
                return;
            };
            let grp = crate::srp::group();
            let b = crate::srp::generate_b();
            let b_pub = crate::srp::compute_b_public(&grp, &v_big, &b);
            let token = crate::srp::hex_encode(&crate::srp::random_bytes(24));
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

            inserer_ou_modifier(
                pool,
                "srp_sessions",
                &[
                    ("token", mysql::Value::from(token.as_str())),
                    ("email", mysql::Value::from(user_email.as_str())),
                    ("b_hex", mysql::Value::from(crate::srp::hex_encode(&b.to_bytes_be()).as_str())),
                    ("created_at", mysql::Value::from(now.as_str())),
                ],
                &[],
            );

            respond_json(
                request,
                json!({
                    "success": true,
                    "salt": salt_hex,
                    "B": crate::srp::hex_encode(&b_pub.to_bytes_be()),
                    "token": token,
                    "file_key_wrapped_pwd": wrapped_pwd,
                }),
                200,
            );
        }

        // ── Changer le mot de passe (SRP-6a) : etape 2 ────────────
        // Le client prouve qu'il connait l'ANCIEN mot de passe (M1, comme
        // au login) puis fournit le nouveau salt/verifier + la masterKey
        // ExoDrive re-enveloppee sous le nouveau mot de passe (calcules
        // localement -- voir account.html). Rien de tout ca ne revele le
        // mot de passe en clair, ni l'ancien ni le nouveau.
        ("POST", "/api/account/password/step2") => {
            let body = read_body(&mut request);
            let token = body.get("token").cloned().unwrap_or_default();
            let a_hex = body.get("A").cloned().unwrap_or_default();
            let m1_hex = body.get("M1").cloned().unwrap_or_default();
            let new_srp_salt = body.get("new_srp_salt").cloned().unwrap_or_default();
            let new_srp_verifier = body.get("new_srp_verifier").cloned().unwrap_or_default();
            let new_wrapped_pwd = body.get("new_file_key_wrapped_pwd").cloned().unwrap_or_default();

            if a_hex.is_empty() || a_hex.len() > 512 || !a_hex.chars().all(|c| c.is_ascii_hexdigit())
                || m1_hex.len() != 64 || !m1_hex.chars().all(|c| c.is_ascii_hexdigit())
                || token.len() != 48 || !token.chars().all(|c| c.is_ascii_hexdigit())
                || new_srp_salt.len() != 32 || !new_srp_salt.chars().all(|c| c.is_ascii_hexdigit())
                || new_srp_verifier.is_empty() || new_srp_verifier.len() > 512 || !new_srp_verifier.chars().all(|c| c.is_ascii_hexdigit())
                || new_wrapped_pwd.is_empty()
            {
                respond_json(request, json!({"success":false,"error":"Champs invalides."}), 400);
                return;
            }

            let sess_rows = selectionner(
                pool,
                "srp_sessions",
                &[
                    ("token", mysql::Value::from(token.as_str())),
                    ("email", mysql::Value::from(user_email.as_str())),
                ],
                &["b_hex", "created_at"],
                None,
                Some(1),
            );
            let Some(sess) = sess_rows.into_iter().next() else {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurMdpActuelIncorrect)}),
                    200,
                );
                return;
            };
            // Session SRP a usage unique.
            supprimer_ligne(pool, "srp_sessions", "token", mysql::Value::from(token.as_str()));

            let created_at = sess.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            if !crate::c::is_recent_local(created_at, 300) {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurMdpActuelIncorrect)}),
                    200,
                );
                return;
            }
            let b_hex = sess.get("b_hex").and_then(|v| v.as_str()).unwrap_or("");

            let rows = selectionner(
                pool,
                "login",
                &[("email", mysql::Value::from(user_email.as_str()))],
                &["srp_salt", "srp_verifier"],
                None,
                Some(1),
            );
            let Some(row) = rows.into_iter().next() else {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurCompteIntrouvable)}),
                    200,
                );
                return;
            };
            let verifier_hex = row.get("srp_verifier").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let salt_hex = row.get("srp_salt").and_then(|v| v.as_str()).unwrap_or("").to_string();

            let (Some(v_big), Some(b_bytes), Some(a_bytes), Some(m1_client), Some(salt_bytes)) = (
                crate::srp::bigint_from_hex(&verifier_hex),
                crate::srp::hex_decode(b_hex),
                crate::srp::hex_decode(&a_hex),
                crate::srp::hex_decode(&m1_hex),
                crate::srp::hex_decode(&salt_hex),
            ) else {
                respond_json(request, json!({"success":false,"error":"Format invalide."}), 400);
                return;
            };

            let grp = crate::srp::group();
            let b = num_bigint::BigUint::from_bytes_be(&b_bytes);
            let a_pub = num_bigint::BigUint::from_bytes_be(&a_bytes);
            if !crate::srp::is_safe_public_value(&a_pub, &grp.n) {
                respond_json(request, json!({"success":false,"error":"Valeur invalide."}), 400);
                return;
            }
            let b_pub = crate::srp::compute_b_public(&grp, &v_big, &b);
            let u = crate::srp::compute_u(&a_pub, &b_pub);
            let s_server = crate::srp::compute_s_server(&grp, &a_pub, &v_big, &u, &b);
            let k_bytes = crate::srp::compute_k(&s_server);
            let m1_expected = crate::srp::compute_m1(&grp, &user_email, &salt_bytes, &a_pub, &b_pub, &k_bytes);

            if !crate::srp::constant_time_eq(&m1_client, &m1_expected) {
                respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurMdpActuelIncorrect)}),
                    200,
                );
                return;
            }

            inserer_ou_modifier(
                pool,
                "login",
                &[
                    ("srp_salt", mysql::Value::from(new_srp_salt.as_str())),
                    ("srp_verifier", mysql::Value::from(new_srp_verifier.as_str())),
                    ("file_key_wrapped_pwd", mysql::Value::from(new_wrapped_pwd.as_str())),
                ],
                &[("email", mysql::Value::from(user_email.as_str()))],
            );

            // Invalide les autres sessions actives (le mot de passe a
            // change) -- garde la session courante pour ne pas deconnecter
            // l'utilisateur qui vient de faire le changement.
            if let Ok(mut conn) = pool.get_conn() {
                use mysql::prelude::Queryable;
                let _ = conn.exec_drop(
                    "DELETE FROM `loginc` WHERE `email` = ? AND `idcokier` != ?",
                    (user_email.as_str(), cookie_val.as_str()),
                );
                let _ = conn.exec_drop("DELETE FROM `srp_sessions` WHERE `email` = ?", (user_email.as_str(),));
            }

            respond_json(
                request,
                json!({"success":true,"message":i18n::t(&langue, Cle::AccMdpMisAJour)}),
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
                    "error":i18n::t(&langue, Cle::AccErreurAutologinNonDisponible)}),
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
                    "error":i18n::t(&langue, Cle::AccErreurMaxLiens)}),
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
                        json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurCreationToken)}),
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
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurCreationToken)}),
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
                    json!({"success":false,"error":i18n::t(&langue, Cle::AccErreurAucunLienActif)}),
                    200,
                );
                return;
            }
            supprimer_ligne(pool, "autologin", "compteid", mysql::Value::from(user_id));
            respond_json(
                request,
                json!({"success":true,"message":i18n::t(&langue, Cle::AccLienSupprime)}),
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
fn build_account_data(pool: &DbPool, config: &VexConfig, user_id: i64, user_email: &str, langue: &str) -> Value {
    let rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(user_email))],
        &["id", "nom", "email", "privilege", "vip", "pseudo"],
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
    let pseudo = row
        .get("pseudo")
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
        &["compteid", "utilisations"],
        None,
        None,
    );
    let has_token = !tokens_rows.is_empty();
    let utilisations = tokens_rows
        .get(0)
        .and_then(|r| r.get("utilisations"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    json!({
        "success": true,
        "data": {
            "user": {
                "id":        user_id,
                "nom":       nom,
                "email":     email,
                "pseudo":    pseudo,
                "privilege": privilege,
                "vip":       vip,
                "privilege_details": pd,
            },
            "theme": theme,
            "langue": langue,
            "supported_languages": get_supported_languages(),
            "autologin": {
                "allowed":      autologin_allowed,
                "enabled":      enabled,
                "token_length": token_length,
                "max_tokens":   max_tokens,
                "has_token":    has_token,
                "utilisations": utilisations,
            }
        }
    })
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Sert un fichier HTML en remplaçant __NAV_HTML__ par la navbar,
/// {{THEME}} par le thème résolu côté serveur ("light" | "dark"), et en
/// appliquant les traductions (placeholders {{T_XXX}} + {{I18N_JS}} pour
/// le JS embarqué) — même mécanisme que login.rs/dashboard.rs.
fn serve_html_with_nav(request: Request, path: &str, nav_html: &str, theme: &str, langue: &str) {
    match std::fs::read_to_string(path) {
        Ok(html) => {
            let html = html
                .replace("__NAV_HTML__", nav_html)
                .replace("{{THEME}}", theme);
            let html = i18n::appliquer_traductions(&html, langue, &[
                ("{{T_TITRE_ONGLET}}", Cle::AccTitreOnglet),
                ("{{T_SIDEBAR_PROFIL}}", Cle::AccSidebarProfil),
                ("{{T_SIDEBAR_PREFERENCES}}", Cle::AccSidebarPreferences),
                ("{{T_SIDEBAR_SECURITE}}", Cle::AccSidebarSecurite),
                ("{{T_SIDEBAR_AUTOLOGIN}}", Cle::AccSidebarAutologin),
                ("{{T_SIDEBAR_SYNC}}", Cle::AccSidebarSync),
                ("{{T_SIDEBAR_NOTIFICATIONS}}", Cle::AccSidebarNotifications),
                ("{{T_SIDEBAR_CONFIDENTIALITE}}", Cle::AccSidebarConfidentialite),
                ("{{T_MON_PROFIL}}", Cle::AccMonProfil),
                ("{{T_PRIVILEGE_DEFAUT}}", Cle::AccPrivilegeDefaut),
                ("{{T_STATS_COMPTE}}", Cle::AccStatsCompte),
                ("{{T_NIVEAU_PRIVILEGE}}", Cle::AccNiveauPrivilege),
                ("{{T_STATUT_ABONNEMENT}}", Cle::AccStatutAbonnement),
                ("{{T_INFOS_DETAILLEES}}", Cle::AccInfosDetaillees),
                ("{{T_NOM_UTILISATEUR}}", Cle::AccNomUtilisateur),
                ("{{T_ADRESSE_EMAIL}}", Cle::AccAdresseEmail),
                ("{{T_IDENTIFIANT_UNIQUE}}", Cle::AccIdentifiantUnique),
                ("{{T_COPIER}}", Cle::AccCopier),
                ("{{T_AFFICHAGE_PERSO}}", Cle::AccAffichagePerso),
                ("{{T_AFFICHAGE_PERSO_DESC}}", Cle::AccAffichagePersoDesc),
                ("{{T_CHARGEMENT}}", Cle::AccChargement),
                ("{{T_ENREGISTRER_AFFICHAGE}}", Cle::AccEnregistrerAffichage),
                ("{{T_PREFERENCES_AFFICHAGE}}", Cle::AccPreferencesAffichage),
                ("{{T_PREFERENCES_AFFICHAGE_DESC}}", Cle::AccPreferencesAffichageDesc),
                ("{{T_THEME_CLAIR}}", Cle::AccThemeClair),
                ("{{T_THEME_CLAIR_DESC}}", Cle::AccThemeClairDesc),
                ("{{T_THEME_SOMBRE}}", Cle::AccThemeSombre),
                ("{{T_THEME_SOMBRE_DESC}}", Cle::AccThemeSombreDesc),
                ("{{T_ENREGISTRER_PREFERENCES}}", Cle::AccEnregistrerPreferences),
                ("{{T_LANGUE_TITRE}}", Cle::AccLangueTitre),
                ("{{T_LANGUE_DESC}}", Cle::AccLangueDesc),
                ("{{T_PARAMETRES_EXTENSIONS}}", Cle::AccParametresExtensions),
                ("{{T_PARAMETRES_EXTENSIONS_DESC}}", Cle::AccParametresExtensionsDesc),
                ("{{T_MDP_ACTUEL}}", Cle::AccMdpActuel),
                ("{{T_NOUVEAU_MDP}}", Cle::AccNouveauMdp),
                ("{{T_MODIFIER_MDP}}", Cle::AccModifierMdp),
                ("{{T_AUTOLOGIN_DESC}}", Cle::AccAutologinDesc),
                ("{{T_GENERER_LIEN}}", Cle::AccGenererLien),
                ("{{T_SUPPRIMER_LIEN}}", Cle::AccSupprimerLien),
                ("{{T_SYNC_TITRE}}", Cle::AccSyncTitre),
                ("{{T_SYNC_DESC}}", Cle::AccSyncDesc),
                ("{{T_TELECHARGER_SYNC}}", Cle::AccTelechargerSync),
                ("{{T_APPAREILS_AUTORISES}}", Cle::AccAppareilsAutorises),
                ("{{T_NOTIFICATIONS_DESC}}", Cle::AccNotificationsDesc),
            ]);
            let i18n_js = i18n::objet_js(langue, &[
                ("REPONSE_INVALIDE", Cle::AccReponseInvalide),
                ("THEME_ENREGISTRE", Cle::AccThemeEnregistre),
                ("LANGUE_ENREGISTREE", Cle::AccLangueEnregistree),
                ("ERREUR_LANGUE", Cle::AccErreurLangue),
                ("ERREUR_THEME", Cle::AccErreurTheme),
                ("PRIVILEGE_LABEL", Cle::AccPrivilegeLabel),
                ("PREMIUM", Cle::AccPremium),
                ("GRATUIT", Cle::AccGratuit),
                ("FREE", Cle::AccFree),
                ("AL_NON_DISPONIBLE", Cle::AccAlNonDisponible),
                ("AL_LIEN_ACTIF_TITRE", Cle::AccAlLienActifTitre),
                ("AL_LIEN_EXISTE_DEJA", Cle::AccAlLienExisteDeja),
                ("AL_UTILISE", Cle::AccAlUtilise),
                ("AL_AUCUN_LIEN", Cle::AccAlAucunLien),
                ("AL_PAS_ENCORE_DE_LIEN", Cle::AccAlPasEncoreDeLien),
                ("AL_CLIQUEZ_POUR_GENERER", Cle::AccAlCliquezPourGenerer),
                ("SYNC_IMPOSSIBLE_CHARGER", Cle::AccSyncImpossibleCharger),
                ("SYNC_AUCUN_APPAREIL", Cle::AccSyncAucunAppareil),
                ("APPAREIL_INCONNU", Cle::AccAppareilInconnu),
                ("REVOQUE", Cle::AccRevoque),
                ("AUTORISE", Cle::AccAutorise),
                ("REVOQUER", Cle::AccRevoquer),
                ("APPAREIL_REVOQUE_MSG", Cle::AccAppareilRevoqueMsg),
                ("ERREUR_REVOCATION", Cle::AccErreurRevocation),
                ("MDP_MIS_A_JOUR", Cle::AccMdpMisAJour),
                ("ERREUR_MDP", Cle::AccErreurMdp),
                ("ERREUR_MDP_TROP_COURT", Cle::AccErreurMdpTropCourt),
                ("ID_COPIE", Cle::AccIdCopie),
                ("COPIE_IMPOSSIBLE", Cle::AccCopieImpossible),
                ("COPIEZ_ID", Cle::AccCopiezId),
                ("LIEN_CREE", Cle::AccLienCree),
                ("CREATION_IMPOSSIBLE", Cle::AccCreationImpossible),
                ("LIEN_SUPPRIME", Cle::AccLienSupprime),
                ("SUPPRESSION_IMPOSSIBLE", Cle::AccSuppressionImpossible),
                ("AFF_TUILES_TITRE", Cle::AccAffTuilesTitre),
                ("AFF_TUILES_DESC", Cle::AccAffTuilesDesc),
                ("AFF_EVENEMENTS_TITRE", Cle::AccAffEvenementsTitre),
                ("AFF_EVENEMENTS_DESC", Cle::AccAffEvenementsDesc),
                ("AFF_APPS_TITRE", Cle::AccAffAppsTitre),
                ("AFF_APPS_DESC", Cle::AccAffAppsDesc),
                ("TOUT", Cle::AccTout),
                ("AUCUN", Cle::AccAucun),
                ("INDISPONIBLE", Cle::AccIndisponible),
                ("ERREUR_CHARGEMENT_AFF", Cle::AccErreurChargementAff),
                ("ENREGISTREMENT", Cle::AccEnregistrement),
                ("ENREGISTRER_AFFICHAGE", Cle::AccEnregistrerAffichage),
                ("AFF_ENREGISTRE", Cle::AccAffEnregistre),
                ("ECHEC_ENREGISTREMENT", Cle::AccEchecEnregistrement),
                ("ERREUR_CHARGEMENT_COMPTE", Cle::AccErreurChargementCompte),
                ("UTILISATEUR_DEFAUT", Cle::AdmLogsUtilisateur),
            ]);
            let html = html.replacen("{{I18N_JS}}", &i18n_js, 1);
            let _ = crate::utils::envoyer(request, Response::from_string(html).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
        }
        Err(_) => {
            let _ = crate::utils::envoyer(request, 
                Response::from_string(format!("Fichier introuvable : {}", path))
                    .with_status_code(500),
            );
        }
    }
}

fn respond_json(request: Request, body: Value, status: u16) {
    let _ = crate::utils::envoyer(request, 
        Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
    );
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

fn maintenant_epoch() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn url_encode_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
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
    let body = crate::utils::lire_corps(request, crate::utils::CORPS_MAX_DEFAUT).unwrap_or_default();
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
