// ══════════════════════════════════════════════════════════════════
// p2p.rs — VEX P2P / AnonNet
//
// Architecture :
//   - Chaque nœud VEX a un node_id unique (SHA256 de vex_url+clé publique)
//   - Clés Ed25519 : auth des nœuds ET chiffrement (via X25519 dérivé)
//   - Le bootstrap est vex.hopto.org/neut
//   - Annuaire répliqué : /p2p/annuaire (fichier texte lisible)
//   - Partage de fichiers avec chunking configurable (config.json → p2p.chunk_size_bytes)
//   - Support Tor : si tor_addr est renseigné, il est annoncé à la place de l'IP réelle
//   - IPs anonymisées dans l'annuaire public (seul le nœud destinataire reçoit l'IP réelle)
//
// Routes HTTP exposées par ce module :
//   GET  /p2p/annuaire         → fichier texte de l'annuaire
//   GET  /p2p/ping             → health check (retourne node_id + version)
//   POST /p2p/register         → un nœud s'enregistre (appelé par lui-même au démarrage)
//   POST /p2p/sync             → reçoit l'annuaire d'un pair, fusionne
//   POST /p2p/transfer/init    → initie un transfert de fichier
//   POST /p2p/transfer/chunk   → reçoit un chunk de fichier
//   GET  /p2p/transfer/status  → état d'un transfert
//
// Routes API admin (/api/admin/p2p/*) → gérées dans Admin.rs
//
// Dépendances Cargo.toml à ajouter :
//   ed25519-dalek = { version = "2", features = ["rand_core"] }
//   x25519-dalek  = { version = "2", features = ["static_secrets"] }
//   sha2          = "0.10"
//   base64        = "0.22"
//   ureq          = { version = "2", features = ["tls"] }
//   uuid          = { version = "1", features = ["v4"] }
//   chrono        = { version = "0.4", features = ["serde"] }
//   serde_json    = "1"
//   rand          = "0.8"
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{
    inserer_ou_modifier, mysql_val_to_json, p2p_chunk_recu, p2p_creer_transfer, p2p_get_peer,
    p2p_get_transfer, p2p_lister_peers, p2p_lister_peers_online, p2p_lister_transfers,
    p2p_lister_users, p2p_peer_offline, p2p_upsert_peer, p2p_upsert_user, selectionner, DbPool,
};
use crate::config_loader::VexConfig;
use crate::utils::{parse_query, url_decode};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use mysql::prelude::Queryable;
use rand::rngs::OsRng;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tiny_http::{Request, Response};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════
// CONFIG P2P (lue depuis config.json → section "p2p")
// ══════════════════════════════════════════════════════════════════
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Taille d'un chunk en octets (défaut 1 MB)
    pub chunk_size_bytes: usize,
    /// URL du serveur bootstrap
    pub bootstrap_url: String,
    /// Adresse Tor .onion optionnelle (None = pas de Tor)
    pub tor_addr: Option<String>,
    /// Active ou non l'usage de Tor ; si false, tor_addr est ignorée
    pub use_tor: bool,
    /// Port d'écoute P2P
    pub port: u16,
    /// Intervalle de sync avec le bootstrap (secondes)
    pub sync_interval_secs: u64,
    /// Dossier de stockage temporaire des chunks entrants
    pub chunks_dir: PathBuf,
    /// Dossier de sortie des fichiers reconstitués
    pub output_dir: PathBuf,
}

