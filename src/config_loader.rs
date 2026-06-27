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
    pub host: String,
    pub user: String,
    pub password: String,
    pub dbname: String,
    #[serde(default = "default_port")]
    pub port: u16,
}
impl Default for DbConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            user: "orsql".into(),
            password: "iDq]25F0u8v*z[1d".into(),
            dbname: "user".into(),
            port: 3306,
        }
    }
}
fn default_port() -> u16 {
    3306
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

/// Charge la config DB depuis config.json (section "db" si présente,
/// sinon les valeurs hardcodées en Default).
pub fn load_db_config(path: &str) -> DbConfig {
    match std::fs::read_to_string(path) {
        Ok(raw) => {
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
            if let Some(db) = v.get("db") {
                serde_json::from_value(db.clone()).unwrap_or_default()
            } else {
                DbConfig::default()
            }
        }
        Err(_) => DbConfig::default(),
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

/// Ajoute la section "db" dans config.json si elle n'existe pas encore.
/// Utile pour le premier lancement.
pub fn ensure_db_section(path: &str, db: &DbConfig) {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
    let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
    if v.get("db").is_none() {
        v["db"] = serde_json::to_value(db).unwrap_or_default();
        let _ = std::fs::write(path, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}
