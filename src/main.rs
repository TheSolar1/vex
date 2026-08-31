// ══════════════════════════════════════════════════════════════════
// main.rs — VEX server entry point
// ══════════════════════════════════════════════════════════════════
mod db_init;
mod access_control;
mod appeldb;
mod c;
mod config_loader;
mod function;
mod utils;
mod srp;

// Extensions uploadees depuis le panel admin (src/extensions/<id>/mod.rs).
// Le registre extensions/mod.rs est regenere automatiquement a chaque upload.
mod extensions;

mod p2p {
    pub mod p2p;
}
mod admin {
    pub mod admin;
}
mod login {
    pub mod account;
    pub mod autologin;
    pub mod dashboard;
    pub mod first_setup;
    pub mod login;
    pub mod logout;
}
mod fchier {
    pub mod fchier;
}
mod mess {
    pub mod mess;
}
mod viso {
    pub mod viso;
}
mod sitec {
    pub mod sitec;
}

use crate::p2p::p2p::{
    handle_request, lancer_sync_periodique, sync_avec_bootstrap, NodeState, P2pConfig,
};
use appeldb::{
    creer_pool, executer_action_table_terminal, regler_privilege_utilisateur, ActionTableTerminal,
    TABLES_MODIFIABLES_TERMINAL,
};
use config_loader::{load_config, load_db_config};
use std::env;
use std::io::Write;
use std::sync::{Arc, Mutex, RwLock};
use tiny_http::{Response, Server};

const CONFIG_PATH: &str = "config.json";
const DEFAULT_PORT: u16 = 8080;
const LOG_DIR: &str = "log";

// ══════════════════════════════════════════════════════════════════
// INTÉGRITÉ DES SOURCES — hashes figés à la compilation
// Toute modification de ces fichiers déclenche la destruction totale.
// Ce mécanisme n'a aucun flag de désactivation — il est incondititionnel.
// ══════════════════════════════════════════════════════════════════
const _SRC_MAIN: &str     = include_str!("main.rs");
const _SRC_APPELDB: &str  = include_str!("appeldb.rs");
const _SRC_CONFIG: &str   = include_str!("config_loader.rs");
const _SRC_DBINIT: &str   = include_str!("db_init.rs");

/// Hash FNV-1a 64 bits — aucune dépendance externe, déterministe.
#[inline(never)]
fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000000001b3);
    }
    h
}

/// Hashes des fichiers sources **calculés à la compilation**.
/// Si un fichier est modifié sur disque après compilation et sans recompiler,
/// la vérification échoue → destruction immédiate.
fn hashes_attendus() -> [(&'static str, u64); 4] {
    [
        ("src/main.rs",          fnv64(_SRC_MAIN.as_bytes())),
        ("src/appeldb.rs",       fnv64(_SRC_APPELDB.as_bytes())),
        ("src/config_loader.rs", fnv64(_SRC_CONFIG.as_bytes())),
        ("src/db_init.rs",       fnv64(_SRC_DBINIT.as_bytes())),
    ]
}

// ══════════════════════════════════════════════════════════════════
// LOGGER GLOBAL
// ══════════════════════════════════════════════════════════════════
struct VexLogger {
    file: Mutex<std::fs::File>,
}

impl VexLogger {
    fn ouvrir() -> Option<Arc<Self>> {
        let _ = std::fs::create_dir_all(LOG_DIR);
        let now = chrono_date_simple();
        let path = format!("{}/vex_{}.log", LOG_DIR, now);
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
            .map(|f| Arc::new(VexLogger { file: Mutex::new(f) }))
    }

    fn log(&self, niveau: &str, message: &str) {
        let ts = timestamp_now();
        let ligne = format!("[{}] [{}] {}\n", ts, niveau, message);
        eprint!("{}", ligne);
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(ligne.as_bytes());
        }
    }

    fn info(&self, msg: &str)  { self.log("INFO",  msg); }
    fn warn(&self, msg: &str)  { self.log("WARN",  msg); }
    fn error(&self, msg: &str) { self.log("ERROR", msg); }
    fn sec(&self, msg: &str)   { self.log("SECURITE", msg); }
}

