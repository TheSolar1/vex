// ═════════════════════════════════════════════════════════════════
// mail.rs — boîte mail @vexs.me de chaque compte (réception)
//
// Le courrier arrive chez Cloudflare (Email Routing, règle « tout ») qui
// le passe au Worker `vex-mail`. Le Worker envoie le mail brut ici :
//   POST /api/mail/recevoir?a=<destinataire>&de=<expéditeur>
//   en-tête X-Vex-Mail-Secret = contenu de mail_secret.txt
// Adresse d'un compte = pseudo (login.pseudo, sinon login.nom) en minuscules, sans accents,
// limité à [a-z0-9._-], suivi de @vexs.me. Compte inconnu → 404, et le
// Worker refuse le mail.
//
// Page : GET /mail. API (session VEX) : GET /api/mail/liste,
// GET /api/mail/lire?id=, POST /api/mail/supprimer (id=).
// ═════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::function::{build_nav_html, get_theme_attr, NavContext};
use crate::p2p::p2p::NodeState;
use std::sync::{Arc, RwLock};
use mail_parser::MimeHeaders;
use mysql::prelude::Queryable;
use serde_json::{json, Value};
use std::io::{Cursor, Read};
use tiny_http::{Header, Method, Request, Response};

const PAGE_HTML: &str = include_str!("../../static/mail/mail.html");
const DOMAINE: &str = "vexs.me";
const TAILLE_MAX: u64 = 15 * 1024 * 1024;

type Rep = Response<Cursor<Vec<u8>>>;

fn reponse(corps: Vec<u8>, type_mime: &str, statut: u16) -> Rep {
    Response::from_data(corps)
        .with_header(Header::from_bytes("Content-Type", type_mime).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        .with_status_code(statut)
}
fn json_rep(v: &Value, statut: u16) -> Rep {
    reponse(v.to_string().into_bytes(), "application/json; charset=utf-8", statut)
}
fn en_tete(request: &Request, nom: &str) -> String {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(nom))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default()
}

/// Pseudo → partie locale de l'adresse.
pub fn adresse_locale(nom: &str) -> String {
    let mut s = String::new();
    for c in nom.trim().to_lowercase().chars() {
        let c = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            ' ' => '.',
            c => c,
        };
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
            s.push(c);
        }
    }
    s.trim_matches('.').to_string()
}

fn creer_table(pool: &DbPool) {
    if let Ok(mut c) = pool.get_conn() {
        let _ = c.query_drop(
            "CREATE TABLE IF NOT EXISTS `mail_recus` (
               `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
               `user_id` INT NOT NULL,
               `destinataire` VARCHAR(255) NOT NULL,
               `expediteur` VARCHAR(512) NOT NULL,
               `sujet` VARCHAR(1000) NOT NULL DEFAULT '',
               `texte` MEDIUMTEXT,
               `html` MEDIUMTEXT,
               `pieces` TEXT,
               `brut` LONGBLOB,
               `lu` TINYINT NOT NULL DEFAULT 0,
               `recu_le` DATETIME NOT NULL DEFAULT NOW(),
               INDEX (`user_id`, `recu_le`)
             ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        );
        let _ = c.query_drop(
            "CREATE TABLE IF NOT EXISTS `mail_envoyes` (
               `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
               `user_id` INT NOT NULL,
               `expediteur` VARCHAR(255) NOT NULL,
               `destinataire` VARCHAR(255) NOT NULL,
               `sujet` VARCHAR(1000) NOT NULL DEFAULT '',
               `texte` MEDIUMTEXT,
               `id_resend` VARCHAR(100) NOT NULL DEFAULT '',
               `envoye_le` DATETIME NOT NULL DEFAULT NOW(),
               INDEX (`user_id`, `envoye_le`)
             ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        );
    }
}

