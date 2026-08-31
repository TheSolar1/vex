// ══════════════════════════════════════════════════════════════════
// srp.rs — VEX SRP-6a (Secure Remote Password), RFC 5054 adapté SHA-256
//
// PRINCIPE :
//   Le serveur ne stocke JAMAIS le mot de passe, ni un hash réutilisable
//   pour se connecter (contrairement à PBKDF2 côté serveur classique).
//   Il stocke un "verifier" (v) et un "salt" (s), dérivés mathématiquement
//   du mot de passe côté client UNE SEULE FOIS à l'inscription.
//
//   Le verifier ne permet PAS de se connecter directement : il faut
//   prouver, via un échange Diffie-Hellman augmenté, qu'on connaît le
//   mot de passe qui l'a généré — sans jamais l'envoyer ni envoyer une
//   valeur "replay-able" (pas de pass-the-hash possible).
//
//   Même avec un accès total au code source ET à la base de données,
//   il est impossible de se connecter à la place de l'utilisateur ou
//   de retrouver son mot de passe (hors brute-force sur le mdp lui-même,
//   qui dépend de sa force, pas du protocole).
//
// GROUPE : RFC 5054, 2048 bits, g = 2.
// HASH   : SHA-256 (RFC 5054 original utilise SHA-1, obsolète — on
//          utilise SHA-256 partout, cohérent avec le reste de VEX).
//
// ⚠️  AVERTISSEMENT : implémentation "maison" suivant RFC 5054 au plus
//     près. SRP est un protocole subtil (padding, ordre de concaténation)
//     — à auditer/tester avant de considérer la sécurité comme acquise
//     à 100%. Ce n'est pas une lib auditée tierce (il n'en existe pas de
//     mûre en pur Rust std sans dépendance C).
// ══════════════════════════════════════════════════════════════════

use num_bigint::BigUint;
use num_traits::{Num, Zero};
use sha2::{Digest, Sha256};

/// N (2048 bits, RFC 5054 groupe standard) — hex, 512 caractères = 256 octets.
const N_HEX: &str = "\
AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

const G_DEC: u32 = 2;
/// Longueur de N en octets (2048 bits / 8) — utilisée pour tout le padding.
pub const N_LEN_BYTES: usize = 256;

pub struct SrpGroup {
    pub n: BigUint,
    pub g: BigUint,
    /// k = H(PAD(N) || PAD(g)) — constante RFC 5054 (multiplicateur anti-attaque).
    pub k: BigUint,
}

pub fn group() -> SrpGroup {
    let n = BigUint::from_str_radix(N_HEX, 16).expect("N_HEX invalide");
    let g = BigUint::from(G_DEC);
    let k_bytes = sha256_concat(&[&pad(&n, N_LEN_BYTES), &pad(&g, N_LEN_BYTES)]);
    let k = BigUint::from_bytes_be(&k_bytes);
    SrpGroup { n, g, k }
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires bas niveau
// ══════════════════════════════════════════════════════════════════

/// Pad un BigUint en big-endian sur exactement `len` octets (zéros à gauche).
/// CRITIQUE : doit produire EXACTEMENT le même résultat que le PAD() côté JS,
/// sinon u/M1/M2 ne correspondront jamais entre client et serveur.
pub fn pad(n: &BigUint, len: usize) -> Vec<u8> {
    let bytes = n.to_bytes_be();
    if bytes.len() >= len {
        // Ne devrait jamais tronquer en pratique (valeurs toujours < N)
        return bytes[bytes.len() - len..].to_vec();
    }
    let mut out = vec![0u8; len - bytes.len()];
    out.extend_from_slice(&bytes);
    out
}

pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().to_vec()
}

pub fn sha256_concat(parts: &[&[u8]]) -> Vec<u8> {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().to_vec()
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

pub fn bigint_from_hex(hex: &str) -> Option<BigUint> {
    BigUint::from_str_radix(hex, 16).ok()
}

/// Génère `len` octets aléatoires cryptographiquement sûrs (getrandom, déjà
/// utilisé ailleurs dans VEX pour les tokens autologin/session).
pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    // getrandom échoue seulement si le générateur OS est indisponible —
    // cas si rare qu'on panique volontairement plutôt que de retomber sur
    // une source non sûre (pas de faux-semblant de sécurité).
    getrandom::getrandom(&mut buf).expect("getrandom indisponible — impossible de générer un secret SRP sûr");
    buf
}

