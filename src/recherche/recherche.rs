// ══════════════════════════════════════════════════════════════════
// recherche.rs — App "Recherche" VEX (étape 2 feuille de route)
//
// Point d'entree unique (page /recherche) : UNE seule barre de
// recherche (style moteur de recherche), dont les resultats melangent
// directement plusieurs sources -- pas d'onglets separes -- chacune
// UNIQUEMENT accessible depuis cette page (jamais dans la sidebar
// principale ni ailleurs sur VEX) :
//   - Wiki         encyclopedie collaborative interne (tout compte
//                  connecte peut ecrire, comme un vrai wiki)
//   - Extensions   catalogue GitHub des extensions Vex
//
//   - GET  /api/recherche/global?q=...  Recherche unifiee (Wiki +
//     Extensions), renvoie une liste triee/melangee prete a afficher.
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
//   - GET  /api/recherche/wikipedia/article?titre=...  Article Wikipedia
//     complet, ouvert DANS VEX (meme lecteur que le wiki interne, pas de
//     redirection vers wikipedia.org) et mis en cache localement
//     (wikipedia_cache) des la premiere lecture -- les lectures suivantes
//     du meme article ne font plus aucun appel reseau.
//   - /api/recherche/faq/*   DESACTIVE (demande utilisateur, 22/09) --
//     renvoie desormais une erreur "fonctionnalite desactivee" sans
//     toucher a la table wiki_faq (donnees existantes conservees si un
//     jour on la reactive). Retirer entierement le code serait plus
//     propre mais la demande etait "desactive", pas "supprime".
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

    if path == "/api/recherche/global" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let q = crate::utils::parse_query(&url).get("q").cloned().unwrap_or_default();
        return api_global(pool, config, &q);
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
    if path == "/api/recherche/actualite/article" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let id = crate::utils::parse_query(&url).get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let rows = selectionner(
            pool,
            "actualites",
            &[("id", mysql::Value::from(id))],
            &["id", "titre", "contenu", "date"],
            None,
            Some(1),
        );
        return match rows.into_iter().next() {
            Some(row) => json_response(200, json!({"success":true,"data":row})),
            None => json_response(404, json!({"success":false,"error":"Actualité introuvable"})),
        };
    }
    // ── Actualites : ecriture reservee aux comptes de confiance
    // (privilege <= 6, meme regle que le wiki/FAQ) -- demande utilisateur :
    // "je veux que ca soit automatise" -- plus besoin d'une intervention
    // manuelle en base a chaque nouvelle actualite, un compte de confiance
    // peut desormais en creer/modifier/supprimer directement depuis
    // l'interface.
    if path == "/api/recherche/actualite/save" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        if user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99) > 6 {
            return json_response(403, json!({"success":false,"error":"Réservé aux comptes de confiance"}));
        }
        let body = lire_body_formulaire(req);
        return actualite_save(pool, &body);
    }
    if path == "/api/recherche/actualite/delete" && req.method() == &tiny_http::Method::Post {
        let user = match verifier_session(pool, req) {
            Some(u) => u,
            None => return json_response(401, json!({"success":false,"error":"Non connecté"})),
        };
        if user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99) > 6 {
            return json_response(403, json!({"success":false,"error":"Réservé aux comptes de confiance"}));
        }
        let body = lire_body_formulaire(req);
        let id = body.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        crate::appeldb::supprimer_ligne(pool, "actualites", "id", mysql::Value::from(id));
        return json_response(200, json!({"success":true,"message":"Actualité supprimée"}));
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

    // ── Wikipedia : article complet, servi depuis le miroir LOCAL
    // (wikipedia_cache) des qu'il a deja ete consulte une fois -- demande
    // utilisateur : "je veux que tout soit en local", pas juste un lien
    // qui renvoie vers wikipedia.org. Ouvert dans le meme lecteur que les
    // articles du wiki interne (interface unifiee, pas un nouvel onglet).
    if path == "/api/recherche/wikipedia/article" {
        if verifier_session(pool, req).is_none() {
            return json_response(401, json!({"success":false,"error":"Non connecté"}));
        }
        let titre = crate::utils::parse_query(&url).get("titre").cloned().unwrap_or_default();
        return wikipedia_article(pool, &titre);
    }

    // ── FAQ : DESACTIVEE (demande utilisateur, 22/09 -- l'app FAQ est
    // retiree de Recherche). Repond avant meme de verifier la session,
    // simple garde-fou statique. La table wiki_faq et les fonctions
    // faq_* restent dans le code (donnees existantes conservees) mais
    // ne sont plus jamais appelees.
    if path.starts_with("/api/recherche/faq") {
        return json_response(410, json!({"success":false,"error":"FAQ désactivée"}));
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

/// Recherche parmi les fichiers PUBLICS de TOUT le reseau VEX (pas
/// seulement les siens) -- demande utilisateur. `visble = '0'` est LA
/// valeur canonique pour "public" (voir static/fchier/fchier.html, le
/// select Visibilite : value="0" -> Public, value="1" -> Prive) --
/// verifiee ici a la source plutot que devinee, et c'est la MEME
/// condition deja utilisee par api_download() (src/fchier/fchier.rs) pour
/// autoriser N'IMPORTE QUEL compte connecte a telecharger un fichier
/// public d'un autre utilisateur. Recherche par nom uniquement (pas de
/// contenu de fichier a indexer), jamais le champ `fichier` (le contenu
/// base64) ni les fichiers prives d'autrui.
fn fichiers_rechercher(pool: &DbPool, q: &str) -> Vec<Value> {
    if q.trim().is_empty() {
        return vec![];
    }
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let motif = format!("%{}%", q.trim());
    let rows: Vec<(i64, String, i64, String)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id, nom, taille, type_fichier FROM fichiers \
         WHERE visble = '0' AND nom LIKE ? ORDER BY id DESC LIMIT 20",
        (&motif,),
        |(id, nom, taille, type_fichier): (i64, String, i64, String)| (id, nom, taille, type_fichier),
    )
    .unwrap_or_default();

    rows.into_iter()
        .map(|(id, nom, taille, type_fichier)| {
            json!({
                "type": "fichier",
                "id": id,
                "titre": nom,
                "extrait": format!("{} · {}", formater_taille(taille as u64), type_fichier),
                "meta": "Fichier public VEX",
                "url": format!("/api/fchier/download?id={}", id),
            })
        })
        .collect()
}

