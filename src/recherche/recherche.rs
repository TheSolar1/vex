// ══════════════════════════════════════════════════════════════════
// recherche.rs — App "Recherche" VEX (étape 2 feuille de route)
//
// Point d'entree unique (page /recherche) qui regroupe plusieurs
// mini-apps de recherche, chacune UNIQUEMENT accessible depuis cette
// page (jamais dans la sidebar principale ni ailleurs sur VEX) :
//   - Extensions   catalogue GitHub des extensions Vex (existant)
//   - Wiki         encyclopedie collaborative interne (tout compte
//                  connecte peut ecrire, comme un vrai wiki)
//   - FAQ          questions/reponses curatees (ecriture reservee aux
//                  comptes de confiance, privilege <= 6)
//
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
//   - /api/recherche/wiki/*  CRUD + recherche des articles wiki.
//   - /api/recherche/faq/*   CRUD + recherche des entrees FAQ.
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
        let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
        return html_response(serve_html(&nav, &langue, uid, privilege));
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

    // ── Wiki (lecture/recherche : tout compte connecte) ─────────────
    if path == "/api/recherche/wiki" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let q = crate::utils::parse_query(&url).get("q").cloned().unwrap_or_default();
        return wiki_liste(pool, &q);
    }
    if path == "/api/recherche/wiki/article" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let id = crate::utils::parse_query(&url).get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        return wiki_article(pool, id);
    }
    if path == "/api/recherche/wiki/save" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        let body = lire_body_formulaire(req);
        return wiki_save(pool, &user, &body);
    }
    if path == "/api/recherche/wiki/delete" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        let body = lire_body_formulaire(req);
        let id = body.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        return wiki_delete(pool, &user, id);
    }

    // ── FAQ (lecture/recherche : tout compte connecte, ecriture :
    // comptes de confiance uniquement, privilege <= 6) ──────────────
    if path == "/api/recherche/faq" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let q = crate::utils::parse_query(&url).get("q").cloned().unwrap_or_default();
        return faq_liste(pool, &q);
    }
    if path == "/api/recherche/faq/save" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        if user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99) > 6 {
            return json_response(403, json!({"success":false,"error":"Réservé aux comptes de confiance"}));
        }
        let body = lire_body_formulaire(req);
        return faq_save(pool, &user, &body);
    }
    if path == "/api/recherche/faq/delete" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        if user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99) > 6 {
            return json_response(403, json!({"success":false,"error":"Réservé aux comptes de confiance"}));
        }
        let body = lire_body_formulaire(req);
        let id = body.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        return faq_delete(pool, id);
    }

    json_response(404, json!({"error":"Route inconnue"}))
}

fn lire_body_formulaire(req: &mut Request) -> HashMap<String, String> {
    let mut contenu = String::new();
    let _ = std::io::Read::read_to_string(req.as_reader(), &mut contenu);
    contenu
        .split('&')
        .filter_map(|paire| {
            let mut it = paire.splitn(2, '=');
            let k = it.next()?;
            let v = it.next().unwrap_or("");
            Some((
                crate::utils::url_decode(k),
                crate::utils::url_decode(v),
            ))
        })
        .collect()
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

// ══════════════════════════════════════════════════════════════════
// Wiki (app interne, visible UNIQUEMENT dans /recherche)
// ══════════════════════════════════════════════════════════════════

const EXTRAIT_LEN: usize = 180;

fn extrait(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let court: String = s.chars().take(n).collect();
        format!("{}…", court.trim_end())
    }
}

fn wiki_liste(pool: &DbPool, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return json_response(200, json!({"success":true,"data":{"items":[]}})),
    };
    let motif = format!("%{}%", q.trim());
    let rows: Vec<(i64, String, String, String, String, i64)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id, titre, contenu, auteur_nom, DATE_FORMAT(maj, '%Y-%m-%d %H:%i'), vues \
         FROM wiki_pages WHERE titre LIKE ? OR contenu LIKE ? ORDER BY maj DESC LIMIT 100",
        (&motif, &motif),
        |(id, titre, contenu, auteur_nom, maj, vues): (i64, String, String, String, String, i64)| {
            (id, titre, contenu, auteur_nom, maj, vues)
        },
    )
    .unwrap_or_default();

    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, titre, contenu, auteur_nom, maj, vues)| {
            json!({
                "id": id,
                "titre": titre,
                "extrait": extrait(&contenu, EXTRAIT_LEN),
                "auteur_nom": auteur_nom,
                "maj": maj,
                "vues": vues,
            })
        })
        .collect();

    json_response(200, json!({"success":true,"data":{"items":items}}))
}