/// Destinataire → id du compte (comparaison sur le pseudo normalisé).
fn compte_pour(pool: &DbPool, destinataire: &str) -> Option<i64> {
    let dest = destinataire.trim().to_lowercase();
    let (local, domaine) = dest.rsplit_once('@')?;
    if domaine != DOMAINE || local.is_empty() {
        return None;
    }
    let local = local.split('+').next().unwrap_or(local); // pseudo+tag@
    let mut c = pool.get_conn().ok()?;
    // postmaster@ et abuse@ (obligatoires pour un domaine mail) → fondateur
    if local == "postmaster" || local == "abuse" {
        return c.query_first("SELECT id FROM login WHERE privilege = 1 ORDER BY id LIMIT 1").ok().flatten();
    }
    let lignes: Vec<(i64, String)> = c.query("SELECT id, COALESCE(NULLIF(pseudo, ''), nom) FROM login").ok()?;
    lignes.into_iter().find(|(_, nom)| adresse_locale(nom) == local).map(|(id, _)| id)
}

/// Une adresse pseudo@vexs.me est-elle libre ? (aucun compte du central
/// ne la porte déjà, et le nom n'est pas réservé)
pub fn adresse_vexs_libre(pool: &DbPool, local: &str) -> bool {
    if local.len() < 2 || super::reseau::RESERVES.contains(&local) || local == "postmaster" || local == "abuse" {
        return false;
    }
    compte_pour(pool, &format!("{}@{}", local, DOMAINE)).is_none()
}

/// Analyse un mail brut et le range dans la boîte d'un compte.
pub fn stocker_mail(pool: &DbPool, user_id: i64, dest: &str, de: &str, brut: &[u8]) -> Result<(), String> {
    let de = de.to_string();
    let msg = mail_parser::MessageParser::default().parse(brut);
    let (sujet, expediteur, texte, html, pieces) = match &msg {
        Some(m) => {
            let exp = m
                .from()
                .and_then(|a| a.first())
                .map(|a| match (a.name(), a.address()) {
                    (Some(n), Some(ad)) => format!("{} <{}>", n, ad),
                    (None, Some(ad)) => ad.to_string(),
                    (Some(n), None) => n.to_string(),
                    _ => de.clone(),
                })
                .unwrap_or_else(|| de.clone());
            let pieces: Vec<String> = m
                .attachments()
                .map(|p| p.attachment_name().unwrap_or("piece-jointe").to_string())
                .collect();
            (
                m.subject().unwrap_or("").to_string(),
                exp,
                m.body_text(0).map(|t| t.to_string()).unwrap_or_default(),
                m.body_html(0).map(|t| t.to_string()).unwrap_or_default(),
                pieces,
            )
        }
        None => (String::new(), de.clone(), String::from_utf8_lossy(brut).to_string(), String::new(), vec![]),
    };
    let sujet: String = sujet.chars().take(990).collect();
    let expediteur: String = expediteur.chars().take(500).collect();
    let mut c = pool.get_conn().map_err(|_| "base".to_string())?;
    c.exec_drop(
        "INSERT INTO mail_recus (user_id, destinataire, expediteur, sujet, texte, html, pieces, brut)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (user_id, dest.to_lowercase(), expediteur, sujet, texte, html, json!(pieces).to_string(), brut),
    )
    .map_err(|e| {
        eprintln!("[mail] insertion impossible : {}", e);
        "base".to_string()
    })
}

