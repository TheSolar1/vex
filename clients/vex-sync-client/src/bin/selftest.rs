// Test interne : inscrit un compte jetable via SRP-6a (salt+verifier
// calcules localement, mot de passe jamais envoye) puis se reconnecte
// avec vex-sync-client::api::VexClient -- valide tout le protocole
// (SRP login + M2) sans toucher a un compte existant.

use num_bigint::BigUint;
use num_traits::Num;
use vex_sync_client::api::VexClient;

const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

fn sha256(d: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(d).to_vec()
}

fn main() {
    let base_url = std::env::args().nth(1).unwrap_or_else(|| "https://vex.hopto.org".into());
    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);

    let email = format!("selftest-{}@example.invalid", std::process::id());
    let nom = format!("selftest{}", std::process::id());
    let password = "Test-Password-1234!";

    // ── genererSaltEtVerifier (identique login.html) ──────────────
    let salt: [u8; 16] = { let mut b = [0u8; 16]; getrandom::getrandom(&mut b).unwrap(); b };
    let inner = sha256(format!("{}:{}", email.to_lowercase(), password).as_bytes());
    let x_bytes = sha256(&[salt.as_slice(), &inner].concat());
    let x = BigUint::from_bytes_be(&x_bytes);
    let v = g.modpow(&x, &n);
    let salt_hex: String = salt.iter().map(|b| format!("{:02x}", b)).collect();
    let verifier_hex = v.to_str_radix(16);

    println!("Inscription du compte de test {email}...");
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
    println!("Reponse inscription : {body}");
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("Inscription echouee -- impossible de tester plus loin.");
        std::process::exit(1);
    }

    println!("\nConnexion via VexClient::login (SRP-6a complet)...");
    let client = match VexClient::login(&base_url, &email, password) {
        Ok(c) => { println!("SUCCES : login + verification M2 valides."); c }
        Err(e) => { eprintln!("ECHEC login : {e}"); std::process::exit(1); }
    };

    println!("\nTest chiffrement : upload d'un fichier test...");
    let contenu = b"Contenu de test vex-sync-client - roundtrip chiffrement.";
    let id = match client.uploader("selftest.txt", contenu, "text/plain") {
        Ok(id) => { println!("SUCCES : uploade, id={id}"); id }
        Err(e) => { eprintln!("ECHEC upload : {e}"); std::process::exit(1); }
    };

    println!("\nTest chiffrement : telechargement + dechiffrement...");
    match client.telecharger(id) {
        Ok(recu) if recu == contenu => println!("SUCCES : contenu dechiffre identique a l'original."),
        Ok(recu) => { eprintln!("ECHEC : contenu different ! recu={} octets, attendu={} octets", recu.len(), contenu.len()); std::process::exit(1); }
        Err(e) => { eprintln!("ECHEC telechargement : {e}"); std::process::exit(1); }
    }

    println!("\nListe des fichiers a la racine...");
    match client.lister_fichiers_racine() {
        Ok(fichiers) => println!("SUCCES : {} fichier(s) trouve(s).", fichiers.len()),
        Err(e) => eprintln!("ECHEC liste : {e}"),
    }

    println!("\nDiagnostic toggle VexIA (widget_on)...");
    let widget_on = |v: &serde_json::Value| v.get("data").and_then(|d| d.get("widget_on")).cloned();
    match client.get_json("/api/ext/vexia/status") {
        Ok(v) => println!("Status initial : widget_on={:?}", widget_on(&v)),
        Err(e) => eprintln!("ECHEC status initial : {e}"),
    }
    match client.post_json("/api/ext/vexia/prefs", serde_json::json!({"widget_on": false})) {
        Ok(v) => println!("Reponse prefs (desactivation) : {v}"),
        Err(e) => eprintln!("ECHEC post prefs : {e}"),
    }
    match client.get_json("/api/ext/vexia/status") {
        Ok(v) => println!("Status apres desactivation : widget_on={:?}", widget_on(&v)),
        Err(e) => eprintln!("ECHEC status apres desactivation : {e}"),
    }
    match client.post_json("/api/ext/vexia/prefs", serde_json::json!({"widget_on": true})) {
        Ok(v) => println!("Reponse prefs (reactivation) : {v}"),
        Err(e) => eprintln!("ECHEC post prefs (reactivation) : {e}"),
    }
    match client.get_json("/api/ext/vexia/status") {
        Ok(v) => println!("Status apres reactivation : widget_on={:?}", widget_on(&v)),
        Err(e) => eprintln!("ECHEC status apres reactivation : {e}"),
    }

    println!("\n=== TOUS LES TESTS SONT PASSES ===");
    println!("(compte de test {email} laisse en base -- purement jetable, jamais utilise ailleurs)");
}
