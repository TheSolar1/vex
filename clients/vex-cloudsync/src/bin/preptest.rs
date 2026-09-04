// Prepare un compte jetable avec plusieurs fichiers + un sous-dossier,
// pour un test plus complet de vex-cloudsync que le cas simple d'avant.
use num_bigint::BigUint;
use num_traits::Num;
use vex_sync_client::api::VexClient;

const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

fn sha256(d: &[u8]) -> Vec<u8> { use sha2::{Digest, Sha256}; Sha256::digest(d).to_vec() }

fn main() {
    let base_url = std::env::args().nth(1).unwrap_or_else(|| "http://192.168.1.14:8080".into());
    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);
    let email = format!("cloudtest-{}@example.invalid", std::process::id());
    let nom = format!("cloudtest{}", std::process::id());
    let password = "Test-Password-1234!";

    let salt: [u8; 16] = { let mut b = [0u8; 16]; getrandom::getrandom(&mut b).unwrap(); b };
    let inner = sha256(format!("{}:{}", email.to_lowercase(), password).as_bytes());
    let x_bytes = sha256(&[salt.as_slice(), &inner].concat());
    let x = BigUint::from_bytes_be(&x_bytes);
    let v = g.modpow(&x, &n);
    let salt_hex: String = salt.iter().map(|b| format!("{:02x}", b)).collect();
    let verifier_hex = v.to_str_radix(16);

    let agent = ureq::AgentBuilder::new().build();
    let r = agent.post(&format!("{}/login/login", base_url.trim_end_matches('/')))
        .send_form(&[("action","signup"),("nom",&nom),("email",&email),("srp_salt",&salt_hex),("srp_verifier",&verifier_hex),("scales","1")]);
    let r = match r { Ok(x) => x, Err(ureq::Error::Status(_, x)) => x, Err(e) => { eprintln!("erreur inscription: {e}"); std::process::exit(1); } };
    let body: serde_json::Value = r.into_json().unwrap_or_default();
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("inscription echouee: {body}"); std::process::exit(1);
    }

    let client = VexClient::login(&base_url, &email, password).expect("login");

    // Plusieurs fichiers a la racine.
    client.uploader("racine1.txt", b"contenu racine 1", "text/plain").expect("upload racine1");
    client.uploader("racine2.txt", b"contenu racine 2, plus long pour varier la taille.", "text/plain").expect("upload racine2");

    // Un sous-dossier avec un fichier dedans.
    let id_dossier = client.creer_dossier("SousDossier", 0).expect("creer dossier");
    client.uploader_dans(id_dossier, "profond.txt", b"contenu dans le sous-dossier", "text/plain").expect("upload profond");

    println!("EMAIL={email}");
    println!("PASSWORD={password}");
    println!("Prepare : 2 fichiers a la racine + 1 sous-dossier avec 1 fichier.");
}
