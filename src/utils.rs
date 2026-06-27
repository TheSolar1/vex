// ══════════════════════════════════════════════════════════════════
// utils.rs — VEX utilitaires partagés
// Importé par access_control, admin, login, c
// ══════════════════════════════════════════════════════════════════

/// Supprime le port d'une adresse IP
/// "127.0.0.1:54321" → "127.0.0.1"
/// "[::1]:8080"       → "::1"
pub fn strip_port(addr: &str) -> String {
    if addr.starts_with('[') {
        if let Some(end) = addr.find(']') {
            return addr[1..end].to_string();
        }
    }
    if let Some(pos) = addr.rfind(':') {
        let before = &addr[..pos];
        if !before.contains(':') {
            return before.to_string();
        }
    }
    addr.to_string()
}

pub fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(qs) = url.split('?').nth(1) {
        for pair in qs.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                map.insert(url_decode(k), url_decode(v));
            }
        }
    }
    map
}

pub fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut r = String::new();
    let mut c = s.chars().peekable();
    while let Some(ch) = c.next() {
        if ch == '%' {
            let h1 = c.next().unwrap_or('0');
            let h2 = c.next().unwrap_or('0');
            if let Ok(b) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                r.push(b as char);
            }
        } else {
            r.push(ch);
        }
    }
    r
}
