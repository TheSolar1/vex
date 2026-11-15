// ═════════════════════════════════════════════════════════════════
// reseau.rs — adresses mail des autres nœuds VEX et alias
//
// Le nœud « central » (config.json : "mail": {"central": true}) reçoit
// tout le courrier de vexs.me (Worker Cloudflare) :
//   pseudo@vexs.me    comptes du nœud central (voir mail.rs)
//   nom@a.vexs.me     alias des comptes du nœud central (auto, 3 max)
//   pseudo@m.vexs.me  comptes des AUTRES nœuds : une adresse par compte,
//                     demandée par le nœud, validée à la main par le
//                     fondateur du central ; le courrier est ensuite
//                     transmis (signé) au nœud, avec file d'attente.
// Les autres nœuds ("central_url", défaut https://app.vexs.me) envoient
// aussi leurs mails par le central (Resend, limite par compte).
//
// Messages entre nœuds = enveloppe signée Ed25519 (clé P2P du nœud) :
//   {type, node_id, vex_url, pub_key, ts, charge (JSON en texte), sig}
//   sig = signe("vexmail|type|node_id|ts|sha256(charge)"), charge.nonce aléatoire
// node_id doit valoir SHA256(vex_url + pub_key)[..16] (même règle que
// p2p.rs) : la clé prouve l'identité, sans annuaire préalable.
// Anti-rejeu : |ts| ≤ 5 min et signature à usage unique (mail_nonces).
// ═════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::p2p::p2p::NodeState;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use mysql::prelude::Queryable;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, RwLock};

pub const DOMAINE_NOEUDS: &str = "m.vexs.me";
pub const DOMAINE_ALIAS: &str = "a.vexs.me";
pub const MAX_ALIAS: i64 = 3;
const MAX_DEMANDES_NOEUD_JOUR: i64 = 10;

pub const RESERVES: &[&str] = &[
    "admin", "administrateur", "root", "postmaster", "abuse", "hostmaster", "webmaster", "noreply",
    "no-reply", "support", "contact", "info", "mail", "email", "security", "securite", "vex", "vexs",
    "app", "www", "api", "dev", "system", "systeme", "fondateur", "moderateur", "staff", "help", "aide",
    "billing", "paiement", "facturation", "reseau", "relais", "wfront", "soundex", "office", "justepoids",
];

// ── Configuration ────────────────────────────────────────────────
fn config_mail() -> Value {
    std::fs::read_to_string("config.json")
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("mail").cloned())
        .unwrap_or(json!({}))
}
pub fn est_central() -> bool {
    config_mail().get("central").and_then(|v| v.as_bool()).unwrap_or(false)
}
pub fn url_central() -> String {
    config_mail()
        .get("central_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://app.vexs.me")
        .trim_end_matches('/')
        .to_string()
}

// ── Règles de nom (alias et adresses m.vexs.me) ──────────────────
pub fn nom_valide(n: &str) -> Result<(), &'static str> {
    if n.len() < 2 || n.len() > 32 {
        return Err("Le nom doit faire entre 2 et 32 caractères.");
    }
    if !n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-') {
        return Err("Lettres minuscules, chiffres, point, tiret et _ seulement.");
    }
    let premier = n.chars().next().unwrap_or('.');
    let dernier = n.chars().last().unwrap_or('.');
    if !premier.is_ascii_alphanumeric() || !dernier.is_ascii_alphanumeric() || n.contains("..") {
        return Err("Le nom doit commencer et finir par une lettre ou un chiffre.");
    }
    if RESERVES.contains(&n) {
        return Err("Ce nom est réservé.");
    }
    Ok(())
}

