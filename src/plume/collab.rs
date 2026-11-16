// ══════════════════════════════════════════════════════════════════
// Plume : coedition en temps reel (EN TEST)
// ══════════════════════════════════════════════════════════════════
// Plume (static/plume/) est une suite bureautique qui tourne entierement
// dans le navigateur. Pour ecrire a plusieurs dans un meme document, les
// navigateurs ont besoin d'un point de rendez-vous : c'est le seul role
// de ce module.
//
// Le serveur ne comprend PAS le contenu des documents : il garde, par
// session, une copie du document (JSON opaque fourni par un client) et
// la suite ordonnee des modifications ("steps", JSON opaque aussi). Les
// clients envoient leurs modifications avec le numero de version qu'ils
// connaissent ; si ce numero est a jour elles sont ajoutees a la suite,
// sinon le client recoit celles des autres, les fusionne, et renvoie
// les siennes (protocole prosemirror-collab, par interrogation).
//
// Tout est en memoire : rien en base, rien sur disque. Un redemarrage
// de VEX oublie les sessions (les clients les recreent a partir de leur
// copie locale). Une session sans activite depuis 24 h est supprimee.
//
// Acces : creer ou rejoindre une session exige d'etre connecte a VEX
// (meme verification allegee que Viso). Le serveur remet alors une cle
// aleatoire propre au participant, exigee pour toutes les autres
// actions -- une expiration du cookie en cours de redaction ne coupe
// donc pas la session.
//
// Une seule route : POST /api/plume/collab, corps JSON {"action": ...}.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response};

use crate::appeldb::DbPool;

/// Taille maximale d'une requete (un document avec ses images integrees).
const CORPS_MAX: u64 = 6 * 1024 * 1024;
/// Nombre maximal de sessions gardees en memoire en meme temps.
const SALLES_MAX: usize = 40;
/// Duree de vie d'une session sans aucune activite.
const INACTIVITE_MAX: Duration = Duration::from_secs(24 * 3600);
/// Un participant qui ne donne plus signe de vie disparait de la liste.
const PRESENCE_MAX: Duration = Duration::from_secs(8);
/// Nombre maximal de modifications acceptees dans un seul envoi.
const STEPS_PAR_ENVOI_MAX: usize = 2000;
/// Au-dela, le serveur demande une copie a jour a un client pour
/// raccourcir la suite (les nouveaux arrivants chargent plus vite).
const STEPS_AVANT_COPIE: usize = 200;
/// Nombre maximal de participants par session.
const PARTICIPANTS_MAX: usize = 30;

struct Participant {
    cle: String,
    nom: String,
    ancre: Option<i64>,
    tete: Option<i64>,
    vu: Instant,
}

struct Salle {
    /// Copie du document a la version `version_copie`.
    copie: Value,
    version_copie: u64,
    /// Modifications depuis la copie : (step, numero du participant).
    steps: Vec<(Value, u64)>,
    /// Styles, mise en page, en-tetes, commentaires... (dernier ecrit gagne).
    meta: Value,
    version_meta: u64,
    meta_par: u64,
    prochain_client: u64,
    participants: HashMap<u64, Participant>,
    activite: Instant,
}

impl Salle {
    fn nouvelle(copie: Value) -> Self {
        Salle {
            copie,
            version_copie: 0,
            steps: Vec::new(),
            meta: Value::Null,
            version_meta: 0,
            meta_par: 0,
            prochain_client: 1,
            participants: HashMap::new(),
            activite: Instant::now(),
        }
    }

    fn version(&self) -> u64 {
        self.version_copie + self.steps.len() as u64
    }

    fn ajouter_participant(&mut self, nom: &str) -> (u64, String) {
        let client = self.prochain_client;
        self.prochain_client += 1;
        let cle = cle_aleatoire(16);
        self.participants.insert(
            client,
            Participant { cle: cle.clone(), nom: nom.to_string(), ancre: None, tete: None, vu: Instant::now() },
        );
        (client, cle)
    }