fn recevoir(pool: &DbPool, request: &mut Request, ns: &Arc<RwLock<NodeState>>) -> Rep {
    let secret_attendu = std::fs::read_to_string("mail_secret.txt").unwrap_or_default();
    let secret_attendu = secret_attendu.trim();
    let secret = en_tete(request, "X-Vex-Mail-Secret");
    let egal = secret.len() == secret_attendu.len()
        && secret.bytes().zip(secret_attendu.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0;
    if secret_attendu.len() < 32 || !egal {
        return json_rep(&json!({"ok": false, "erreur": "interdit"}), 403);
    }
    let requete = request.url().split('?').nth(1).unwrap_or("").to_string();
    let params = crate::utils::parse_query(&format!("?{}", requete));
    let dest = params.get("a").cloned().unwrap_or_default();
    let de = params.get("de").cloned().unwrap_or_default();

    // Où va ce mail ? compte local (vexs.me ou alias a.vexs.me) ou autre nœud (m.vexs.me)
    enum Cible { Local(i64), Noeud }
    let dest_min = dest.trim().to_lowercase();
    let domaine = dest_min.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    let cible = if domaine == DOMAINE {
        compte_pour(pool, &dest_min).map(Cible::Local)
    } else if domaine == super::reseau::DOMAINE_ALIAS || domaine == super::reseau::DOMAINE_NOEUDS {
        match super::reseau::trouver(pool, &dest_min) {
            Some((t, uid, _, _)) if t == "alias" => Some(Cible::Local(uid)),
            Some((t, _, _, _)) if t == "noeud" => Some(Cible::Noeud),
            _ => None,
        }
    } else {
        None
    };
    let Some(cible) = cible else {
        return json_rep(&json!({"ok": false, "erreur": "adresse inconnue"}), 404);
    };

    let mut brut = Vec::new();
    if request.as_reader().take(TAILLE_MAX + 1).read_to_end(&mut brut).is_err() {
        return json_rep(&json!({"ok": false, "erreur": "lecture"}), 400);
    }
    if brut.len() as u64 > TAILLE_MAX {
        return json_rep(&json!({"ok": false, "erreur": "trop gros"}), 413);
    }
    match cible {
        Cible::Local(user_id) => match stocker_mail(pool, user_id, &dest_min, &de, &brut) {
            Ok(()) => json_rep(&json!({"ok": true}), 200),
            Err(_) => json_rep(&json!({"ok": false, "erreur": "base"}), 503),
        },
        // Mis en file : livré au nœud tout de suite ou plus tard (relances).
        Cible::Noeud => {
            if super::reseau::relayer(pool, ns, &dest_min, &de, &brut) {
                json_rep(&json!({"ok": true}), 200)
            } else {
                json_rep(&json!({"ok": false, "erreur": "base"}), 503)
            }
        }
    }
}

// ── Envoi (Resend, clé dans resend_key.txt, droit « envoi » seulement) ──
const LIMITE_ENVOIS_JOUR: i64 = 20;

fn envois_du_jour(pool: &DbPool, uid: i64) -> i64 {
    pool.get_conn()
        .ok()
        .and_then(|mut c| {
            c.exec_first::<i64, _, _>(
                "SELECT COUNT(*) FROM mail_envoyes WHERE user_id = ? AND envoye_le > NOW() - INTERVAL 1 DAY",
                (uid,),
            )
            .ok()
            .flatten()
        })
        .unwrap_or(0)
}

fn adresse_valide(a: &str) -> bool {
    let a = a.trim();
    a.len() <= 254
        && !a.chars().any(|c| c.is_whitespace() || c.is_control() || c == '<' || c == '>' || c == ',' || c == ';' || c == '"')
        && matches!(a.rsplit_once('@'), Some((l, d)) if !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'))
}

/// Envoi par Resend. Renvoie l'id Resend, ou un message d'erreur lisible.
pub fn envoyer_resend(de: &str, a: &str, sujet: &str, texte: &str, reponse_a: &str) -> Result<String, String> {
    let a = a.trim();
    let sujet = sujet.replace(['\r', '\n'], " ");
    if !adresse_valide(a) {
        return Err("Adresse du destinataire invalide.".into());
    }
    if sujet.trim().is_empty() || sujet.chars().count() > 300 || texte.trim().is_empty() || texte.len() > 100_000 {
        return Err("Objet ou message vide ou trop long.".into());
    }
    let cle = std::fs::read_to_string("resend_key.txt").unwrap_or_default();
    let cle = cle.trim();
    if cle.is_empty() {
        return Err("Envoi non configuré sur ce serveur.".into());
    }
    let corps = json!({"from": de, "to": [a], "subject": sujet, "text": texte, "reply_to": reponse_a});
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(20)).build();
    match agent.post("https://api.resend.com/emails").set("Authorization", &format!("Bearer {}", cle)).send_json(corps) {
        Ok(r) => Ok(r
            .into_json::<Value>()
            .ok()
            .and_then(|v| v.get("id").and_then(|x| x.as_str()).map(String::from))
            .unwrap_or_default()),
        Err(ureq::Error::Status(code, r)) => {
            let detail = r.into_string().unwrap_or_default();
            eprintln!("[mail] Resend a refusé ({}) : {}", code, detail);
            let msg = serde_json::from_str::<Value>(&detail)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| format!("erreur {}", code));
            Err(format!("Envoi refusé : {}", msg))
        }
        Err(e) => {
            eprintln!("[mail] Resend injoignable : {}", e);
            Err("Service d'envoi injoignable, réessaie plus tard.".into())
        }
    }
}

