// ══════════════════════════════════════════════════════════════════
// config_loader.rs — VEX config.json
// Charge et expose toutes les sections de config.json
// sous forme de structs Rust typées.
// Appelé une fois dans main.rs, le résultat est passé en paramètre.
// ══════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════════
// STRUCT RACINE
// ══════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VexConfig {
    pub app: AppConfig,
    pub users: UsersConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
    pub editor: EditorConfig,
    pub onlyoffice_server: OnlyofficeServerConfig,
    pub plans: PlansConfig,
    pub autologin: AutologinConfig,
    pub payment: PaymentConfig,
    pub extensions: ExtensionsConfig,

    // Conserve toutes les clés _comment etc. pour la réécriture sans perte
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ══════════════════════════════════════════════════════════════════
// SECTIONS
// ══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_lang")]
    pub default_language: String,
    #[serde(default)]
    pub available_languages: Vec<String>,
    #[serde(default)]
    pub maintenance_mode: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for AppConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_name() -> String {
    "VEX".into()
}
fn default_version() -> String {
    "alpha-0.3".into()
}
fn default_lang() -> String {
    "fr".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersConfig {
    #[serde(default = "default_100")]
    pub max_users: u64,
    #[serde(default = "default_open")]
    pub registration_mode: String,
    #[serde(default)]
    pub activation_key_required: bool,
    #[serde(default)]
    pub activation_key: String,
    #[serde(default = "default_60")]
    pub session_expiration_minutes: u64,
    #[serde(default)]
    pub account_expiration_days: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for UsersConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_100() -> u64 {
    100
}
fn default_60() -> u64 {
    60
}
fn default_open() -> String {
    "open".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_8")]
    pub password_min_length: u32,
    #[serde(default = "s_true")]
    pub password_require_uppercase: bool,
    #[serde(default = "s_true")]
    pub password_require_number: bool,
    #[serde(default)]
    pub password_require_special_char: bool,
    #[serde(default = "default_5")]
    pub max_login_attempts: u32,
    #[serde(default = "default_15")]
    pub lockout_duration_minutes: u32,
    #[serde(default)]
    pub two_factor_auth: bool,
    #[serde(default)]
    pub ip_whitelist_enabled: bool,
    #[serde(default)]
    pub ip_whitelist: Vec<String>,
    #[serde(default)]
    pub ip_blacklist_enabled: bool,
    #[serde(default)]
    pub ip_blacklist: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_8() -> u32 {
    8
}
fn default_5() -> u32 {
    5
}
fn default_15() -> u32 {
    15
}
fn s_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub quota_by_plan: HashMap<String, Value>,
    #[serde(default = "default_512")]
    pub max_file_size_mb: u64,
    #[serde(default)]
    pub allowed_extensions: Vec<String>,
    #[serde(default)]
    pub blocked_extensions: Vec<String>,
    #[serde(default = "default_90")]
    pub file_retention_days: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for StorageConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_512() -> u64 {
    512
}
fn default_90() -> u64 {
    90
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "s_true")]
    pub online_editing_enabled: bool,
    #[serde(default = "default_onlyoffice")]
    pub provider: String,
    #[serde(default = "s_true")]
    pub collaborative_editing: bool,
    #[serde(default)]
    pub supported_formats: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for EditorConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_onlyoffice() -> String {
    "onlyoffice".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlyofficeServerConfig {
    #[serde(default = "s_true")]
    pub enabled: bool,
    #[serde(default = "default_oo_url")]
    pub server_url: String,
    #[serde(default = "default_health_path")]
    pub healthcheck_path: String,
    #[serde(default)]
    pub start_cmd: String,
    #[serde(default)]
    pub stop_cmd: String,
    #[serde(default = "default_wait_ms")]
    pub wait_boot_ms: u64,
    #[serde(default)]
    pub auto_stop_after_idle_seconds: u64,
    #[serde(default)]
    pub auto_stop: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for OnlyofficeServerConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_oo_url() -> String {
    "http://127.0.0.1:8080".into()
}
fn default_health_path() -> String {
    "/healthcheck".into()
}
fn default_wait_ms() -> u64 {
    8000
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlansConfig {
    #[serde(default)]
    pub enforce_plan_restrictions: bool,
    #[serde(default)]
    pub available_plans: Vec<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutologinConfig {
    #[serde(default = "s_true")]
    pub enabled: bool,
    #[serde(default = "default_200")]
    pub token_length: u32,
    #[serde(default = "default_1")]
    pub max_tokens_per_user: u32,
    #[serde(default = "default_30")]
    pub cookie_duration_days: u32,
    #[serde(default = "default_1")]
    pub session_duration_hours: u32,
    #[serde(default = "s_true")]
    pub require_same_ip: bool,
    #[serde(default = "s_true")]
    pub require_same_browser: bool,
    #[serde(default = "s_true")]
    pub admin_can_block: bool,
    #[serde(default)]
    pub plans_autorises: Vec<String>,
    #[serde(default = "default_10")]
    pub privilege_min: u32,
    #[serde(default)]
    pub domaine: String,
    #[serde(default)]
    pub server_secret: String,
    #[serde(default = "s_true")]
    pub log_autologin: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for AutologinConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_200() -> u32 {
    200
}
fn default_1() -> u32 {
    1
}
fn default_30() -> u32 {
    30
}
fn default_10() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentConfig {
    #[serde(default = "default_free")]
    pub mode: String,
    #[serde(default = "default_eur")]
    pub currency: String,
    #[serde(default = "default_stripe")]
    pub provider: String,
    #[serde(default)]
    pub stripe_public_key: String,
    #[serde(default)]
    pub stripe_secret_key: String,
    #[serde(default = "default_14")]
    pub trial_days: u32,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for PaymentConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
fn default_free() -> String {
    "free".into()
}
fn default_eur() -> String {
    "EUR".into()
}
fn default_stripe() -> String {
    "stripe".into()
}
fn default_14() -> u32 {
    14
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtensionsConfig {
    #[serde(default = "s_true")]
    pub enabled: bool,
    #[serde(default)]
    pub allow_user_install: bool,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default)]
    pub marketplace_url: String,
    // Clé = id extension ("onlyoffice", "vexmail", ...)
    // Valeur = objet JSON arbitraire (enabled, version, params, ...)
    #[serde(default)]
    pub extension_params: HashMap<String, Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ══════════════════════════════════════════════════════════════════
// CONFIG DB — séparé de VexConfig pour ne pas l'exposer dans l'API
// ══════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    /// Nom de la base. `database` est accepte comme alias.
    #[serde(default, alias = "database")]
    pub dbname: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            // 127.0.0.1 et non "localhost" : sous Debian / Raspberry Pi OS,
            // MariaDB n'ecoute qu'en IPv4, alors que "localhost" se resout
            // souvent en ::1 d'abord — la connexion echoue alors sans raison
            // apparente. L'adresse litterale evite ce piege.
            host: default_host(),
            user: String::new(),
            password: String::new(),
            dbname: String::new(),
            port: default_port(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    3306
}

/// Fichier de configuration dedie a la base, hors de config.json :
/// il contient un mot de passe et n'a pas vocation a etre versionne.
pub const DB_CONFIG_PATH: &str = "db.json";

/// Variable d'environnement pour pointer ailleurs que sur ./db.json.
pub const DB_CONFIG_ENV: &str = "VEX_DB_CONFIG";

/// Exemple montre a l'utilisateur quand rien n'est configure.
pub fn db_config_exemple() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "host": "127.0.0.1",
        "port": 3306,
        "user": "vex",
        "password": "<mot de passe>",
        "database": "vex"
    }))
    .unwrap_or_default()
}

/// Chemin effectif du fichier de configuration base.
pub fn db_config_path() -> String {
    std::env::var(DB_CONFIG_ENV).unwrap_or_else(|_| DB_CONFIG_PATH.to_string())
}

/// Charge la configuration de la base, par ordre de priorite :
///   1. variables d'environnement VEX_DB_HOST / _PORT / _USER /
///      _PASSWORD / _NAME (pratique en conteneur ou en service systemd) ;
///   2. le fichier dedie (db.json, ou VEX_DB_CONFIG) ;
///   3. la section "db" de config.json, pour les installations anciennes.
///
/// Si aucune source ne fournit d'identifiants, on renvoie une erreur
/// explicite plutot que de tenter une connexion avec des valeurs par
/// defaut : un echec silencieux ici se traduit sinon par des erreurs
/// MySQL incomprehensibles bien plus loin dans le demarrage.
pub fn load_db_config(path: &str) -> Result<DbConfig, String> {
    let mut cfg = DbConfig::default();
    let mut source = String::new();

    // ── 3. config.json (le moins prioritaire) ────────────────────
    if let Some(db) = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("db").cloned())
    {
        if let Ok(c) = serde_json::from_value::<DbConfig>(db) {
            if !c.user.is_empty() {
                cfg = c;
                source = format!("section \"db\" de {}", path);
            }
        }
    }

    // ── 2. fichier dedie ─────────────────────────────────────────
    let chemin = db_config_path();
    match std::fs::read_to_string(&chemin) {
        Ok(brut) => match serde_json::from_str::<DbConfig>(&brut) {
            Ok(c) => {
                if c.user.is_empty() {
                    return Err(format!(
                        "{} ne renseigne pas \"user\".\n\nExemple attendu :\n{}",
                        chemin,
                        db_config_exemple()
                    ));
                }
                cfg = c;
                source = chemin.clone();
            }
            Err(e) => {
                return Err(format!(
                    "{} est illisible : {}\n\nFormat attendu :\n{}",
                    chemin,
                    e,
                    db_config_exemple()
                ));
            }
        },
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("{} n'a pas pu etre lu : {}", chemin, e));
        }
        Err(_) => { /* absent : on continue, une autre source suffit peut-etre */ }
    }

    // ── 1. variables d'environnement ─────────────────────────────
    let mut par_env = false;
    if let Ok(v) = std::env::var("VEX_DB_HOST") {
        cfg.host = v;
        par_env = true;
    }
    if let Ok(v) = std::env::var("VEX_DB_PORT") {
        cfg.port = v.parse().unwrap_or(cfg.port);
        par_env = true;
    }
    if let Ok(v) = std::env::var("VEX_DB_USER") {
        cfg.user = v;
        par_env = true;
    }
    if let Ok(v) = std::env::var("VEX_DB_PASSWORD") {
        cfg.password = v;
        par_env = true;
    }
    if let Ok(v) = std::env::var("VEX_DB_NAME") {
        cfg.dbname = v;
        par_env = true;
    }
    if par_env {
        source = if source.is_empty() {
            "variables d'environnement VEX_DB_*".to_string()
        } else {
            format!("{} + variables d'environnement VEX_DB_*", source)
        };
    }

    // ── Rien de configure : on s'arrete net ──────────────────────
    if cfg.user.is_empty() || cfg.dbname.is_empty() {
        return Err(format!(
            "Configuration de la base de donnees introuvable.\n\n\
             Aucune des sources suivantes ne fournit d'identifiants :\n\
             \u{20} 1. variables VEX_DB_HOST / VEX_DB_PORT / VEX_DB_USER / \
             VEX_DB_PASSWORD / VEX_DB_NAME\n\
             \u{20} 2. fichier {} (chemin modifiable via {})\n\
             \u{20} 3. section \"db\" de {}\n\n\
             Creez {} avec ce contenu, puis relancez :\n\n{}\n\n\
             Pensez a l'exclure du depot : il contient un mot de passe.",
            chemin,
            DB_CONFIG_ENV,
            path,
            chemin,
            db_config_exemple()
        ));
    }

    // "localhost" se resout souvent en ::1 avant 127.0.0.1 sous Debian /
    // Raspberry Pi OS, ou MariaDB n'ecoute qu'en IPv4 : on bascule en clair.
    if cfg.host.trim().eq_ignore_ascii_case("localhost") {
        eprintln!(
            "[config_loader] host=\"localhost\" remplace par \"127.0.0.1\" \
             (MariaDB n'ecoute souvent qu'en IPv4)."
        );
        cfg.host = default_host();
    }

    eprintln!(
        "[config_loader] Base : {}@{}:{}/{} (source : {})",
        cfg.user, cfg.host, cfg.port, cfg.dbname, source
    );
    Ok(cfg)
}

// ══════════════════════════════════════════════════════════════════
// CHARGEMENT
// ══════════════════════════════════════════════════════════════════

/// Charge config.json depuis le chemin donné.
/// En cas d'erreur retourne la config par défaut + log stderr.
pub fn load_config(path: &str) -> VexConfig {
    match std::fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<VexConfig>(&raw) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("[config_loader] JSON invalide dans {} : {}", path, e);
                VexConfig::default()
            }
        },
        Err(e) => {
            eprintln!("[config_loader] Impossible de lire {} : {}", path, e);
            VexConfig::default()
        }
    }
}