impl P2pConfig {
    pub fn from_vex_config(cfg: &VexConfig) -> Self {
        let p = cfg.extra.get("p2p").cloned().unwrap_or_else(|| json!({}));
        let tor_addr_raw = p.get("tor_addr").and_then(|v| v.as_str());
        // Si tor_addr est renseigné et use_tor absent, on active automatiquement Tor
        let use_tor = p
            .get("use_tor")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| tor_addr_raw.map(|s| !s.is_empty()).unwrap_or(false));
        let tor_addr = if use_tor {
            tor_addr_raw
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        };
        Self {
            chunk_size_bytes: p
                .get("chunk_size_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(1_048_576) as usize,
            bootstrap_url: normalize_bootstrap_url(
                p.get("bootstrap_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://vex.hopto.org/neut"),
            ),
            tor_addr,
            use_tor,
            port: p.get("port").and_then(|v| v.as_u64()).unwrap_or(7700) as u16,
            // Par dÃ©faut, on synchronise toutes les 30 minutes (1800 s)
            sync_interval_secs: p
                .get("sync_interval_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(1_800),
            chunks_dir: PathBuf::from(
                p.get("chunks_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/tmp/vex_chunks"),
            ),
            output_dir: PathBuf::from(
                p.get("output_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/tmp/vex_received"),
            ),
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// ÉTAT GLOBAL DU NŒUD
// Initialisé une fois dans main.rs, partagé via Arc<RwLock<NodeState>>
// ══════════════════════════════════════════════════════════════════
pub struct NodeState {
    pub node_id: String,
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub vex_url: String,
    pub config: P2pConfig,
}

impl NodeState {
    /// Charge ou génère la paire de clés Ed25519.
    /// La clé privée est stockée dans `keys/p2p_private.key` (base64).
    pub fn init(vex_url: &str, p2p_cfg: P2pConfig) -> Self {
        let key_path = PathBuf::from("keys/p2p_private.key");
        fs::create_dir_all("keys").ok();

        let signing_key = if key_path.exists() {
            let b64 = fs::read_to_string(&key_path).unwrap_or_default();
            let bytes = B64.decode(b64.trim()).unwrap_or_default();
            if bytes.len() == 32 {
                let arr: [u8; 32] = bytes.try_into().unwrap();
                SigningKey::from_bytes(&arr)
            } else {
                let k = SigningKey::generate(&mut OsRng);
                fs::write(&key_path, B64.encode(k.to_bytes())).ok();
                k
            }
        } else {
            let k = SigningKey::generate(&mut OsRng);
            fs::write(&key_path, B64.encode(k.to_bytes())).ok();
            k
        };

        let verifying_key = signing_key.verifying_key();
        let pub_b64 = B64.encode(verifying_key.to_bytes());

        // node_id = SHA256(vex_url + pub_key)[..16] en hex
        let mut hasher = Sha256::new();
        hasher.update(vex_url.as_bytes());
        hasher.update(pub_b64.as_bytes());
        let node_id = hex::encode(&hasher.finalize()[..16]);

        fs::create_dir_all(&p2p_cfg.chunks_dir).ok();
        fs::create_dir_all(&p2p_cfg.output_dir).ok();

        Self {
            node_id,
            signing_key,
            verifying_key,
            vex_url: vex_url.to_string(),
            config: p2p_cfg,
        }
    }

    pub fn pub_key_b64(&self) -> String {
        B64.encode(self.verifying_key.to_bytes())
    }

    /// Signe un message, retourne la signature en base64.
    pub fn signer(&self, msg: &[u8]) -> String {
        B64.encode(self.signing_key.sign(msg).to_bytes())
    }

    /// Vérifie une signature base64 d'un message avec une clé publique base64.
    pub fn verifier_signature(pub_key_b64: &str, msg: &[u8], sig_b64: &str) -> bool {
        let pk_bytes = match B64.decode(pub_key_b64) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match B64.decode(sig_b64) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let arr: [u8; 32] = match pk_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let vk = match VerifyingKey::from_bytes(&arr) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        vk.verify(msg, &Signature::from_bytes(&sig_arr)).is_ok()
    }
}

// ══════════════════════════════════════════════════════════════════
// ANNUAIRE TEXTE
//
// Format du fichier /p2p/annuaire (servi en clair) :
//
// # VEX AnonNet — Annuaire des nœuds
// # Généré le 2025-01-01 12:00:00 UTC
// # Format : NODE_ID | VEX_URL | TOR_ADDR_OU_ANONYME | PUB_KEY | STATUS | LAST_SEEN | VERSION
//
// [NODES]
// abc123... | https://vex.example.com | onion.onion:1234 | base64pubkey | online | 2025-01-01T12:00:00Z | 1.0.0
// ...
//
// [USERS]
// # Format : USER_ID@NODE_ID | NOM | PUB_KEY | LAST_SEEN
// 42@abc123... | Alice | base64pubkey | 2025-01-01T12:00:00Z
// ...
// ══════════════════════════════════════════════════════════════════
pub fn generer_annuaire(pool: &DbPool, node_state: &NodeState) -> String {
    let now = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let mut out = format!(
        "# VEX AnonNet — Annuaire des nœuds\n\
         # Généré le {}\n\
         # Ce fichier est répliqué entre tous les nœuds du réseau.\n\
         # Les IPs réelles sont anonymisées — seule l'adresse Tor ou ANONYME est visible.\n\n\
         [NODES]\n\
         # NODE_ID | VEX_URL | TOR_OU_ANONYME | PUB_KEY | STATUT | VU_LE | VERSION\n",
        now
    );

    for peer in p2p_lister_peers(pool) {
        let node_id = peer.get("node_id").and_then(|v| v.as_str()).unwrap_or("");
        let vex_url = peer.get("vex_url").and_then(|v| v.as_str()).unwrap_or("");
        let tor_addr = peer.get("tor_addr").and_then(|v| v.as_str()).unwrap_or("");
        // IP anonymisée : on affiche l'adresse Tor si disponible, sinon ANONYME
        let addr_pub = if tor_addr.is_empty() {
            "ANONYME".to_string()
        } else {
            tor_addr.to_string()
        };
        let pub_key = peer.get("pub_key").and_then(|v| v.as_str()).unwrap_or("");
        let status = peer
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("offline");
        let seen = peer.get("last_seen").and_then(|v| v.as_str()).unwrap_or("");
        let version = peer.get("version").and_then(|v| v.as_str()).unwrap_or("?");
        out += &format!(
            "{} | {} | {} | {} | {} | {} | {}\n",
            node_id, vex_url, addr_pub, pub_key, status, seen, version
        );
    }

    out += "\n[USERS]\n# USER_ID@NODE_ID | NOM | PUB_KEY | MIS_A_JOUR\n";
    for user in p2p_lister_users(pool) {
        let uid = user.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let node_id = user.get("node_id").and_then(|v| v.as_str()).unwrap_or("");
        let nom = user.get("nom").and_then(|v| v.as_str()).unwrap_or("");
        let pub_key = user.get("pub_key").and_then(|v| v.as_str()).unwrap_or("");
        let updated = user
            .get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        out += &format!(
            "{}@{} | {} | {} | {}\n",
            uid, node_id, nom, pub_key, updated
        );
    }

    // Signature de l'annuaire par ce nœud
    let sig = node_state.signer(out.as_bytes());
    out += &format!("\n[SIGNATURE]\n{}:{}\n", node_state.node_id, sig);
    out
}

/// Parse un annuaire texte reçu d'un pair et fusionne dans la DB locale.
pub fn fusionner_annuaire(pool: &DbPool, texte: &str) {
    let mut section = "";
    for line in texte.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line == "[NODES]" {
            section = "nodes";
            continue;
        }
        if line == "[USERS]" {
            section = "users";
            continue;
        }
        if line == "[SIGNATURE]" {
            section = "sig";
            continue;
        }

        match section {
            "nodes" => {
                let parts: Vec<&str> = line.splitn(7, " | ").collect();
                if parts.len() < 7 {
                    continue;
                }
                let (node_id, vex_url, tor_or_anon, pub_key, status, _, version) = (
                    parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], parts[6],
                );
                let tor = if tor_or_anon == "ANONYME" {
                    None
                } else {
                    Some(tor_or_anon)
                };
                // On ne connaît pas l'IP réelle des nœuds distants — on met "p2p"
                if status == "online" {
                    p2p_upsert_peer(pool, node_id, vex_url, "p2p", 0, tor, pub_key, version);
                }
            }
            "users" => {
                // USER_ID@NODE_ID | NOM | PUB_KEY | MIS_A_JOUR
                let parts: Vec<&str> = line.splitn(4, " | ").collect();
                if parts.len() < 3 {
                    continue;
                }
                let id_node: Vec<&str> = parts[0].splitn(2, '@').collect();
                if id_node.len() < 2 {
                    continue;
                }
                let user_id: i64 = id_node[0].parse().unwrap_or(0);
                let node_id = id_node[1];
                let nom = parts[1];
                let pub_key = parts[2];
                p2p_upsert_user(pool, user_id, node_id, nom, pub_key);
            }
            _ => {}
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// SYNC BOOTSTRAP
// Appelé au démarrage et toutes les `sync_interval_secs` secondes.
// 1. Envoie notre annuaire au bootstrap (POST /neut/sync)
// 2. Récupère l'annuaire du bootstrap (GET /neut/annuaire)
// 3. Fusionne dans la DB locale
// ══════════════════════════════════════════════════════════════════
pub fn sync_avec_bootstrap(pool: &DbPool, node_state: &NodeState) {
    let base = node_state.config.bootstrap_url.trim_end_matches('/');
    let annuaire_local = generer_annuaire(pool, node_state);

    // 0. Pousser notre annuaire complet (nœuds + users) vers le bootstrap
    //    pour qu’il puisse le répliquer aux autres pairs.
    let _ = ureq::post(&format!("{}/sync", base))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(std::time::Duration::from_secs(10))
        .send_string(&format!("annuaire={}", urlenc(&annuaire_local)));

    // 1. S'enregistrer / envoyer notre annuaire
    let reg_body = format!(
        "node_id={}&vex_url={}&pub_key={}&version={}&sig={}",
        urlenc(&node_state.node_id),
        urlenc(&node_state.vex_url),
        urlenc(&node_state.pub_key_b64()),
        urlenc(env!("CARGO_PKG_VERSION")),
        urlenc(&node_state.signer(node_state.node_id.as_bytes())),
    );

    let _ = ureq::post(&format!("{}/register", base))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(std::time::Duration::from_secs(10))
        .send_string(&reg_body);

    // 2. Récupérer l'annuaire du bootstrap
    if let Ok(resp) = ureq::get(&format!("{}/annuaire", base))
        .timeout(std::time::Duration::from_secs(15))
        .call()
    {
        if let Ok(body) = resp.into_string() {
            fusionner_annuaire(pool, &body);
        }
    }

    // 3. Récupérer les nouveaux nœuds ajoutés depuis la dernière sync
    if let Ok(resp) = ureq::get(&format!("{}/nouveautes", base))
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        if let Ok(body) = resp.into_string() {
            fusionner_annuaire(pool, &body);
        }
    }

    eprintln!("[p2p] Sync bootstrap terminée — {}", chrono::Local::now());
}

/// Lance la sync en arrière-plan dans un thread dédié.
pub fn lancer_sync_periodique(pool: DbPool, node_state: Arc<RwLock<NodeState>>) {
    let interval = {
        let ns = node_state.read().unwrap();
        std::time::Duration::from_secs(ns.config.sync_interval_secs)
    };

    std::thread::spawn(move || loop {
        {
            let ns = node_state.read().unwrap();
            sync_avec_bootstrap(&pool, &ns);
        }
        std::thread::sleep(interval);
    });
}

// ══════════════════════════════════════════════════════════════════
// TRANSFERT DE FICHIERS — CHUNKING
// ══════════════════════════════════════════════════════════════════

/// Calcule le nombre de chunks nécessaires pour un fichier.
pub fn nombre_chunks(taille: usize, chunk_size: usize) -> usize {
    if taille == 0 {
        return 1;
    }
    (taille + chunk_size - 1) / chunk_size
}

/// Envoie un fichier à un nœud pair en le découpant en chunks.
/// Retourne le transfer_id ou une erreur.
pub fn envoyer_fichier(
    pool: &DbPool,
    node_state: &NodeState,
    to_node_id: &str,
    from_user: i64,
    to_user: i64,
    file_path: &PathBuf,
    file_name: &str,
) -> Result<String, String> {
    // 1. Lire le fichier complet
    let mut f = fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    f.read_to_end(&mut data).map_err(|e| e.to_string())?;

    let file_size = data.len();
    let chunk_size = node_state.config.chunk_size_bytes;
    let n_chunks = nombre_chunks(file_size, chunk_size);
    let tid = Uuid::new_v4().to_string();

    // 2. Trouver l'URL du nœud destinataire
    let peer =
        p2p_get_peer(pool, to_node_id).ok_or_else(|| format!("Nœud {} inconnu", to_node_id))?;
    let peer_url = peer
        .get("vex_url")
        .and_then(|v| v.as_str())
        .ok_or("URL pair manquante")?
        .trim_end_matches('/')
        .to_string();

    // Utiliser l'adresse Tor si disponible
    let base_url = peer
        .get("tor_addr")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|tor| format!("http://{}", tor))
        .unwrap_or(peer_url.clone());

    // 3. Enregistrer le transfert en DB
    p2p_creer_transfer(
        pool,
        &tid,
        &node_state.node_id,
        to_node_id,
        from_user,
        to_user,
        file_name,
        file_size as i64,
        chunk_size as i64,
        n_chunks as i32,
    );

    // 4. Initier le transfert côté destinataire
    let init_sig = node_state.signer(tid.as_bytes());
    let init_body = format!(
        "transfer_id={}&from_node={}&to_user={}&file_name={}&file_size={}&chunk_size={}&chunks_total={}&sig={}",
        urlenc(&tid),
        urlenc(&node_state.node_id),
        to_user,
        urlenc(file_name),
        file_size,
        chunk_size,
        n_chunks,
        urlenc(&init_sig),
    );
    ureq::post(&format!("{}/p2p/transfer/init", base_url))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(std::time::Duration::from_secs(15))
        .send_string(&init_body)
        .map_err(|e| format!("Init transfer échoué : {}", e))?;

    // 5. Envoyer les chunks un par un
    for (idx, chunk) in data.chunks(chunk_size).enumerate() {
        let chunk_b64 = B64.encode(chunk);
        let chunk_hash = hex::encode(Sha256::digest(chunk));
        let sig = node_state.signer(format!("{}:{}", tid, idx).as_bytes());

        let body = format!(
            "transfer_id={}&chunk_idx={}&data={}&hash={}&sig={}",
            urlenc(&tid),
            idx,
            urlenc(&chunk_b64),
            urlenc(&chunk_hash),
            urlenc(&sig),
        );

        for attempt in 0..3u8 {
            match ureq::post(&format!("{}/p2p/transfer/chunk", base_url))
                .set("Content-Type", "application/x-www-form-urlencoded")
                .timeout(std::time::Duration::from_secs(30))
                .send_string(&body)
            {
                Ok(_) => break,
                Err(e) => {
                    if attempt == 2 {
                        return Err(format!("Chunk {} échoué après 3 tentatives : {}", idx, e));
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
    }

    Ok(tid)
}

// ══════════════════════════════════════════════════════════════════
// GESTIONNAIRE HTTP — routes /p2p/*
// ══════════════════════════════════════════════════════════════════
pub fn handle_request(
    mut request: Request,
    pool: &DbPool,
    node_state: &Arc<RwLock<NodeState>>,
    _config: &VexConfig,
) {
    let url = request.url().to_string();
    let method = request.method().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let query = parse_query(&url);

    // Corps POST
    let body = if method == "POST" {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(request.as_reader(), &mut s);
        let mut m = HashMap::new();
        for pair in s.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                m.insert(url_decode(k), url_decode(v));
            }
        }
        m
    } else {
        HashMap::new()
    };

    let ns = node_state.read().unwrap();

    match path.as_str() {
        // ── Health check ─────────────────────────────────────────
        "/p2p/ping" => {
            let r = json!({
                "node_id": ns.node_id,
                "vex_url": ns.vex_url,
                "pub_key": ns.pub_key_b64(),
                "version": env!("CARGO_PKG_VERSION"),
                "time":    chrono::Utc::now().to_rfc3339(),
            });
            respond_json(request, r);
        }

        // ── Annuaire public ──────────────────────────────────────
        "/p2p/annuaire" | "/neut/annuaire" => {
            let texte = generer_annuaire(pool, &ns);
            let _ = request.respond(Response::from_string(texte).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap(),
            ));
        }

        // ── Nouveautés (nœuds/users ajoutés récemment) ──────────
        "/p2p/nouveautes" | "/neut/nouveautes" => {
            // Retourne l'annuaire des nœuds mis à jour dans les 24h
            let texte = generer_annuaire_nouveautes(pool, &ns);
            let _ = request.respond(Response::from_string(texte).with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap(),
            ));
        }

        // ── Enregistrement d'un nœud ─────────────────────────────
        "/p2p/register" | "/neut/register" => {
            let node_id = body.get("node_id").cloned().unwrap_or_default();
            let vex_url = body.get("vex_url").cloned().unwrap_or_default();
            let pub_key = body.get("pub_key").cloned().unwrap_or_default();
            let version = body.get("version").cloned().unwrap_or_default();
            let sig = body.get("sig").cloned().unwrap_or_default();

            if node_id.is_empty() || pub_key.is_empty() {
                return respond_json(request, json!({"success":false,"error":"Champs manquants"}));
            }
            // Vérifie signature : le nœud a signé son propre node_id
            if !NodeState::verifier_signature(&pub_key, node_id.as_bytes(), &sig) {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Signature invalide"}),
                );
            }

            // Déterminer l'IP réelle (ne sera pas publiée dans l'annuaire)
            let remote_ip = request
                .remote_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_default();
            // Port P2P annoncé par le pair
            let port: u16 = body
                .get("port")
                .and_then(|v| v.parse().ok())
                .unwrap_or(7700);
            let tor = body.get("tor_addr").map(|s| s.as_str());

            let ok = p2p_upsert_peer(
                pool, &node_id, &vex_url, &remote_ip, port, tor, &pub_key, &version,
            );
            respond_json(
                request,
                json!({"success":ok,"node_id":ns.node_id,"pub_key":ns.pub_key_b64()}),
            );
        }

        // ── Sync annuaire entrant ────────────────────────────────
        "/p2p/sync" | "/neut/sync" => {
            let texte = body.get("annuaire").cloned().unwrap_or_default();
            if !texte.is_empty() {
                fusionner_annuaire(pool, &texte);
            }
            respond_json(request, json!({"success":true}));
        }

        // ── Init transfert entrant ───────────────────────────────
        "/p2p/transfer/init" => {
            let tid = body
                .get("transfer_id")
                .cloned()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let from_node = body.get("from_node").cloned().unwrap_or_default();
            let to_user: i64 = body
                .get("to_user")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let file_name = body.get("file_name").cloned().unwrap_or_default();
            let file_size: i64 = body
                .get("file_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let chunk_size: i64 = body
                .get("chunk_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1_048_576);
            let chunks_total: i32 = body
                .get("chunks_total")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            let sig = body.get("sig").cloned().unwrap_or_default();

            // Vérifie la signature du nœud émetteur
            if let Some(peer) = p2p_get_peer(pool, &from_node) {
                let pub_key = peer.get("pub_key").and_then(|v| v.as_str()).unwrap_or("");
                if !NodeState::verifier_signature(pub_key, tid.as_bytes(), &sig) {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Signature invalide"}),
                    );
                }
            } else {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Nœud émetteur inconnu"}),
                );
            }

            // Prépare le dossier de réception des chunks
            let chunk_dir = ns.config.chunks_dir.join(&tid);
            fs::create_dir_all(&chunk_dir).ok();

            p2p_creer_transfer(
                pool,
                &tid,
                &from_node,
                &ns.node_id,
                0,
                to_user,
                &file_name,
                file_size,
                chunk_size,
                chunks_total,
            );

            respond_json(request, json!({"success":true,"transfer_id":tid}));
        }

        // ── Réception d'un chunk ─────────────────────────────────
        "/p2p/transfer/chunk" => {
            let tid = body.get("transfer_id").cloned().unwrap_or_default();
            let chunk_idx: usize = body
                .get("chunk_idx")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let data_b64 = body.get("data").cloned().unwrap_or_default();
            let expected_hash = body.get("hash").cloned().unwrap_or_default();
            let sig = body.get("sig").cloned().unwrap_or_default();

            if tid.is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":"transfer_id manquant"}),
                );
            }

            // Récupère le transfert pour vérifier la signature
            let transfer = match p2p_get_transfer(pool, &tid) {
                Some(t) => t,
                None => {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Transfert inconnu"}),
                    )
                }
            };
            let from_node = transfer
                .get("from_node")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(peer) = p2p_get_peer(pool, from_node) {
                let pub_key = peer.get("pub_key").and_then(|v| v.as_str()).unwrap_or("");
                let msg = format!("{}:{}", tid, chunk_idx);
                if !NodeState::verifier_signature(pub_key, msg.as_bytes(), &sig) {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Signature chunk invalide"}),
                    );
                }
            }

