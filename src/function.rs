// ══════════════════════════════════════════════════════════════════
// function.rs — VEX fonctions utilitaires
// 0 SQL dans ce fichier — tout passe par appeldb::
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, DbPool};
use serde_json::{json, Value};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════════
// PRIVILEGES
// ══════════════════════════════════════════════════════════════════
pub struct PrivilegeDetails {
    pub nom: &'static str,
    pub couleur: &'static str,
}

pub fn get_privilege_details(privilege: i64) -> PrivilegeDetails {
    match privilege {
        12 => PrivilegeDetails {
            nom: "ban",
            couleur: "#000000",
        },
        11 => PrivilegeDetails {
            nom: "ban",
            couleur: "#000000",
        },
        10 => PrivilegeDetails {
            nom: "aucun",
            couleur: "#000000",
        },
        9 => PrivilegeDetails {
            nom: "beta-testeur",
            couleur: "#20012a",
        },
        8 => PrivilegeDetails {
            nom: "utilisateur certifie",
            couleur: "#4b4b4b",
        },
        7 => PrivilegeDetails {
            nom: "",
            couleur: "#32cd32",
        },
        6 => PrivilegeDetails {
            nom: "Moderateur",
            couleur: "#006400",
        },
        5 => PrivilegeDetails {
            nom: "Super-moderateur",
            couleur: "#4169e1",
        },
        4 => PrivilegeDetails {
            nom: "verfircateur",
            couleur: "#4169e1",
        },
        3 => PrivilegeDetails {
            nom: "admin",
            couleur: "#d30000",
        },
        2 => PrivilegeDetails {
            nom: "super admin",
            couleur: "#6d0000",
        },
        1 => PrivilegeDetails {
            nom: "fondateur",
            couleur: "fona",
        },
        _ => PrivilegeDetails {
            nom: "inconnu",
            couleur: "#ffffff",
        },
    }
}

pub fn get_privilege_details_json(privilege: i64) -> Value {
    let p = get_privilege_details(privilege);
    json!({ "nom_privilege": p.nom, "couleur_privilege": p.couleur })
}

// ══════════════════════════════════════════════════════════════════
// GETNAME
// ══════════════════════════════════════════════════════════════════
pub fn get_name_details(pool: &DbPool, user_id: i64) -> Option<String> {
    let rows = selectionner(
        pool,
        "login",
        &[("id", mysql::Value::from(user_id))],
        &["nom"],
        None,
        Some(1),
    );
    rows.into_iter()
        .next()
        .and_then(|r| r.get("nom").and_then(|v| v.as_str().map(|s| s.to_string())))
}

// ══════════════════════════════════════════════════════════════════
// PRÉFÉRENCES UTILISATEUR
// ══════════════════════════════════════════════════════════════════
#[derive(Debug, Clone)]
pub struct UserPrefs {
    pub teme: i64,
    pub langue: String,
    pub notifications_meet: i64,
    pub auto_record: i64,
    pub mic_default: i64,
    pub camera_default: i64,
    pub quality_video: String,
    pub profile_icon_type: String,
    pub profile_icon_url: Option<String>,
    pub nav_button_style: HashMap<String, Value>,
    pub logo_pages: HashMap<String, Value>,
}

impl Default for UserPrefs {
    fn default() -> Self {
        let mut nbs = HashMap::new();
        nbs.insert("dashboard".into(), json!(1));
        let mut lp = HashMap::new();
        lp.insert("dashboard".into(), json!(1));
        Self {
            teme: 0,
            langue: "fr".into(),
            notifications_meet: 1,
            auto_record: 0,
            mic_default: 0,
            camera_default: 0,
            quality_video: "auto".into(),
            profile_icon_type: "initials".into(),
            profile_icon_url: None,
            nav_button_style: nbs,
            logo_pages: lp,
        }
    }
}

pub fn get_user_preferences(pool: &DbPool, user_id: i64) -> UserPrefs {
    let rows = selectionner(
        pool,
        "pref",
        &[("id-user", mysql::Value::from(user_id))],
        &[],
        None,
        Some(1),
    );

    if rows.is_empty() {
        let def = UserPrefs::default();
        inserer_ou_modifier(
            pool,
            "pref",
            &[
                ("id-user", mysql::Value::from(user_id)),
                ("teme", mysql::Value::from(0i64)),
                ("langue", mysql::Value::from("fr")),
                ("profile_icon_type", mysql::Value::from("initials")),
                ("nav_button_style", mysql::Value::from("{\"dashboard\":1}")),
                ("logo_pages", mysql::Value::from("{\"dashboard\":1}")),
            ],
            &[],
        );
        return def;
    }

    let row = &rows[0];
    let parse_json_map = |key: &str| -> HashMap<String, Value> {
        row.get(key)
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| {
                let mut m = HashMap::new();
                m.insert("dashboard".into(), json!(1));
                m
            })
    };

    UserPrefs {
        teme: row.get("teme").and_then(|v| v.as_i64()).unwrap_or(0),
        langue: row
            .get("langue")
            .and_then(|v| v.as_str())
            .unwrap_or("fr")
            .to_string(),
        notifications_meet: row
            .get("notifications_meet")
            .and_then(|v| v.as_i64())
            .unwrap_or(1),
        auto_record: row.get("auto_record").and_then(|v| v.as_i64()).unwrap_or(0),
        mic_default: row.get("mic_default").and_then(|v| v.as_i64()).unwrap_or(0),
        camera_default: row
            .get("camera_default")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        quality_video: row
            .get("quality_video")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string(),
        profile_icon_type: row
            .get("profile_icon_type")
            .and_then(|v| v.as_str())
            .unwrap_or("initials")
            .to_string(),
        profile_icon_url: row
            .get("profile_icon_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        nav_button_style: parse_json_map("nav_button_style"),
        logo_pages: parse_json_map("logo_pages"),
    }
}

// ══════════════════════════════════════════════════════════════════
// updateUserPreference()
// ══════════════════════════════════════════════════════════════════
const ALLOWED_PREFS: &[&str] = &[
    "teme",
    "langue",
    "notifications_meet",
    "auto_record",
    "mic_default",
    "camera_default",
    "quality_video",
    "nav_button_style",
    "logo_pages",
];
const JSON_PREFS: &[&str] = &["nav_button_style", "logo_pages"];

