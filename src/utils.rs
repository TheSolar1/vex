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
    // On decode vers des octets puis on reconstruit l'UTF-8 : un caractere
    // accentue est encode sur plusieurs %XX, les pousser un par un dans une
    // String les transformerait en mojibake (ex. "é" -> "Ã©").
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            // Lecture des deux chiffres hexa octet par octet : slicer la
            // String paniquerait si le % est suivi d'un caractere multi-octets.
            let hex = |b: u8| -> Option<u8> {
                match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    b'A'..=b'F' => Some(b - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