            // Décode et vérifie le hash SHA256 du chunk
            let chunk_data = match B64.decode(&data_b64) {
                Ok(d) => d,
                Err(_) => {
                    return respond_json(
                        request,
                        json!({"success":false,"error":"Données invalides"}),
                    )
                }
            };
            let actual_hash = hex::encode(Sha256::digest(&chunk_data));
            if actual_hash != expected_hash {
                return respond_json(
                    request,
                    json!({"success":false,"error":"Hash chunk invalide"}),
                );
            }

            // Écrit le chunk sur disque
            let chunk_dir = ns.config.chunks_dir.join(&tid);
            let chunk_path = chunk_dir.join(format!("{:06}.chunk", chunk_idx));
            if let Err(e) = fs::write(&chunk_path, &chunk_data) {
                return respond_json(request, json!({"success":false,"error":e.to_string()}));
            }

            // Met à jour le compteur en DB
            p2p_chunk_recu(pool, &tid);

            // Vérifie si le transfert est complet → reconstitue le fichier
            if let Some(t) = p2p_get_transfer(pool, &tid) {
                let ok = t.get("chunks_ok").and_then(|v| v.as_i64()).unwrap_or(0);
                let total = t.get("chunks_total").and_then(|v| v.as_i64()).unwrap_or(1);
                if ok >= total {
                    let file_name = t
                        .get("fichier_nom")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    reconstituer_fichier(
                        pool,
                        &tid,
                        &chunk_dir,
                        &ns.config.output_dir,
                        file_name,
                        total as usize,
                    );
                }
            }