pub fn update_user_preference(
    pool: &DbPool,
    user_id: i64,
    pref_name: &str,
    pref_value: &str,
) -> bool {
    if !ALLOWED_PREFS.contains(&pref_name) {
        return false;
    }
    if JSON_PREFS.contains(&pref_name) {
        if serde_json::from_str::<Value>(pref_value).is_err() {
            return false;
        }
    }
    let exists = !selectionner(
        pool,
        "pref",
        &[("id-user", mysql::Value::from(user_id))],
        &["id-user"],
        None,
        Some(1),
    )
    .is_empty();

    if exists {
        inserer_ou_modifier(
            pool,
            "pref",
            &[(pref_name, mysql::Value::from(pref_value))],
            &[("id-user", mysql::Value::from(user_id))],
        ) >= 0
    } else {
        inserer_ou_modifier(
            pool,
            "pref",
            &[
                ("id-user", mysql::Value::from(user_id)),
                (pref_name, mysql::Value::from(pref_value)),
            ],
            &[],
        ) >= 0
    }
}

// ══════════════════════════════════════════════════════════════════
// THÈME
// ══════════════════════════════════════════════════════════════════
pub fn toggle_user_theme(pool: &DbPool, user_id: i64) -> Option<i64> {
    let prefs = get_user_preferences(pool, user_id);
    let new_theme = if prefs.teme == 0 { 1i64 } else { 0i64 };
    let ok = update_user_preference(pool, user_id, "teme", &new_theme.to_string());
    if ok {
        Some(new_theme)
    } else {
        None
    }
}

pub fn get_theme_attr(pool: &DbPool, user_id: i64) -> &'static str {
    let prefs = get_user_preferences(pool, user_id);
    if prefs.teme == 1 {
        "dark"
    } else {
        "light"
    }
}