// ── Tables ───────────────────────────────────────────────────────
pub fn creer_tables(pool: &DbPool) {
    let Ok(mut c) = pool.get_conn() else { return };
    let _ = c.query_drop(
        "CREATE TABLE IF NOT EXISTS `mail_adresses` (
           `adresse` VARCHAR(255) NOT NULL PRIMARY KEY,
           `type` VARCHAR(10) NOT NULL,
           `user_id` BIGINT NOT NULL,
           `nom_compte` VARCHAR(191) NOT NULL DEFAULT '',
           `node_id` VARCHAR(64) NOT NULL DEFAULT '',
           `node_url` VARCHAR(255) NOT NULL DEFAULT '',
           `node_pub` VARCHAR(100) NOT NULL DEFAULT '',
           `statut` VARCHAR(10) NOT NULL,
           `cree_le` DATETIME NOT NULL DEFAULT NOW(),
           `decide_le` DATETIME NULL,
           INDEX (`node_id`, `user_id`), INDEX (`user_id`, `type`)
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    );
    let _ = c.query_drop(
        "CREATE TABLE IF NOT EXISTS `mail_relais` (
           `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
           `adresse` VARCHAR(255) NOT NULL,
           `expediteur` VARCHAR(512) NOT NULL DEFAULT '',
           `brut` LONGBLOB NOT NULL,
           `essais` INT NOT NULL DEFAULT 0,
           `cree_le` DATETIME NOT NULL DEFAULT NOW(),
           `prochain` DATETIME NOT NULL DEFAULT NOW(),
           INDEX (`prochain`)
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    );
    let _ = c.query_drop(
        "CREATE TABLE IF NOT EXISTS `mail_nonces` (
           `empreinte` CHAR(64) NOT NULL PRIMARY KEY,
           `vu_le` DATETIME NOT NULL DEFAULT NOW()
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    );
    let _ = c.query_drop(
        "CREATE TABLE IF NOT EXISTS `mail_envois_reseau` (
           `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
           `node_id` VARCHAR(64) NOT NULL,
           `user_id` BIGINT NOT NULL,
           `envoye_le` DATETIME NOT NULL DEFAULT NOW(),
           INDEX (`node_id`, `user_id`, `envoye_le`)
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    );
    // Côté nœud (non central) : état de l'adresse m.vexs.me de chaque compte.
    let _ = c.query_drop(
        "CREATE TABLE IF NOT EXISTS `mail_reseau_local` (
           `user_id` BIGINT NOT NULL PRIMARY KEY,
           `adresse` VARCHAR(255) NOT NULL,
           `statut` VARCHAR(10) NOT NULL,
           `maj_le` DATETIME NOT NULL DEFAULT NOW()
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    );
}

// ── Enveloppes signées ───────────────────────────────────────────
fn sha_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
fn node_id_de(url: &str, pub_b64: &str) -> String {
    let mut h = Sha256::new();
    h.update(url.as_bytes());
    h.update(pub_b64.as_bytes());
    hex::encode(&h.finalize()[..16])
}

pub fn enveloppe(ns: &Arc<RwLock<NodeState>>, typ: &str, charge: &Value) -> Value {
    let ns = ns.read().unwrap();
    // Ed25519 est déterministe : sans ce nombre aléatoire, deux messages
    // identiques dans la même seconde auraient la même signature et le
    // second serait pris pour un rejeu.
    let mut charge = charge.clone();
    charge["nonce"] = json!(uuid::Uuid::new_v4().simple().to_string());
    let c = charge.to_string();
    let ts = chrono::Utc::now().timestamp();
    let msg = format!("vexmail|{}|{}|{}|{}", typ, ns.node_id, ts, sha_hex(c.as_bytes()));
    json!({"type": typ, "node_id": ns.node_id, "vex_url": ns.vex_url, "pub_key": ns.pub_key_b64(),
           "ts": ts, "charge": c, "sig": ns.signer(msg.as_bytes())})
}

pub struct Verifiee {
    pub node_id: String,
    pub vex_url: String,
    pub pub_key: String,
    pub charge: Value,
}

