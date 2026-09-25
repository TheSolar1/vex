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

// ══════════════════════════════════════════════════════════════════
// IP réelle du client + ligne du journal d'accès
// ══════════════════════════════════════════════════════════════════

/// Adresse de la connexion TCP est-elle un proxy de confiance possible
/// (boucle locale ou réseau privé : Apache/nginx sur la même machine, box,
/// tunnel) ? Seules ces adresses peuvent imposer une IP via un en-tête --
/// un client venant d'Internet ne peut pas se faire passer pour un autre.
fn proxy_de_confiance(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 (ULA)
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10
                || v6.to_ipv4_mapped().map(|v4| v4.is_loopback() || v4.is_private()).unwrap_or(false)
        }
    }
}

/// IP réelle du client, **pour les logs uniquement**.
///
/// Derrière un reverse proxy (Apache/nginx) ou un tunnel (Cloudflare…),
/// `remote_addr()` donne l'IP du proxy, pas celle du visiteur. Si la
/// connexion vient d'un proxy de confiance, on lit dans l'ordre
/// `CF-Connecting-IP`, `X-Real-IP`, puis la première IP de
/// `X-Forwarded-For`. Sinon, l'IP TCP est gardée (en-têtes ignorés : un
/// client direct pourrait les falsifier).
///
/// NE PAS utiliser pour les sessions : `verifier_session` compare l'IP
/// brute de `remote_addr()` (voir la note dans fchier::remote_ip).
///
/// Renvoie `(ip_client, Some(ip_proxy))` si l'IP vient d'un en-tête.
pub fn client_ip(request: &tiny_http::Request) -> (String, Option<String>) {
    let brute = request
        .remote_addr()
        .map(|a| a.ip().to_canonical())
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "inconnue".into());
    let ip_tcp: Option<std::net::IpAddr> = brute.parse().ok();
    if !ip_tcp.as_ref().map(proxy_de_confiance).unwrap_or(false) {
        return (brute, None);
    }
    for nom in ["CF-Connecting-IP", "X-Real-IP", "X-Forwarded-For"] {
        let valeur = request
            .headers()
            .iter()
            .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(nom))
            .map(|h| h.value.as_str().to_string());
        if let Some(v) = valeur {
            let premiere = v.split(',').next().unwrap_or("").trim();
            let premiere = strip_port(premiere);
            if let Ok(ip) = premiere.parse::<std::net::IpAddr>() {
                return (ip.to_canonical().to_string(), Some(brute));
            }
        }
    }
    (brute, None)
}

/// Une entrée du journal d'accès (log/acces_<date>.log).
pub struct LigneAcces {
    pub ip: String,
    pub via: Option<String>,
    pub methode: String,
    pub chemin: String,
    pub statut: Option<u16>,
    pub duree_ms: u128,
    pub user_agent: String,
    pub referer: String,
}

impl LigneAcces {
    pub fn formater(&self) -> String {
        let nettoyer = |s: &str, max: usize| -> String {
            let s: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
            s.replace('"', "'")
        };
        format!(
            "ip={}{} {} {} statut={} {}ms ua=\"{}\" ref=\"{}\"",
            self.ip,
            self.via.as_ref().map(|v| format!(" (via {})", v)).unwrap_or_default(),
            self.methode,
            masquer_secrets_chemin(&nettoyer(&self.chemin, 300)),
            self.statut.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
            self.duree_ms,
            nettoyer(&self.user_agent, 200),
            nettoyer(&self.referer, 200),
        )
    }
}

/// Retire les jetons portés dans le chemin (liens d'autologin, partages)
/// avant écriture dans un log lisible par les admins.
pub fn masquer_secrets_chemin(chemin: &str) -> String {
    for prefixe in ["/autologin/", "/login/autologin/", "/autoriser-appareil/"] {
        if chemin.len() > prefixe.len() && chemin.starts_with(prefixe) {
            return format!("{}***", prefixe);
        }
    }
    chemin.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_port_ipv4_ipv6() {
        assert_eq!(strip_port("127.0.0.1:8080"), "127.0.0.1");
        assert_eq!(strip_port("[::1]:8080"), "::1");
        assert_eq!(strip_port("::1"), "::1");
    }

    #[test]
    fn proxy_confiance() {
        assert!(proxy_de_confiance(&"127.0.0.1".parse().unwrap()));
        assert!(proxy_de_confiance(&"192.168.1.10".parse().unwrap()));
        assert!(proxy_de_confiance(&"fd00::1".parse().unwrap()));
        assert!(!proxy_de_confiance(&"109.222.185.182".parse().unwrap()));
        assert!(!proxy_de_confiance(&"2a01:e0a::1".parse().unwrap()));
    }

    #[test]
    fn masque_autologin() {
        assert_eq!(masquer_secrets_chemin("/autologin/abcdef"), "/autologin/***");
        assert_eq!(masquer_secrets_chemin("/autologin/"), "/autologin/");
        assert_eq!(masquer_secrets_chemin("/admin"), "/admin");
    }

    #[test]
    fn ligne_acces_sans_injection() {
        let l = LigneAcces {
            ip: "1.2.3.4".into(), via: Some("127.0.0.1".into()),
            methode: "GET".into(), chemin: "/x\n[FAUX] log".into(),
            statut: Some(404), duree_ms: 3,
            user_agent: "UA\"x".into(), referer: String::new(),
        };
        let s = l.formater();
        assert!(!s.contains('\n'));
        assert!(s.contains("ip=1.2.3.4 (via 127.0.0.1)"));
        assert!(s.contains("statut=404"));
        assert!(s.contains("ua=\"UA'x\""));
    }
}