fn envoyer(pool: &DbPool, ns: &Arc<RwLock<NodeState>>, uid: i64, pseudo: &str, adresse: &str, params: &std::collections::HashMap<String, String>) -> Rep {
    let a = params.get("a").map(|s| s.trim().to_string()).unwrap_or_default();
    let sujet: String = params.get("sujet").map(|s| s.replace(['\r', '\n'], " ")).unwrap_or_default();
    let texte = params.get("texte").cloned().unwrap_or_default();
    if !adresse_valide(&a) {
        return json_rep(&json!({"ok": false, "erreur": "Adresse du destinataire invalide."}), 400);
    }
    if sujet.trim().is_empty() || sujet.chars().count() > 300 {
        return json_rep(&json!({"ok": false, "erreur": "Objet vide ou trop long (300 caractères max)."}), 400);
    }
    if texte.trim().is_empty() || texte.len() > 100_000 {
        return json_rep(&json!({"ok": false, "erreur": "Message vide ou trop long."}), 400);
    }
    if envois_du_jour(pool, uid) >= LIMITE_ENVOIS_JOUR {
        return json_rep(
            &json!({"ok": false, "erreur": format!("Limite atteinte : {} mails par 24 h.", LIMITE_ENVOIS_JOUR)}),
            429,
        );
    }
    let resultat = if super::reseau::est_central() {
        let nom_affiche: String = pseudo.chars().filter(|c| !c.is_control() && *c != '"' && *c != '<' && *c != '>').collect();
        envoyer_resend(&format!("{} <{}>", nom_affiche, adresse), &a, &sujet, &texte, adresse)
    } else {
        // Autre nœud : l'envoi passe par le central (adresse m.vexs.me active requise).
        super::reseau::noeud_envoyer(ns, uid, &a, &sujet, &texte).map(|_| String::new())
    };
    let id_resend = match resultat {
        Ok(id) => id,
        Err(e) => return json_rep(&json!({"ok": false, "erreur": e}), 502),
    };
    if let Ok(mut c) = pool.get_conn() {
        let _ = c.exec_drop(
            "INSERT INTO mail_envoyes (user_id, expediteur, destinataire, sujet, texte, id_resend) VALUES (?, ?, ?, ?, ?, ?)",
            (uid, adresse, &a, &sujet, &texte, id_resend),
        );
    }
    json_rep(&json!({"ok": true}), 200)
}

fn lire_json(request: &mut Request, max: u64) -> Value {
    let mut corps = Vec::new();
    let _ = request.as_reader().take(max).read_to_end(&mut corps);
    serde_json::from_slice(&corps).unwrap_or(json!({}))
}

