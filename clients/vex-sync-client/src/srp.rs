// ══════════════════════════════════════════════════════════════════
// srp.rs — Client SRP-6a, port EXACT de src/srp.rs (serveur VEX) +
// src/login/login.html (JS). Meme N/g/k, meme padding, meme ordre de
// concatenation des hash -- toute divergence casse l'authentification.
// ══════════════════════════════════════════════════════════════════

use num_bigint::BigUint;
use num_traits::{Num, Zero};
use sha2::{Digest, Sha256};

const N_HEX: &str = "\
AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";
const G_DEC: u32 = 2;
pub const N_LEN_BYTES: usize = 256;

pub struct SrpGroup {
    pub n: BigUint,
    pub g: BigUint,
    pub k: BigUint,
}

pub fn group() -> SrpGroup {
    let n = BigUint::from_str_radix(N_HEX, 16).expect("N_HEX invalide");
    let g = BigUint::from(G_DEC);
    let k_bytes = sha256_concat(&[&pad(&n, N_LEN_BYTES), &pad(&g, N_LEN_BYTES)]);
    let k = BigUint::from_bytes_be(&k_bytes);
    SrpGroup { n, g, k }
}

pub fn pad(n: &BigUint, len: usize) -> Vec<u8> {
    let bytes = n.to_bytes_be();
    if bytes.len() >= len {
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

/// Hex "minimal" d'un BigUint, SANS padding a N_LEN_BYTES -- reproduit
/// exactement bigIntToHex() cote JS (juste un `0` prefixe si longueur impaire).
/// C'est cette forme (pas la forme paddee) qui est envoyee sur le fil pour A.
fn bigint_to_hex_minimal(n: &BigUint) -> String {
    let h = n.to_str_radix(16);
    if h.len() % 2 != 0 {
        format!("0{}", h)
    } else {
        h
    }
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

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).expect("getrandom indisponible");
    buf
}

/// x = H(salt || H(email_lowercase + ":" + password)) -- computeX() en JS.
fn compute_x(salt: &[u8], email: &str, password: &str) -> BigUint {
    let inner = sha256(format!("{}:{}", email.to_lowercase(), password).as_bytes());
    let h = sha256_concat(&[salt, &inner]);
    BigUint::from_bytes_be(&h)
}

/// a = 256 bits aleatoires (jamais nul), A = g^a mod N.
fn generate_a(grp: &SrpGroup) -> (BigUint, BigUint) {
    loop {
        let a = BigUint::from_bytes_be(&random_bytes(32));
        if !a.is_zero() {
            let a_pub = grp.g.modpow(&a, &grp.n);
            return (a, a_pub);
        }
    }
}

/// u = H(PAD(A) || PAD(B))
fn compute_u(a_pub: &BigUint, b_pub: &BigUint) -> BigUint {
    let h = sha256_concat(&[&pad(a_pub, N_LEN_BYTES), &pad(b_pub, N_LEN_BYTES)]);
    BigUint::from_bytes_be(&h)
}

/// S (cote client) = (B - k*g^x)^(a + u*x) mod N
fn compute_s_client(
    grp: &SrpGroup,
    b_pub: &BigUint,
    x: &BigUint,
    a: &BigUint,
    u: &BigUint,
) -> BigUint {
    let gx = grp.g.modpow(x, &grp.n);
    let kgx = (&grp.k * &gx) % &grp.n;
    // BigUint ne supporte pas les negatifs -- on ajoute N avant de soustraire
    // pour rester dans l'anneau positif (equivalent au "if base<0 base+=N" JS).
    let base = (&grp.n + b_pub - kgx) % &grp.n;
    let exponent = a + u * x;
    base.modpow(&exponent, &grp.n)
}

pub fn compute_k(s: &BigUint) -> Vec<u8> {
    sha256(&pad(s, N_LEN_BYTES))
}

/// M1 = H( H(N) XOR H(g), H(identity_tel_quel), salt, PAD(A), PAD(B), K )
/// ATTENTION : `identity` n'est PAS mis en minuscules ici (cf. login.html
/// ligne 296 : `sha256(strToBytes(email))`, contrairement a computeX qui
/// lowercase l'email -- divergence volontaire du code d'origine a respecter).
fn compute_m1(
    grp: &SrpGroup,
    identity: &str,
    salt: &[u8],
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
        salt,
        &pad(a_pub, N_LEN_BYTES),
        &pad(b_pub, N_LEN_BYTES),
        k_bytes,
    ])
}

fn compute_m2_expected(a_pub: &BigUint, m1: &[u8], k_bytes: &[u8]) -> Vec<u8> {
    sha256_concat(&[&pad(a_pub, N_LEN_BYTES), m1, k_bytes])
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Resultat de l'etape 2 : preuve M1 a envoyer, plus K pour verifier M2.
pub struct Etape2 {
    pub a_hex: String,
    pub m1_hex: String,
    pub k_bytes: Vec<u8>,
}

/// Calcule A (a envoyer avec /srp_step1 implicitement -- en pratique ici on
/// fait tout en une fois : on a deja salt/B via step1) et M1 (a envoyer a
/// /srp_step2), a partir des reponses serveur.
pub fn calculer_preuve(
    email: &str,
    password: &str,
    salt_hex: &str,
    b_hex: &str,
) -> Result<Etape2, String> {
    let grp = group();
    let salt = hex_decode(salt_hex).ok_or("salt invalide")?;
    let b_pub = BigUint::from_bytes_be(&hex_decode(b_hex).ok_or("B invalide")?);
    if &b_pub % &grp.n == BigUint::zero() {
        return Err("Valeur B du serveur invalide.".into());
    }

    let (a, a_pub) = generate_a(&grp);
    let u = compute_u(&a_pub, &b_pub);
    if u.is_zero() {
        return Err("Valeur u invalide.".into());
    }

    let x = compute_x(&salt, email, password);
    let s = compute_s_client(&grp, &b_pub, &x, &a, &u);
    let k_bytes = compute_k(&s);
    let m1 = compute_m1(&grp, email, &salt, &a_pub, &b_pub, &k_bytes);

    Ok(Etape2 {
        a_hex: bigint_to_hex_minimal(&a_pub),
        m1_hex: hex_encode(&m1),
        k_bytes,
    })
}

/// Verifie M2 recu du serveur (authenticite du serveur).
pub fn verifier_m2(a_hex: &str, m1_hex: &str, k_bytes: &[u8], m2_recu_hex: &str) -> bool {
    let Some(a_bytes) = hex_decode(a_hex) else { return false };
    let a_pub = BigUint::from_bytes_be(&a_bytes);
    let Some(m1) = hex_decode(m1_hex) else { return false };
    let Some(m2_recu) = hex_decode(m2_recu_hex) else { return false };
    let m2_attendu = compute_m2_expected(&a_pub, &m1, k_bytes);
    constant_time_eq(&m2_attendu, &m2_recu)
}