/// Apps VEX de base -- memes entrees/URLs que default_apps() dans
/// function.rs (sidebar principale), dupliquees ici en dur plutot que
/// partagees : ce module n'a pas acces au type NavApp sans creer une
/// dependance croisee, et cette liste change rarement.
const APPS_VEX: &[(&str, &str, &str)] = &[
    // (titre, url, description courte)
    ("Accueil", "/login/dashboard", "Tableau de bord VEX"),
    ("Mail", "/mess/", "Messagerie VEX"),
    ("Fichiers", "/fchier/", "Stockage et partage de fichiers"),
    ("Vidéos", "/viso/", "Visioconférence"),
    ("Sitec", "/sitec/", "Éditeur de sites web"),
    ("Compte", "/login/account", "Paramètres du compte et abonnement"),
];

/// Recherche par TITRE uniquement (une app n'a pas de "contenu") parmi les
/// apps VEX de base -- demande utilisateur : une categorie "Apps" dans les
/// resultats de Recherche.
fn apps_rechercher(q: &str) -> Vec<Value> {
    let motif = q.trim().to_lowercase();
    if motif.is_empty() {
        return vec![];
    }
    APPS_VEX
        .iter()
        .filter(|(titre, _, _)| titre.to_lowercase().contains(&motif))
        .map(|(titre, url, description)| {
            json!({
                "type": "app",
                "titre": titre,
                "extrait": description,
                "meta": "App VEX",
                "url": url,
            })
        })
        .collect()
}

/// Coeur de la recherche actualites -- meme logique titre-avant-contenu que
/// le wiki, ecriture reservee aux comptes de confiance (verifiee dans
/// handle() avant actualite_save/actualite_delete).
fn actualites_rechercher(pool: &DbPool, q: &str) -> Vec<Value> {
    if q.trim().is_empty() {
        return vec![];
    }
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let motif_titre = format!("%{}%", q.trim());
    let rows_titre: Vec<(i64, String, String, String)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id, titre, contenu, DATE_FORMAT(date, '%Y-%m-%d') FROM actualites \
         WHERE titre LIKE ? ORDER BY date DESC LIMIT 20",
        (&motif_titre,),
        |(id, titre, contenu, date): (i64, String, String, String)| (id, titre, contenu, date),
    )
    .unwrap_or_default();
    let rows = if !rows_titre.is_empty() {
        rows_titre
    } else if let Some(requete) = requete_fulltext(q) {
        mysql::prelude::Queryable::exec_map(
            &mut conn,
            "SELECT id, titre, contenu, DATE_FORMAT(date, '%Y-%m-%d') FROM actualites \
             WHERE MATCH(titre, contenu) AGAINST(? IN BOOLEAN MODE) \
             ORDER BY MATCH(titre, contenu) AGAINST(? IN BOOLEAN MODE) DESC LIMIT 20",
            (&requete, &requete),
            |(id, titre, contenu, date): (i64, String, String, String)| (id, titre, contenu, date),
        )
        .unwrap_or_default()
    } else {
        vec![]
    };
    rows.into_iter()
        .map(|(id, titre, contenu, date)| {
            json!({
                "id": id,
                "titre": titre,
                "extrait": extrait(&contenu, EXTRAIT_LEN),
                "date": date,
            })
        })
        .collect()
}