pub fn verifier_enveloppe(pool: &DbPool, e: &Value, typ: &str, cle_attendue: Option<&str>) -> Result<Verifiee, String> {
    let s = |k: &str| e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let (t, node_id, vex_url, pub_key, charge, sig) = (s("type"), s("node_id"), s("vex_url"), s("pub_key"), s("charge"), s("sig"));
    let ts = e.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
    if t != typ {
        return Err("type".into());
    }
    if (chrono::Utc::now().timestamp() - ts).abs() > 300 {
        return Err("horloge".into());
    }
    if node_id.is_empty() || node_id != node_id_de(&vex_url, &pub_key) {
        return Err("identité".into());
    }
    if let Some(k) = cle_attendue {
        if k != pub_key {
            return Err("clé inattendue".into());
        }
    }
    let msg = format!("vexmail|{}|{}|{}|{}", t, node_id, ts, sha_hex(charge.as_bytes()));
    if !NodeState::verifier_signature(&pub_key, msg.as_bytes(), &sig) {
        return Err("signature".into());
    }
    let mut c = pool.get_conn().map_err(|_| "base".to_string())?;
    let _ = c.query_drop("DELETE FROM mail_nonces WHERE vu_le < NOW() - INTERVAL 1 DAY");
    c.exec_drop("INSERT IGNORE INTO mail_nonces (empreinte) VALUES (?)", (sha_hex(sig.as_bytes()),))
        .map_err(|_| "base".to_string())?;
    if c.affected_rows() == 0 {
        return Err("rejeu".into());
    }
    let charge: Value = serde_json::from_str(&charge).map_err(|_| "charge".to_string())?;
    Ok(Verifiee { node_id, vex_url, pub_key, charge })
}

fn poster(url: &str, corps: &Value, delai: u64) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(delai)).build();
    match agent.post(url).send_json(corps.clone()) {
        Ok(r) => r.into_json::<Value>().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or(json!({}));
            Err(v.get("erreur").and_then(|x| x.as_str()).map(String::from).unwrap_or(format!("erreur {}", code)))
        }
        Err(e) => Err(e.to_string()),
    }
}

// ═════════════════════════════════════════════════════════════════
// CÔTÉ CENTRAL
// ═════════════════════════════════════════════════════════════════

/// Adresse (alias a.vexs.me ou m.vexs.me active) → (type, user_id, node_url, node_pub).
pub fn trouver(pool: &DbPool, adresse: &str) -> Option<(String, i64, String, String)> {
    let mut c = pool.get_conn().ok()?;
    c.exec_first(
        "SELECT type, user_id, node_url, node_pub FROM mail_adresses WHERE adresse = ? AND statut = 'actif'",
        (adresse,),
    )
    .ok()
    .flatten()
}

/// Met un mail pour un autre nœud en file et tente la livraison tout de suite.
pub fn relayer(pool: &DbPool, ns: &Arc<RwLock<NodeState>>, adresse: &str, expediteur: &str, brut: &[u8]) -> bool {
    let Ok(mut c) = pool.get_conn() else { return false };
    if c.exec_drop(
        "INSERT INTO mail_relais (adresse, expediteur, brut) VALUES (?, ?, ?)",
        (adresse, expediteur, brut),
    )
    .is_err()
    {
        return false;
    }
    let id = c.last_insert_id() as i64;
    let pool2 = pool.clone();
    let ns2 = Arc::clone(ns);
    std::thread::spawn(move || livrer_un(&pool2, &ns2, id));
    true
}

