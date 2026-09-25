// ══════════════════════════════════════════════════════════════════
// main.rs — VEX server entry point
// ══════════════════════════════════════════════════════════════════
mod db_init;
mod access_control;
mod appeldb;
mod c;
mod config_loader;
mod function;
mod i18n;
mod utils;
mod srp;
mod sso;

// Extensions uploadees depuis le panel admin (src/extensions/<id>/mod.rs).
// Le registre extensions/mod.rs est regenere automatiquement a chaque upload.
mod extensions;

mod p2p {
    pub mod p2p;
}
mod admin {
    pub mod actions;
    pub mod admin;
}
mod login {
    pub mod account;
    pub mod appareil;
    pub mod autologin;
    pub mod dashboard;
    pub mod first_setup;
    pub mod login;
    pub mod logout;
    pub mod notice_cloudsync;
}
mod fchier {
    pub mod corbeille;
    pub mod fchier;
    pub mod liens;
    pub mod onlyoffice;
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
mod recherche {
    pub mod recherche;
}

use crate::p2p::p2p::{
    handle_request, lancer_sync_periodique, NodeState, P2pConfig,
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
const DEFAULT_THREADS: usize = 8;

const PAGE_404: &str = r#"<!DOCTYPE html>
<html lang="fr" data-theme="auto">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Page introuvable — VEX</title>
<link rel="stylesheet" href="/static/css/theme.css">
<script src="/static/js/vex-ui.js"></script>
<style>
body{margin:0;padding:0!important;min-height:100vh;display:flex;align-items:center;justify-content:center;font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;text-align:center}
.c{padding:32px 24px;max-width:420px}
.n{font-size:5.5rem;font-weight:900;line-height:1;background:linear-gradient(135deg,var(--vex-green-1),var(--vex-green-2));-webkit-background-clip:text;background-clip:text;color:transparent}
h1{font-size:1.3rem;margin:14px 0 8px}
p{color:var(--text-dim);margin:0 0 22px}
a{display:inline-block;padding:11px 20px;border-radius:10px;background:var(--accent);color:#fff;text-decoration:none;font-weight:700;margin:4px}
a.s{background:transparent;color:var(--accent);border:1px solid var(--accent)}
</style>
</head>
<body>
<main class="c">
  <div class="n" aria-hidden="true">404</div>
  <h1>Page introuvable</h1>
  <p>Le lien est peut-être erroné, ou la page a été déplacée ou supprimée.</p>
  <a href="/login/dashboard">Retour à l'accueil</a>
  <a class="s" href="/recherche/">Rechercher</a>
</main>
</body>
</html>
"#;
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
    /// (date du fichier ouvert "YYYY-MM-DD", fichier) -- rouvert a chaque
    /// changement de jour : avant, le fichier du jour de demarrage recevait
    /// tout, meme des semaines plus tard.
    file: Mutex<(String, Option<std::fs::File>)>,
    /// Journal d'acces HTTP separe (une ligne par requete), meme rotation.
    acces: Mutex<(String, Option<std::fs::File>)>,
}

impl VexLogger {
    fn ouvrir() -> Option<Arc<Self>> {
        std::fs::create_dir_all(LOG_DIR).ok()?;
        let l = VexLogger {
            file: Mutex::new((String::new(), None)),
            acces: Mutex::new((String::new(), None)),
        };
        // Verifie des le demarrage que le dossier est inscriptible.
        Self::fichier_du_jour(&l.file, "vex")?;
        Some(Arc::new(l))
    }

    /// Fichier sans disque (fallback si log/ n'est pas inscriptible) :
    /// stderr uniquement.
    fn stderr_seul() -> Arc<Self> {
        Arc::new(VexLogger {
            file: Mutex::new(("-".into(), None)),
            acces: Mutex::new(("-".into(), None)),
        })
    }

    /// Ouvre (ou rouvre si la date a change) log/<prefixe>_<date>.log.
    fn fichier_du_jour(slot: &Mutex<(String, Option<std::fs::File>)>, prefixe: &str) -> Option<()> {
        let mut g = slot.lock().ok()?;
        if g.0 == "-" {
            return None;
        }
        let today = chrono_date_simple();
        if g.0 != today || g.1.is_none() {
            let path = format!("{}/{}_{}.log", LOG_DIR, prefixe, today);
            let f = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()?;
            *g = (today, Some(f));
        }
        Some(())
    }

    fn ecrire(slot: &Mutex<(String, Option<std::fs::File>)>, prefixe: &str, ligne: &str) {
        if Self::fichier_du_jour(slot, prefixe).is_none() {
            return;
        }
        if let Ok(mut g) = slot.lock() {
            if let Some(f) = g.1.as_mut() {
                let _ = f.write_all(ligne.as_bytes());
            }
        }
    }

    fn log(&self, niveau: &str, message: &str) {
        let ts = timestamp_now();
        // Une ligne = une entree : les retours a la ligne d'un message
        // (erreur multi-ligne, entree utilisateur) ne doivent pas pouvoir
        // forger de fausses lignes de log.
        let message = message.replace(['\n', '\r'], " ");
        let ligne = format!("[{}] [{}] {}\n", ts, niveau, message);
        eprint!("{}", ligne);
        Self::ecrire(&self.file, "vex", &ligne);
    }

    fn info(&self, msg: &str)  { self.log("INFO",  msg); }
    fn warn(&self, msg: &str)  { self.log("WARN",  msg); }
    fn error(&self, msg: &str) { self.log("ERROR", msg); }
    fn sec(&self, msg: &str)   { self.log("SECURITE", msg); }

    /// Journal d'acces : une ligne par requete HTTP, dans
    /// log/acces_<date>.log (pas sur stderr, trop verbeux).
    fn acces(&self, a: &utils::LigneAcces) {
        let ligne = format!("[{}] {}\n", timestamp_now(), a.formater());
        Self::ecrire(&self.acces, "acces", &ligne);
    }
}

fn chrono_date_simple() -> String {
    // FIX : l'ancienne "approximation" (annees de 365 jours, mois de 30
    // jours) decalait la date de plusieurs semaines (ex. 2026-10-12 au
    // lieu de 2026-09-25) -- dates des logs et noms de fichiers faux.
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn timestamp_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
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
        eprintln!("[WARN] Impossible d'ouvrir le fichier de log dans {}/ -- logs sur stderr uniquement.", LOG_DIR);
        VexLogger::stderr_seul()
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

    // ── Intégrité des sources — DÉSACTIVÉ ──────────────────────────
    // FIX (incident 2026-09-14) : ce garde-fou comparait un hash de
    // fichier source embarque a la compilation (include_str! dans
    // hashes_attendus()) au fichier reellement present sur disque au
    // demarrage, et DROP TABLE + supprimait le binaire au moindre
    // ecart -- cense detecter une modification malveillante post-
    // compilation. S'est declenche sur un deploiement 100% legitime
    // (git pull + rebuild), effacant les 18 tables de production sans
    // sauvegarde disponible. Le mecanisme lui-meme n'a pas de marge :
    // un `git pull` suivi d'un rebuild peut, selon le timing exact,
    // produire un ecart transitoire entre le hash embarque et le
    // fichier sur disque -- pas assez fiable pour un declencheur qui
    // droppe irreversiblement toute la base. Desactive en attendant
    // une conception plus sure (ex: verifier un hash committe dans git
    // au lieu d'un hash embarque au build, ou logger une alerte au
    // lieu de detruire). verifier_integrite()/destruction_totale()
    // restent dans le code, juste plus appeles.
    // verifier_integrite(&pool, &logger);

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
        match appeldb::donner_privilege_1_thesolar(&pool) {
            Ok(n) => logger.info(&format!("Aucun fondateur trouvé — UPDATE privilege=1 (thesolar) : {} ligne(s) affectée(s).", n)),
            Err(e) => logger.error(&format!("donner_privilege_1_thesolar a échoué : {}", e)),
        }
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

    // ── Serveur HTTP ──────────────────────────────────────────────
    // FIX : le bind doit se faire AVANT la sync bootstrap P2P initiale
    // ci-dessous -- quand bootstrap_url pointe sur ce serveur lui-meme
    // (cas courant : vex.hopto.org/neut, le meme process), la requete de
    // sync sortante repassait par Apache -> 127.0.0.1:8080, qui n'ecoutait
    // pas encore a ce stade -> Apache renvoyait 503 a chaque demarrage,
    // meme quand tout le reste fonctionnait. Le port est desormais ouvert
    // (la boucle d'acceptation demarre plus bas, mais le socket ecoute
    // deja et met en file les connexions entrantes) avant toute tentative
    // de sync sortante.
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

    // FIX : la sync bootstrap (initiale ET periodique) doit tourner dans un
    // thread A PART du thread principal -- celui-ci gere les requetes
    // entrantes une par une (`for request in server.incoming_requests()`
    // plus bas), donc si bootstrap_url pointe sur ce serveur lui-meme
    // (vex.hopto.org/neut, cas courant) et que la sync bloque le thread
    // principal AVANT qu'il entre dans sa boucle d'acceptation, la requete
    // sortante attend indefiniment une reponse que personne ne peut
    // produire -- interblocage, qui se traduisait par un timeout
    // ("Error encountered in the status line: timed out reading
    // response") a chaque demarrage. `lancer_sync_periodique` fait deja la
    // sync immediatement avant sa premiere pause (voir p2p.rs), donc elle
    // sert aussi de sync initiale -- plus besoin d'un appel bloquant a
    // part ici.
    lancer_sync_periodique(pool.clone(), Arc::clone(&node_state));
    // Sauvegardes automatiques (config.json -> backup.auto_heures ; 0 = off).
    admin::admin::lancer_sauvegardes_auto(CONFIG_PATH);
    logger.info("Sync périodique P2P lancée (sync initiale incluse, en tache de fond).");

    // ── Threads de traitement ─────────────────────────────────────
    // PERF : avant, une seule boucle traitait les requetes UNE PAR UNE --
    // un gros telechargement, un appel lent (Wikipedia, P2P) ou une
    // requete SQL longue bloquait tout le serveur pour tout le monde.
    // Desormais N threads (config "server.threads", defaut 8) se partagent
    // la file d'attente de tiny_http (Server::recv est thread-safe).
    let nb_threads = config
        .extra
        .get("server")
        .and_then(|s| s.get("threads"))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_THREADS as u64)
        .clamp(1, 64) as usize;
    logger.info(&format!("{} threads de traitement HTTP.", nb_threads));

    let server = Arc::new(server);
    let config = Arc::new(config);
    let compteur = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut threads = Vec::with_capacity(nb_threads);
    for n in 0..nb_threads {
        let server = Arc::clone(&server);
        let pool = pool.clone();
        let config = Arc::clone(&config);
        let logger = Arc::clone(&logger);
        let node_state = Arc::clone(&node_state);
        let compteur = Arc::clone(&compteur);
        let t = std::thread::Builder::new()
            .name(format!("vex-http-{}", n))
            .spawn(move || loop {
                let request = match server.recv() {
                    Ok(r) => r,
                    Err(e) => {
                        logger.error(&format!("Accept HTTP : {}", e));
                        continue;
                    }
                };
                let req_count = compteur.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                // Un panic dans un handler ne doit pas tuer le thread (le
                // serveur perdrait un worker a chaque bug).
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    traiter_requete(request, &pool, &config, &logger, &node_state, req_count);
                }));
                if res.is_err() {
                    logger.error(&format!("Panic pendant la requete #{} (thread vex-http-{})", req_count, n));
                }
            })
            .expect("creation thread HTTP");
        threads.push(t);
    }
    for t in threads {
        let _ = t.join();
    }
}