            respond_json(request, json!({"success":true}));
        }

        // ── État d'un transfert ──────────────────────────────────
        "/p2p/transfer/status" => {
            let tid = query.get("transfer_id").cloned().unwrap_or_default();
            match p2p_get_transfer(pool, &tid) {
                Some(t) => respond_json(request, json!({"success":true,"data":t})),
                None => respond_json(
                    request,
                    json!({"success":false,"error":"Transfert inconnu"}),
                ),
            }
        }

        _ => {
            respond_json(
                request,
                json!({"success":false,"error":"Route P2P inconnue"}),
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// RECONSTITUTION DU FICHIER
// Une fois tous les chunks reçus, on les réassemble dans l'ordre.
// ══════════════════════════════════════════════════════════════════
fn reconstituer_fichier(
    pool: &DbPool,
    tid: &str,
    chunk_dir: &PathBuf,
    output_dir: &PathBuf,
    file_name: &str,
    n_chunks: usize,
) {
    // Nom de sortie avec le transfer_id pour éviter les collisions
    let safe_name = file_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let output_path = output_dir.join(format!("{}_{}", &tid[..8], safe_name));

    let mut out = match fs::File::create(&output_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[p2p] Erreur création fichier reconstitué : {}", e);
            return;
        }
    };

    for idx in 0..n_chunks {
        let chunk_path = chunk_dir.join(format!("{:06}.chunk", idx));
        match fs::read(&chunk_path) {
            Ok(data) => {
                if let Err(e) = out.write_all(&data) {
                    eprintln!("[p2p] Erreur écriture chunk {} : {}", idx, e);
                    return;
                }
            }
            Err(e) => {
                eprintln!("[p2p] Chunk {} manquant : {}", idx, e);
                return;
            }
        }
    }

    // Nettoie les chunks temporaires
    fs::remove_dir_all(chunk_dir).ok();

    // Enregistre dans la table fichiers pour que l'utilisateur puisse y accéder
    let to_user = {
        // On re-lit le transfert pour avoir to_user
        if let Some(t) = p2p_get_transfer(pool, tid) {
            t.get("to_user").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        }
    };

    if to_user > 0 {
        let taille = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0) as i64;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        inserer_ou_modifier(
            pool,
            "fichiers",
            &[
                ("nom", mysql::Value::from(safe_name.as_str())),
                (
                    "fichier",
                    mysql::Value::from(output_path.to_str().unwrap_or("")),
                ),
                (
                    "type_fichier",
                    mysql::Value::from("application/octet-stream"),
                ),
                ("taille", mysql::Value::from(taille)),
                ("visble", mysql::Value::from("prive")),
                ("id_utilisateur", mysql::Value::from(to_user)),
                ("partage", mysql::Value::from("")),
                ("date", mysql::Value::from(now.as_str())),
            ],
            &[],
        );
    }

    eprintln!("[p2p] Fichier reconstitué : {:?}", output_path);
}