fn livrer_un(pool: &DbPool, ns: &Arc<RwLock<NodeState>>, id: i64) {
    let Ok(mut c) = pool.get_conn() else { return };
    let ligne: Option<(String, String, Vec<u8>, i64)> = c
        .exec_first("SELECT adresse, expediteur, brut, essais FROM mail_relais WHERE id = ?", (id,))
        .ok()
        .flatten();
    let Some((adresse, expediteur, brut, essais)) = ligne else { return };
    let cible: Option<(i64, String)> = c
        .exec_first(
            "SELECT user_id, node_url FROM mail_adresses WHERE adresse = ? AND type = 'noeud' AND statut = 'actif'",
            (&adresse,),
        )
        .ok()
        .flatten();
    let Some((user_id, node_url)) = cible else {
        let _ = c.exec_drop("DELETE FROM mail_relais WHERE id = ?", (id,));
        return;
    };
    let env = enveloppe(
        ns,
        "livrer",
        &json!({"user_id": user_id, "adresse": adresse, "de": expediteur, "brut": B64.encode(&brut)}),
    );
    let url = format!("{}/api/mail/reseau/livrer", node_url.trim_end_matches('/'));
    match poster(&url, &env, 60) {
        Ok(v) if v.get("ok").and_then(|x| x.as_bool()) == Some(true) => {
            let _ = c.exec_drop("DELETE FROM mail_relais WHERE id = ?", (id,));
        }
        r => {
            eprintln!("[mail] livraison {} vers {} : échec ({:?})", adresse, node_url, r.err());
            // 5 min, 10, 20… plafonné à 6 h
            let attente = (5i64 << essais.min(6)).min(360);
            let _ = c.exec_drop(
                "UPDATE mail_relais SET essais = essais + 1, prochain = NOW() + INTERVAL ? MINUTE WHERE id = ?",
                (attente, id),
            );
        }
    }
}

/// Fil de relance : lancé une fois, relivre la file toutes les 2 min.
pub fn lancer_relances(pool: &DbPool, ns: &Arc<RwLock<NodeState>>) {
    static DEJA: std::sync::Once = std::sync::Once::new();
    let pool = pool.clone();
    let ns = Arc::clone(ns);
    DEJA.call_once(move || {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(120));
            if !est_central() {
                continue;
            }
            if let Ok(mut c) = pool.get_conn() {
                let _ = c.query_drop("DELETE FROM mail_relais WHERE cree_le < NOW() - INTERVAL 7 DAY");
                let ids: Vec<i64> = c
                    .query("SELECT id FROM mail_relais WHERE prochain <= NOW() ORDER BY id LIMIT 20")
                    .unwrap_or_default();
                for id in ids {
                    livrer_un(&pool, &ns, id);
                }
            }
        });
    });
}