fn wiki_article(pool: &DbPool, id: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    if id <= 0 {
        return json_response(404, json!({"success":false,"error":"Article introuvable"}));
    }
    let rows = selectionner(
        pool,
        "wiki_pages",
        &[("id", mysql::Value::from(id))],
        &["id", "titre", "contenu", "auteur_id", "auteur_nom", "maj", "vues"],
        None,
        Some(1),
    );
    let Some(row) = rows.into_iter().next() else {
        return json_response(404, json!({"success":false,"error":"Article introuvable"}));
    };
    // Compteur de vues -- best effort, pas critique si ça rate.
    if let Ok(mut conn) = pool.get_conn() {
        let _ = mysql::prelude::Queryable::exec_drop(
            &mut conn,
            "UPDATE wiki_pages SET vues = vues + 1 WHERE id = ?",
            (id,),
        );
    }
    json_response(200, json!({"success":true,"data":row}))
}

fn wiki_save(
    pool: &DbPool,
    user: &HashMap<String, Value>,
    body: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
    let nom = user.get("nom").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let titre = body.get("titre").map(|s| s.trim().to_string()).unwrap_or_default();
    let contenu = body.get("contenu").map(|s| s.trim().to_string()).unwrap_or_default();
    if titre.is_empty() || contenu.is_empty() {
        return json_response(200, json!({"success":false,"error":"Titre et contenu obligatoires"}));
    }
    if titre.chars().count() > 255 {
        return json_response(200, json!({"success":false,"error":"Titre trop long (255 caractères max)"}));
    }
    let id = body.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);

    if id > 0 {
        // Edition d'un article existant : l'auteur peut toujours modifier
        // le sien, un tiers doit etre un compte de confiance (privilege
        // <= 6) -- meme logique de limitation du vandalisme que la
        // suppression.
        let cible = selectionner(pool, "wiki_pages", &[("id", mysql::Value::from(id))], &["auteur_id"], None, Some(1));
        let Some(row) = cible.first() else {
            return json_response(200, json!({"success":false,"error":"Article introuvable"}));
        };
        let auteur_id = row.get("auteur_id").and_then(|v| v.as_i64()).unwrap_or(-1);
        if auteur_id != uid && privilege > 6 {
            return json_response(200, json!({"success":false,"error":"Seul l'auteur ou un compte de confiance peut modifier cet article"}));
        }
        crate::appeldb::inserer_ou_modifier(
            pool,
            "wiki_pages",
            &[("titre", mysql::Value::from(titre)), ("contenu", mysql::Value::from(contenu))],
            &[("id", mysql::Value::from(id))],
        );
        return json_response(200, json!({"success":true,"message":"Article mis à jour","data":{"id":id}}));
    }

    let nouvel_id = crate::appeldb::inserer_ou_modifier(
        pool,
        "wiki_pages",
        &[
            ("titre", mysql::Value::from(titre)),
            ("contenu", mysql::Value::from(contenu)),
            ("auteur_id", mysql::Value::from(uid)),
            ("auteur_nom", mysql::Value::from(nom)),
        ],
        &[],
    );
    if nouvel_id < 0 {
        return json_response(200, json!({"success":false,"error":"Erreur lors de la création"}));
    }
    json_response(200, json!({"success":true,"message":"Article créé","data":{"id":nouvel_id}}))
}