pub fn handle(pool: &DbPool, request: &mut Request, ns: &Arc<RwLock<NodeState>>) -> Rep {
    static TABLES: std::sync::Once = std::sync::Once::new();
    TABLES.call_once(|| {
        creer_table(pool);
        super::reseau::creer_tables(pool);
    });
    super::reseau::lancer_relances(pool, ns);
    let central = super::reseau::est_central();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    let methode = request.method().clone();

    if path == "/api/mail/recevoir" {
        if methode != Method::Post || !central {
            return json_rep(&json!({"ok": false}), 405);
        }
        return recevoir(pool, request, ns);
    }
    // Domaine des adresses de ce nœud (formulaire d'inscription)
    if path == "/api/mail/domaine" {
        let d = if central { DOMAINE } else { super::reseau::DOMAINE_NOEUDS };
        return json_rep(&json!({"ok": true, "domaine": d, "central": central}), 200);
    }
    // Échanges signés entre nœuds (pas de session)
    if path.starts_with("/api/mail/reseau/") && methode == Method::Post && path != "/api/mail/reseau/demander" {
        let (v, code) = match (path.as_str(), central) {
            ("/api/mail/reseau/demande", true) => super::reseau::central_demande(pool, &lire_json(request, 64 * 1024)),
            ("/api/mail/reseau/statut", true) => super::reseau::central_statut(pool, &lire_json(request, 64 * 1024)),
            ("/api/mail/reseau/envoyer", true) => {
                super::reseau::central_envoyer(pool, &lire_json(request, 512 * 1024), LIMITE_ENVOIS_JOUR)
            }
            ("/api/mail/reseau/livrer", false) => super::reseau::noeud_livrer(pool, &lire_json(request, 25 * 1024 * 1024)),
            _ => (json!({"ok": false, "erreur": "introuvable"}), 404),
        };
        return json_rep(&v, code);
    }

    let cookie = crate::access_control::get_cookie(request, "connexion_cookie");
    let ua = en_tete(request, "User-Agent");
    let (ip, _) = crate::utils::client_ip(request);
    let session = crate::c::verifier_session(pool, &cookie, &ip, &ua);

    if !session.connecte {
        if path.starts_with("/api/") {
            return json_rep(&json!({"ok": false, "erreur": "non connecté"}), 401);
        }
        return Response::from_data(Vec::new())
            .with_header(Header::from_bytes("Location", "/login").unwrap())
            .with_status_code(302);
    }
    let uid = session.user_id;
    let pseudo: String = pool
        .get_conn()
        .ok()
        .and_then(|mut c| {
            c.exec_first::<String, _, _>("SELECT COALESCE(NULLIF(pseudo, ''), nom) FROM login WHERE id = ?", (uid,))
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| session.user_nom.clone());
    // Adresse principale : pseudo@vexs.me sur le central ; sur un autre
    // nœud, l'adresse m.vexs.me une fois validée par le central.
    let (adresse, statut_reseau) = if central {
        (format!("{}@{}", adresse_locale(&pseudo), DOMAINE), "actif".to_string())
    } else {
        let (a, s) = super::reseau::noeud_etat(pool, ns, uid);
        // Compte créé avec « Utiliser mon adresse VEX » (email = x@m.vexs.me) :
        // la demande part toute seule au premier passage.
        let suffixe = format!("@{}", super::reseau::DOMAINE_NOEUDS);
        if s == "aucune" && session.user_email.to_lowercase().ends_with(&suffixe) {
            let local = session.user_email.to_lowercase().trim_end_matches(&suffixe).to_string();
            let (v, _) = super::reseau::noeud_demander(pool, ns, uid, &pseudo, &local);
            (
                v.get("adresse").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                v.get("statut").and_then(|x| x.as_str()).unwrap_or("aucune").to_string(),
            )
        } else {
            (a, s)
        }
    };
    let fondateur = central && session.user_privilege == 1;
    let corps_form = |request: &mut Request| {
        let corps = crate::utils::lire_corps(request, 8 * 1024).unwrap_or_default();
        crate::utils::parse_query(&format!("?{}", corps))
    };

    match (path.as_str(), &methode) {
        ("/api/mail/envoyer", Method::Post) => {
            if !central && statut_reseau != "actif" {
                return json_rep(&json!({"ok": false, "erreur": "Ton adresse mail n'est pas encore validée."}), 403);
            }
            let corps = crate::utils::lire_corps(request, 300 * 1024).unwrap_or_default();
            let params = crate::utils::parse_query(&format!("?{}", corps));
            envoyer(pool, ns, uid, &pseudo, &adresse, &params)
        }
        ("/api/mail/adresses", Method::Get) => {
            let alias = if central { super::reseau::alias_du_compte(pool, uid) } else { vec![] };
            json_rep(
                &json!({"ok": true, "central": central, "principale": adresse, "statut": statut_reseau,
                        "alias": alias, "max_alias": super::reseau::MAX_ALIAS, "fondateur": fondateur,
                        "suggestion": adresse_locale(&pseudo), "domaine_noeuds": super::reseau::DOMAINE_NOEUDS}),
                200,
            )
        }
        ("/api/mail/alias/ajouter", Method::Post) if central => {
            let p = corps_form(request);
            match super::reseau::ajouter_alias(pool, uid, &pseudo, p.get("nom").map(|s| s.as_str()).unwrap_or("")) {
                Ok(a) => json_rep(&json!({"ok": true, "adresse": a}), 200),
                Err(e) => json_rep(&json!({"ok": false, "erreur": e}), 400),
            }
        }
        ("/api/mail/alias/supprimer", Method::Post) if central => {
            let p = corps_form(request);
            let ok = super::reseau::supprimer_alias(pool, uid, p.get("adresse").map(|s| s.as_str()).unwrap_or(""));
            json_rep(&json!({"ok": ok}), 200)
        }
        ("/api/mail/reseau/demander", Method::Post) if !central => {
            let p = corps_form(request);
            let local = p.get("nom").cloned().unwrap_or_else(|| adresse_locale(&pseudo));
            let (v, code) = super::reseau::noeud_demander(pool, ns, uid, &pseudo, &local);
            json_rep(&v, code)
        }
        ("/api/mail/admin/attente", Method::Get) if fondateur => {
            json_rep(&json!({"ok": true, "demandes": super::reseau::liste_attente(pool)}), 200)
        }
        ("/api/mail/admin/decider", Method::Post) if fondateur => {
            let p = corps_form(request);
            let ok = super::reseau::decider(
                pool,
                p.get("adresse").map(|s| s.as_str()).unwrap_or(""),
                p.get("decision").map(|s| s.as_str()).unwrap_or(""),
            );
            json_rep(&json!({"ok": ok}), 200)
        }
        ("/api/mail/envoyes", Method::Get) => {
            let Ok(mut c) = pool.get_conn() else { return json_rep(&json!({"ok": false}), 503) };
            let lignes: Vec<(i64, String, String, String)> = c
                .exec(
                    "SELECT id, destinataire, sujet, DATE_FORMAT(envoye_le, '%Y-%m-%d %H:%i')
                     FROM mail_envoyes WHERE user_id = ? ORDER BY id DESC LIMIT 500",
                    (uid,),
                )
                .unwrap_or_default();
            let mails: Vec<Value> = lignes
                .into_iter()
                .map(|(id, a, sujet, date)| json!({"id": id, "a": a, "sujet": sujet, "date": date}))
                .collect();
            let restants = LIMITE_ENVOIS_JOUR - envois_du_jour(pool, uid);
            json_rep(&json!({"ok": true, "mails": mails, "restants": restants.max(0)}), 200)
        }
        ("/api/mail/envoye", Method::Get) => {
            let requete = url.split('?').nth(1).unwrap_or("").to_string();
            let params = crate::utils::parse_query(&format!("?{}", requete));
            let id: i64 = params.get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
            let Ok(mut c) = pool.get_conn() else { return json_rep(&json!({"ok": false}), 503) };
            let ligne: Option<(String, String, String, String)> = c
                .exec_first(
                    "SELECT destinataire, sujet, texte, DATE_FORMAT(envoye_le, '%Y-%m-%d %H:%i')
                     FROM mail_envoyes WHERE id = ? AND user_id = ?",
                    (id, uid),
                )
                .unwrap_or(None);
            let Some((a, sujet, texte, date)) = ligne else {
                return json_rep(&json!({"ok": false, "erreur": "introuvable"}), 404);
            };
            json_rep(&json!({"ok": true, "de": adresse, "a": a, "sujet": sujet, "texte": texte, "date": date}), 200)
        }
        ("/api/mail/liste", Method::Get) => {
            let Ok(mut c) = pool.get_conn() else { return json_rep(&json!({"ok": false}), 503) };
            let lignes: Vec<(i64, String, String, i64, String)> = c
                .exec(
                    "SELECT id, expediteur, sujet, lu, DATE_FORMAT(recu_le, '%Y-%m-%d %H:%i')
                     FROM mail_recus WHERE user_id = ? ORDER BY id DESC LIMIT 500",
                    (uid,),
                )
                .unwrap_or_default();
            let mails: Vec<Value> = lignes
                .into_iter()
                .map(|(id, de, sujet, lu, date)| json!({"id": id, "de": de, "sujet": sujet, "lu": lu != 0, "date": date}))
                .collect();
            json_rep(&json!({"ok": true, "adresse": adresse, "mails": mails}), 200)
        }
        ("/api/mail/lire", Method::Get) => {
            let requete = url.split('?').nth(1).unwrap_or("").to_string();
            let params = crate::utils::parse_query(&format!("?{}", requete));
            let id: i64 = params.get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
            let Ok(mut c) = pool.get_conn() else { return json_rep(&json!({"ok": false}), 503) };
            let ligne: Option<(String, String, String, Option<String>, Option<String>, Option<String>, String)> = c
                .exec_first(
                    "SELECT expediteur, destinataire, sujet, texte, html, pieces, DATE_FORMAT(recu_le, '%Y-%m-%d %H:%i')
                     FROM mail_recus WHERE id = ? AND user_id = ?",
                    (id, uid),
                )
                .unwrap_or(None);
            let Some((de, a, sujet, texte, html, pieces, date)) = ligne else {
                return json_rep(&json!({"ok": false, "erreur": "introuvable"}), 404);
            };
            let _ = c.exec_drop("UPDATE mail_recus SET lu = 1 WHERE id = ? AND user_id = ?", (id, uid));
            let pieces: Value = serde_json::from_str(&pieces.unwrap_or_default()).unwrap_or(json!([]));
            json_rep(
                &json!({"ok": true, "de": de, "a": a, "sujet": sujet, "texte": texte.unwrap_or_default(),
                        "html": html.unwrap_or_default(), "pieces": pieces, "date": date}),
                200,
            )
        }
        ("/api/mail/supprimer", Method::Post) => {
            let corps = crate::utils::lire_corps(request, 4096).unwrap_or_default();
            let params = crate::utils::parse_query(&format!("?{}", corps));
            let id: i64 = params.get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
            let Ok(mut c) = pool.get_conn() else { return json_rep(&json!({"ok": false}), 503) };
            let ok = c.exec_drop("DELETE FROM mail_recus WHERE id = ? AND user_id = ?", (id, uid)).is_ok();
            json_rep(&json!({"ok": ok}), 200)
        }
        (p, Method::Get) if p == "/mail" || p == "/mail/" => {
            let ctx = NavContext {
                pool,
                user_id: Some(uid),
                page_key: "mail",
                cookie_val: &cookie,
                remote_ip: &ip,
                user_agent: &ua,
                query_id: None,
                apps: vec![],
                admin_apps: vec![],
            };
            let nav = build_nav_html(&ctx);
            let page = PAGE_HTML
                .replacen("{{NAV_HTML}}", &nav, 1)
                .replace("{{THEME}}", get_theme_attr(pool, uid))
                .replace("{{ADRESSE}}", &crate::function::html_escape(&adresse));
            reponse(page.into_bytes(), "text/html; charset=utf-8", 200)
        }
        _ => json_rep(&json!({"ok": false, "erreur": "introuvable"}), 404),
    }
}