/// Creation/modification d'une actualite (compte de confiance uniquement,
/// verifie dans handle() avant l'appel). UPSERT explicite : `id` fourni ->
/// UPDATE, sinon INSERT.
fn actualite_save(pool: &DbPool, body: &HashMap<String, String>) -> Response<std::io::Cursor<Vec<u8>>> {
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
        crate::appeldb::inserer_ou_modifier(
            pool,
            "actualites",
            &[("titre", mysql::Value::from(titre)), ("contenu", mysql::Value::from(contenu))],
            &[("id", mysql::Value::from(id))],
        );
        return json_response(200, json!({"success":true,"message":"Actualité mise à jour","data":{"id":id}}));
    }

    let nouvel_id = crate::appeldb::inserer_ou_modifier(
        pool,
        "actualites",
        &[("titre", mysql::Value::from(titre)), ("contenu", mysql::Value::from(contenu))],
        &[],
    );
    if nouvel_id < 0 {
        return json_response(200, json!({"success":false,"error":"Erreur lors de la création"}));
    }
    json_response(200, json!({"success":true,"message":"Actualité créée","data":{"id":nouvel_id}}))
}

/// Coeur de la recherche extensions, partagé entre /api/recherche/extensions
/// (reponse detaillee, format historique) et /api/recherche/global (format
/// unifie). Renvoie (items au format detaille, erreur eventuelle).
///
/// FIX (qualite du contenu) : une extension publie plusieurs fichiers dans
/// la meme release GitHub (ex. qseal.extension.json, qseal.mod.rs,
/// qseal.qseal-core.js...) -- l'ancienne version listait chacun comme un
/// resultat separe, donc chercher "qseal" faisait apparaitre 2-3 entrees
/// quasi identiques avec des noms de fichiers techniques. Regroupe
/// desormais par id d'extension : un seul resultat, taille cumulee, nom
/// lisible (le manifeste .extension.json sert de nom d'affichage/lien
/// quand present).
fn extensions_rechercher(q: &str) -> (Vec<Value>, Option<String>) {
    let cfg = crate::admin::admin::read_config("config.json");
    let recherche = q.trim().to_lowercase();

    let rel = match crate::admin::admin::market_release(&cfg, false, "fr") {
        Ok(r) => r,
        Err(e) => return (vec![], Some(e)),
    };

    struct Groupe {
        nom_affiche: String,
        taille: u64,
        url: String,
        maj: String,
        telechargements: u64,
        a_manifeste: bool,
    }
    let mut groupes: std::collections::HashMap<String, Groupe> = std::collections::HashMap::new();
    let mut ordre: Vec<String> = Vec::new();

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
            let taille = a["size"].as_u64().unwrap_or(0);
            let url = a["browser_download_url"].as_str().unwrap_or("").to_string();
            let maj = a["updated_at"].as_str().unwrap_or("").to_string();
            let telechargements = a["download_count"].as_u64().unwrap_or(0);

            if !groupes.contains_key(&id) {
                ordre.push(id.clone());
            }
            let g = groupes.entry(id.clone()).or_insert(Groupe {
                nom_affiche: nom_lisible(&id),
                taille: 0,
                url: url.clone(),
                maj: maj.clone(),
                telechargements: 0,
                a_manifeste: false,
            });
            g.taille += taille;
            g.telechargements = g.telechargements.max(telechargements);
            // Le manifeste sert de lien/date canonique quand il existe.
            if manifeste || !g.a_manifeste {
                g.url = url;
                g.maj = maj;
            }
            if manifeste {
                g.a_manifeste = true;
            }
        }
    }

    let items = ordre
        .into_iter()
        .map(|id| {
            let g = &groupes[&id];
            json!({
                "nom": g.nom_affiche,
                "id": id,
                "taille": g.taille,
                "url": g.url,
                "maj": g.maj,
                "telechargements": g.telechargements,
            })
        })
        .collect();
    (items, None)
}

