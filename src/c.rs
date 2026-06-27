// ══════════════════════════════════════════════════════════════════
// c.rs — VEX session / cookie / blocage
// 0 SQL direct — tout passe par appeldb::
// Fix fuseau horaire : datecra stockée en heure locale (SYSTEM),
// is_recent() compare avec l'heure locale Rust via chrono::Local
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, DbPool};

#[derive(Debug, Clone, Default)]
pub struct SessionInfo {
    pub connecte: bool,
    pub user_id: i64,
    pub user_privilege: i64,
    pub user_vip: i64,
    pub user_email: String,
    pub user_nom: String,
    pub user_idcokier: String,
}

// ══════════════════════════════════════════════════════════════════
// verifier_session()
// ══════════════════════════════════════════════════════════════════
pub fn verifier_session(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> SessionInfo {
    let mut info = SessionInfo {
        user_privilege: 10,
        user_id: 3,
        ..Default::default()
    };

    if cookie_val.is_empty() {
        return info;
    }

    let loginc = selectionner(
        pool,
        "loginc",
        &[("idcokier", mysql::Value::from(cookie_val))],
        &["idcokier", "datecra", "pc", "navi", "email", "nom"],
        None,
        Some(1),
    );

    if loginc.is_empty() {
        return info;
    }

    let row = &loginc[0];
    let pc = row.get("pc").and_then(|v| v.as_str()).unwrap_or("");
    let navi = row.get("navi").and_then(|v| v.as_str()).unwrap_or("");
    let date = row.get("datecra").and_then(|v| v.as_str()).unwrap_or("");

    let pc_stripped = crate::utils::strip_port(pc);

    if pc_stripped != remote_ip || navi != user_agent {
        return info;
    }

    // Comparaison en heure locale (chrono::Local) — cohérent avec
    // l'heure locale stockée par login.rs via chrono::Utc::now()
    // (qui retourne UTC, mais MySQL avec SET time_zone=SYSTEM stocke local)
    // On accepte 3600s = 1h de session
    if !is_recent_local(date, 3600) {
        return info;
    }

    let email = row
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let nom = row
        .get("nom")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let idcok = row
        .get("idcokier")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let login_rows = selectionner(
        pool,
        "login",
        &[("email", mysql::Value::from(email.as_str()))],
        &["id", "vip", "privilege"],
        None,
        Some(1),
    );

    if login_rows.is_empty() {
        return info;
    }

    let lr = &login_rows[0];
    info.user_id = lr.get("id").and_then(|v| v.as_i64()).unwrap_or(3);
    info.user_vip = lr.get("vip").and_then(|v| v.as_i64()).unwrap_or(0);
    info.user_privilege = lr.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);

    info.connecte = true;
    info.user_email = email;
    info.user_nom = nom;
    info.user_idcokier = idcok;
    info
}

// ══════════════════════════════════════════════════════════════════
// verifier_blocage()
// ══════════════════════════════════════════════════════════════════
pub fn verifier_blocage(
    pool: &DbPool,
    user_id: i64,
    user_privilege: i64,
    current_path: &str,
) -> bool {
    let mut blocages = selectionner(
        pool,
        "bloqpage",
        &[("iduserb", mysql::Value::from(user_id))],
        &["pageb", "priviautro"],
        None,
        None,
    );
    let blocages_all = selectionner(
        pool,
        "bloqpage",
        &[("iduserb", mysql::Value::from("all"))],
        &["pageb", "priviautro"],
        None,
        None,
    );
    blocages.extend(blocages_all);

    for row in &blocages {
        let priviautro = row.get("priviautro").and_then(|v| v.as_i64()).unwrap_or(0);
        let pageb = row.get("pageb").and_then(|v| v.as_str()).unwrap_or("");

        if user_privilege <= priviautro {
            continue;
        }

        for page in pageb.split(',') {
            let page = page.trim();
            if page.is_empty() {
                continue;
            }
            if page == "all" {
                return true;
            }
            if current_path.contains(page) {
                return true;
            }
        }
    }
    false
}

// ══════════════════════════════════════════════════════════════════
// aquete()
// ══════════════════════════════════════════════════════════════════
pub fn aquete(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    raison: &str,
    current_path: &str,
) -> bool {
    let session = verifier_session(pool, cookie_val, remote_ip, user_agent);
    let idcokier = if session.user_idcokier.is_empty() {
        "pas".to_string()
    } else {
        session.user_idcokier
    };
    let auteur = format!("{}{}", current_path, raison);
    inserer_ou_modifier(
        pool,
        "sus-hac",
        &[
            ("id-c", mysql::Value::from(idcokier.as_str())),
            ("auteur", mysql::Value::from(auteur.as_str())),
        ],
        &[],
    ) >= 0
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

/// Vérifie si date_str (format "YYYY-MM-DD HH:MM:SS", heure locale)
/// est dans les `seconds` dernières secondes par rapport à l'heure locale.
pub fn is_recent_local(date_str: &str, seconds: i64) -> bool {
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    if parts.len() != 2 {
        return false;
    }
    let dp: Vec<i32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let tp: Vec<i32> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();
    if dp.len() < 3 || tp.len() < 3 {
        return false;
    }

    // Reconstruit un NaiveDateTime depuis la string
    let naive = match chrono::NaiveDate::from_ymd_opt(dp[0], dp[1] as u32, dp[2] as u32)
        .and_then(|d| d.and_hms_opt(tp[0] as u32, tp[1] as u32, tp[2] as u32))
    {
        Some(dt) => dt,
        None => return false,
    };

    // Heure locale actuelle
    let now_local = chrono::Local::now().naive_local();
    let diff = now_local.signed_duration_since(naive).num_seconds();

    diff >= 0 && diff < seconds
}

/// Ancienne version gardée pour compatibilité (comparaison UTC)
pub fn is_recent(date_str: &str, seconds: i64) -> bool {
    is_recent_local(date_str, seconds)
}

fn unix_timestamp_approx(y: u32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
    let (year, month, day) = (y as i64, m as i64, d as i64);
    let a = (14 - month) / 12;
    let y2 = year + 4800 - a;
    let m2 = month + 12 * a - 3;
    let jdn = day + (153 * m2 + 2) / 5 + 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 - 32045;
    (jdn - 2440588) * 86400 + h as i64 * 3600 + mi as i64 * 60 + s as i64
}

pub fn random_hex_id() -> String {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let mut state = ns as u64 ^ 0xdeadbeef_cafebabe;
    let mut bytes = [0u8; 5];
    for b in &mut bytes {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state & 0xFF) as u8;
    }
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}