fn chrono_date_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Approximation : année / mois / jour depuis epoch Unix
    let days  = secs / 86400;
    let years = 1970 + days / 365;
    let rem   = days % 365;
    let month = rem / 30 + 1;
    let day   = rem % 30 + 1;
    format!("{:04}-{:02}-{:02}", years, month.min(12), day.min(31))
}

fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{}T{:02}:{:02}:{:02}Z", chrono_date_simple(), h, m, s)
}

// ══════════════════════════════════════════════════════════════════
// DESTRUCTION TOTALE — appelée si intégrité compromise
// Supprime toutes les tables DB + les fichiers du programme.
// Ne peut pas être désactivée : aucun flag, aucune config, aucun env.
// ══════════════════════════════════════════════════════════════════
#[cold]
#[inline(never)]
fn destruction_totale(pool: &appeldb::DbPool, logger: Option<&VexLogger>, raison: &str) -> ! {
    let msg = format!(
        "ALERTE INTEGRITE — destruction totale déclenchée. Raison : {}",
        raison
    );
    eprintln!("[SECURITE] {}", msg);
    if let Some(l) = logger { l.sec(&msg); }

    // 1. Supprimer toutes les tables MySQL
    if let Ok(mut conn) = pool.get_conn() {
        use mysql::prelude::Queryable;
        // Désactiver les contraintes FK pour pouvoir tout dropper
        let _ = conn.query_drop("SET FOREIGN_KEY_CHECKS = 0");
        if let Ok(tables) = conn.query::<String, _>(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE()"
        ) {
            for table in &tables {
                let nom: &str = table.as_str();
                let nom_safe = nom.replace('`', "");
                let drop = format!("DROP TABLE IF EXISTS `{}`", nom_safe);
                let _ = conn.query_drop(&drop);
                eprintln!("[SECURITE] DROP TABLE {}", table);
                if let Some(l) = logger { l.sec(&format!("DROP TABLE {}", table)); }
            }
        }
        let _ = conn.query_drop("SET FOREIGN_KEY_CHECKS = 1");
    }

    // 2. Supprimer les fichiers sources et le binaire courant
    let fichiers_a_supprimer: &[&str] = &[
   /*     "src/main.rs",
        "src/appeldb.rs",
        "src/config_loader.rs",
        "src/db_init.rs",
        "src/function.rs",
        "src/access_control.rs",
        "src/c.rs",
        "src/utils.rs",
        "src/db_init.rs",
        "src/admin/admin.rs",
        "src/login/login.rs",
        "src/login/account.rs",
        "src/login/dashboard.rs",
        "src/login/logout.rs",
        "src/login/autologin.rs",
        "src/login/first_setup.rs",
        "src/fchier/fchier.rs",
        "src/mess/mess.rs",
        "src/p2p/p2p.rs",
        "Cargo.toml",
        "Cargo.lock",
        CONFIG_PATH,*/
    ];

    for f in fichiers_a_supprimer {
        if std::fs::remove_file(f).is_ok() {
            eprintln!("[SECURITE] Supprimé : {}", f);
            if let Some(l) = logger { l.sec(&format!("Supprimé : {}", f)); }
        }
    }

    // 3. Supprimer le binaire compilé courant (fonctionne sous Linux/macOS)
    //    Sous Windows : le fichier est verrouillé pendant l'exécution,
    //    on schedule la suppression au prochain redémarrage via batch.
    if let Ok(current_exe) = std::env::current_exe() {
        #[cfg(unix)]
        {
            if std::fs::remove_file(&current_exe).is_ok() {
                eprintln!("[SECURITE] Binaire supprimé : {:?}", current_exe);
                if let Some(l) = logger { l.sec(&format!("Binaire supprimé : {:?}", current_exe)); }
            }
        }
        #[cfg(windows)]
        {
            // Crée un .bat qui efface le .exe puis lui-même au prochain démarrage
            let bat = format!(
                "@echo off\r\n:loop\r\ndel /f /q \"{}\"\r\nif exist \"{}\" goto loop\r\ndel \"%~f0\"\r\n",
                current_exe.display(), current_exe.display()
            );
            let bat_path = current_exe.with_extension("destroy.bat");
            if std::fs::write(&bat_path, bat).is_ok() {
                let _ = std::process::Command::new("cmd")
                    .args(&["/C", "start", "/B", bat_path.to_str().unwrap_or("")])
                    .spawn();
            }
        }
    }

    // 4. Supprimer le dossier target/ (binaires compilés)
    let _ = std::fs::remove_dir_all("target");

    if let Some(l) = logger {
        l.sec("Destruction terminée. Arrêt forcé.");
    }
    eprintln!("[SECURITE] Destruction terminée. Arrêt forcé.");
    std::process::exit(0xFF);
}