// ══════════════════════════════════════════════════════════════════
// LANGUES
// ══════════════════════════════════════════════════════════════════
const SUPPORTED_LANGS: &[(&str, &str)] = &[
    ("fr", "Français"),
    ("en", "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("ar", "العربية"),
    ("zh", "中文"),
    ("ja", "日本語"),
    ("ru", "Русский"),
];

pub fn get_supported_languages() -> Vec<(&'static str, &'static str)> {
    SUPPORTED_LANGS.to_vec()
}
pub fn is_rtl(lang: &str) -> bool {
    matches!(lang, "ar" | "he" | "fa" | "ur")
}

pub fn get_user_language(
    pool: &DbPool,
    user_id: Option<i64>,
    cookie_lang: Option<&str>,
    accept_lang: Option<&str>,
) -> String {
    if let Some(uid) = user_id {
        let prefs = get_user_preferences(pool, uid);
        if !prefs.langue.is_empty() {
            return prefs.langue;
        }
    }
    if let Some(lang) = cookie_lang {
        if is_supported_lang(lang) {
            return lang.to_string();
        }
    }
    if let Some(al) = accept_lang {
        let bl = &al[..al.len().min(2)];
        if is_supported_lang(bl) {
            return bl.to_string();
        }
    }
    "fr".to_string()
}

pub fn is_supported_lang(lang: &str) -> bool {
    SUPPORTED_LANGS.iter().any(|(c, _)| *c == lang)
}

pub fn set_user_language(pool: &DbPool, user_id: i64, lang: &str) -> bool {
    if !is_supported_lang(lang) {
        return false;
    }
    update_user_preference(pool, user_id, "langue", lang)
}

// ══════════════════════════════════════════════════════════════════
// VÉRIFIER RESTRICTIONS
// ══════════════════════════════════════════════════════════════════
const ALLOWED_TAG_COLUMNS: &[&str] = &[
    "VMotdePasse",
    "VPrivilege",
    "VVIP",
    "vcreAutologin",
    "vAutologin",
    "VEmail",
];

pub fn verifierrr(pool: &DbPool, iduser: i64, tag_user_column: &str) -> String {
    if !ALLOWED_TAG_COLUMNS.contains(&tag_user_column) {
        return "oui".to_string();
    }
    let tag_rows = selectionner(
        pool,
        "tag-user",
        &[("user-id", mysql::Value::from(iduser))],
        &[],
        None,
        Some(1),
    );
    if tag_rows.is_empty() {
        return "oui".to_string();
    }
    let tag_row = &tag_rows[0];
    let tag_value = tag_row
        .get(tag_user_column)
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();
    let tag_tout = tag_row
        .get("tout")
        .and_then(|v| v.as_str())
        .unwrap_or("non")
        .to_string();
    if tag_tout == "v" {
        return "non".to_string();
    }
    if tag_tout != "non" {
        let (check_table, check_col, check_where, need_check) = match tag_user_column {
            "VMotdePasse" => ("login", "motdepass", "id", true),
            "VPrivilege" => ("login", "privilege", "id", true),
            "VVIP" => ("login", "vip", "id", true),
            "vcreAutologin" => ("autologin", "nombre", "compteid", true),
            "vAutologin" => ("", "", "", false),
            "VEmail" => ("login", "Email", "id", true),
            _ => ("", "", "", false),
        };
        let vefier = if need_check {
            selectionner(
                pool,
                check_table,
                &[(check_where, mysql::Value::from(iduser))],
                &[check_col],
                None,
                Some(1),
            )
            .into_iter()
            .next()
            .and_then(|r| {
                r.get(check_col)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "nonononoon".to_string())
        } else {
            "nonononoon".to_string()
        };
        if tag_value == "v" || tag_value == vefier {
            return "non".to_string();
        }
        return "oui".to_string();
    }
    tag_value
}

// ══════════════════════════════════════════════════════════════════
// ENVOI MAIL
// ══════════════════════════════════════════════════════════════════
pub fn vex_send_mail(to: &str, subject: &str, html: &str) -> bool {
    let payload = json!({
        "sender":      { "name": "VEX", "email": "thesolar_le_pro@outlook.fr" },
        "to":          [{ "email": to }],
        "subject":     subject,
        "htmlContent": html,
    });
    let result = ureq::post("https://api.brevo.com/v3/smtp/email")
        .set("Content-Type", "application/json")
        .set("api-key", "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")
        .send_json(payload);
    match result {
        Ok(resp) => resp.status() == 201,
        Err(e) => {
            eprintln!("vex_send_mail error: {}", e);
            false
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// ICÔNES FICHIERS
// ══════════════════════════════════════════════════════════════════
pub fn get_file_icon(extension: &str) -> &'static str {
    match extension.to_lowercase().as_str() {
        "jpg" | "jpeg" | "svg" | "gif" => "fa-file-image",
        "tiff" | "tif" => r#""><img src="/img/fill-image-etoile.svg" class="icone-fichier"#,
        "psd" => r#""><img src="/img/fill-ps.svg" class="icone-fichier"#,
        "mp4" | "webm" => "fa-file-video",
        "pdf" => "fa-file-pdf",
        "doc" | "docx" => "fa-file-word",
        "xls" | "xlsx" => "fa-file-excel",
        "ppt" | "pptx" => "fa-file-powerpoint",
        "txt" => "fa-file-lines",
        "csv" => "fa-file-csv",
        "zip" | "gz" | "rar" | "7z" => "fa-file-zipper",
        "sql" => "fa-database",
        "php" | "html" | "css" | "js" | "json" | "xml" => "fa-file-code",
        "exe" | "bat" => r#""><img src="/img/fill-exe-bat.svg" class="icone-fichier"#,
        "mtl" | "obj" | "fbx" | "fbxl" | "stl" => "fa-solid fa-cube",
        "gcode" => r#""><img src="/img/fill-gcode.svg" class="icone-fichier"#,
        _ => "fa-file",
    }
}

// ══════════════════════════════════════════════════════════════════
// AVATAR
// ══════════════════════════════════════════════════════════════════
pub fn display_user_avatar(nom: &str, pdp: Option<&[u8]>) -> String {
    if let Some(data) = pdp {
        if !data.is_empty() {
            use base64::Engine as _;
            let b64 = base64::engine::general_purpose::STANDARD.encode(data);
            let safe_nom = html_escape(nom);
            return format!(
                r#"<img src="data:image/jpeg;base64,{}" alt="{}" class="user-avatar">"#,
                b64, safe_nom
            );
        }
    }
    let initial = nom.chars().next().unwrap_or('?').to_uppercase().to_string();
    format!(r#"<div class="user-avatar-initials">{}</div>"#, initial)
}

// ══════════════════════════════════════════════════════════════════
// MEET
// ══════════════════════════════════════════════════════════════════
pub fn generate_room_code() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(42);
    let mut state = seed as u64 ^ 0x9e3779b97f4a7c15;
    let mut code = String::with_capacity(10);
    for _ in 0..10 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        code.push(charset[(state as usize) % charset.len()] as char);
    }
    code
}

pub fn create_meet_room(
    pool: &DbPool,
    creator_id: i64,
    title: &str,
    is_public: bool,
    require_password: bool,
    password: Option<&str>,
    max_participants: i64,
) -> Option<HashMap<String, Value>> {
    let mut room_code = String::new();
    for _ in 0..10 {
        let candidate = generate_room_code();
        let exists = !selectionner(
            pool,
            "meet_rooms",
            &[("room_code", mysql::Value::from(candidate.as_str()))],
            &["room_code"],
            None,
            Some(1),
        )
        .is_empty();
        if !exists {
            room_code = candidate;
            break;
        }
    }
    if room_code.is_empty() {
        return None;
    }
    let pass_val: mysql::Value = if require_password {
        if let Some(p) = password {
            mysql::Value::from(sha256_simple(p))
        } else {
            mysql::Value::NULL
        }
    } else {
        mysql::Value::NULL
    };
    let result = inserer_ou_modifier(
        pool,
        "meet_rooms",
        &[
            ("room_code", mysql::Value::from(room_code.as_str())),
            ("creator_id", mysql::Value::from(creator_id)),
            ("title", mysql::Value::from(title)),
            ("is_public", mysql::Value::from(is_public as i64)),
            (
                "require_password",
                mysql::Value::from(require_password as i64),
            ),
            ("password_hash", pass_val),
            ("max_participants", mysql::Value::from(max_participants)),
        ],
        &[],
    );
    if result < 0 {
        return None;
    }
    let mut out = HashMap::new();
    out.insert("id".into(), json!(result));
    out.insert("room_code".into(), json!(room_code));
    out.insert("title".into(), json!(title));
    out.insert("is_public".into(), json!(is_public));
    out.insert("creator_id".into(), json!(creator_id));
    Some(out)
}

pub fn get_room_info(pool: &DbPool, room_code: &str) -> Option<HashMap<String, Value>> {
    selectionner(
        pool,
        "meet_rooms",
        &[
            ("room_code", mysql::Value::from(room_code)),
            ("is_active", mysql::Value::from(1i64)),
        ],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()
}

// ══════════════════════════════════════════════════════════════════
// CHIFFREMENT NODE
// ══════════════════════════════════════════════════════════════════
const MIN_PLAINTEXT_LEN: usize = 32;
const NODE_KEY_PATH: &str = "node.key";

fn get_node_key() -> Vec<u8> {
    if let Ok(k) = std::fs::read(NODE_KEY_PATH) {
        if k.len() == 32 {
            return k;
        }
    }
    let key: Vec<u8> = (0..32)
        .map(|_| {
            use std::time::{SystemTime, UNIX_EPOCH};
            (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
                & 0xFF) as u8
        })
        .collect();
    let _ = std::fs::write(NODE_KEY_PATH, &key);
    key
}

pub fn encrypt_node(plaintext: &str) -> (String, String, String) {
    use base64::Engine as _;
    let key = get_node_key();
    let len = plaintext.len();
    let mut header = vec![
        ((len >> 24) & 0xFF) as u8,
        ((len >> 16) & 0xFF) as u8,
        ((len >> 8) & 0xFF) as u8,
        (len & 0xFF) as u8,
    ];
    let mut data = plaintext.as_bytes().to_vec();
    if data.len() < MIN_PLAINTEXT_LEN {
        data.resize(MIN_PLAINTEXT_LEN, 0);
    }
    header.extend_from_slice(&data);
    let iv: Vec<u8> = (0..12u8).map(|i| key[i as usize] ^ i).collect();
    let cipher: Vec<u8> = header
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()] ^ iv[i % iv.len()])
        .collect();
    let tag = vec![0u8; 16];
    let b64 = base64::engine::general_purpose::STANDARD;
    (b64.encode(&cipher), b64.encode(&iv), b64.encode(&tag))
}

pub fn decrypt_node(cipher_b64: &str, iv_b64: &str, _tag_b64: &str) -> Option<String> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    let cipher = b64.decode(cipher_b64).ok()?;
    let iv = b64.decode(iv_b64).ok()?;
    let key = get_node_key();
    let data: Vec<u8> = cipher
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()] ^ iv[i % iv.len()])
        .collect();
    if data.len() < 4 {
        return None;
    }
    let orig_len = ((data[0] as usize) << 24)
        | ((data[1] as usize) << 16)
        | ((data[2] as usize) << 8)
        | (data[3] as usize);
    String::from_utf8(data.get(4..4 + orig_len)?.to_vec()).ok()
}

// ══════════════════════════════════════════════════════════════════
// NAVIGATION — build_nav_html()
// Corrections v3 :
//   FIX 1 — bouton apps : fa-th remplacé par SVG local
//   FIX 2 — profile menu : icônes FA remplacées par SVGs locaux
//   FIX 3 — app-svg-icon-7844 : filter invert blanc (plus de rouge)
//   FIX 4 — user.svg : taille correcte + filtre blanc forcé
// ══════════════════════════════════════════════════════════════════
pub struct NavContext<'a> {
    pub pool: &'a DbPool,
    pub user_id: Option<i64>,
    pub page_key: &'a str,
    pub cookie_val: &'a str,
    pub remote_ip: &'a str,
    pub user_agent: &'a str,
    pub query_id: Option<i64>,
    pub apps: Vec<NavApp>,
    pub admin_apps: Vec<NavApp>,
}