/// Sauvegarde config.json en fusionnant `patch` dans l'existant.
/// Conserve toutes les clés _comment, _version, etc.
pub fn save_config(path: &str, patch: &serde_json::Value) -> Result<(), String> {
    // Lit l'existant comme Value brut (pour conserver les _comment)
    let existing: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let merged = merge_json(existing, patch.clone());

    let out =
        serde_json::to_string_pretty(&merged).map_err(|e| format!("Sérialisation : {}", e))?;

    std::fs::write(path, out).map_err(|e| format!("Écriture : {}", e))
}

fn merge_json(base: serde_json::Value, new: serde_json::Value) -> serde_json::Value {
    match (base, new) {
        (serde_json::Value::Object(mut b), serde_json::Value::Object(n)) => {
            for (k, v) in n {
                let merged = if let Some(bv) = b.remove(&k) {
                    merge_json(bv, v)
                } else {
                    v
                };
                b.insert(k, merged);
            }
            serde_json::Value::Object(b)
        }
        (_, new) => new,
    }
}

/// Ecrit un db.json d'exemple si aucun n'existe. Rend true s'il a ete cree.
/// Le fichier est volontairement incomplet (mot de passe a remplir) :
/// il sert d'amorce, pas de configuration utilisable telle quelle.
pub fn ecrire_db_config_exemple() -> bool {
    let chemin = db_config_path();
    if std::path::Path::new(&chemin).exists() {
        return false;
    }
    std::fs::write(&chemin, db_config_exemple() + "\n").is_ok()
}