fn wiki_delete(
    pool: &DbPool,
    user: &HashMap<String, Value>,
    id: i64,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
    let cible = selectionner(pool, "wiki_pages", &[("id", mysql::Value::from(id))], &["auteur_id"], None, Some(1));
    let Some(row) = cible.first() else {
        return json_response(200, json!({"success":false,"error":"Article introuvable"}));
    };
    let auteur_id = row.get("auteur_id").and_then(|v| v.as_i64()).unwrap_or(-1);
    if auteur_id != uid && privilege > 6 {
        return json_response(200, json!({"success":false,"error":"Seul l'auteur ou un compte de confiance peut supprimer cet article"}));
    }
    crate::appeldb::supprimer_ligne(pool, "wiki_pages", "id", mysql::Value::from(id));
    json_response(200, json!({"success":true,"message":"Article supprimé"}))
}

// ══════════════════════════════════════════════════════════════════
// FAQ (app interne, visible UNIQUEMENT dans /recherche) -- lecture
// ouverte, ecriture reservee aux comptes de confiance (privilege <= 6,
// verifie dans handle() avant d'appeler faq_save/faq_delete).
// ══════════════════════════════════════════════════════════════════

fn faq_liste(pool: &DbPool, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return json_response(200, json!({"success":true,"data":{"items":[]}})),
    };
    let motif = format!("%{}%", q.trim());
    let rows: Vec<(i64, String, String)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id, question, reponse FROM wiki_faq \
         WHERE question LIKE ? OR reponse LIKE ? ORDER BY id DESC LIMIT 100",
        (&motif, &motif),
        |(id, question, reponse): (i64, String, String)| (id, question, reponse),
    )
    .unwrap_or_default();

    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, question, reponse)| json!({"id": id, "question": question, "reponse": reponse}))
        .collect();

    json_response(200, json!({"success":true,"data":{"items":items}}))
}

fn faq_save(
    pool: &DbPool,
    user: &HashMap<String, Value>,
    body: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let uid = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let question = body.get("question").map(|s| s.trim().to_string()).unwrap_or_default();
    let reponse = body.get("reponse").map(|s| s.trim().to_string()).unwrap_or_default();
    if question.is_empty() || reponse.is_empty() {
        return json_response(200, json!({"success":false,"error":"Question et réponse obligatoires"}));
    }
    if question.chars().count() > 500 {
        return json_response(200, json!({"success":false,"error":"Question trop longue (500 caractères max)"}));
    }
    let id = body.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);

    if id > 0 {
        crate::appeldb::inserer_ou_modifier(
            pool,
            "wiki_faq",
            &[("question", mysql::Value::from(question)), ("reponse", mysql::Value::from(reponse))],
            &[("id", mysql::Value::from(id))],
        );
        return json_response(200, json!({"success":true,"message":"Entrée mise à jour","data":{"id":id}}));
    }

    let nouvel_id = crate::appeldb::inserer_ou_modifier(
        pool,
        "wiki_faq",
        &[
            ("question", mysql::Value::from(question)),
            ("reponse", mysql::Value::from(reponse)),
            ("auteur_id", mysql::Value::from(uid)),
        ],
        &[],
    );
    if nouvel_id < 0 {
        return json_response(200, json!({"success":false,"error":"Erreur lors de la création"}));
    }
    json_response(200, json!({"success":true,"message":"Entrée créée","data":{"id":nouvel_id}}))
}

fn faq_delete(pool: &DbPool, id: i64) -> Response<std::io::Cursor<Vec<u8>>> {
    crate::appeldb::supprimer_ligne(pool, "wiki_faq", "id", mysql::Value::from(id));
    json_response(200, json!({"success":true,"message":"Entrée supprimée"}))
}

fn serve_html(nav_html: &str, langue: &str, uid: i64, privilege: i64) -> String {
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
    // MON_ID/MON_PRIVILEGE : utilisees cote JS uniquement pour l'affichage
    // (afficher les boutons modifier/supprimer sur un article dont on est
    // l'auteur, ou les boutons d'ecriture FAQ pour un compte de confiance)
    // -- jamais une source de verite, le serveur revalide tout dans
    // wiki_save/wiki_delete/faq_save/faq_delete.
    html.replacen(
        "{{I18N_JS}}",
        &format!("const I18N = {{}}; const MON_ID = {}; const MON_PRIVILEGE = {};", uid, privilege),
        1,
    )
}