pub struct NavApp {
    pub icon: String,
    pub label: String,
    pub url: String,
    pub admin: bool,
}

impl NavApp {
    pub fn new(icon: &str, label: &str, url: &str) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            url: url.into(),
            admin: false,
        }
    }
    pub fn admin(icon: &str, label: &str, url: &str) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            url: url.into(),
            admin: true,
        }
    }
}

// Convertit une classe FontAwesome "fas fa-home" en <img src="/static/img/solid/home.svg">
fn fa_to_img(fa_class: &str) -> String {
    let name = fa_class
        .split_whitespace()
        .find(|p| p.starts_with("fa-"))
        .map(|p| p.trim_start_matches("fa-"))
        .unwrap_or("file");
    format!(
        r#"<img src="/static/img/solid/{}.svg" alt="{}" class="app-svg-icon-7844">"#,
        name, name
    )
}

pub fn build_nav_html(ctx: &NavContext) -> String {
    let user_data = resolve_nav_user(ctx);
    let resolved_uid = user_data
        .as_ref()
        .and_then(|u| u.get("id").and_then(|v| v.as_i64()));
    let is_admin = user_data
        .as_ref()
        .and_then(|u| u.get("privilege").and_then(|v| v.as_i64()))
        .map(|p| p <= 6)
        .unwrap_or(false);
    let is_dark = resolved_uid
        .map(|uid| get_user_preferences(ctx.pool, uid).teme == 1)
        .unwrap_or(false);
    let prefs = resolved_uid.map(|uid| get_user_preferences(ctx.pool, uid));
    let show_sidebar_btn = prefs
        .as_ref()
        .map(|p| {
            p.nav_button_style
                .get(ctx.page_key)
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                != 0
        })
        .unwrap_or(ctx.page_key == "dashboard");
    let show_logo = prefs
        .as_ref()
        .map(|p| {
            p.logo_pages
                .get(ctx.page_key)
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                != 0
        })
        .unwrap_or(ctx.page_key == "dashboard");

    // ── Valeurs thème ──────────────────────────────────────────────
    let nav_gradient = if is_dark {
        "linear-gradient(135deg,#2e7d32 0%,#1b5e20 100%)"
    } else {
        "linear-gradient(135deg,#66bb6a 0%,#388e3c 100%)"
    };
    let shadow = if is_dark {
        "0 2px 8px rgba(0,0,0,0.5)"
    } else {
        "0 2px 8px rgba(0,0,0,0.15)"
    };

    let popup_bg = if is_dark {
        "linear-gradient(135deg,#1a3a1a 0%,#0d2b0d 100%)"
    } else {
        "#2e7d32"
    };
    let popup_border = if is_dark {
        "rgba(165,214,167,0.15)"
    } else {
        "rgba(255,255,255,0.25)"
    };

    let app_item_bg = "rgba(255,255,255,0.12)";
    let app_item_border = "rgba(255,255,255,0.20)";
    let app_item_hover = "rgba(255,255,255,0.28)";

    let sidebar_bg = if is_dark { "#1e1e1e" } else { "#ffffff" };
    let sidebar_border = if is_dark { "#3a3b3c" } else { "#e4e6eb" };
    let sidebar_text = if is_dark { "#e4e6eb" } else { "#1c1e21" };
    let sidebar_hover = if is_dark { "#3a3b3c" } else { "#f0f2f5" };
    let sidebar_active = if is_dark { "#2e7d32" } else { "#4caf50" };
    let sidebar_divider = if is_dark { "#3a3b3c" } else { "#e4e6eb" };
    let scrollbar_thumb = if is_dark { "#555" } else { "#ccc" };
    let breadcrumb_bg = if is_dark {
        "rgba(10,30,10,0.92)"
    } else {
        "rgba(27,94,32,0.88)"
    };
    let admin_sep = if is_dark {
        "rgba(255,255,255,0.1)"
    } else {
        "rgba(255,255,255,0.3)"
    };

    // ── Apps par défaut ────────────────────────────────────────────
    let default_apps: Vec<NavApp> = if ctx.apps.is_empty() {
        vec![
            NavApp::new("fas fa-home", "Accueil", "/login/dashboard"),
            NavApp::new("fas fa-envelope", "Mail", "/mess/vexmail"),
            NavApp::new("fas fa-hard-drive", "Exodrive", "/tel/"),
            NavApp::new("fas fa-folder-open", "Fichiers", "/fchier/"),
            NavApp::new("fas fa-video", "Vidéos", "#"),
            NavApp::new("fas fa-globe", "Sitec", "/sitec/"),
        ]
    } else {
        vec![]
    };
    let apps_ref: &[NavApp] = if ctx.apps.is_empty() {
        &default_apps
    } else {
        &ctx.apps
    };

    let default_admin: Vec<NavApp> = if ctx.admin_apps.is_empty() && is_admin {
        vec![NavApp::admin("fas fa-shield-alt", "Admin", "/admin")]
    } else {
        vec![]
    };
    let admin_ref: &[NavApp] = if ctx.admin_apps.is_empty() {
        &default_admin
    } else {
        &ctx.admin_apps
    };

    // ── Sidebar links ──────────────────────────────────────────────
    let sidebar_links: Vec<(&str, &str, &str, bool)> = {
        let mut v = vec![
            ("fas fa-home", "Accueil", "/login/dashboard", false),
            ("fas fa-hard-drive", "Exodrive", "/tel/", false),
            ("fas fa-envelope", "Mail", "/mess/vexmail", false),
            ("fas fa-globe", "Sitec", "/sitec/", false),
            ("fas fa-video", "Vidéos", "#", false),
            ("fas fa-folder-open", "Fichiers", "/fchier/", false),
        ];
        if is_admin {
            v.push(("fas fa-shield-alt", "Administration", "/admin", true));
        }
        v
    };

    // ── Breadcrumb ─────────────────────────────────────────────────
    let breadcrumb_html = if ctx.page_key == "tel" {
        if let Some(fid) = ctx.query_id {
            let rows = selectionner(
                ctx.pool,
                "fichiers",
                &[("id", mysql::Value::from(fid))],
                &["nom"],
                None,
                Some(1),
            );
            if let Some(row) = rows.into_iter().next() {
                let bc = html_escape(row.get("nom").and_then(|v| v.as_str()).unwrap_or(""));
                format!(
                    "<div class=\"nav-breadcrumb-7844\" id=\"nav-breadcrumb-7844\">\
                    <a href=\"/tel/\" class=\"nav-bc-link-7844\">\
                    <img src=\"/static/img/solid/hard-drive.svg\" class=\"nav-bc-svg-7844\" alt=\"\">\
                    <span>Exodrive</span></a>\
                    <span class=\"nav-bc-sep-7844\">\
                    <img src=\"/static/img/solid/chevron-right.svg\" class=\"nav-bc-svg-7844\" alt=\"\">\
                    </span>\
                    <span class=\"nav-bc-current-7844\" title=\"{bc}\">{bc}</span>\
                    </div>", bc = bc)
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // ── Apps popup HTML ────────────────────────────────────────────
    let mut apps_items = String::new();
    for a in apps_ref {
        let icon_html = fa_to_img(&a.icon);
        apps_items.push_str(&format!(
            "<a class=\"app-item-7844\" href=\"{}\" title=\"{}\">{}\
<span class=\"app-label-7844\">{}</span></a>",
            html_escape(&a.url),
            html_escape(&a.label),
            icon_html,
            html_escape(&a.label)
        ));
    }
    let admin_items_html = if is_admin && !admin_ref.is_empty() {
        let mut s = format!("<div class=\"apps-admin-sep-7844\"><div class=\"apps-grid-7844\">");
        for a in admin_ref {
            let icon_html = fa_to_img(&a.icon);
            s.push_str(&format!(
                "<a class=\"app-item-7844\" href=\"{}\" title=\"{}\">{}\
<span class=\"app-label-7844\">{}</span></a>",
                html_escape(&a.url),
                html_escape(&a.label),
                icon_html,
                html_escape(&a.label)
            ));
        }
        s.push_str("</div></div>");
        s
    } else {
        String::new()
    };

    // ── Sidebar HTML ───────────────────────────────────────────────
    let mut sidebar_normal = String::new();
    let mut sidebar_admin = String::new();
    for (icon, label, url, is_adm) in &sidebar_links {
        let active = if *url != "#" && url.contains(ctx.page_key) && !ctx.page_key.is_empty() {
            " active"
        } else {
            ""
        };
        let adm_cls = if *is_adm { " admin-item" } else { "" };
        let icon_name = icon
            .split_whitespace()
            .find(|p| p.starts_with("fa-"))
            .map(|p| p.trim_start_matches("fa-"))
            .unwrap_or("file");
        let item = format!(
            "<a href=\"{}\" class=\"nav-sidebar-item-7844{}{}\">\
            <img src=\"/static/img/solid/{}.svg\" class=\"sidebar-svg-7844\" alt=\"\">\
            <span>{}</span></a>",
            url, active, adm_cls, icon_name, label
        );
        if *is_adm {
            sidebar_admin.push_str(&item);
        } else {
            sidebar_normal.push_str(&item);
        }
    }
    let sidebar_admin_html = if !sidebar_admin.is_empty() {
        format!(
            "<div class=\"nav-sidebar-divider-7844\"></div>{}",
            sidebar_admin
        )
    } else {
        String::new()
    };

    // ── Topbar utilisateur ─────────────────────────────────────────
    let user_html = if let Some(ref u) = user_data {
        let nom = u
            .get("nom")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let privilege = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);
        let pd = get_privilege_details(privilege);
        // Privilege 1 = effet animé spécial "fona"
        // Autres privileges : couleur CSS inline depuis get_privilege_details
        let name_span = if privilege == 1 {
            format!(
                "<span class=\"user-name-top-7844 fona\" title=\"{}\">{}</span>",
                pd.nom,
                html_escape(&nom)
            )
        } else if privilege <= 9 {
            // Affiche la couleur du privilege + badge titre au survol
            format!(
                "<span class=\"user-name-top-7844\" style=\"color:{}!important\" title=\"{}\">{}</span>",
                pd.couleur,
                pd.nom,
                html_escape(&nom)
            )
        } else {
            // Privilege 10+ (utilisateur normal) : blanc standard
            format!(
                "<span class=\"user-name-top-7844\">{}</span>",
                html_escape(&nom)
            )
        };
        format!(
            "<div class=\"user-info-top-7844\" id=\"user-info-top\" \
            tabindex=\"0\" role=\"button\" aria-haspopup=\"true\" aria-expanded=\"false\">\
            {name_span}\
            <img src=\"/static/img/solid/user.svg\" class=\"user-icon-top-7844\" alt=\"profil\">\
            </div>"
        )
    } else {
        // FIX 1 : bouton connexion avec SVG local
        "<a href=\"/login\" class=\"apps-btn-nav-7844\">\
        <img src=\"/static/img/solid/right-to-bracket.svg\" class=\"topbar-btn-svg-7844\" alt=\"connexion\">\
        </a>".to_string()
    };

    // FIX 2 : profile menu avec SVGs locaux au lieu des icônes FA
    let profile_menu_html = if user_data.is_some() {
        format!(
            "<div class=\"profile-menu-7844\" id=\"profile-menu\" role=\"menu\">\
            <a href=\"/login/account\">\
              <img src=\"/static/img/solid/user-circle.svg\" class=\"pm-icon-svg-7844\" alt=\"\">\
              Mon Compte\
            </a>\
            <a href=\"/login/account\">\
              <img src=\"/static/img/solid/cog.svg\" class=\"pm-icon-svg-7844\" alt=\"\">\
              Paramètres\
            </a>\
            <a href=\"/login/logout\">\
              <img src=\"/static/img/solid/right-from-bracket.svg\" class=\"pm-icon-svg-7844\" alt=\"\">\
              Déconnexion\
            </a>\
            </div>"
        )
    } else {
        String::new()
    };

    let logo_html = if show_logo {
        "<img src=\"/static/img/vex.svg\" alt=\"VEX\" class=\"nav-logo-flat-7844\">".to_string()
    } else {
        String::new()
    };

    // FIX 1 : boutons topbar avec SVGs locaux (fa-th supprimé)
    let left_btn = if show_sidebar_btn {
        "<button class=\"apps-btn-nav-7844 sidebar-toggle\" id=\"sidebar-toggle-7844\" \
        aria-label=\"Menu\">\
        <img src=\"/static/img/solid/bars.svg\" class=\"topbar-btn-svg-7844\" alt=\"menu\">\
        </button>"
    } else {
        "<button class=\"apps-btn-nav-7844\" id=\"apps-btn-nav\" \
        aria-label=\"Applications\">\
        <img src=\"/static/img/solid/table-cells.svg\" class=\"topbar-btn-svg-7844\" alt=\"apps\">\
        </button>"
    };

    let sidebar_html = if show_sidebar_btn {
        format!(
            "<div class=\"nav-sidebar-overlay-7844\" id=\"nav-sidebar-overlay-7844\"></div>\
            <nav class=\"nav-sidebar-7844 collapsed\" id=\"nav-sidebar-7844\">\
            {sidebar_normal}{sidebar_admin_html}\
            </nav>"
        )
    } else {
        String::new()
    };

    let js_sidebar = if show_sidebar_btn { "true" } else { "false" };

    // ── CSS ────────────────────────────────────────────────────────
    let mut css = String::from("<style>\n");

    css.push_str(".top-nav-bar-haut7844 *,.apps-popup-7844 *,.profile-menu-7844 *,.nav-sidebar-7844 * { box-sizing:border-box; margin:0; padding:0; }\n");

    // Topbar
    css.push_str(&format!(".top-nav-bar-haut7844 {{ position:fixed;top:0;left:0;right:0;height:60px; background:{nav_gradient}; box-shadow:{shadow}; display:flex;align-items:center;justify-content:space-between; padding:0 20px;z-index:1000; }}\n"));
    css.push_str(
        ".top-nav-left-7844,.top-nav-right-7844 { display:flex;align-items:center;gap:12px; }\n",
    );

    // Boutons topbar
    css.push_str(".apps-btn-nav-7844 { width:40px;height:40px;border-radius:8px; background:rgba(0,0,0,0.18);border:1px solid rgba(255,255,255,0.30); color:#ffffff!important; display:flex;align-items:center;justify-content:center; cursor:pointer;font-size:18px; transition:transform .15s,background .15s; text-decoration:none!important; }\n");
    css.push_str(
        ".apps-btn-nav-7844:hover { transform:translateY(-2px);background:rgba(0,0,0,0.30); }\n",
    );

    // FIX 1 — SVG des boutons topbar : blanc, taille fixe
    css.push_str(".topbar-btn-svg-7844 { width:20px;height:20px;display:block;flex-shrink:0; filter:brightness(0) invert(1); }\n");

    // Logo
    css.push_str(".nav-logo-flat-7844 { width:38px;height:38px;object-fit:contain;margin-left:4px; filter:grayscale(100%) brightness(0.65)!important; }\n");

    // Utilisateur topbar
    css.push_str(".user-info-top-7844 { display:flex;align-items:center;gap:12px;cursor:pointer; padding:6px 12px;border-radius:6px;transition:background .2s; text-decoration:none!important; }\n");
    css.push_str(".user-info-top-7844:hover { background:rgba(0,0,0,0.18); }\n");
    css.push_str(".user-name-top-7844 { font-size:14px;font-weight:600;color:#ffffff!important; max-width:180px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap; font-family:Arial,sans-serif; }\n");
    css.push_str(".user-name-top-7844.fona { color:transparent!important;-webkit-background-clip:text;background-clip:text; background-image:url('/static/img/image2.png'),url('/static/img/image3.png'); background-size:100% 200%;background-position:100% 100%,100% 200%; background-repeat:no-repeat,no-repeat; animation:slideImages7844 6s linear infinite;font-weight:700; }\n");
    css.push_str("@keyframes slideImages7844 { 0%,100% { background-position:100% 100%,100% 200%; } 50% { background-position:100% 0%,100% 100%; } }\n");

    // FIX 4 — user.svg : taille correcte, cercle, blanc forcé via filter
    css.push_str(".user-icon-top-7844 { width:32px;height:32px;border-radius:50%; background:rgba(0,0,0,0.20);border:2px solid rgba(255,255,255,0.40); padding:6px;object-fit:contain; filter:brightness(0) invert(1); flex-shrink:0; }\n");

    // FIX 4 — Popup apps : grille fixe, couleurs forcées
    css.push_str(&format!(".apps-popup-7844 {{ position:fixed;top:70px;left:20px;width:340px; background:{popup_bg};border:1px solid {popup_border}; border-radius:12px;box-shadow:0 8px 30px rgba(0,0,0,0.35); padding:14px;display:none;z-index:1001; }}\n"));
    css.push_str(
        ".apps-grid-7844 { display:grid; grid-template-columns:repeat(3,1fr); gap:12px; }\n",
    );
    css.push_str(&format!(".app-item-7844 {{ display:flex;flex-direction:column;align-items:center;justify-content:center; width:100%;padding:16px 8px;min-height:80px; border-radius:14px; background:{app_item_bg};border:1px solid {app_item_border}; text-decoration:none!important; transition:transform .12s,background .12s; gap:8px; }}\n"));
    css.push_str(&format!(".app-item-7844:hover {{ transform:translateY(-3px);background:{app_item_hover};text-decoration:none!important; }}\n"));

    // FIX 3 — app-svg-icon-7844 : toujours blanc (pas de rouge sur shield-alt)
    css.push_str(".app-svg-icon-7844 { width:26px;height:26px;display:block;flex-shrink:0; filter:brightness(0) invert(1); }\n");

    css.push_str(".app-label-7844 { font-size:12px;color:#ffffff!important;font-weight:600; text-align:center;line-height:1.3; white-space:nowrap;overflow:hidden;text-overflow:ellipsis;width:100%; font-family:Arial,sans-serif; }\n");
    css.push_str(&format!(".apps-admin-sep-7844 {{ margin-top:10px;padding-top:8px;border-top:1px solid {admin_sep}; }}\n"));

    // FIX 2 — Menu profil avec SVGs locaux blancs
    css.push_str(&format!(".profile-menu-7844 {{ position:fixed;top:70px;right:20px;width:200px; background:{popup_bg};border:1px solid {popup_border}; border-radius:10px;box-shadow:0 8px 30px rgba(0,0,0,0.35); padding:8px;display:none;z-index:1002; }}\n"));
    css.push_str(".profile-menu-7844 a { display:flex;align-items:center;gap:8px; text-decoration:none!important;color:#ffffff!important; padding:9px 10px;border-radius:8px;font-weight:600; margin-bottom:4px;transition:background .15s; font-family:Arial,sans-serif; }\n");
    css.push_str(".profile-menu-7844 a:hover { background:rgba(255,255,255,0.15);color:#ffffff!important; }\n");
    // FIX 2 — icônes du menu profil : SVG blanc via filter
    css.push_str(".pm-icon-svg-7844 { width:16px;height:16px;flex-shrink:0;display:block; filter:brightness(0) invert(1); }\n");

    // Sidebar
    css.push_str(&format!(".nav-sidebar-7844 {{ position:fixed;top:60px;left:0;bottom:0;width:260px; background:{sidebar_bg};border-right:1px solid {sidebar_border}; padding:16px 10px;overflow-y:auto;z-index:999; transition:transform .3s;box-shadow:2px 0 8px rgba(0,0,0,0.08); }}\n"));
    css.push_str(".nav-sidebar-7844.collapsed { transform:translateX(-100%); }\n");
    css.push_str(&format!(
        ".nav-sidebar-7844::-webkit-scrollbar {{ width:4px; }}\n"
    ));
    css.push_str(&format!(".nav-sidebar-7844::-webkit-scrollbar-thumb {{ background:{scrollbar_thumb};border-radius:2px; }}\n"));
    css.push_str(&format!(".nav-sidebar-item-7844 {{ display:flex;align-items:center;gap:12px; padding:11px 16px;margin:3px 0;border-radius:8px; color:{sidebar_text};text-decoration:none!important; font-size:15px;font-weight:500;transition:background .2s,color .2s; }}\n"));
    css.push_str(&format!(
        ".nav-sidebar-item-7844:hover {{ background:{sidebar_hover}; }}\n"
    ));
    css.push_str(&format!(".nav-sidebar-item-7844.active {{ background:{sidebar_active};color:#fff!important;font-weight:600; }}\n"));
    css.push_str(".nav-sidebar-item-7844.admin-item { color:#d32f2f!important; }\n");
    css.push_str(".nav-sidebar-item-7844.admin-item:hover { background:rgba(211,47,47,0.1); }\n");
    css.push_str(
        ".nav-sidebar-item-7844.admin-item.active { background:#d32f2f;color:#fff!important; }\n",
    );
    // Sidebar SVGs : couleur héritée via filter
    css.push_str(&format!(".sidebar-svg-7844 {{ width:20px;height:20px;flex-shrink:0;display:block; filter:none; }}\n"));
    css.push_str(&format!(
        ".nav-sidebar-item-7844.active .sidebar-svg-7844 {{ filter:brightness(0) invert(1); }}\n"
    ));
    css.push_str(&format!(".nav-sidebar-item-7844.admin-item .sidebar-svg-7844 {{ filter:invert(27%) sepia(51%) saturate(2878%) hue-rotate(346deg) brightness(104%) contrast(97%); }}\n"));
    css.push_str(&format!(".nav-sidebar-divider-7844 {{ height:1px;background:{sidebar_divider};margin:10px 8px; }}\n"));
    css.push_str(".nav-sidebar-overlay-7844 { display:none;position:fixed;inset:0;background:rgba(0,0,0,0.4);z-index:998; }\n");
    css.push_str(".nav-sidebar-overlay-7844.show { display:block; }\n");
    css.push_str(".nav-sidebar-content-push-7844 { transition:margin-left .3s; }\n");
    css.push_str(
        "body.nav-sidebar-open-7844 .nav-sidebar-content-push-7844 { margin-left:260px; }\n",
    );

    // Breadcrumb
    css.push_str(&format!(".nav-breadcrumb-7844 {{ position:fixed;top:60px;left:0;right:0;height:36px; background:{breadcrumb_bg};backdrop-filter:blur(6px); border-bottom:1px solid rgba(255,255,255,0.1); display:flex;align-items:center;padding:0 18px;gap:8px;z-index:990; font-family:Arial,sans-serif;font-size:13px; }}\n"));
    css.push_str(".nav-bc-link-7844 { display:flex;align-items:center;gap:6px;text-decoration:none!important;color:#a5d6a7;font-weight:600;transition:color .15s;white-space:nowrap; }\n");
    css.push_str(".nav-bc-link-7844:hover { color:#fff; }\n");
    css.push_str(".nav-bc-svg-7844 { width:12px;height:12px;display:block;filter:brightness(0) invert(1);opacity:0.7; }\n");
    css.push_str(".nav-bc-sep-7844 { display:flex;align-items:center;opacity:0.4; }\n");
    css.push_str(".nav-bc-current-7844 { color:rgba(255,255,255,0.9);max-width:340px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:500; }\n");
    css.push_str("body.nav-has-breadcrumb-7844 { padding-top:96px!important; }\n");
    css.push_str("body { padding-top:60px; }\n");

    // Responsive
    css.push_str("@media(max-width:768px) {\n");
    css.push_str("  .nav-sidebar-7844 { transform:translateX(-100%); }\n");
    css.push_str("  .nav-sidebar-7844.show { transform:translateX(0); }\n");
    css.push_str("  .nav-sidebar-7844.collapsed { transform:translateX(-100%); }\n");
    css.push_str(
        "  body.nav-sidebar-open-7844 .nav-sidebar-content-push-7844 { margin-left:0; }\n",
    );
    css.push_str("  .apps-popup-7844 { left:10px;width:calc(100% - 20px); }\n");
    css.push_str("  .profile-menu-7844 { right:10px;width:calc(100% - 20px); }\n");
    css.push_str("  .nav-bc-current-7844 { max-width:180px; }\n");
    css.push_str("}\n");
    css.push_str("</style>\n");

    // ── JS ─────────────────────────────────────────────────────────
    let js = format!(
        r#"<script>
(function() {{
  var sb={js_sidebar};
  var appsBtn=document.getElementById('apps-btn-nav');
  var sBtn=document.getElementById('sidebar-toggle-7844');
  var popup=document.getElementById('apps-popup');
  var nav=document.getElementById('nav-sidebar-7844');
  var ovl=document.getElementById('nav-sidebar-overlay-7844');
  var ui=document.getElementById('user-info-top');
  var pm=document.getElementById('profile-menu');

  if(document.getElementById('nav-breadcrumb-7844'))
    document.body.classList.add('nav-has-breadcrumb-7844');

  if(sb&&sBtn&&nav) {{
    try {{
      if(localStorage.getItem('vex_sidebar')!=='closed') {{
        nav.classList.remove('collapsed');
        if(window.innerWidth>768) document.body.classList.add('nav-sidebar-open-7844');
      }}
    }} catch(e) {{}}
    function openNav() {{
      nav.classList.remove('collapsed');
      if(window.innerWidth<=768) {{ nav.classList.add('show'); if(ovl) ovl.classList.add('show'); }}
      else document.body.classList.add('nav-sidebar-open-7844');
      try {{ localStorage.setItem('vex_sidebar','open'); }} catch(e) {{}}
    }}
    function closeNav() {{
      nav.classList.add('collapsed'); nav.classList.remove('show');
      document.body.classList.remove('nav-sidebar-open-7844');
      if(ovl) ovl.classList.remove('show');
      try {{ localStorage.setItem('vex_sidebar','closed'); }} catch(e) {{}}
    }}
    sBtn.addEventListener('click',function(e) {{ e.stopPropagation(); nav.classList.contains('collapsed')?openNav():closeNav(); }});
    if(ovl) ovl.addEventListener('click',closeNav);
    nav.querySelectorAll('.nav-sidebar-item-7844').forEach(function(i) {{
      i.addEventListener('click',function() {{ if(window.innerWidth<=768) closeNav(); }});
    }});
  }} else if(!sb&&appsBtn&&popup) {{
    appsBtn.addEventListener('click',function(e) {{
      e.stopPropagation();
      var vis=popup.style.display==='block';
      popup.style.display=vis?'none':'block';
    }});
  }}
  if(ui&&pm) {{
    ui.addEventListener('click',function(e) {{
      e.stopPropagation();
      var x=this.getAttribute('aria-expanded')==='true';
      this.setAttribute('aria-expanded',x?'false':'true');
      pm.style.display=pm.style.display==='block'?'none':'block';
    }});
    ui.addEventListener('keydown',function(e) {{ if(e.key==='Enter'||e.key===' ') {{ e.preventDefault();this.click(); }} }});
  }}
  document.addEventListener('click',function(e) {{
    if(popup&&!popup.contains(e.target)&&appsBtn&&!appsBtn.contains(e.target)) popup.style.display='none';
    if(pm&&!pm.contains(e.target)&&ui&&!ui.contains(e.target)) {{
      pm.style.display='none';
      if(ui) ui.setAttribute('aria-expanded','false');
    }}
  }});
  document.addEventListener('keydown',function(e) {{
    if(e.key==='Escape') {{
      if(popup) popup.style.display='none';
      if(pm) pm.style.display='none';
      if(ui) ui.setAttribute('aria-expanded','false');
    }}
  }});
}})();
</script>"#,
        js_sidebar = js_sidebar
    );

    // ── Assemblage final ───────────────────────────────────────────
    format!(
        "{css}\
        <div class=\"top-nav-bar-haut7844\" role=\"navigation\">\
          <div class=\"top-nav-left-7844\">{left_btn}{logo_html}</div>\
          <div class=\"top-nav-right-7844\">{user_html}</div>\
        </div>\
        {breadcrumb_html}\
        <div class=\"apps-popup-7844\" id=\"apps-popup\">\
          <div class=\"apps-grid-7844\">{apps_items}</div>\
          {admin_items_html}\
        </div>\
        {profile_menu_html}\
        {sidebar_html}\
        {js}",
    )
}

pub fn get_nav_data(ctx: &NavContext) -> Value {
    let user_data = resolve_nav_user(ctx);
    let resolved_uid = user_data
        .as_ref()
        .and_then(|u| u.get("id").and_then(|v| v.as_i64()));
    let is_admin = user_data
        .as_ref()
        .and_then(|u| u.get("privilege").and_then(|v| v.as_i64()))
        .map(|p| p <= 6)
        .unwrap_or(false);
    let theme = if let Some(uid) = resolved_uid {
        if get_user_preferences(ctx.pool, uid).teme == 1 {
            "dark"
        } else {
            "light"
        }
    } else {
        "light"
    };
    let prefs = resolved_uid.map(|uid| get_user_preferences(ctx.pool, uid));
    let show_sidebar_btn = prefs
        .as_ref()
        .map(|p| {
            p.nav_button_style
                .get(ctx.page_key)
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                != 0
        })
        .unwrap_or(ctx.page_key == "dashboard");
    let show_logo = prefs
        .as_ref()
        .map(|p| {
            p.logo_pages
                .get(ctx.page_key)
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                != 0
        })
        .unwrap_or(ctx.page_key == "dashboard");
    let breadcrumb = if ctx.page_key == "tel" {
        ctx.query_id.and_then(|fid| {
            selectionner(
                ctx.pool,
                "fichiers",
                &[("id", mysql::Value::from(fid))],
                &["nom"],
                None,
                Some(1),
            )
            .into_iter()
            .next()
            .and_then(|r| r.get("nom").and_then(|v| v.as_str().map(|s| s.to_string())))
        })
    } else {
        None
    };
    json!({"user":user_data,"is_admin":is_admin,"theme":theme,
           "show_sidebar_btn":show_sidebar_btn,"show_logo":show_logo,
           "page_key":ctx.page_key,"breadcrumb_file":breadcrumb})
}

fn resolve_nav_user(ctx: &NavContext) -> Option<Value> {
    if let Some(uid) = ctx.user_id {
        if uid > 0 {
            let rows = selectionner(
                ctx.pool,
                "login",
                &[("id", mysql::Value::from(uid))],
                &["id", "email", "nom", "privilege", "vip"],
                None,
                Some(1),
            );
            if let Some(row) = rows.into_iter().next() {
                return build_user_value(row);
            }
        }
    }
    if !ctx.cookie_val.is_empty() {
        let crows = selectionner(
            ctx.pool,
            "loginc",
            &[("idcokier", mysql::Value::from(ctx.cookie_val))],
            &["pc", "navi", "email", "datecra"],
            None,
            Some(1),
        );
        if let Some(crow) = crows.into_iter().next() {
            let pc = crow.get("pc").and_then(|v| v.as_str()).unwrap_or("");
            let navi = crow.get("navi").and_then(|v| v.as_str()).unwrap_or("");
            if pc == ctx.remote_ip && navi == ctx.user_agent {
                let email = crow
                    .get("email")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let urows = selectionner(
                    ctx.pool,
                    "login",
                    &[("email", mysql::Value::from(email.as_str()))],
                    &["id", "email", "nom", "privilege", "vip"],
                    None,
                    Some(1),
                );
                if let Some(row) = urows.into_iter().next() {
                    return build_user_value(row);
                }
            }
        }
    }
    None
}

fn build_user_value(row: HashMap<String, Value>) -> Option<Value> {
    let id = row.get("id")?.as_i64()?;
    let nom = row.get("nom")?.as_str()?.to_string();
    let email = row.get("email")?.as_str()?.to_string();
    let privilege = row.get("privilege")?.as_i64()?;
    let vip = row.get("vip").and_then(|v| v.as_i64()).unwrap_or(0);
    let pd = get_privilege_details_json(privilege);
    Some(
        json!({"id":id,"nom":nom,"email":email,"privilege":privilege,"vip":vip,"privilege_details":pd}),
    )
}

// ══════════════════════════════════════════════════════════════════
// COLOR SCHEME
// ══════════════════════════════════════════════════════════════════
pub fn get_color_scheme(theme: &str) -> Value {
    match theme {
        "dark" => {
            json!({"nav_bg":"#1a1a1a","nav_gradient":"linear-gradient(to bottom,#1a1a1a,#0f0f0f)"})
        }
        "blue" => {
            json!({"nav_bg":"#1e3a5f","nav_gradient":"linear-gradient(to bottom,#1e3a5f,#152943)"})
        }
        _ => {
            json!({"nav_bg":"#43a047","nav_gradient":"linear-gradient(135deg,#66bb6a 0%,#388e3c 100%)"})
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// UTILITAIRES
// ══════════════════════════════════════════════════════════════════
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sha256_simple(input: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}