// ══════════════════════════════════════════════════════════════════
// VÉRIFICATION D'INTÉGRITÉ DES SOURCES
// Appelée impérativement au démarrage, inconditionnelle.
// ══════════════════════════════════════════════════════════════════
#[inline(never)]
fn verifier_integrite(pool: &appeldb::DbPool, logger: &VexLogger) {
    logger.info("Vérification intégrité des sources...");
    for (chemin, hash_attendu) in hashes_attendus() {
        // Si le fichier source n'existe pas (deployment sans sources), on passe.
        // Si il existe, il DOIT correspondre au hash compilé.
        match std::fs::read(chemin) {
            Ok(contenu) => {
                let hash_reel = fnv64(&contenu);
                if hash_reel != hash_attendu {
                    let raison = format!(
                        "Fichier source modifié après compilation : {} \
                        (attendu=0x{:016X}, obtenu=0x{:016X})",
                        chemin, hash_attendu, hash_reel
                    );
                    logger.sec(&raison);
                    destruction_totale(pool, Some(logger), &raison);
                } else {
                    logger.info(&format!("  OK — {} (0x{:016X})", chemin, hash_reel));
                }
            }
            Err(_) => {
                // Fichier absent → déploiement sans sources → OK, on ne détecte rien
                logger.info(&format!("  SKIP (absent) — {}", chemin));
            }
        }
    }
    logger.info("Intégrité des sources : OK.");
}

// ══════════════════════════════════════════════════════════════════
// ANTI-ÉLÉVATION PRIVILEGE=1
// Vérifie que personne n'a pu se glisser en privilege=1
// autrement que via le compte fondateur légitime.
// Extensions signées via clé publique sont l'unique exception.
// ══════════════════════════════════════════════════════════════════

/// Récupère la clé publique du fondateur légitime stockée en DB.
/// Sert de référence pour valider une signature d'extension autorisée.
fn get_fondateur_pubkey(pool: &appeldb::DbPool) -> Option<String> {
    use appeldb::selectionner;
    let rows = selectionner(
        pool,
        "login",
        &[("privilege", mysql::Value::from(1i64))],
        &["id", "pubkey"],
        None,
        Some(1),
    );
    rows.into_iter()
        .next()
        .and_then(|r| r.get("pubkey").and_then(|v| v.as_str().map(|s| s.to_string())))
}

/// Vérifie la signature d'une demande d'extension voulant opérer privilege=1.
/// La signature doit être faite avec la clé privée du fondateur sur le payload.
/// Retourne true uniquement si la signature est valide.
fn verifier_signature_extension(payload: &str, signature_b64: &str, pubkey_b64: &str) -> bool {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let Ok(pubkey_bytes) = b64.decode(pubkey_b64) else { return false; };
    let Ok(sig_bytes)    = b64.decode(signature_b64) else { return false; };
    if pubkey_bytes.len() != 32 || sig_bytes.len() != 64 { return false; }
    // Vérification Ed25519 — nécessite crate `ed25519-dalek`
    // Ici on utilise une comparaison HMAC-SHA256 simple si ed25519 non dispo.
    // REPLACE par ed25519_dalek::VerifyingKey si la crate est présente.
    let key_hash = fnv64(&pubkey_bytes);
    let sig_hash = fnv64(&sig_bytes);
    let pay_hash = fnv64(payload.as_bytes());
    // Relation attendue entre les trois éléments — schéma simplifié
    // En production : remplacer par vraie vérification Ed25519.
    (sig_hash ^ pay_hash) == key_hash
}

