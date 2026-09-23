// Test cible pour /api/fchier/attendre (long-poll) -- compte de test
// jetable, meme pattern que synctest.rs/versiontest.rs. Verifie :
//   1. Un attendre() se reveille QUASI INSTANTANEMENT (pas au bout du
//      timeout serveur ~25s) quand une mutation a lieu pendant l'attente.
//   2. Le SERVEUR PRINCIPAL reste reactif pour d'autres requetes PENDANT
//      qu'un attendre() est en cours -- preuve que le long-poll tourne
//      bien sur un thread dedie, pas sur la boucle d'acceptation
//      principale (qui gelerait tout le serveur sinon).

use num_bigint::BigUint;
use num_traits::Num;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vex_sync_client::api::VexClient;

const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

fn sha256(d: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(d).to_vec()
}

fn assert_ok(cond: bool, msg: &str) {
    if !cond {
        eprintln!("ECHEC : {msg}");
        std::process::exit(1);
    }
    println!("OK : {msg}");
}

fn inscrire_et_login(base_url: &str, prefixe: &str) -> VexClient {
    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);
    let email = format!("{prefixe}-{}@example.invalid", std::process::id());
    let nom = format!("{prefixe}{}", std::process::id());
    let password = "Test-Password-1234!";

    let salt: [u8; 16] = { let mut b = [0u8; 16]; getrandom::getrandom(&mut b).unwrap(); b };
    let inner = sha256(format!("{}:{}", email.to_lowercase(), password).as_bytes());
    let x_bytes = sha256(&[salt.as_slice(), &inner].concat());
    let x = BigUint::from_bytes_be(&x_bytes);
    let v = g.modpow(&x, &n);
    let salt_hex: String = salt.iter().map(|b| format!("{:02x}", b)).collect();
    let verifier_hex = v.to_str_radix(16);

    let agent = ureq::AgentBuilder::new().build();
    let r = agent
        .post(&format!("{}/login/login", base_url.trim_end_matches('/')))
        .send_form(&[
            ("action", "signup"),
            ("nom", &nom),
            ("email", &email),
            ("srp_salt", &salt_hex),
            ("srp_verifier", &verifier_hex),
            ("scales", "1"),
        ]);
    let r = match r {
        Ok(resp) => resp,
        Err(ureq::Error::Status(_, resp)) => resp,
        Err(e) => { eprintln!("Erreur reseau inscription : {e}"); std::process::exit(1); }
    };
    let body: serde_json::Value = r.into_json().unwrap_or_default();
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("Inscription echouee : {body}");
        std::process::exit(1);
    }
    VexClient::login(base_url, &email, password).unwrap_or_else(|e| {
        eprintln!("ECHEC login : {e}");
        std::process::exit(1);
    })
}

fn main() {
    let base_url = std::env::args().nth(1).unwrap_or_else(|| "http://127.0.0.1:8080".into());

    println!("Inscription du compte de test attendre...");
    let client = Arc::new(inscrire_et_login(&base_url, "longpolltest"));
    println!("Connecte.\n");

    // ── Test 1 : reveil rapide sur mutation pendant l'attente ────────
    println!("── Test 1 : reveil rapide sur mutation ──");
    let v0 = client.version().expect("version initiale");

    let client_attente = Arc::clone(&client);
    let poignee = std::thread::spawn(move || {
        let debut = Instant::now();
        let v = client_attente.attendre(v0).expect("attendre");
        (v, debut.elapsed())
    });

    // Laisse le long-poll s'enregistrer cote serveur avant de muter.
    std::thread::sleep(Duration::from_millis(800));
    client.uploader("reveil.txt", b"contenu", "text/plain").expect("upload pendant l'attente");

    let (v_apres, duree) = poignee.join().expect("thread attendre");
    assert_ok(v_apres == v0 + 1, &format!("version mise a jour apres reveil ({v0} -> {v_apres})"));
    assert_ok(
        duree < Duration::from_secs(5),
        &format!("reveil rapide, PAS le timeout serveur ~25s (duree observee: {duree:?})"),
    );

    // ── Test 2 : le serveur principal reste reactif PENDANT une attente ──
    println!("\n── Test 2 : serveur principal non bloque pendant une attente en cours ──");
    let v1 = client.version().expect("version apres test 1");
    let client_attente2 = Arc::clone(&client);
    let _poignee2 = std::thread::spawn(move || {
        // Attente longue (rien ne va changer) -- doit tourner en tache de
        // fond SANS empecher les requetes suivantes d'aboutir.
        let _ = client_attente2.attendre(v1);
    });
    std::thread::sleep(Duration::from_millis(500)); // laisse l'attente demarrer

    let debut = Instant::now();
    let ok_pendant_attente = ureq::get(&format!("{}/login", base_url)).call().is_ok();
    let duree_requete = debut.elapsed();
    assert_ok(ok_pendant_attente, "requete /login reussie PENDANT qu'un attendre() est en cours ailleurs");
    assert_ok(
        duree_requete < Duration::from_secs(2),
        &format!("requete /login rapide, serveur non gele par l'attente en cours (duree: {duree_requete:?})"),
    );
    // Ne joint pas _poignee2 : elle se terminera au timeout serveur (~25s),
    // le process se termine avant, pas la peine d'attendre.

    println!("\n=== TOUS LES TESTS DE LONG-POLL SONT PASSES ===");
}