/// POST /api/mail/reseau/demande — un nœud demande pseudo@m.vexs.me.
pub fn central_demande(pool: &DbPool, corps: &Value) -> (Value, u16) {
    let v = match verifier_enveloppe(pool, corps, "demande", None) {
        Ok(v) => v,
        Err(e) => return (json!({"ok": false, "erreur": format!("Demande refusée ({}).", e)}), 403),
    };
    let user_id = v.charge.get("user_id").and_then(|x| x.as_i64()).unwrap_or(0);
    let local = v.charge.get("local").and_then(|x| x.as_str()).unwrap_or("").to_lowercase();
    let nom_compte: String = v.charge.get("pseudo").and_then(|x| x.as_str()).unwrap_or("").chars().take(190).collect();
    if user_id <= 0 {
        return (json!({"ok": false, "erreur": "Compte invalide."}), 400);
    }
    let Ok(mut c) = pool.get_conn() else { return (json!({"ok": false, "erreur": "base"}), 503) };
    // Une seule adresse par compte : on renvoie l'existante.
    let deja: Option<(String, String)> = c
        .exec_first(
            "SELECT adresse, statut FROM mail_adresses WHERE type = 'noeud' AND node_id = ? AND user_id = ? AND statut <> 'refuse'",
            (&v.node_id, user_id),
        )
        .ok()
        .flatten();
    if let Some((adresse, statut)) = deja {
        return (json!({"ok": true, "adresse": adresse, "statut": statut}), 200);
    }
    if let Err(e) = nom_valide(&local) {
        return (json!({"ok": false, "erreur": e}), 400);
    }
    let bloque: i64 = c
        .exec_first("SELECT COUNT(*) FROM mail_adresses WHERE node_id = ? AND statut = 'bloque'", (&v.node_id,))
        .ok()
        .flatten()
        .unwrap_or(0);
    if bloque > 0 {
        return (json!({"ok": false, "erreur": "Ce nœud n'est pas autorisé à demander des adresses."}), 403);
    }
    let recentes: i64 = c
        .exec_first(
            "SELECT COUNT(*) FROM mail_adresses WHERE node_id = ? AND cree_le > NOW() - INTERVAL 1 DAY",
            (&v.node_id,),
        )
        .ok()
        .flatten()
        .unwrap_or(0);
    if recentes >= MAX_DEMANDES_NOEUD_JOUR {
        return (json!({"ok": false, "erreur": "Trop de demandes depuis ce nœud aujourd'hui."}), 429);
    }
    let adresse = format!("{}@{}", local, DOMAINE_NOEUDS);
    let pris: i64 = c
        .exec_first("SELECT COUNT(*) FROM mail_adresses WHERE adresse = ?", (&adresse,))
        .ok()
        .flatten()
        .unwrap_or(0);
    if pris > 0 {
        return (json!({"ok": false, "erreur": "Cette adresse est déjà prise."}), 409);
    }
    let ok = c.exec_drop(
        "INSERT INTO mail_adresses (adresse, type, user_id, nom_compte, node_id, node_url, node_pub, statut)
         VALUES (?, 'noeud', ?, ?, ?, ?, ?, 'attente')",
        (&adresse, user_id, &nom_compte, &v.node_id, v.vex_url.trim_end_matches('/'), &v.pub_key),
    );
    if ok.is_err() {
        return (json!({"ok": false, "erreur": "Cette adresse est déjà prise."}), 409);
    }
    (json!({"ok": true, "adresse": adresse, "statut": "attente"}), 200)
}

/// POST /api/mail/reseau/statut — état de l'adresse d'un compte du nœud.
pub fn central_statut(pool: &DbPool, corps: &Value) -> (Value, u16) {
    let v = match verifier_enveloppe(pool, corps, "statut", None) {
        Ok(v) => v,
        Err(e) => return (json!({"ok": false, "erreur": format!("Refusé ({}).", e)}), 403),
    };
    let user_id = v.charge.get("user_id").and_then(|x| x.as_i64()).unwrap_or(0);
    let Ok(mut c) = pool.get_conn() else { return (json!({"ok": false}), 503) };
    let r: Option<(String, String)> = c
        .exec_first(
            "SELECT adresse, statut FROM mail_adresses WHERE type = 'noeud' AND node_id = ? AND user_id = ?
             ORDER BY cree_le DESC LIMIT 1",
            (&v.node_id, user_id),
        )
        .ok()
        .flatten();
    match r {
        Some((a, s)) => (json!({"ok": true, "adresse": a, "statut": s}), 200),
        None => (json!({"ok": true, "adresse": "", "statut": "aucune"}), 200),
    }
}