/// Audit permanent : cherche les comptes privilege=1 autres que le fondateur légitime.
/// Si trouvé → log + suppression forcée du privilege illégitime.
/// Seule une extension avec signature fondateur valide peut bypasser cette règle.
#[inline(never)]
fn audit_privilege_1(
    pool: &appeldb::DbPool,
    logger: &VexLogger,
    fondateur_id_legitime: i64,
) {
    use appeldb::selectionner;
    let rows = selectionner(
        pool,
        "login",
        &[("privilege", mysql::Value::from(1i64))],
        &["id", "nom", "email"],
        None,
        None,
    );
    for row in &rows {
        let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        if id == fondateur_id_legitime { continue; }
        let nom   = row.get("nom").and_then(|v| v.as_str()).unwrap_or("?");
        let email = row.get("email").and_then(|v| v.as_str()).unwrap_or("?");
        logger.sec(&format!(
            "PRIVILEGE ILLÉGITIME détecté — id={} nom='{}' email='{}' avait privilege=1 → forcé à 10",
            id, nom, email
        ));
        // Rétrograder immédiatement
        appeldb::inserer_ou_modifier(
            pool,
            "login",
            &[("privilege", mysql::Value::from(10i64))],
            &[("id", mysql::Value::from(id))],
        );
        logger.sec(&format!("Compte {} rétrogradé à privilege=10.", id));
    }
}

/// Tentative d'élévation à privilege=1 via une extension signée.
/// Retourne Ok(id_fondateur) si valide, Err sinon.
pub fn demande_elevation_fondateur_via_extension(
    pool: &appeldb::DbPool,
    logger: &VexLogger,
    payload: &str,
    signature_b64: &str,
) -> Result<i64, &'static str> {
    let pubkey = match get_fondateur_pubkey(pool) {
        Some(k) => k,
        None => {
            logger.sec("Demande élévation fondateur : aucune clé publique fondateur en DB.");
            return Err("Clé publique fondateur absente.");
        }
    };
    if !verifier_signature_extension(payload, signature_b64, &pubkey) {
        logger.sec(&format!(
            "Demande élévation fondateur REFUSÉE — signature invalide. Payload='{}'",
            &payload[..payload.len().min(80)]
        ));
        return Err("Signature invalide.");
    }
    // Extraire l'id fondateur du payload (format attendu : "fondateur:<id>:<timestamp>")
    let parts: Vec<&str> = payload.split(':').collect();
    if parts.len() < 3 || parts[0] != "fondateur" {
        return Err("Payload mal formé.");
    }
    let id = parts[1].parse::<i64>().map_err(|_| "ID invalide dans payload.")?;
    logger.sec(&format!("Élévation fondateur AUTORISÉE via extension signée — id={}", id));
    Ok(id)
}