// ══════════════════════════════════════════════════════════════════
// ANNUAIRE NOUVEAUTÉS — nœuds/users mis à jour dans les 24h
// ══════════════════════════════════════════════════════════════════
fn generer_annuaire_nouveautes(pool: &DbPool, node_state: &NodeState) -> String {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let now = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let mut out = format!(
        "# VEX AnonNet — Nouveautés (dernières 24h)\n# {}\n\n[NODES]\n\
         # NODE_ID | VEX_URL | TOR_OU_ANONYME | PUB_KEY | STATUT | VU_LE | VERSION\n",
        now
    );

    let peer_rows: Vec<mysql::Row> = conn
        .exec(
            "SELECT node_id, vex_url, tor_addr, pub_key, status, last_seen, version \
         FROM `p2p_peers` WHERE last_seen > NOW() - INTERVAL 24 HOUR ORDER BY last_seen DESC",
            (),
        )
        .unwrap_or_default();

    for row in peer_rows {
        let node_id = row.get::<String, &str>("node_id").unwrap_or_default();
        let vex_url = row.get::<String, &str>("vex_url").unwrap_or_default();
        let tor_raw = row
            .get::<Option<String>, &str>("tor_addr")
            .unwrap_or(None)
            .unwrap_or_default();
        let addr_pub = if tor_raw.is_empty() {
            "ANONYME".to_string()
        } else {
            tor_raw
        };
        let pub_key = row.get::<String, &str>("pub_key").unwrap_or_default();
        let status = row.get::<String, &str>("status").unwrap_or_default();
        let seen = row.get::<String, &str>("last_seen").unwrap_or_default();
        let version = row.get::<String, &str>("version").unwrap_or_default();
        out += &format!(
            "{} | {} | {} | {} | {} | {} | {}\n",
            node_id, vex_url, addr_pub, pub_key, status, seen, version
        );
    }

    out += "\n[USERS]\n# USER_ID@NODE_ID | NOM | PUB_KEY | MIS_A_JOUR\n";

    let user_rows: Vec<mysql::Row> = conn
        .exec(
            "SELECT user_id, node_id, nom, pub_key, updated_at \
         FROM `p2p_users` WHERE updated_at > NOW() - INTERVAL 24 HOUR ORDER BY updated_at DESC",
            (),
        )
        .unwrap_or_default();

    for row in user_rows {
        let uid = row.get::<i64, &str>("user_id").unwrap_or(0);
        let node_id = row.get::<String, &str>("node_id").unwrap_or_default();
        let nom = row.get::<String, &str>("nom").unwrap_or_default();
        let pub_key = row.get::<String, &str>("pub_key").unwrap_or_default();
        let updated = row.get::<String, &str>("updated_at").unwrap_or_default();
        out += &format!(
            "{}@{} | {} | {} | {}\n",
            uid, node_id, nom, pub_key, updated
        );
    }

    let sig = node_state.signer(out.as_bytes());
    out += &format!("\n[SIGNATURE]\n{}:{}\n", node_state.node_id, sig);
    out
}