/// POST /api/mail/reseau/envoyer — un nœud envoie au nom d'un de ses comptes.
pub fn central_envoyer(pool: &DbPool, corps: &Value, limite: i64) -> (Value, u16) {
    let v = match verifier_enveloppe(pool, corps, "envoi", None) {
        Ok(v) => v,
        Err(e) => return (json!({"ok": false, "erreur": format!("Refusé ({}).", e)}), 403),
    };
    let g = |k: &str| v.charge.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let user_id = v.charge.get("user_id").and_then(|x| x.as_i64()).unwrap_or(0);
    let Ok(mut c) = pool.get_conn() else { return (json!({"ok": false, "erreur": "base"}), 503) };
    let adresse: Option<String> = c
        .exec_first(
            "SELECT adresse FROM mail_adresses WHERE type = 'noeud' AND node_id = ? AND user_id = ? AND statut = 'actif'
             AND node_pub = ?",
            (&v.node_id, user_id, &v.pub_key),
        )
        .ok()
        .flatten();
    let Some(adresse) = adresse else {
        return (json!({"ok": false, "erreur": "Ce compte n'a pas d'adresse active."}), 403);
    };
    let n: i64 = c
        .exec_first(
            "SELECT COUNT(*) FROM mail_envois_reseau WHERE node_id = ? AND user_id = ? AND envoye_le > NOW() - INTERVAL 1 DAY",
            (&v.node_id, user_id),
        )
        .ok()
        .flatten()
        .unwrap_or(0);
    if n >= limite {
        return (json!({"ok": false, "erreur": format!("Limite atteinte : {} mails par 24 h.", limite)}), 429);
    }
    let local = adresse.split('@').next().unwrap_or("").to_string();
    // Resend gratuit = un seul domaine vérifié : le mail part de vexs.me,
    // les réponses reviennent à l'adresse m.vexs.me (Reply-To).
    let de = format!("{} <reseau@vexs.me>", local);
    match crate::mail::mail::envoyer_resend(&de, &g("a"), &g("sujet"), &g("texte"), &adresse) {
        Ok(_) => {
            let _ = c.exec_drop("INSERT INTO mail_envois_reseau (node_id, user_id) VALUES (?, ?)", (&v.node_id, user_id));
            (json!({"ok": true, "adresse": adresse}), 200)
        }
        Err(e) => (json!({"ok": false, "erreur": e}), 502),
    }
}

/// Demandes en attente (fondateur du central).
pub fn liste_attente(pool: &DbPool) -> Vec<Value> {
    let Ok(mut c) = pool.get_conn() else { return vec![] };
    let lignes: Vec<(String, String, String, String)> = c
        .query(
            "SELECT adresse, nom_compte, node_url, DATE_FORMAT(cree_le, '%Y-%m-%d %H:%i')
             FROM mail_adresses WHERE type = 'noeud' AND statut = 'attente' ORDER BY cree_le",
        )
        .unwrap_or_default();
    lignes
        .into_iter()
        .map(|(a, n, u, d)| json!({"adresse": a, "compte": n, "noeud": u, "date": d}))
        .collect()
}

pub fn decider(pool: &DbPool, adresse: &str, decision: &str) -> bool {
    let statut = match decision {
        "accepter" => "actif",
        "refuser" => "refuse",
        "bloquer" => "bloque",
        _ => return false,
    };
    let Ok(mut c) = pool.get_conn() else { return false };
    c.exec_drop(
        "UPDATE mail_adresses SET statut = ?, decide_le = NOW() WHERE adresse = ? AND type = 'noeud'",
        (statut, adresse),
    )
    .is_ok()
        && c.affected_rows() > 0
}

// ── Alias a.vexs.me (comptes du central) ─────────────────────────
pub fn alias_du_compte(pool: &DbPool, uid: i64) -> Vec<String> {
    let Ok(mut c) = pool.get_conn() else { return vec![] };
    c.exec(
        "SELECT adresse FROM mail_adresses WHERE type = 'alias' AND user_id = ? AND statut = 'actif' ORDER BY adresse",
        (uid,),
    )
    .unwrap_or_default()
}

pub fn ajouter_alias(pool: &DbPool, uid: i64, nom_compte: &str, nom: &str) -> Result<String, String> {
    let nom = nom.trim().to_lowercase();
    nom_valide(&nom).map_err(String::from)?;
    if alias_du_compte(pool, uid).len() as i64 >= MAX_ALIAS {
        return Err(format!("Maximum {} alias par compte.", MAX_ALIAS));
    }
    let adresse = format!("{}@{}", nom, DOMAINE_ALIAS);
    let mut c = pool.get_conn().map_err(|_| "base".to_string())?;
    c.exec_drop(
        "INSERT INTO mail_adresses (adresse, type, user_id, nom_compte, statut) VALUES (?, 'alias', ?, ?, 'actif')",
        (&adresse, uid, nom_compte),
    )
    .map_err(|_| "Cet alias est déjà pris.".to_string())?;
    Ok(adresse)
}

