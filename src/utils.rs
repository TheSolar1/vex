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
    for prefixe in ["/autologin/", "/login/autologin/", "/autoriser-appareil/", "/partage/", "/api/partage/"] {
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

// ══════════════════════════════════════════════════════════════════
// Compression gzip des réponses texte
// ══════════════════════════════════════════════════════════════════

/// Taille minimale (en dessous, gzip n'apporte rien) et maximale (au-delà,
/// le coût CPU sur un Raspberry Pi dépasse le gain) compressées.
const GZIP_MIN: usize = 1024;
const GZIP_MAX: usize = 8 * 1024 * 1024;

fn type_compressible(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.starts_with("text/")
        || ct.starts_with("application/json")
        || ct.starts_with("application/javascript")
        || ct.starts_with("image/svg+xml")
}

/// Le client accepte-t-il gzip (en-tête Accept-Encoding) ?
pub fn accepte_gzip(request: &tiny_http::Request) -> bool {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Accept-Encoding"))
        .map(|h| {
            h.value.as_str().split(',').any(|e| {
                let mut parts = e.trim().split(';');
                let nom = parts.next().unwrap_or("").trim();
                let q0 = parts.any(|p| p.trim().replace(' ', "") == "q=0");
                (nom.eq_ignore_ascii_case("gzip") || nom == "*") && !q0
            })
        })
        .unwrap_or(false)
}

/// Compresse en gzip une réponse texte (HTML, JSON, CSS, JS, SVG) si le
/// client l'accepte. Toute autre réponse est renvoyée intacte.
pub fn compresser_reponse(
    resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>,
    gzip_ok: bool,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if !gzip_ok || resp.status_code().0 != 200 {
        return resp;
    }
    let len = resp.data_length().unwrap_or(0);
    if !(GZIP_MIN..=GZIP_MAX).contains(&len) {
        return resp;
    }
    let deja_encode = resp.headers().iter().any(|h| h.field.equiv("Content-Encoding"));
    let compressible = resp
        .headers()
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .map(|h| type_compressible(h.value.as_str()))
        .unwrap_or(false);
    if deja_encode || !compressible {
        return resp;
    }
    let status = resp.status_code();
    let mut headers: Vec<tiny_http::Header> = resp
        .headers()
        .iter()
        .filter(|h| !h.field.equiv("Content-Length"))
        .cloned()
        .collect();
    let donnees = resp.into_reader().into_inner();
    let compresse = {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::with_capacity(donnees.len() / 3), flate2::Compression::new(5));
        if enc.write_all(&donnees).is_err() {
            None
        } else {
            enc.finish().ok()
        }
    };
    match compresse {
        Some(gz) if gz.len() < donnees.len() => {
            headers.push(tiny_http::Header::from_bytes("Content-Encoding", "gzip").unwrap());
            headers.push(tiny_http::Header::from_bytes("Vary", "Accept-Encoding").unwrap());
            let n = gz.len();
            tiny_http::Response::new(status, headers, std::io::Cursor::new(gz), Some(n), None)
                // Pas de chunked (passe mal a travers Apache, voir serve_static).
                .with_chunked_threshold(usize::MAX)
        }
        _ => {
            let n = donnees.len();
            tiny_http::Response::new(status, headers, std::io::Cursor::new(donnees), Some(n), None)
                .with_chunked_threshold(usize::MAX)
        }
    }
}

/// Envoie une reponse de module : compression gzip si le client
/// l'accepte (HTML/JSON/CSS/JS/SVG) + en-tetes de securite sur le HTML.
/// Renvoie le code de statut (journal d'acces).
pub fn envoyer(request: tiny_http::Request, resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>) -> u16 {
    envoyer_opts(request, resp, true)
}

/// `anti_iframe` = false pour les pages publiques Sitec (/page/...), que
/// leurs auteurs peuvent vouloir integrer sur un autre site.
pub fn envoyer_opts(request: tiny_http::Request, resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>, anti_iframe: bool) -> u16 {
    let gzip_ok = accepte_gzip(&request);
    let mut resp = compresser_reponse(resp, gzip_ok);
    let est_html = resp
        .headers()
        .iter()
        .any(|h| h.field.equiv("Content-Type") && h.value.as_str().starts_with("text/html"));
    if est_html {
        for (k, v) in [
            // Empeche l'affichage de VEX dans une iframe d'un autre site
            // (clickjacking) ; les iframes internes (meme origine) restent OK.
            ("X-Frame-Options", "SAMEORIGIN"),
            ("X-Content-Type-Options", "nosniff"),
            ("Referrer-Policy", "strict-origin-when-cross-origin"),
        ] {
            if k == "X-Frame-Options" && !anti_iframe {
                continue;
            }
            if !resp.headers().iter().any(|h| h.field.equiv(k)) {
                resp.add_header(tiny_http::Header::from_bytes(k, v).unwrap());
            }
        }
    }
    let statut = resp.status_code().0;
    let _ = request.respond(resp);
    statut
}

#[cfg(test)]
mod tests_gzip {
    use super::*;

    #[test]
    fn compresse_html_et_decompresse() {
        use std::io::Read;
        let html = "<p>bonjour</p>".repeat(500);
        let resp = tiny_http::Response::from_string(html.clone()).with_header(
            tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
        );
        let r = compresser_reponse(resp, true);
        assert!(r.headers().iter().any(|h| h.field.equiv("Content-Encoding")));
        let gz = r.into_reader().into_inner();
        assert!(gz.len() < html.len() / 5);
        let mut out = String::new();
        flate2::read::GzDecoder::new(&gz[..]).read_to_string(&mut out).unwrap();
        assert_eq!(out, html);
    }

    #[test]
    fn ne_compresse_pas_binaire_ni_petit() {
        let bin = tiny_http::Response::from_data(vec![0u8; 5000]).with_header(
            tiny_http::Header::from_bytes("Content-Type", "image/png").unwrap(),
        );
        assert!(!compresser_reponse(bin, true).headers().iter().any(|h| h.field.equiv("Content-Encoding")));
        let petit = tiny_http::Response::from_string("ok").with_header(
            tiny_http::Header::from_bytes("Content-Type", "text/plain").unwrap(),
        );
        assert!(!compresser_reponse(petit, true).headers().iter().any(|h| h.field.equiv("Content-Encoding")));
    }
}