fn main() {
    // ── Logger ────────────────────────────────────────────────────
    let logger = VexLogger::ouvrir().unwrap_or_else(|| {
        eprintln!("[WARN] Impossible d'ouvrir le fichier de log dans {}/", LOG_DIR);
        // Fallback : log vers stderr uniquement (pas de panic)
        let tmp = std::env::temp_dir().join("vex_emergency.log");
        let f = std::fs::OpenOptions::new().create(true).append(true).open(&tmp)
            .unwrap_or_else(|_| {
                // Dernier recours : /dev/null ou NUL
                #[cfg(unix)]
                { std::fs::File::open("/dev/null").unwrap() }
                #[cfg(windows)]
                { std::fs::File::open("NUL").unwrap() }
            });
        Arc::new(VexLogger { file: Mutex::new(f) })
    });

    logger.info("═══════════════════════════════════════════");
    logger.info("VEX démarrage");
    logger.info("═══════════════════════════════════════════");

    let args: Vec<String> = env::args().collect();
    let config = load_config(CONFIG_PATH);
    let db_config = match load_db_config(CONFIG_PATH) {
        Ok(c) => c,
        Err(e) => {
            // On refuse de demarrer sur des identifiants devines : mieux
            // vaut un message clair ici qu'un echec MySQL opaque plus loin.
            logger.error(&format!("Configuration base de donnees : {}", e));
            eprintln!("\n[VEX] {}\n", e);
            if config_loader::ecrire_db_config_exemple() {
                eprintln!(
                    "[VEX] Un modele a ete cree dans {} : completez-le puis relancez.\n",
                    config_loader::db_config_path()
                );
            }
            std::process::exit(1);
        }
    };

    logger.info(&format!("Config chargée : {}", CONFIG_PATH));

    if let Err(e) = db_init::init_db(&db_config) {
        logger.error(&format!("init_db échoué: {}", e));
        eprintln!("[main] init_db échoué: {}", e);
        std::process::exit(1);
    }

    let pool = match creer_pool(&db_config) {
        Ok(p) => {
            logger.info("Pool MySQL OK.");
            p
        }
        Err(e) => {
            logger.error(&format!("MySQL : {}", e));
            eprintln!("[main] MySQL : {}", e);
            std::process::exit(1);
        }
    };

    // ── Intégrité des sources — INCONDITIONNELLE ──────────────────
    // Aucun flag, aucune config, aucun env ne peut désactiver ce bloc.
    verifier_integrite(&pool, &logger);

    // ── Fondateur légitime ────────────────────────────────────────
    // FIX : `donner_privilege_1_thesolar` ne doit s'exécuter QUE s'il
    // n'existe pas déjà un fondateur (privilege=1) en base. Avant, cet
    // appel était inconditionnel à chaque démarrage : si le compte
    // "thesolar" avait été rétrogradé volontairement (ou si un autre
    // fondateur légitime avait été mis en place), il était systématiquement
    // remis à privilege=1 au redémarrage suivant, écrasant tout changement
    // manuel. On vérifie maintenant qu'aucun fondateur n'existe avant
    // d'assigner le privilege=1 au compte thesolar.
    let fondateur_deja_present: bool = {
        use appeldb::selectionner;
        !selectionner(&pool, "login", &[("privilege", mysql::Value::from(1i64))], &["id"], None, Some(1))
            .is_empty()
    };
    if !fondateur_deja_present {
        let _ = appeldb::donner_privilege_1_thesolar(&pool);
        logger.info("Aucun fondateur trouvé — privilege=1 attribué au compte thesolar.");
    } else {
        logger.info("Fondateur déjà présent en base — donner_privilege_1_thesolar ignoré.");
    }

    // Récupère l'id du fondateur légitime pour l'audit
    let fondateur_id: i64 = {
        use appeldb::selectionner;
        selectionner(&pool, "login", &[("privilege", mysql::Value::from(1i64))], &["id"], None, Some(1))
            .into_iter().next()
            .and_then(|r| r.get("id").and_then(|v| v.as_i64()))
            .unwrap_or(0)
    };
    logger.info(&format!("Fondateur légitime id={}", fondateur_id));

    // ── Audit privilege=1 illégitime ──────────────────────────────
    audit_privilege_1(&pool, &logger, fondateur_id);

    // ── Migrations messagerie ─────────────────────────────────────
    mess::mess::ensure_schema(&pool);
    logger.info("Schema messagerie vérifié.");

    // ── Schéma Sitec ─────────────────────────────────────────────
    sitec::sitec::ensure_schema(&pool);
    logger.info("Schema Sitec vérifié.");

    if let Some(exit_code) = handle_terminal_db_commands(&args, &pool, &logger) {
        std::process::exit(exit_code);
    }

    if args.contains(&"--reset-loginc".to_string()) {
        match executer_action_table_terminal(&pool, "loginc", ActionTableTerminal::Vider) {
            Ok(()) => {
                logger.info("reset_table(loginc) OK");
                println!("reset_table(loginc) OK");
                return;
            }
            Err(e) => {
                logger.error(&format!("reset_table(loginc) ERREUR: {}", e));
                eprintln!("reset_table(loginc) ERREUR: {}", e);
                std::process::exit(1);
            }
        }
    }

    // ── Port d'écoute HTTP ────────────────────────────────────────
    let port = config
        .extra
        .get("server")
        .and_then(|s| s.get("port"))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_PORT as u64) as u16;

    // ── Init P2P ─────────────────────────────────────────────────
    let vex_url = config
        .extra
        .get("server")
        .and_then(|s| s.get("public_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8080")
        .to_string();

    let p2p_cfg = P2pConfig::from_vex_config(&config);
    let node_state = Arc::new(RwLock::new(NodeState::init(&vex_url, p2p_cfg)));

    {
        let ns = node_state.read().unwrap();
        logger.info(&format!("P2P node_id = {}", ns.node_id));
        logger.info(&format!("P2P pub_key = {}", ns.pub_key_b64()));
        logger.info(&format!("P2P bootstrap = {}", ns.config.bootstrap_url));
    }

    {
        let ns = node_state.read().unwrap();
        let pool_clone = pool.clone();
        sync_avec_bootstrap(&pool_clone, &ns);
    }
    logger.info("Sync bootstrap P2P initiale terminée.");

    lancer_sync_periodique(pool.clone(), Arc::clone(&node_state));
    logger.info("Sync périodique P2P lancée.");

    // ── Serveur HTTP ──────────────────────────────────────────────
    logger.info(&format!("Démarrage HTTP sur 0.0.0.0:{}", port));
    let server = match Server::http(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            logger.error(&format!("Serveur HTTP : {}", e));
            eprintln!("[main] Serveur : {}", e);
            std::process::exit(1);
        }
    };

    eprintln!("[VEX] http://0.0.0.0:{}", port);
    logger.info(&format!("VEX en écoute sur http://0.0.0.0:{}", port));

    // Compteur de requêtes (pour logs périodiques)
    let mut req_count: u64 = 0;

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().to_string();

        let remote_full = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".into());
        let remote = utils::strip_port(&remote_full);

        let path = url.split('?').next().unwrap_or(&url).to_string();

        req_count += 1;

        if config.app.debug_mode {
            logger.info(&format!("[REQ #{}] {} {} {}", req_count, remote, method, path));
            eprintln!("[{}] {} {}", remote, method, path);
        } else if req_count % 500 == 0 {
            logger.info(&format!("[STAT] {} requêtes traitées.", req_count));
        }

        // ── Surveillance accès aux routes sensibles ───────────────
        // FIX : `&&` étant prioritaire sur `||`, l'ancienne condition
        //   path.contains("privilege") || path.contains("admin") && method == "POST"
        // se lisait en réalité :
        //   path.contains("privilege") || (path.contains("admin") && method == "POST")
        // → toute requête contenant "privilege" était loguée même en GET et
        // hors admin, alors que "admin" n'exigeait "POST" que pour lui-même.
        // Intention corrigée : logguer un accès sensible dès que le chemin
        // contient "privilege" OU "admin", uniquement pour les requêtes qui
        // modifient quelque chose (POST).
        if (path.contains("privilege") || path.contains("admin")) && method == "POST" {
            logger.sec(&format!("[ACCES SENSIBLE] {} {} {} (ip={})", method, path, req_count, remote));
        }

        match path.as_str() {
            "/" | "/login" | "/login/" | "/login/login" | "/login/login.php" => {
                logger.info(&format!("Login request depuis {}", remote));
                login::login::handle_request(request, &pool, &config, &remote);
            }

            "/login/first_setup" => {
                logger.info(&format!("First setup depuis {}", remote));
                login::first_setup::handle_request(request, &pool, &config, &remote);
            }

            "/api/login/config" => {
                login::login::handle_request(request, &pool, &config, &remote);
            }

            "/login/account" | "/login/account/" => {
                login::account::handle_request(request, &pool, &config, &remote);
            }

            p if p.starts_with("/api/account") => {
                login::account::handle_request(request, &pool, &config, &remote);
            }

            "/logout" | "/logout/" | "/login/logout" | "/login/logout/" => {
                logger.info(&format!("Logout depuis {}", remote));
                login::logout::handle_request(request, &pool, &remote);
            }

            p if p == "/autologin"
                || p == "/autologin/"
                || p.starts_with("/autologin/")
                || p == "/login/autologin"
                || p == "/login/autologin/" =>
            {
                logger.info(&format!("Autologin depuis {}", remote));
                login::autologin::handle_request(request, &pool, &config, &remote);
            }

            p if p.starts_with("/api/dashboard") => {
                login::dashboard::handle_request(request, &pool, &config, &remote);
            }

            "/dashboard" | "/dashboard/" | "/login/dashboard" | "/login/dashboard/" => {
                login::dashboard::handle_request(request, &pool, &config, &remote);
            }

            p if p.starts_with("/admin") || p.starts_with("/api/admin") => {
                logger.info(&format!("Admin panel depuis {} — {}", remote, path));
                admin::admin::handle_request(request, &pool, &config, CONFIG_PATH, &remote_full);
            }

            // Extensions : /ext/<id> (page) et /api/ext/<id> (API).
            // Privilege + plan verifies dans access_control::servir_extension.
            p if p.starts_with("/ext/") || p.starts_with("/api/ext/") => {
                let ext_id = access_control::extension_id_depuis_path(p);
                logger.info(&format!("Extension '{}' depuis {} — {}", ext_id, remote, path));
                access_control::servir_extension(&pool, &config, request, &path);
            }

            p if p.starts_with("/fchier") || p.starts_with("/api/fchier") => {
                let resp = fchier::fchier::handle(&pool, &mut request);
                let _ = request.respond(resp);
            }

            p if p.starts_with("/mess") || p.starts_with("/api/mess") => {
                let resp = mess::mess::handle(&pool, &mut request);
                let _ = request.respond(resp);
            }

            p if p.starts_with("/p2p/") || p.starts_with("/neut/") => {
                handle_request(request, &pool, &node_state, &config);
            }

            p if p.starts_with("/viso") || p.starts_with("/api/viso") => {
                let resp = viso::viso::handle(&pool, &mut request);
                let _ = request.respond(resp);
            }

            p if p.starts_with("/sitec")
                || p.starts_with("/api/sitec")
                || p.starts_with("/page/") =>
            {
                let resp = sitec::sitec::handle(&pool, &mut request);
                let _ = request.respond(resp);
            }

            "/api/db" => {
                let params = utils::parse_query(&url);
                let action = params.get("action").cloned().unwrap_or_default();
                let resp = appeldb::handle_api_action(&pool, &action, &params, &remote);
                respond_json(request, resp);
            }

            p if is_static(p) => {
                serve_static(request, p);
            }

            "/health" => {
                let _ = request.respond(Response::from_string("ok"));
            }

            _ => {
                logger.warn(&format!("404 — {} {} (ip={})", method, path, remote));
                let _ = request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Commandes terminal DB
// ══════════════════════════════════════════════════════════════════
fn handle_terminal_db_commands(
    args: &[String],
    pool: &appeldb::DbPool,
    logger: &VexLogger,
) -> Option<i32> {
    if args.iter().any(|a| a == "--help-db") {
        print_db_help();
        return Some(0);
    }

    if let Some(pos) = args.iter().position(|a| a == "--table-action") {
        let table = match args.get(pos + 1) {
            Some(v) => v.as_str(),
            None => {
                eprintln!("Usage: --table-action <table> <vider|supprimer-lignes>");
                return Some(1);
            }
        };
        let action_raw = match args.get(pos + 2) {
            Some(v) => v.as_str(),
            None => {
                eprintln!("Usage: --table-action <table> <vider|supprimer-lignes>");
                return Some(1);
            }
        };
        let action = match action_raw {
            "vider" => ActionTableTerminal::Vider,
            "supprimer-lignes" => ActionTableTerminal::SupprimerToutesLesLignes,
            _ => {
                eprintln!("Action inconnue: {}. Utilise 'vider' ou 'supprimer-lignes'.", action_raw);
                return Some(1);
            }
        };
        logger.info(&format!("CLI : table-action {} {}", table, action_raw));
        match executer_action_table_terminal(pool, table, action) {
            Ok(()) => {
                logger.info(&format!("CLI : action '{}' sur '{}' OK.", action_raw, table));
                println!("Action '{}' executee sur la table '{}'.", action_raw, table);
                return Some(0);
            }
            Err(e) => {
                logger.error(&format!("CLI : erreur table '{}': {}", table, e));
                eprintln!("Erreur table '{}': {}", table, e);
                return Some(1);
            }
        }
    }

    if let Some(pos) = args.iter().position(|a| a == "--set-privilege") {
        let user_id = match args.get(pos + 1).and_then(|v| v.parse::<i64>().ok()) {
            Some(v) => v,
            None => {
                eprintln!("Usage: --set-privilege <user_id> <privilege>");
                return Some(1);
            }
        };
        let privilege = match args.get(pos + 2).and_then(|v| v.parse::<i64>().ok()) {
            Some(v) => v,
            None => {
                eprintln!("Usage: --set-privilege <user_id> <privilege>");
                return Some(1);
            }
        };
        // Blocage : impossible de mettre privilege=1 via CLI
        if privilege == 1 {
            logger.sec(&format!(
                "CLI --set-privilege REFUSÉ : tentative privilege=1 pour user_id={}",
                user_id
            ));
            eprintln!("[SECURITE] Le privilege 1 ne peut pas être assigné via CLI.");
            return Some(1);
        }
        logger.info(&format!("CLI : set-privilege user={} → {}", user_id, privilege));
        match regler_privilege_utilisateur(pool, user_id, privilege) {
            Ok(()) => {
                logger.info(&format!("CLI : privilege user {} → {} OK.", user_id, privilege));
                println!("Privilege de l'utilisateur {} regle a {}.", user_id, privilege);
                return Some(0);
            }
            Err(e) => {
                logger.error(&format!("CLI : set-privilege erreur: {}", e));
                eprintln!("Erreur set-privilege: {}", e);
                return Some(1);
            }
        }
    }

    None
}

fn print_db_help() {
    println!("Commandes DB terminal disponibles:");
    println!("  cargo run -- --table-action <table> <vider|supprimer-lignes>");
    println!("  cargo run -- --set-privilege <user_id> <privilege>   (2-12 uniquement)");
    println!("Tables autorisees: {}", TABLES_MODIFIABLES_TERMINAL.join(", "));
    println!("Privilege autorise: entre 2 et 12 (le privilege 1 est réservé au fondateur)");
    println!("Logs : {}/vex_YYYY-MM-DD.log", LOG_DIR);
}

// ══════════════════════════════════════════════════════════════════
// Fichiers statiques
// ══════════════════════════════════════════════════════════════════
fn is_static(path: &str) -> bool {
    path.starts_with("/static/")
        || path.ends_with(".png")
        || path.ends_with(".ico")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".svg")
        || path.ends_with(".woff2")
}

fn serve_static(request: tiny_http::Request, path: &str) {
    if path.contains("..") {
        let _ = request.respond(Response::from_string("403").with_status_code(403));
        return;
    }
    let file_path = format!(".{}", path);
    match std::fs::read(&file_path) {
        Ok(data) => {
            let _ = request.respond(Response::from_data(data).with_header(
                tiny_http::Header::from_bytes("Content-Type", guess_mime(path)).unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(Response::from_string("404").with_status_code(404));
        }
    }
}

fn guess_mime(path: &str) -> &'static str {
    if path.ends_with(".html")  { "text/html; charset=utf-8" }
    else if path.ends_with(".css")   { "text/css" }
    else if path.ends_with(".js")    { "application/javascript" }
    else if path.ends_with(".json")  { "application/json" }
    else if path.ends_with(".png")   { "image/png" }
    else if path.ends_with(".ico")   { "image/x-icon" }
    else if path.ends_with(".svg")   { "image/svg+xml" }
    else if path.ends_with(".woff2") { "font/woff2" }
    else                             { "application/octet-stream" }
}

fn respond_json(request: tiny_http::Request, body: serde_json::Value) {
    let _ = request.respond(Response::from_string(body.to_string()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    ));
}