pub fn supprimer_alias(pool: &DbPool, uid: i64, adresse: &str) -> bool {
    let Ok(mut c) = pool.get_conn() else { return false };
    c.exec_drop("DELETE FROM mail_adresses WHERE adresse = ? AND type = 'alias' AND user_id = ?", (adresse, uid))
        .is_ok()
        && c.affected_rows() > 0
}

// ═════════════════════════════════════════════════════════════════
// CÔTÉ NŒUD (non central)
// ═════════════════════════════════════════════════════════════════

/// Clé publique du central, épinglée au premier usage (keys/mail_central.json).
fn cle_centrale() -> Option<(String, String)> {
    let chemin = "keys/mail_central.json";
    if let Some(v) = std::fs::read_to_string(chemin).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
        let id = v.get("node_id")?.as_str()?.to_string();
        let pk = v.get("pub_key")?.as_str()?.to_string();
        return Some((id, pk));
    }
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(15)).build();
    let ping: Value = agent.get(&format!("{}/p2p/ping", url_central())).call().ok()?.into_json().ok()?;
    let id = ping.get("node_id")?.as_str()?.to_string();
    let pk = ping.get("pub_key")?.as_str()?.to_string();
    let url = ping.get("vex_url")?.as_str()?.to_string();
    if node_id_de(&url, &pk) != id {
        return None;
    }
    let _ = std::fs::create_dir_all("keys");
    let _ = std::fs::write(chemin, json!({"node_id": id, "pub_key": pk, "vex_url": url}).to_string());
    Some((id, pk))
}

/// POST /api/mail/reseau/livrer — le central nous livre un mail.
pub fn noeud_livrer(pool: &DbPool, corps: &Value) -> (Value, u16) {
    if est_central() {
        return (json!({"ok": false, "erreur": "nœud central"}), 400);
    }
    let Some((id_central, cle)) = cle_centrale() else {
        return (json!({"ok": false, "erreur": "central inconnu"}), 503);
    };
    let v = match verifier_enveloppe(pool, corps, "livrer", Some(&cle)) {
        Ok(v) if v.node_id == id_central => v,
        Ok(_) => return (json!({"ok": false, "erreur": "expéditeur"}), 403),
        Err(e) => return (json!({"ok": false, "erreur": e}), 403),
    };
    let user_id = v.charge.get("user_id").and_then(|x| x.as_i64()).unwrap_or(0);
    let adresse = v.charge.get("adresse").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let de = v.charge.get("de").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let brut = B64.decode(v.charge.get("brut").and_then(|x| x.as_str()).unwrap_or("")).unwrap_or_default();
    let existe: i64 = pool
        .get_conn()
        .ok()
        .and_then(|mut c| c.exec_first("SELECT COUNT(*) FROM login WHERE id = ?", (user_id,)).ok().flatten())
        .unwrap_or(0);
    if existe == 0 {
        return (json!({"ok": false, "erreur": "compte inconnu"}), 404);
    }
    match crate::mail::mail::stocker_mail(pool, user_id, &adresse, &de, &brut) {
        Ok(()) => {
            if let Ok(mut c) = pool.get_conn() {
                let _ = c.exec_drop(
                    "INSERT INTO mail_reseau_local (user_id, adresse, statut) VALUES (?, ?, 'actif')
                     ON DUPLICATE KEY UPDATE adresse = VALUES(adresse), statut = 'actif', maj_le = NOW()",
                    (user_id, &adresse),
                );
            }
            (json!({"ok": true}), 200)
        }
        Err(e) => (json!({"ok": false, "erreur": e}), 503),
    }
}

