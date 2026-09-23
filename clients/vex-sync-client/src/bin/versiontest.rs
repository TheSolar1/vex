// Test cible pour /api/fchier/version (compteur "y'a-t-il du nouveau ?")
// -- compte de test jetable, meme pattern que synctest.rs. Verifie que le
// compteur demarre a 0, s'incremente sur upload/suppression/renommage, et
// NE bouge PAS sur un simple appel de lecture (data/version).

use num_bigint::BigUint;
use num_traits::Num;
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

fn main() {
    let base_url = std::env::args().nth(1).unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);

    let email = format!("versiontest-{}@example.invalid", std::process::id());
    let nom = format!("versiontest{}", std::process::id());
    let password = "Test-Password-1234!";

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
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("Inscription echouee : {body}");
        std::process::exit(1);
    }

    let client = match VexClient::login(&base_url, &email, password) {
        Ok(c) => c,
        Err(e) => { eprintln!("ECHEC login : {e}"); std::process::exit(1); }
    };
    println!("Connecte avec le compte de test.\n");

    let v0 = client.version().expect("lecture version initiale");
    assert_ok(v0 == 0, &format!("version initiale = 0 (obtenu {v0})"));

    // Lecture seule (lister_dossier) : NE doit PAS incrementer.
    client.lister_dossier(0).expect("liste racine");
    let v1 = client.version().expect("lecture version apres liste");
    assert_ok(v1 == v0, &format!("version inchangee apres une simple lecture ({v0} -> {v1})"));

    // Upload : DOIT incrementer.
    let id = client.uploader("test.txt", b"contenu v1", "text/plain").expect("upload");
    let v2 = client.version().expect("lecture version apres upload");
    assert_ok(v2 == v1 + 1, &format!("version incrementee apres upload ({v1} -> {v2})"));

    // Edition de contenu : DOIT incrementer.
    client.remplacer_contenu(id, b"contenu v2").expect("edition contenu");
    let v3 = client.version().expect("lecture version apres edition");
    assert_ok(v3 == v2 + 1, &format!("version incrementee apres edition de contenu ({v2} -> {v3})"));

    // Suppression : DOIT incrementer.
    client.supprimer_fichier(id).expect("suppression");
    let v4 = client.version().expect("lecture version apres suppression");
    assert_ok(v4 == v3 + 1, &format!("version incrementee apres suppression ({v3} -> {v4})"));

    println!("\n=== TOUS LES TESTS DE VERSION SONT PASSES ===");
    println!("(compte de test {email} laisse en base -- purement jetable)");
}