/// "qseal" -> "Qseal", "mon_extension" -> "Mon extension" -- nom d'affichage
/// lisible a partir d'un id technique (minuscules + underscores/tirets).
fn nom_lisible(id: &str) -> String {
    let mut mots: Vec<String> = id
        .split(|c: char| c == '_' || c == '-')
        .filter(|m| !m.is_empty())
        .map(|m| {
            let mut c = m.chars();
            match c.next() {
                Some(premiere) => premiere.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if mots.is_empty() {
        mots.push(id.to_string());
    }
    mots.join(" ")
}

/// Recherche dans le catalogue d'extensions Vex (source GitHub configurée
/// en admin). Version PUBLIQUE (tout utilisateur connecté) du catalogue
/// admin -- ne renvoie que ce qui est pertinent pour un utilisateur final
/// (nom, taille, lien, popularité), jamais l'état d'installation/
/// compilation du serveur local (réservé à /api/admin/marketplace).
fn api_extensions(config: &VexConfig, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let _ = config;
    let cfg = crate::admin::admin::read_config("config.json");
    let (items, erreur) = extensions_rechercher(q);
    json_response(200, json!({
        "success": true,
        "data": {
            "items": items,
            "erreur": erreur,
            "source": cfg["extensions"]["marketplace_url"].as_str().unwrap_or(""),
        }
    }))
}

/// Recherche unifiee (Wiki + Extensions) -- resultats melanges dans une
/// seule liste prete a afficher, chaque item porte un `type` pour que le
/// front sache quoi faire au clic (ouvrir l'article / telecharger).
/// Wiki d'abord (contenu propre a VEX, plus pertinent qu'un catalogue
/// externe), puis Extensions.
fn api_global(pool: &DbPool, config: &VexConfig, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let _ = config;
    let mut items: Vec<Value> = Vec::new();

    // PERF : les deux seules sources qui font un appel HTTP sortant
    // (Wikipedia live, jusqu'a 5s ; catalogue d'extensions GitHub, jusqu'a
    // 8s sur cache froid) sont lancees en threads des le debut de la
    // requete, EN PARALLELE des lectures locales (DB) qui suivent juste en
    // dessous -- le serveur traite les requetes HTTP ENTRANTES une par une
    // sur un seul thread (voir main.rs), donc cette requete-ci bloque de
    // toute facon ce thread jusqu'a sa reponse complete ; mais rien
    // n'empeche SES DEUX appels sortants d'attendre en parallele plutot
    // qu'en serie, ce qui fait passer le pire cas de (5s+8s=13s) a
    // max(5s,8s)=8s, et le cas courant (les deux caches chauds) reste
    // quasi instantane. join() plus bas, une fois le travail local termine.
    let handle_wikipedia = if q.trim().chars().count() >= 2 {
        let q_owned = q.to_string();
        Some(std::thread::spawn(move || wikipedia_rechercher(&q_owned)))
    } else {
        None
    };
    let handle_extensions = {
        let q_owned = q.to_string();
        std::thread::spawn(move || extensions_rechercher(&q_owned))
    };

    // Apps VEX (liste statique, memes apps/URLs que la sidebar principale
    // -- voir default_apps dans function.rs) : taper "mail" ou "fichiers"
    // doit amener directement vers l'app, comme le ferait un vrai moteur
    // de recherche pour une application installee. Recherche par titre
    // uniquement (une app n'a pas de "contenu").
    for app in apps_rechercher(q) {
        items.push(app);
    }

    for a in actualites_rechercher(pool, q) {
        items.push(json!({
            "type": "actualite",
            "id": a["id"],
            "titre": a["titre"],
            "extrait": a["extrait"],
            "meta": format!("Actualités VEX · {}", a["date"].as_str().unwrap_or("")),
        }));
    }

    for f in fichiers_rechercher(pool, q) {
        items.push(f);
    }

    for w in wiki_rechercher(pool, q) {
        items.push(json!({
            "type": "wiki",
            "id": w["id"],
            "titre": w["titre"],
            "extrait": w["extrait"],
            "meta": format!("Wiki VEX · {} · maj {}", w["auteur_nom"].as_str().unwrap_or("?"), w["maj"].as_str().unwrap_or("")),
        }));
    }

    // Miroir Wikipedia LOCAL d'abord (FULLTEXT, instantane, zero appel
    // reseau -- ~3000 articles pre-charges, voir la migration wikipedia_cache) :
    // pas de raison de le limiter a 2 caracteres puisqu'il ne coute rien,
    // contrairement a l'appel reseau live juste en dessous. FIX (retour
    // utilisateur : "je ne vois pas les articles de wikipedia") -- la limite
    // de 2 caracteres bloquait AUSSI ce miroir local, qui contient
    // maintenant largement assez de contenu pour repondre a 1 seule lettre.
    let locaux = if !q.trim().is_empty() { wikipedia_cache_rechercher(pool, q) } else { vec![] };
    let titres_locaux: std::collections::HashSet<String> =
        locaux.iter().filter_map(|it| it["titre"].as_str().map(|s| s.to_lowercase())).collect();
    for it in locaux {
        items.push(it);
    }

    // Wikipedia LIVE : reste limite a partir de 2 caracteres -- ca, c'est un
    // vrai appel reseau a chaque frappe, "e" seul faisait remonter l'article
    // sur la lettre E a chaque caractere tape, perçu comme du bruit. Lancee
    // en parallele tout en haut de la fonction (voir handle_wikipedia) --
    // on recupere juste le resultat ici, le temps d'attente reseau est deja
    // ecoule pendant les lectures locales ci-dessus.
    let mut erreur_wikipedia = None;
    if let Some(handle) = handle_wikipedia {
        match handle.join() {
            Ok(Ok(resultats)) => {
                for it in resultats {
                    let deja_local = it["titre"].as_str().map(|t| titres_locaux.contains(&t.to_lowercase())).unwrap_or(false);
                    if !deja_local {
                        items.push(it);
                    }
                }
            }
            Ok(Err(e)) => erreur_wikipedia = Some(e),
            Err(_) => erreur_wikipedia = Some("Erreur interne lors de la recherche Wikipédia".to_string()),
        }
    }

    // Catalogue d'extensions : lance en parallele tout en haut de la
    // fonction (voir handle_extensions), meme logique que Wikipedia.
    let (extensions, erreur_extensions) = handle_extensions
        .join()
        .unwrap_or_else(|_| (vec![], Some("Erreur interne lors de la recherche d'extensions".to_string())));
    for e in &extensions {
        items.push(json!({
            "type": "extension",
            "id": e["id"],
            "titre": e["nom"],
            "extrait": format!("{} · {} téléchargement(s)", formater_taille(e["taille"].as_u64().unwrap_or(0)), e["telechargements"].as_u64().unwrap_or(0)),
            "meta": "Extension VEX",
            "url": e["url"],
        }));
    }

    // PAS de raccourci Google (retire, demande utilisateur 22/09) : un
    // simple lien de redirection n'est ni "une vraie interface unifiee" ni
    // "en local" -- Google n'a pas d'API de recherche gratuite/locale
    // possible (scraper serait fragile et contraire a ses CGU), donc pas
    // d'equivalent honnete a proposer ici.

    json_response(200, json!({
        "success": true,
        "data": {
            "items": items,
            "erreur_extensions": erreur_extensions,
            "erreur_wikipedia": erreur_wikipedia,
        }
    }))
}

/// Recherche Wikipedia (fr.wikipedia.org, API publique officielle, pas de
/// cle requise) -- source externe qui donne toujours de vrais resultats
/// pertinents, contrairement au wiki interne VEX qui demarre vide. Timeout
/// court (5s) : ne doit jamais bloquer longtemps une requete sur ce serveur
/// mono-thread.
///
/// generator=search (au lieu de list=search) permet de recuperer en UN SEUL
/// appel le resultat de recherche ET un vrai extrait d'introduction (plus
/// propre que le snippet tronque avec des balises <span> a nettoyer) ET une
/// miniature (piprop=thumbnail) -- demande utilisateur : "tu prends la page
/// et tu la restylise", une vraie page Wikipedia a une image, pas
/// seulement du texte.
fn wikipedia_rechercher(q: &str) -> Result<Vec<Value>, String> {
    // FIX (demande utilisateur : "je veux que la recherche soit faite en
    // fonction du titre") -- "intitle:" est un operateur natif de la
    // recherche Wikipedia qui restreint aux articles dont le TITRE
    // correspond (pas juste le contenu). Repli sur une recherche normale
    // seulement si ca ne renvoie rien, meme logique que les deux fonctions
    // de recherche locale juste au-dessus.
    let items = wikipedia_rechercher_brut(&format!("intitle:{}", q))?;
    if !items.is_empty() {
        return Ok(items);
    }
    wikipedia_rechercher_brut(q)
}

fn wikipedia_rechercher_brut(recherche: &str) -> Result<Vec<Value>, String> {
    let url = format!(
        "https://fr.wikipedia.org/w/api.php?action=query&format=json&generator=search&gsrlimit=10&gsrsearch={}\
         &prop=extracts|pageimages&exintro=1&explaintext=1&piprop=thumbnail&pithumbsize=200",
        urlencoding_simple(recherche)
    );
    let rep = ureq::get(&url)
        .set("User-Agent", "VEX/1.0 (https://vex.hopto.org)")
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Wikipédia injoignable : {}", e))?;
    let v: Value = rep
        .into_json()
        .map_err(|e| format!("Réponse Wikipédia illisible : {}", e))?;

    let mut items = Vec::new();
    if let Some(pages) = v["query"]["pages"].as_object() {
        // generator=search ne garantit pas l'ordre de pertinence dans l'objet
        // (les cles JSON sont des pageid) -- trie par "index" (rang de
        // pertinence donne par l'API elle-meme) pour ne pas perdre le
        // classement.
        let mut pages: Vec<&Value> = pages.values().collect();
        pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(i64::MAX));
        for r in pages {
            let titre = r["title"].as_str().unwrap_or("").to_string();
            let ext = r["extract"].as_str().unwrap_or("");
            if titre.is_empty() || ext.is_empty() {
                continue;
            }
            let image = r["thumbnail"]["source"].as_str();
            // Pas de champ "url" externe : l'article s'ouvre DANS VEX (voir
            // wikipedia_article()), pas dans un nouvel onglet vers
            // wikipedia.org -- demande utilisateur, interface unifiee.
            items.push(json!({
                "type": "wikipedia",
                "titre": titre,
                "extrait": extrait(ext, EXTRAIT_LEN),
                "meta": "Wikipédia",
                "image": image,
            }));
        }
    }
    Ok(items)
}

/// Recherche dans le miroir LOCAL des articles Wikipedia deja consultes
/// (FULLTEXT, meme logique de pertinence que wiki_rechercher) -- zero appel
/// reseau, contrairement a wikipedia_rechercher (API live). Le miroir
/// grandit a chaque nouvel article ouvert (voir wikipedia_article), donc ce
/// qui est deja recherche localement devient de plus en plus complet a
/// l'usage : "un vrai moteur de recherche" qui s'ameliore avec le temps
/// plutot qu'un simple cache passe-plat.
fn wikipedia_cache_rechercher(pool: &DbPool, q: &str) -> Vec<Value> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    // FIX (demande utilisateur : "je veux que la recherche soit faite en
    // fonction du titre") -- meme logique stricte que wiki_rechercher :
    // si au moins un TITRE correspond, ce sont les seuls resultats
    // renvoyes ; le contenu ne sert de repli que si aucun titre ne
    // correspond. Limite remontee a 20 -- demande "je veux voir plus".
    let motif_titre = format!("%{}%", q.trim());
    let rows_titre: Vec<(String, String, Option<String>)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT titre, extrait, image_url FROM wikipedia_cache WHERE titre LIKE ? LIMIT 20",
        (&motif_titre,),
        |(titre, extrait, image_url): (String, String, Option<String>)| (titre, extrait, image_url),
    )
    .unwrap_or_default();
    let rows = if !rows_titre.is_empty() {
        rows_titre
    } else if let Some(requete) = requete_fulltext(q) {
        mysql::prelude::Queryable::exec_map(
            &mut conn,
            "SELECT titre, extrait, image_url FROM wikipedia_cache \
             WHERE MATCH(titre, extrait) AGAINST(? IN BOOLEAN MODE) \
             ORDER BY MATCH(titre, extrait) AGAINST(? IN BOOLEAN MODE) DESC LIMIT 20",
            (&requete, &requete),
            |(titre, extrait, image_url): (String, String, Option<String>)| (titre, extrait, image_url),
        )
        .unwrap_or_default()
    } else {
        vec![]
    };

    rows.into_iter()
        .map(|(titre, extrait_complet, image_url)| {
            json!({
                "type": "wikipedia",
                "titre": titre,
                "extrait": extrait(&extrait_complet, EXTRAIT_LEN),
                "meta": "Wikipédia · miroir local",
                "image": image_url,
            })
        })
        .collect()
}