/// État local de l'adresse m.vexs.me d'un compte, rafraîchi auprès du
/// central si elle est en attente (ou inconnue).
pub fn noeud_etat(pool: &DbPool, ns: &Arc<RwLock<NodeState>>, uid: i64) -> (String, String) {
    let Ok(mut c) = pool.get_conn() else { return (String::new(), "aucune".into()) };
    let local: Option<(String, String)> = c
        .exec_first("SELECT adresse, statut FROM mail_reseau_local WHERE user_id = ?", (uid,))
        .ok()
        .flatten();
    if let Some((a, s)) = &local {
        if s == "actif" || s == "refuse" || s == "bloque" {
            return (a.clone(), s.clone());
        }
    }
    let env = enveloppe(ns, "statut", &json!({"user_id": uid}));
    if let Ok(r) = poster(&format!("{}/api/mail/reseau/statut", url_central()), &env, 15) {
        let a = r.get("adresse").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let s = r.get("statut").and_then(|x| x.as_str()).unwrap_or("aucune").to_string();
        if !a.is_empty() {
            let _ = c.exec_drop(
                "INSERT INTO mail_reseau_local (user_id, adresse, statut) VALUES (?, ?, ?)
                 ON DUPLICATE KEY UPDATE adresse = VALUES(adresse), statut = VALUES(statut), maj_le = NOW()",
                (uid, &a, &s),
            );
        }
        return (a, s);
    }
    local.unwrap_or((String::new(), "aucune".into()))
}

pub fn noeud_demander(pool: &DbPool, ns: &Arc<RwLock<NodeState>>, uid: i64, pseudo: &str, local: &str) -> (Value, u16) {
    let local = local.trim().to_lowercase();
    if let Err(e) = nom_valide(&local) {
        return (json!({"ok": false, "erreur": e}), 400);
    }
    let env = enveloppe(ns, "demande", &json!({"user_id": uid, "pseudo": pseudo, "local": local}));
    match poster(&format!("{}/api/mail/reseau/demande", url_central()), &env, 20) {
        Ok(r) if r.get("ok").and_then(|x| x.as_bool()) == Some(true) => {
            let a = r.get("adresse").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let s = r.get("statut").and_then(|x| x.as_str()).unwrap_or("attente").to_string();
            if let Ok(mut c) = pool.get_conn() {
                let _ = c.exec_drop(
                    "INSERT INTO mail_reseau_local (user_id, adresse, statut) VALUES (?, ?, ?)
                     ON DUPLICATE KEY UPDATE adresse = VALUES(adresse), statut = VALUES(statut), maj_le = NOW()",
                    (uid, &a, &s),
                );
            }
            (json!({"ok": true, "adresse": a, "statut": s}), 200)
        }
        Ok(r) => (json!({"ok": false, "erreur": r.get("erreur").cloned().unwrap_or(json!("Refusé."))}), 400),
        Err(e) => (json!({"ok": false, "erreur": e}), 502),
    }
}

pub fn noeud_envoyer(ns: &Arc<RwLock<NodeState>>, uid: i64, a: &str, sujet: &str, texte: &str) -> Result<String, String> {
    let env = enveloppe(ns, "envoi", &json!({"user_id": uid, "a": a, "sujet": sujet, "texte": texte}));
    match poster(&format!("{}/api/mail/reseau/envoyer", url_central()), &env, 30) {
        Ok(r) if r.get("ok").and_then(|x| x.as_bool()) == Some(true) => {
            Ok(r.get("adresse").and_then(|x| x.as_str()).unwrap_or("").to_string())
        }
        Ok(r) => Err(r.get("erreur").and_then(|x| x.as_str()).unwrap_or("Envoi refusé.").to_string()),
        Err(e) => Err(e),
    }
}