// ══════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════
fn respond_json(request: Request, body: Value) {
    let _ = request.respond(Response::from_string(body.to_string()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    ));
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// Supprime un suffixe /index.php éventuel et les slashs finaux pour compatibilité bootstrap
fn normalize_bootstrap_url(raw: &str) -> String {
    let mut url = raw.trim().trim_end_matches('/').to_string();
    if url.to_lowercase().ends_with("index.php") {
        url = url[..url.len() - "index.php".len()]
            .trim_end_matches('/')
            .to_string();
    }
    url
}

// ══════════════════════════════════════════════════════════════════
// SECTION ADMIN P2P — appelée depuis Admin.rs dans /api/admin/p2p/*
// ══════════════════════════════════════════════════════════════════
pub fn admin_handle_api(
    pool: &DbPool,
    sub: &str,
    body: &HashMap<String, String>,
    method: &str,
    node_state: &Arc<RwLock<NodeState>>,
) -> Value {
    let ns = node_state.read().unwrap();

    match sub {
        "/p2p" => {
            // Dashboard P2P
            let peers = p2p_lister_peers(pool);
            let online = peers
                .iter()
                .filter(|p| p.get("status").and_then(|v| v.as_str()) == Some("online"))
                .count();
            json!({
                "success": true,
                "data": {
                    "node_id":  ns.node_id,
                "vex_url":  ns.vex_url,
                "pub_key":  ns.pub_key_b64(),
                "version":  env!("CARGO_PKG_VERSION"),
                "bootstrap": ns.config.bootstrap_url,
                "use_tor": ns.config.use_tor,
                "tor_addr": ns.config.tor_addr,
                "chunk_size_bytes": ns.config.chunk_size_bytes,
                "peers_total":  peers.len(),
                "peers_online": online,
            }
            })
        }

        "/p2p/peers" => {
            let peers = p2p_lister_peers(pool);
            let total = peers.len();
            let online = peers
                .iter()
                .filter(|p| p.get("status").and_then(|v| v.as_str()) == Some("online"))
                .count();
            json!({"success":true,"data":{"peers":peers,"total":total,"online":online}})
        }

        "/p2p/users" => {
            let users = p2p_lister_users(pool);
            json!({"success":true,"data":{"users":users,"total":users.len()}})
        }

        "/p2p/annuaire" => {
            let texte = generer_annuaire(pool, &ns);
            json!({"success":true,"data":{"annuaire":texte}})
        }

        "/p2p/sync_now" => {
            // Force une sync immédiate avec le bootstrap (bloquant ~5s max)
            drop(ns); // libère le lock avant la sync
            let ns2 = node_state.read().unwrap();
            sync_avec_bootstrap(pool, &ns2);
            json!({"success":true,"message":"Sync bootstrap effectuée."})
        }

        "/p2p/kick" => {
            let peer_id = body
                .get("peer_id")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            crate::appeldb::supprimer_ligne(pool, "p2p_peers", "id", mysql::Value::from(peer_id));
            json!({"success":true,"message":"Peer supprimé."})
        }

        "/p2p/transfers" => {
            let user_id = body
                .get("user_id")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let transfers = if user_id > 0 {
                p2p_lister_transfers(pool, user_id)
            } else {
                // Admin voit tout
                let mut conn = match pool.get_conn() {
                    Ok(c) => c,
                    Err(_) => return json!({"success":false,"error":"DB error"}),
                };
                let rows: Vec<mysql::Row> = conn
                    .exec(
                        "SELECT * FROM `p2p_transfers` ORDER BY created_at DESC LIMIT 100",
                        (),
                    )
                    .unwrap_or_default();
                rows.into_iter()
                    .map(|row: mysql::Row| {
                        let cols = row.columns_ref();
                        let mut map: HashMap<String, serde_json::Value> = HashMap::new();
                        for (i, col) in cols.iter().enumerate() {
                            let val: mysql::Value = row
                                .get::<mysql::Value, usize>(i)
                                .unwrap_or(mysql::Value::NULL);
                            map.insert(col.name_str().to_string(), mysql_val_to_json(val));
                        }
                        map
                    })
                    .collect()
            };
            json!({"success":true,"data":{"transfers":transfers}})
        }

        _ => json!({"success":false,"error":"Route P2P admin inconnue."}),
    }
}
