// ══════════════════════════════════════════════════════════════════
// filecrypto.rs — Port EXACT de static/crypto.js (VEX.chiffrerFichier /
// VEX.dechiffrerFichier) cote client natif.
//
// Cle = HKDF-SHA256( SHA-512(mot_de_passe) || pqSalt, salt="VEX-PQ-salt",
//                     info="VEX-PQ-file-v1" ) -> AES-256-GCM
// pqSalt = 32 octets aleatoires par fichier, IV = 12 octets aleatoires.
// Blob = MAGIC(4="VEX2") || pqSalt(32) || IV(12) || ciphertext+tag(16)
// ══════════════════════════════════════════════════════════════════

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256, Sha512};

const FILE_MAGIC: [u8; 4] = [0x56, 0x45, 0x58, 0x32]; // "VEX2"
const PQ_SALT_BYTES: usize = 32;
const IV_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const HKDF_SALT_FILE: &[u8] = b"VEX-PQ-salt";
const HKDF_INFO_FILE: &[u8] = b"VEX-PQ-file-v1";

fn deriver_cle_fichier(password: &str, pq_salt: &[u8]) -> [u8; 32] {
    let sha = Sha512::digest(password.as_bytes());
    let mut master = Vec::with_capacity(sha.len() + pq_salt.len());
    master.extend_from_slice(&sha);
    master.extend_from_slice(pq_salt);

    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT_FILE), &master);
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO_FILE, &mut okm)
        .expect("longueur de cle HKDF invalide");
    okm
}

/// Chiffre un fichier en clair -> blob pret a uploader (memes octets que
/// ce que produirait VEX.chiffrerFichier() dans le navigateur).
pub fn chiffrer_fichier(password: &str, plaintext: &[u8]) -> Vec<u8> {
    let mut pq_salt = [0u8; PQ_SALT_BYTES];
    rand::thread_rng().fill_bytes(&mut pq_salt);
    let mut iv = [0u8; IV_BYTES];
    rand::thread_rng().fill_bytes(&mut iv);

    let key_bytes = deriver_cle_fichier(password, &pq_salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = Nonce::from_slice(&iv);
    let ciphertext = cipher
        .encrypt(nonce, Payload { msg: plaintext, aad: b"" })
        .expect("chiffrement AES-GCM impossible");

    let mut out = Vec::with_capacity(4 + PQ_SALT_BYTES + IV_BYTES + ciphertext.len());
    out.extend_from_slice(&FILE_MAGIC);
    out.extend_from_slice(&pq_salt);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    out
}

/// Dechiffre un blob recu du serveur -> contenu en clair.
pub fn dechiffrer_fichier(password: &str, blob: &[u8]) -> Result<Vec<u8>, String> {
    let min_len = 4 + PQ_SALT_BYTES + IV_BYTES + TAG_BYTES;
    if blob.len() < min_len {
        return Err("Fichier non chiffre ou trop court".into());
    }
    if blob[..4] != FILE_MAGIC {
        return Err("Fichier non chiffre VEX".into());
    }
    let pq_salt = &blob[4..4 + PQ_SALT_BYTES];
    let iv = &blob[4 + PQ_SALT_BYTES..4 + PQ_SALT_BYTES + IV_BYTES];
    let data = &blob[4 + PQ_SALT_BYTES + IV_BYTES..];

    let key_bytes = deriver_cle_fichier(password, pq_salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = Nonce::from_slice(iv);
    cipher
        .decrypt(nonce, Payload { msg: data, aad: b"" })
        .map_err(|_| "Dechiffrement echoue (mauvais mot de passe ou fichier corrompu)".to_string())
}

/// Hash PBKDF2-like... non -- juste un utilitaire SHA256 hex, reutilise
/// pour d'autres besoins simples (pas la crypto principale).
pub fn sha256_hex(data: &[u8]) -> String {
    let h = Sha256::digest(data);
    h.iter().map(|b| format!("{:02x}", b)).collect()
}