/// Recupere UNIQUEMENT la miniature d'un article (pas le texte) -- utilise
/// pour completer une ligne de cache mise en place avant l'ajout de la
/// colonne image_url, sans re-telecharger tout l'article. Best effort :
/// None en cas d'echec, jamais une erreur bloquante (l'image est un plus,
/// pas le contenu principal).
fn recuperer_image_seule(titre: &str) -> Option<String> {
    let url = format!(
        "https://fr.wikipedia.org/w/api.php?action=query&format=json&prop=pageimages\
         &piprop=thumbnail&pithumbsize=500&titles={}",
        urlencoding_simple(titre)
    );
    let rep = ureq::get(&url)
        .set("User-Agent", "VEX/1.0 (https://vex.hopto.org)")
        .timeout(std::time::Duration::from_secs(4))
        .call()
        .ok()?;
    let v: Value = rep.into_json().ok()?;
    v["query"]["pages"]
        .as_object()?
        .values()
        .next()?["thumbnail"]["source"]
        .as_str()
        .map(|s| s.to_string())
}

/// Article Wikipedia complet, servi depuis le miroir LOCAL (wikipedia_cache)
/// s'il a deja ete consulte, sinon recupere une seule fois puis mis en
/// cache pour toutes les lectures suivantes -- c'est ca, concretement,
/// "tout en local" pour du contenu qui vient d'une source externe : apres
/// le premier appel, plus aucune requete reseau n'est necessaire pour relire
/// le meme article.
fn wikipedia_article(pool: &DbPool, titre: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let titre = titre.trim();
    if titre.is_empty() {
        return json_response(404, json!({"success":false,"error":"Article introuvable"}));
    }

    let cache = selectionner(
        pool,
        "wikipedia_cache",
        &[("titre", mysql::Value::from(titre))],
        &["extrait", "recupere_le", "image_url"],
        None,
        Some(1),
    );
    // Seuil de longueur : le pre-chargement en masse (voir commentaire sur
    // la table) ne stocke que l'INTRODUCTION de chaque article (seul mode
    // qui permet a l'API Wikipedia de repondre par lots de plusieurs
    // titres -- un extrait COMPLET est limite a 1 titre par requete cote
    // API elle-meme). Un article reellement complet fait quasi toujours
    // plus de 400 caracteres ; en dessous, on considere le cache "partiel"
    // et on va chercher le texte complet a l'ouverture reelle par
    // l'utilisateur, plutot que de le laisser coince sur une simple intro.
    const SEUIL_CACHE_COMPLET: usize = 400;
    if let Some(row) = cache.into_iter().next() {
        let extrait_cache = row.get("extrait").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let image_cache = row.get("image_url").and_then(|v| v.as_str()).map(|s| s.to_string());
        if extrait_cache.chars().count() >= SEUIL_CACHE_COMPLET {
            // Migration : une ligne mise en cache avant l'ajout de la
            // colonne image_url n'en a pas -- un seul petit appel (juste
            // l'image, pas le texte) suffit a la completer sans tout
            // re-telecharger.
            let image_cache = image_cache.or_else(|| recuperer_image_seule(titre));
            return json_response(200, json!({
                "success": true,
                "data": {
                    "titre": titre,
                    "contenu": extrait_cache,
                    "source_locale": true,
                    "recupere_le": row.get("recupere_le").cloned().unwrap_or(json!("")),
                    "image": image_cache,
                }
            }));
        }
    }

    let url = format!(
        "https://fr.wikipedia.org/w/api.php?action=query&format=json&prop=extracts|pageimages\
         &explaintext=1&piprop=thumbnail&pithumbsize=500&titles={}",
        urlencoding_simple(titre)
    );
    let rep = match ureq::get(&url)
        .set("User-Agent", "VEX/1.0 (https://vex.hopto.org)")
        .timeout(std::time::Duration::from_secs(8))
        .call()
    {
        Ok(r) => r,
        Err(e) => return json_response(200, json!({"success":false,"error":format!("Wikipédia injoignable : {}", e)})),
    };
    let v: Value = match rep.into_json() {
        Ok(v) => v,
        Err(e) => return json_response(200, json!({"success":false,"error":format!("Réponse Wikipédia illisible : {}", e)})),
    };

    let pages = &v["query"]["pages"];
    let page = pages.as_object().and_then(|obj| obj.values().next());
    let extrait = page.and_then(|p| p["extract"].as_str()).unwrap_or("").to_string();
    if extrait.is_empty() {
        return json_response(200, json!({"success":false,"error":"Article introuvable sur Wikipédia"}));
    }
    let image = page.and_then(|p| p["thumbnail"]["source"].as_str()).map(|s| s.to_string());

    // UPSERT (pas inserer_ou_modifier, qui ne fait qu'INSERT ou qu'UPDATE
    // selon where_c fourni a l'avance) : `titre` est la cle primaire, et un
    // article deja pre-charge en masse (intro courte) existe potentiellement
    // deja -- on remplace alors son extrait court par le texte complet.
    if let Ok(mut conn) = pool.get_conn() {
        let _ = mysql::prelude::Queryable::exec_drop(
            &mut conn,
            "INSERT INTO wikipedia_cache (titre, extrait, image_url) VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE extrait = VALUES(extrait), image_url = VALUES(image_url), recupere_le = CURRENT_TIMESTAMP",
            (titre, &extrait, &image),
        );
    }

    json_response(200, json!({
        "success": true,
        "data": {
            "titre": titre,
            "contenu": extrait,
            "source_locale": false,
            "image": image,
        }
    }))
}