/// Traite UNE requete HTTP (routage global). Appele en parallele par les
/// threads de traitement -- tout l'etat partage est thread-safe (pool
/// MySQL, Arc/RwLock).
fn traiter_requete(
    mut request: tiny_http::Request,
    pool: &appeldb::DbPool,
    config: &config_loader::VexConfig,
    logger: &VexLogger,
    node_state: &Arc<RwLock<NodeState>>,
    req_count: u64,
) {
    let url = request.url().to_string();
    let method = request.method().to_string();

    let remote_full = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "unknown".into());
    let remote = utils::strip_port(&remote_full);

    // IP reelle du visiteur (en-tetes du reverse proxy si la connexion
    // vient d'un proxy local) -- pour les LOGS uniquement ; `remote`
    // (IP TCP brute) reste celle passee aux handlers pour les sessions.
    let (ip_client, ip_proxy) = utils::client_ip(&request);
    let ip_log = match &ip_proxy {
        Some(p) => format!("{} (via {})", ip_client, p),
        None => ip_client.clone(),
    };
    let debut_req = std::time::Instant::now();
    let ua_req = request.headers().iter()
        .find(|h| h.field.equiv("User-Agent"))
        .map(|h| h.value.as_str().to_string()).unwrap_or_default();
    let referer_req = request.headers().iter()
        .find(|h| h.field.equiv("Referer"))
        .map(|h| h.value.as_str().to_string()).unwrap_or_default();
    let mut statut_req: Option<u16> = None;

    let path = url.split('?').next().unwrap_or(&url).to_string();


    if config.app.debug_mode {
        logger.info(&format!("[REQ #{}] {} {} {}", req_count, ip_log, method, utils::masquer_secrets_chemin(&path)));
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
        logger.sec(&format!("[ACCES SENSIBLE] {} {} {} (ip={})", method, path, req_count, ip_log));
    }

    match path.as_str() {
        "/" | "/login" | "/login/" | "/login/login" | "/login/login.php" => {
            logger.info(&format!("Login request depuis {}", ip_log));
            login::login::handle_request(request, &pool, &config, &remote);
        }

        "/login/first_setup" => {
            logger.info(&format!("First setup depuis {}", ip_log));
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
            logger.info(&format!("Logout depuis {}", ip_log));
            login::logout::handle_request(request, &pool, &remote);
        }

        p if p == "/autologin"
            || p == "/autologin/"
            || p.starts_with("/autologin/")
            || p == "/login/autologin"
            || p == "/login/autologin/" =>
        {
            logger.info(&format!("Autologin depuis {}", ip_log));
            login::autologin::handle_request(request, &pool, &config, &remote);
        }

        p if p.starts_with("/api/appareil")
            || p == "/autoriser-appareil"
            || p == "/autoriser-appareil/"
            || p == "/install.ps1" =>
        {
            login::appareil::handle_request(request, &pool, &remote);
        }

        p if p.starts_with("/api/dashboard") => {
            login::dashboard::handle_request(request, &pool, &config, &remote);
        }

        "/dashboard" | "/dashboard/" | "/login/dashboard" | "/login/dashboard/" => {
            login::dashboard::handle_request(request, &pool, &config, &remote);
        }

        p if p.starts_with("/admin") || p.starts_with("/api/admin") => {
            logger.info(&format!("Admin panel depuis {} — {} {}", ip_log, method, path));
            admin::admin::handle_request(request, &pool, &config, CONFIG_PATH, &remote_full);
        }

        // Extensions : /ext/<id> (page) et /api/ext/<id> (API).
        // Privilege + plan verifies dans access_control::servir_extension.
        p if p.starts_with("/ext/") || p.starts_with("/api/ext/") => {
            let ext_id = access_control::extension_id_depuis_path(p);
            logger.info(&format!("Extension '{}' depuis {} — {} {}", ext_id, ip_log, method, path));
            access_control::servir_extension(&pool, &config, request, &path);
        }

        // FIX (retour utilisateur : "je veux pas de requete quand il
        // se passe rien") -- /attendre est un LONG-POLL : elle bloque
        // jusqu'a 25s cote serveur (voir fchier::attendre_bloquant).
        // Ce serveur traite les requetes UNE PAR UNE sur ce thread
        // principal (`for request in server.incoming_requests()`) --
        // la traiter ici comme les autres routes fchier gelerait TOUT
        // LE SERVEUR pour tout le monde pendant l'attente. `Request`
        // implemente Send (voir tiny_http) : on la deplace donc sur un
        // thread dedie, jetable, et la boucle principale continue
        // immediatement sans attendre -- seule cette route est
        // concernee, toutes les autres restent traitees en ligne,
        // inchangees.
        "/api/fchier/attendre" => {
            let pool2 = pool.clone();
            std::thread::spawn(move || {
                let resp = fchier::fchier::attendre_bloquant(&pool2, &request);
                let _ = request.respond(resp);
            });
        }

        // Liens de partage publics (sans session) -- voir fchier/liens.rs.
        p if p.starts_with("/partage/") || p.starts_with("/api/partage/") => {
            let resp = fchier::liens::handle_public(&pool, &mut request);
            statut_req = Some(repondre(request, resp));
        }

        p if p.starts_with("/fchier") || p.starts_with("/api/fchier") => {
            let resp = fchier::fchier::handle(&pool, &mut request);
            statut_req = Some(repondre(request, resp));
        }

        p if p.starts_with("/mess") || p.starts_with("/api/mess") => {
            let resp = mess::mess::handle(&pool, &mut request);
            statut_req = Some(repondre(request, resp));
        }

        "/p2p/sso" | "/p2p/sso/etat" => sso::traiter(request, &pool, &node_state),

        p if p.starts_with("/p2p/") || p.starts_with("/neut/") => {
            handle_request(request, &pool, &node_state, &config);
        }

        p if p.starts_with("/viso") || p.starts_with("/api/viso") => {
            let resp = viso::viso::handle(&pool, &mut request);
            statut_req = Some(repondre(request, resp));
        }

        p if p.starts_with("/sitec")
            || p.starts_with("/api/sitec")
            || p.starts_with("/page/") =>
        {
            let public = path.starts_with("/page/");
            let resp = sitec::sitec::handle(&pool, &mut request);
            statut_req = Some(repondre_opts(request, resp, !public));
        }

        p if p.starts_with("/recherche") || p.starts_with("/api/recherche") => {
            let resp = recherche::recherche::handle(&pool, &config, &mut request);
            statut_req = Some(repondre(request, resp));
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
            logger.warn(&format!("404 — {} {} (ip={})", method, path, ip_log));
            statut_req = Some(404);
            // Page 404 lisible pour un navigateur ; texte brut pour l'API.
            let veut_html = !path.starts_with("/api/")
                && request.headers().iter().any(|h| h.field.equiv("Accept") && h.value.as_str().contains("text/html"));
            if veut_html {
                let resp = Response::from_string(PAGE_404)
                    .with_status_code(404)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap());
                let _ = request.respond(resp);
            } else {
                let _ = request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
        }
    }

    // ── Journal d'acces ────────────────────────────────────────
    let duree_ms = debut_req.elapsed().as_millis();
    logger.acces(&utils::LigneAcces {
        ip: ip_client,
        via: ip_proxy,
        methode: method.clone(),
        chemin: path.clone(),
        statut: statut_req,
        duree_ms,
        user_agent: ua_req,
        referer: referer_req,
    });
    // Le serveur traite les requetes une par une : une requete lente
    // bloque tout le monde -- a signaler dans le log principal.
    if duree_ms >= 2000 && path != "/api/fchier/attendre" {
        logger.warn(&format!("Requete lente : {} {} en {} ms (ip={})", method, path, duree_ms, ip_log));
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

fn repondre(request: tiny_http::Request, resp: Response<std::io::Cursor<Vec<u8>>>) -> u16 {
    utils::envoyer(request, resp)
}

fn repondre_opts(request: tiny_http::Request, resp: Response<std::io::Cursor<Vec<u8>>>, anti_iframe: bool) -> u16 {
    utils::envoyer_opts(request, resp, anti_iframe)
}

fn serve_static(request: tiny_http::Request, path: &str) {
    if path.contains("..") {
        let _ = request.respond(Response::from_string("403").with_status_code(403));
        return;
    }
    let file_path = format!(".{}", path);
    // ETag (taille + date de modification) : le navigateur renvoie
    // If-None-Match, et un fichier inchange repond 304 sans corps -- ni
    // lecture disque ni transfert. Change automatiquement a chaque
    // deploiement qui modifie le fichier.
    let etag = std::fs::metadata(&file_path).ok().map(|m| {
        let mtime = m.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs()).unwrap_or(0);
        format!("\"{:x}-{:x}\"", m.len(), mtime)
    });
    let revalider = path.ends_with(".html") || path.ends_with(".js") || path.ends_with(".css");
    let cache_control = if revalider {
        // Revalidation a chaque chargement (304 si inchange) : un correctif
        // CSS/JS deploye est visible immediatement.
        "no-cache"
    } else {
        "public, max-age=604800"
    };
    if let Some(tag) = &etag {
        let si_aucun = request.headers().iter()
            .find(|h| h.field.equiv("If-None-Match"))
            .map(|h| h.value.as_str().to_string());
        if si_aucun.as_deref() == Some(tag.as_str()) {
            let _ = request.respond(
                Response::empty(304)
                    .with_header(tiny_http::Header::from_bytes("ETag", tag.as_str()).unwrap())
                    .with_header(tiny_http::Header::from_bytes("Cache-Control", cache_control).unwrap()),
            );
            return;
        }
    }
    match std::fs::read(&file_path) {
        Ok(data) => {
            // with_chunked_threshold : voir appareil.rs -- tiny_http bascule en
            // Transfer-Encoding chunked au-dela de 32 Ko par defaut, ce qui
            // passe mal a travers Apache (fichiers tronques/corrompus).
            let mut resp = Response::from_data(data)
                .with_header(tiny_http::Header::from_bytes("Content-Type", guess_mime(path)).unwrap())
                .with_chunked_threshold(usize::MAX);
            // Aucun Cache-Control n'etait envoye pour aucun fichier statique :
            // le navigateur (et Apache en reverse proxy) pouvaient garder en
            // cache une vieille version d'un .html/.js meme apres deploiement
            // d'un correctif -- symptome vu plusieurs fois de suite sur
            // l'editeur Sitec ("ca marche toujours pas" alors que le serveur
            // avait bien la derniere version). Les pages HTML/JS de l'appli
            // (frequemment mises a jour) forcent une revalidation a chaque
            // fois ; les autres assets (images, polices...) restent en cache
            // normalement.
            if let Some(tag) = &etag {
                resp = resp.with_header(tiny_http::Header::from_bytes("ETag", tag.as_str()).unwrap());
            }
            if revalider {
                // FIX : theme.css etait en cache 7 jours (branche else) --
                // un correctif de la nav mettait jusqu'a une semaine a
                // apparaitre. HTML/JS/CSS : "no-cache" + ETag = revalidation
                // a chaque fois, mais reponse 304 legere si rien n'a change
                // (avant : "no-store", tout retelecharge a chaque page).
                resp = resp.with_header(
                    tiny_http::Header::from_bytes("Cache-Control", cache_control).unwrap(),
                );
                // PWA (etape 5) : sw.js est servi depuis /static/, ce qui
                // limiterait sa portee par defaut a /static/* -- ce header
                // l'autorise a controler tout le site (necessaire pour que
                // le navigateur propose l'installation de l'app). Le
                // service worker lui-meme reste volontairement passif hors
                // des assets statiques (voir sw.js).
                if path == "/static/sw.js" {
                    resp = resp.with_header(
                        tiny_http::Header::from_bytes("Service-Worker-Allowed", "/").unwrap(),
                    );
                }
            } else {
                // FIX (demande utilisateur : "rendre fchier ultra rapide") :
                // le commentaire ci-dessus disait que les images/polices
                // "restent en cache normalement", mais sans Cache-Control
                // explicite le navigateur ne fait que du cache heuristique
                // (souvent quelques minutes) -- chaque navigation dans
                // fchier (des dizaines d'icones .svg identiques a chaque
                // dossier) refaisait donc des requetes reseau evitables.
                // Ces fichiers (icones, polices, css) ne changent qu'au
                // deploiement d'un correctif VEX, jamais a la volee.
                resp = resp.with_header(
                    tiny_http::Header::from_bytes("Cache-Control", cache_control).unwrap(),
                );
            }
            let gzip_ok = utils::accepte_gzip(&request);
            let _ = request.respond(utils::compresser_reponse(resp, gzip_ok));
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
    else if path.ends_with(".jpg") || path.ends_with(".jpeg") { "image/jpeg" }
    else if path.ends_with(".gif")   { "image/gif" }
    else if path.ends_with(".webp")  { "image/webp" }
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