    fn actifs(&self) -> usize {
        self.participants.values().filter(|p| p.vu.elapsed() < PRESENCE_MAX).count()
    }
}

static SALLES: Mutex<Option<HashMap<String, Salle>>> = Mutex::new(None);

fn cle_aleatoire(octets: usize) -> String {
    let mut buf = vec![0u8; octets];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

fn id_valide(id: &str) -> bool {
    (8..=40).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn refus(code: &str) -> Value {
    json!({"ok": false, "error": code})
}

fn json_response(v: &Value, status: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(v.to_string().into_bytes())
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        .with_chunked_threshold(usize::MAX)
}

pub fn handle(pool: &DbPool, request: &mut Request) -> Response<Cursor<Vec<u8>>> {
    if *request.method() != Method::Post {
        return json_response(&refus("methode"), 405);
    }
    let Some(corps) = crate::utils::lire_corps(request, CORPS_MAX) else {
        return json_response(&refus("toobig"), 413);
    };
    let Ok(req) = serde_json::from_str::<Value>(&corps) else {
        return json_response(&refus("json"), 400);
    };
    let action = req.get("action").and_then(|v| v.as_str()).unwrap_or("");

    // Creer ou rejoindre : il faut etre connecte a VEX. Le nom affiche
    // aux autres est celui du compte, pas celui envoye par le navigateur.
    let nom_compte = if action == "create" || action == "join" {
        let cookie = crate::viso::viso::lire_cookie(request, "connexion_cookie");
        match crate::viso::viso::auth_viso(pool, &cookie) {
            Some((_, nom, _)) => Some(nom),
            None => return json_response(&refus("auth"), 200),
        }
    } else {
        None
    };

    let mut verrou = SALLES.lock().unwrap_or_else(|e| e.into_inner());
    let salles = verrou.get_or_insert_with(HashMap::new);
    json_response(&traiter(salles, action, &req, nom_compte.as_deref()), 200)
}

/// Coeur du protocole, sans reseau ni base (teste plus bas).
fn traiter(salles: &mut HashMap<String, Salle>, action: &str, req: &Value, nom_compte: Option<&str>) -> Value {
    let id = req.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let nom_demande = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let nom: String = nom_compte
        .filter(|n| !n.is_empty())
        .unwrap_or(nom_demande)
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    let nom = if nom.trim().is_empty() { "Invité".to_string() } else { nom };

    match action {
        "create" => {
            let Some(doc) = req.get("doc").filter(|d| d.is_object()) else {
                return refus("doc");
            };
            salles.retain(|_, s| s.activite.elapsed() < INACTIVITE_MAX);
            // Un client peut recreer SA session (meme identifiant) apres un
            // redemarrage du serveur ; jamais ecraser une session vivante.
            let id = if id.is_empty() { cle_aleatoire(9) } else { id.to_string() };
            if !id_valide(&id) {
                return refus("id");
            }
            if salles.contains_key(&id) {
                return refus("exists");
            }
            if salles.len() >= SALLES_MAX {
                return refus("full");
            }
            let mut salle = Salle::nouvelle(doc.clone());
            let (client, cle) = salle.ajouter_participant(&nom);
            salles.insert(id.clone(), salle);
            json!({"ok": true, "id": id, "client": client, "key": cle, "version": 0})
        }

        "join" => {
            let Some(salle) = salles.get_mut(id) else {
                return refus("gone");
            };
            if salle.actifs() >= PARTICIPANTS_MAX {
                return refus("full");
            }
            salle.activite = Instant::now();
            let (client, cle) = salle.ajouter_participant(&nom);
            json!({
                "ok": true,
                "client": client,
                "key": cle,
                "snapshot": salle.copie,
                "snapVersion": salle.version_copie,
                "steps": salle.steps.iter().map(|(s, c)| json!({"s": s, "c": c})).collect::<Vec<_>>(),
                "version": salle.version(),
                "meta": salle.meta,
                "metaVersion": salle.version_meta,
            })
        }

        "sync" | "snapshot" | "leave" => {
            let Some(salle) = salles.get_mut(id) else {
                return refus("gone");
            };
            let client = req.get("client").and_then(|v| v.as_u64()).unwrap_or(0);
            let cle = req.get("key").and_then(|v| v.as_str()).unwrap_or("");
            match salle.participants.get(&client) {
                Some(p) if !cle.is_empty() && p.cle == cle => {}
                // Session recreee apres un redemarrage : le participant
                // n'y est pas encore connu, il doit la rejoindre a nouveau.
                _ => return refus("rejoin"),
            }
            if action == "leave" {
                salle.participants.remove(&client);
                return json!({"ok": true});
            }
            salle.activite = Instant::now();
            let version_client = req.get("version").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
            if version_client < salle.version_copie || version_client > salle.version() {
                return refus("resync");
            }

            if action == "snapshot" {
                // Le client affirme : "voici le document a la version N".
                let Some(doc) = req.get("doc").filter(|d| d.is_object()) else {
                    return refus("doc");
                };
                if version_client > salle.version_copie {
                    let garder = (version_client - salle.version_copie) as usize;
                    salle.steps.drain(..garder);
                    salle.copie = doc.clone();
                    salle.version_copie = version_client;
                }
                return json!({"ok": true, "snapVersion": salle.version_copie});
            }

            // sync : 1) presence
            if let Some(p) = salle.participants.get_mut(&client) {
                p.vu = Instant::now();
                p.ancre = req.pointer("/sel/a").and_then(|v| v.as_i64());
                p.tete = req.pointer("/sel/h").and_then(|v| v.as_i64());
            }
            // 2) modifications du client, acceptees seulement s'il est a jour
            let a_envoye = match req.get("steps").and_then(|v| v.as_array()) {
                Some(steps) if !steps.is_empty() => {
                    if steps.len() > STEPS_PAR_ENVOI_MAX {
                        return refus("toobig");
                    }
                    if version_client == salle.version() {
                        salle.steps.extend(steps.iter().map(|s| (s.clone(), client)));
                    }
                    true
                }
                _ => false,
            };
            // 3) styles / mise en page / commentaires : dernier ecrit gagne
            if let Some(meta) = req.get("meta").filter(|m| m.is_object()) {
                salle.meta = meta.clone();
                salle.version_meta += 1;
                salle.meta_par = client;
            }
            let meta_connue = req.get("metaVersion").and_then(|v| v.as_u64()).unwrap_or(0);
            let envoyer_meta = salle.version_meta > meta_connue && salle.meta_par != client && !salle.meta.is_null();

            // 4) tout ce que le client n'a pas encore (y compris ses
            //    propres modifications s'il vient d'etre accepte)
            let depuis = (version_client - salle.version_copie) as usize;
            let steps: Vec<Value> = salle.steps[depuis..].iter().map(|(s, c)| json!({"s": s, "c": c})).collect();
            salle.participants.retain(|_, p| p.vu.elapsed() < Duration::from_secs(600));
            let pairs: Vec<Value> = salle
                .participants
                .iter()
                .filter(|(c, p)| **c != client && p.vu.elapsed() < PRESENCE_MAX)
                .map(|(c, p)| json!({"c": c, "name": p.nom, "a": p.ancre, "h": p.tete}))
                .collect();
            let mut rep = json!({
                "ok": true,
                "version": salle.version(),
                "steps": steps,
                "peers": pairs,
                "metaVersion": salle.version_meta,
                "wantSnapshot": salle.steps.len() > STEPS_AVANT_COPIE && !a_envoye,
            });
            if envoyer_meta {
                rep["meta"] = salle.meta.clone();
            }
            rep
        }

        _ => refus("action"),
    }
}

#[cfg(test)]
mod tests_collab {
    use super::*;

    fn appel(salles: &mut HashMap<String, Salle>, v: Value) -> Value {
        let action = v["action"].as_str().unwrap().to_string();
        traiter(salles, &action, &v, None)
    }

    #[test]
    fn deux_participants_s_echangent_leurs_modifications() {
        let mut salles = HashMap::new();
        let a = appel(&mut salles, json!({"action": "create", "doc": {"doc": 1}, "name": "Ana"}));
        assert_eq!(a["ok"], true);
        let id = a["id"].as_str().unwrap().to_string();
        let b = appel(&mut salles, json!({"action": "join", "id": id, "name": "Bo"}));
        assert_eq!(b["snapVersion"], 0);
        assert_ne!(a["client"], b["client"]);

        // Ana envoie deux modifications depuis la version 0.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": a["key"], "version": 0, "steps": [{"n": 1}, {"n": 2}]}));
        assert_eq!(r["version"], 2);
        assert_eq!(r["steps"].as_array().unwrap().len(), 2);

        // Bo, en retard, envoie la sienne : refusee, il recoit celles d'Ana.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": b["client"], "key": b["key"], "version": 0, "steps": [{"n": 9}], "sel": {"a": 3, "h": 5}}));
        assert_eq!(r["version"], 2);
        assert_eq!(r["steps"].as_array().unwrap().len(), 2);
        assert_eq!(r["steps"][0]["c"], a["client"]);
        // Une fois a jour, sa modification est acceptee et lui revient.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": b["client"], "key": b["key"], "version": 2, "steps": [{"n": 9}], "sel": {"a": 3, "h": 5}}));
        assert_eq!(r["version"], 3);
        assert_eq!(r["steps"][0]["c"], b["client"]);

        // Ana voit Bo et son curseur.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": a["key"], "version": 2}));
        assert_eq!(r["steps"].as_array().unwrap().len(), 1);
        assert_eq!(r["peers"][0]["name"], "Bo");
        assert_eq!(r["peers"][0]["h"], 5);
    }

    #[test]
    fn cle_exigee_et_copie_raccourcit_la_suite() {
        let mut salles = HashMap::new();
        let a = appel(&mut salles, json!({"action": "create", "doc": {"v": 0}}));
        let id = a["id"].as_str().unwrap().to_string();
        // Sans la bonne cle : refuse.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": "fausse", "version": 0}));
        assert_eq!(r["ok"], false);
        appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": a["key"], "version": 0, "steps": [{"n": 1}, {"n": 2}, {"n": 3}]}));
        let r = appel(&mut salles, json!({"action": "snapshot", "id": id, "client": a["client"], "key": a["key"], "version": 2, "doc": {"v": 2}}));
        assert_eq!(r["snapVersion"], 2);
        // Un nouvel arrivant recoit la copie v2 + la seule modification restante.
        let b = appel(&mut salles, json!({"action": "join", "id": id}));
        assert_eq!(b["snapshot"]["v"], 2);
        assert_eq!(b["steps"].as_array().unwrap().len(), 1);
        assert_eq!(b["version"], 3);
        // Un client reste avant la copie doit se resynchroniser.
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": a["key"], "version": 1}));
        assert_eq!(r["error"], "resync");
    }

    #[test]
    fn meta_dernier_ecrit_gagne_et_session_inconnue() {
        let mut salles = HashMap::new();
        let a = appel(&mut salles, json!({"action": "create", "doc": {}}));
        let id = a["id"].as_str().unwrap().to_string();
        let b = appel(&mut salles, json!({"action": "join", "id": id}));
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": a["client"], "key": a["key"], "version": 0, "meta": {"title": "T"}, "metaVersion": 0}));
        assert_eq!(r["metaVersion"], 1);
        assert!(r.get("meta").is_none());
        let r = appel(&mut salles, json!({"action": "sync", "id": id, "client": b["client"], "key": b["key"], "version": 0, "metaVersion": 0}));
        assert_eq!(r["meta"]["title"], "T");
        // Session inconnue, identifiant invalide, recreation interdite si elle existe.
        assert_eq!(appel(&mut salles, json!({"action": "join", "id": "inconnue123"}))["error"], "gone");
        assert_eq!(appel(&mut salles, json!({"action": "create", "id": "../x", "doc": {}}))["error"], "id");
        assert_eq!(appel(&mut salles, json!({"action": "create", "id": id, "doc": {}}))["error"], "exists");
    }
}