/// Exposant privé serveur `b` — 256 bits aléatoires, jamais nul.
pub fn generate_b() -> BigUint {
    loop {
        let b = BigUint::from_bytes_be(&random_bytes(32));
        if !b.is_zero() {
            return b;
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Étape 1 (serveur) : calcule B à partir du verifier stocké
//   B = (k*v + g^b) mod N
// ══════════════════════════════════════════════════════════════════
pub fn compute_b_public(grp: &SrpGroup, v: &BigUint, b: &BigUint) -> BigUint {
    let g_pow_b = grp.g.modpow(b, &grp.n);
    let term = (&grp.k * v) % &grp.n;
    (term + g_pow_b) % &grp.n
}

// ══════════════════════════════════════════════════════════════════
// Étape 2 (serveur) : vérifie la preuve client (M1) et calcule M2
// ══════════════════════════════════════════════════════════════════

/// u = H(PAD(A) || PAD(B))
pub fn compute_u(a_pub: &BigUint, b_pub: &BigUint) -> BigUint {
    let h = sha256_concat(&[&pad(a_pub, N_LEN_BYTES), &pad(b_pub, N_LEN_BYTES)]);
    BigUint::from_bytes_be(&h)
}

/// S côté serveur = (A * v^u)^b mod N
pub fn compute_s_server(
    grp: &SrpGroup,
    a_pub: &BigUint,
    v: &BigUint,
    u: &BigUint,
    b: &BigUint,
) -> BigUint {
    let v_pow_u = v.modpow(u, &grp.n);
    let base = (a_pub * v_pow_u) % &grp.n;
    base.modpow(b, &grp.n)
}

/// K = H(PAD(S))
pub fn compute_k(s: &BigUint) -> Vec<u8> {
    sha256(&pad(s, N_LEN_BYTES))
}

/// M1 attendu = H( H(N) XOR H(g) , H(I) , s(salt) , PAD(A) , PAD(B) , K )
/// Doit être identique au calcul client pour que la preuve soit validée.
pub fn compute_m1(
    grp: &SrpGroup,
    identity: &str,
    salt_bytes: &[u8],
    a_pub: &BigUint,
    b_pub: &BigUint,
    k_bytes: &[u8],
) -> Vec<u8> {
    let h_n = sha256(&pad(&grp.n, N_LEN_BYTES));
    let h_g = sha256(&pad(&grp.g, N_LEN_BYTES));
    let xor_ng: Vec<u8> = h_n.iter().zip(h_g.iter()).map(|(a, b)| a ^ b).collect();
    let h_i = sha256(identity.as_bytes());

    sha256_concat(&[
        &xor_ng,
        &h_i,
        salt_bytes,
        &pad(a_pub, N_LEN_BYTES),
        &pad(b_pub, N_LEN_BYTES),
        k_bytes,
    ])
}

/// M2 = H(PAD(A), M1, K) — preuve retournée par le serveur au client,
/// confirme que le serveur connaît bien S (donc possède v correspondant à P).
pub fn compute_m2(a_pub: &BigUint, m1: &[u8], k_bytes: &[u8]) -> Vec<u8> {
    sha256_concat(&[&pad(a_pub, N_LEN_BYTES), m1, k_bytes])
}

/// Comparaison en temps constant (évite les timing attacks sur M1/M2).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Valide qu'un A envoyé par le client n'est pas 0 mod N (sinon un attaquant
/// pourrait forcer S=0 et donc K/M1/M2 prévisibles — vérification obligatoire
/// dans toute implémentation SRP sérieuse).
pub fn is_safe_public_value(v: &BigUint, n: &BigUint) -> bool {
    !v.is_zero() && v % n != BigUint::zero()
}