/// Encodage URL minimal (espace -> %20, etc.) pour le parametre `srsearch` --
/// pas besoin d'une dependance complete, la requete utilisateur ne contient
/// jamais que du texte libre.
fn urlencoding_simple(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn formater_taille(o: u64) -> String {
    if o < 1024 {
        return format!("{} o", o);
    }
    let unites = ["Ko", "Mo", "Go"];
    let mut n = o as f64;
    let mut i = -1i32;
    loop {
        n /= 1024.0;
        i += 1;
        if !(n >= 1024.0 && (i as usize) < unites.len() - 1) {
            break;
        }
    }
    format!("{:.1} {}", n, unites[i.max(0) as usize])
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

/// Construit une requete MySQL "boolean full-text search" a partir du texte
/// tape par l'utilisateur : chaque mot devient un prefixe ("wik" -> "wik*"),
/// ce qui permet de trouver un article des les premiers caracteres d'un mot
/// tapes, comme un vrai moteur de recherche (pas seulement des mots
/// complets). Les operateurs boolean de MySQL (+-<>()~*"@) sont retires du
/// texte utilisateur avant construction pour eviter toute syntaxe surprise.
fn requete_fulltext(q: &str) -> Option<String> {
    let mots: Vec<String> = q
        .split_whitespace()
        .map(|m| m.chars().filter(|c| !"+-<>()~*\"@".contains(*c)).collect::<String>())
        .filter(|m: &String| !m.is_empty())
        .map(|m| format!("{}*", m))
        .collect();
    if mots.is_empty() {
        None
    } else {
        Some(mots.join(" "))
    }
}

/// Coeur de la recherche wiki, partagé entre /api/recherche/wiki et
/// /api/recherche/global.
///
/// FIX (demande utilisateur : "un vrai moteur de recherche") : recherche
/// desormais par PERTINENCE (index FULLTEXT MySQL, voir la migration dans
/// db_init.rs) au lieu d'un simple LIKE '%...%' qui ne classait rien et ne
/// renvoyait les resultats que par date de modification. Le LIKE reste en
/// repli (mots trop courts pour l'index -- ft_min_word_len exclut les mots
/// de moins de 4 caracteres --, ou correspondance au milieu d'un mot que
/// FULLTEXT en mode prefixe ne trouve pas) pour ne jamais renvoyer moins de
/// resultats qu'avant ce changement.
fn wiki_rechercher(pool: &DbPool, q: &str) -> Vec<Value> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // FIX (demande utilisateur : "je veux que la recherche soit faite en
    // fonction du titre") -- recherche STRICTEMENT par titre d'abord : si
    // au moins un titre correspond, ce sont les SEULS resultats renvoyes
    // (un article qui ne matche que par son contenu, souvent plus long
    // donc plus susceptible de contenir n'importe quel mot au hasard,
    // n'est plus mélangé et ne noie plus les vrais résultats pertinents).
    // Le contenu ne sert de repli que si AUCUN titre ne correspond.
    let motif_titre = format!("%{}%", q.trim());
    let rows_titre: Vec<(i64, String, String, String, String, i64)> = mysql::prelude::Queryable::exec_map(
        &mut conn,
        "SELECT id, titre, contenu, auteur_nom, DATE_FORMAT(maj, '%Y-%m-%d %H:%i'), vues \
         FROM wiki_pages WHERE titre LIKE ? ORDER BY maj DESC LIMIT 100",
        (&motif_titre,),
        |(id, titre, contenu, auteur_nom, maj, vues): (i64, String, String, String, String, i64)| {
            (id, titre, contenu, auteur_nom, maj, vues)
        },
    )
    .unwrap_or_default();
    if !rows_titre.is_empty() {
        return rows_titre.into_iter().map(vers_item_wiki).collect();
    }

    if let Some(requete) = requete_fulltext(q) {
        let rows: Vec<(i64, String, String, String, String, i64)> = mysql::prelude::Queryable::exec_map(
            &mut conn,
            "SELECT id, titre, contenu, auteur_nom, DATE_FORMAT(maj, '%Y-%m-%d %H:%i'), vues \
             FROM wiki_pages WHERE MATCH(titre, contenu) AGAINST(? IN BOOLEAN MODE) \
             ORDER BY MATCH(titre, contenu) AGAINST(? IN BOOLEAN MODE) DESC LIMIT 100",
            (&requete, &requete),
            |(id, titre, contenu, auteur_nom, maj, vues): (i64, String, String, String, String, i64)| {
                (id, titre, contenu, auteur_nom, maj, vues)
            },
        )
        .unwrap_or_default();
        if !rows.is_empty() {
            return rows.into_iter().map(vers_item_wiki).collect();
        }
    }

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
    rows.into_iter().map(vers_item_wiki).collect()
}

fn vers_item_wiki((id, titre, contenu, auteur_nom, maj, vues): (i64, String, String, String, String, i64)) -> Value {
    json!({
        "id": id,
        "titre": titre,
        "extrait": extrait(&contenu, EXTRAIT_LEN),
        "auteur_nom": auteur_nom,
        "maj": maj,
        "vues": vues,
    })
}

fn wiki_liste(pool: &DbPool, q: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let items = wiki_rechercher(pool, q);
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
// FAQ -- DESACTIVEE (voir garde-fou "/api/recherche/faq" dans handle()
// qui court-circuite tout appel a ces fonctions). Code garde pour une
// eventuelle reactivation future plutot que supprime.
// ══════════════════════════════════════════════════════════════════

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
        ("{{T_CAT_APPS}}", Cle::RechCatApps),
        ("{{T_CAT_ACTUALITES}}", Cle::RechCatActualites),
        ("{{T_CAT_WIKI}}", Cle::RechCatWiki),
        ("{{T_CAT_FICHIERS}}", Cle::RechCatFichiers),
        ("{{T_CAT_WIKIPEDIA}}", Cle::RechCatWikipedia),
        ("{{T_CAT_EXTENSION}}", Cle::RechCatExtension),
    ]);
    // I18N.xxx cote JS : uniquement les libelles qui ont besoin d'etre
    // recomposes dynamiquement (ex: "Afficher {n} resultat(s) de plus",
    // qui depend du nombre de resultats calcule cote client) -- tout le
    // reste passe par les placeholders {{T_...}} statiques ci-dessus,
    // deja substitues cote serveur. json!() echappe correctement guillemets/
    // apostrophes pour toutes les langues (ar/zh/ja compris).
    // MON_ID/MON_PRIVILEGE : utilisees cote JS uniquement pour l'affichage
    // (afficher les boutons modifier/supprimer sur un article dont on est
    // l'auteur, ou les boutons d'ecriture FAQ pour un compte de confiance)
    // -- jamais une source de verite, le serveur revalide tout dans
    // wiki_save/wiki_delete/faq_save/faq_delete.
    let i18n_js = json!({
        "enCours": i18n::t(langue, Cle::RechEnCours),
        "invite": i18n::t(langue, Cle::RechInvite),
        "afficherMoins": i18n::t(langue, Cle::RechAfficherMoins),
        "afficherPlusSing": i18n::t(langue, Cle::RechAfficherPlusSing),
        "afficherPlusPlur": i18n::t(langue, Cle::RechAfficherPlusPlur),
        "telecharger": i18n::t(langue, Cle::RechTelecharger),
        "erreurReseau": i18n::t(langue, Cle::RechErreurReseau),
    });
    html.replacen(
        "{{I18N_JS}}",
        &format!("const I18N = {}; const MON_ID = {}; const MON_PRIVILEGE = {};", i18n_js, uid, privilege),
        1,
    )
}
