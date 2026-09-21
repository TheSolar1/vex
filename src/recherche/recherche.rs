// ══════════════════════════════════════════════════════════════════
// recherche.rs — App "Recherche" VEX (étape 2 feuille de route)
//
// Deux fonctions :
//   - GET  /api/recherche/extensions?q=...  Recherche dans le catalogue
//     d'extensions Vex (réutilise le catalogue GitHub deja utilise par
//     l'admin, mais version publique : aucune info operationnelle
//     serveur (installee/compilee) n'est exposee ici).
//   - GET  /api/recherche/abonnement  Auto-verification de l'abonnement
//     de l'UTILISATEUR CONNECTE UNIQUEMENT (jamais d'un tiers -- une
//     recherche d'abonnement d'autrui serait une fuite de donnees
//     financieres sur un projet axe confidentialite). Prevu pour que
//     les extensions/mini-apps trouvees ici puissent debloquer des
//     fonctionnalites premium sans dupliquer la logique de plan.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{selectionner, DbPool};
use crate::config_loader::VexConfig;
use crate::function::{build_nav_html, get_user_language, NavContext};
use crate::i18n::{self, Cle};
use serde_json::{json, Value};
use std::collections::HashMap;
use tiny_http::{Request, Response};

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
    req.remote_addr().map(|a| a.ip().to_string()).unwrap_or_default()
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
    Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
        )
}

fn redirect_response(location: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("")
        .with_status_code(302)
        .with_header(tiny_http::Header::from_bytes("Location", location).unwrap())
}

fn html_response(html: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
    )
}

pub fn handle(pool: &DbPool, config: &VexConfig, req: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();

    if path == "/recherche" || path == "/recherche/" {
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
            page_key: "recherche",
            cookie_val: &cookie,
            remote_ip: &ip,
            user_agent: &ua,
            query_id: None,
            apps: vec![],
            admin_apps: vec![],
        });
        let langue = get_user_language(pool, Some(uid), None, None);
        return html_response(serve_html(&nav, &langue));
    }

    if path == "/api/recherche/extensions" {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        let _ = user;
        let q = crate::utils::parse_query(&url).get("q").cloned().unwrap_or_default();
        return api_extensions(config, &q);
    }

    if path == "/api/recherche/abonnement" {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        return api_abonnement(pool, config, uid);
    }

    json_response(404, json!({"error":"Route inconnue"}))
}

/// Recherche dans le catalogue d'extensions Vex (source GitHub configurée
/// en admin). Version PUBLIQUE (tout utilisateur connecté) du catalogue
/// admin -- ne renvoie que ce qui est pertinent pour un utilisateur final
/// (nom, taille, lien, popularité), jamais l'état d'installation/
/// compilation du serveur local (réservé à /api/admin/marketplace).
fn api_extensions(config: &VexConfig, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let cfg = crate::admin::admin::read_config("config.json");
    let recherche = q.trim().to_lowercase();
    let _ = config;

    let rel = match crate::admin::admin::market_release(&cfg, false, "fr") {
        Ok(r) => r,
        Err(e) => return json_response(200, json!({"success":true,"data":{"items":[],"erreur":e}})),
    };

    let mut items = Vec::new();
    if let Some(assets) = rel["assets"].as_array() {
        for a in assets {
            let nom = a["name"].as_str().unwrap_or("").to_string();
            let bas = nom.to_lowercase();
            let manifeste = bas.ends_with(".extension.json");
            if !(manifeste || bas.ends_with(".rs") || bas.ends_with(".zip")) {
                continue;
            }
            let id = if manifeste {
                nom.trim_end_matches(".extension.json").to_lowercase()
            } else {
                crate::admin::admin::market_id_depuis_nom(&nom)
            };
            if !recherche.is_empty() && !bas.contains(&recherche) && !id.contains(&recherche) {
                continue;
            }
            items.push(json!({
                "nom": nom,
                "id": id,
                "taille": a["size"].as_u64().unwrap_or(0),
                "url": a["browser_download_url"].as_str().unwrap_or(""),
                "maj": a["updated_at"].as_str().unwrap_or(""),
                "telechargements": a["download_count"].as_u64().unwrap_or(0),
            }));
        }
    }

    json_response(200, json!({
        "success": true,
        "data": {
            "items": items,
            "source": cfg["extensions"]["marketplace_url"].as_str().unwrap_or(""),
        }
    }))
}

/// Statut d'abonnement de l'utilisateur COURANT uniquement (jamais d'un
/// tiers), en euros -- pour que les extensions/mini-apps trouvées via
/// Recherche puissent débloquer leurs fonctionnalités premium.
fn api_abonnement(pool: &DbPool, config: &VexConfig, uid: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    let rows = selectionner(
        pool,
        "login",
        &[("id", mysql::Value::from(uid))],
        &["vip", "vip_paye"],
        None,
        Some(1),
    );
    let Some(row) = rows.into_iter().next() else {
        return json_response(404, json!({"success":false,"error":"Compte introuvable"}));
    };
    let vip = row.get("vip").and_then(|v| v.as_i64()).unwrap_or(0);
    let vip_paye = row.get("vip_paye").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
    let plan_id = if vip == 1 { "vip" } else { "free" };

    let plan = config
        .plans
        .available_plans
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(plan_id))
        .cloned()
        .unwrap_or(json!({}));
    let montant_eur = plan.get("price_eur_month").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let plan_nom = plan.get("name").and_then(|v| v.as_str()).unwrap_or(plan_id).to_string();

    json_response(200, json!({
        "success": true,
        "data": {
            "actif": vip == 1,
            "paye": vip_paye,
            "plan_id": plan_id,
            "plan_nom": plan_nom,
            "montant_eur": montant_eur,
            "devise": "EUR",
        }
    }))
}

fn serve_html(nav_html: &str, langue: &str) -> String {
    let html = include_str!("../../static/recherche/recherche.html").replace("__NAV_HTML__", nav_html);
    let html = i18n::appliquer_traductions(&html, langue, &[
        ("{{T_TITRE_ONGLET}}", Cle::RechTitreOnglet),
        ("{{T_TITRE}}", Cle::RechTitre),
        ("{{T_PLACEHOLDER}}", Cle::RechPlaceholder),
        ("{{T_AUCUN_RESULTAT}}", Cle::RechAucunResultat),
        ("{{T_TELECHARGEMENTS}}", Cle::RechTelechargements),
    ]);
    // FIX (bug, recherche silencieuse) : {{I18N_JS}} n'etait jamais
    // remplace ici (contrairement a fchier.rs/mess.rs/...) -- il restait
    // tel quel dans le <script>, ce qui cassait TOUT le JS de la page des
    // la premiere ligne (ReferenceError) et empechait le moindre appel a
    // /api/recherche/extensions. La page n'utilise aucune variable
    // I18N.xxx cote JS (tout passe par les placeholders {{T_...}}
    // ci-dessus, deja substitues cote serveur), donc un objet vide suffit.
    html.replacen("{{I18N_JS}}", "const I18N = {};", 1)
}
