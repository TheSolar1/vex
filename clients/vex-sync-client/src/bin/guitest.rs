// Test d'integration de la page de config locale (main.rs) : cree un
// compte jetable, poste sur /api/connecter comme le ferait la page web,
// et verifie que le dossier distant est cree et qu'un fichier depose
// AVANT la connexion est bien remonte par la premiere synchro automatique.

use num_bigint::BigUint;
use num_traits::Num;
use std::fs;
use vex_sync_client::api::VexClient;

const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

fn sha256(d: &[u8]) -> Vec<u8> { use sha2::{Digest, Sha256}; Sha256::digest(d).to_vec() }

fn main() {
    let base_url_serveur = std::env::args().nth(1).unwrap_or_else(|| "http://192.168.1.14:8080".into());
    let port_local: u16 = std::env::args().nth(2).and_then(|s| s.parse().ok()).expect("port local requis en 2e argument");
    let url_locale = format!("http://127.0.0.1:{port_local}");

    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);
    let email = format!("guitest-{}@example.invalid", std::process::id());
    let nom = format!("guitest{}", std::process::id());
    let password = "Test-Password-1234!";

    let salt: [u8; 16] = { let mut b = [0u8; 16]; getrandom::getrandom(&mut b).unwrap(); b };
    let inner = sha256(format!("{}:{}", email.to_lowercase(), password).as_bytes());
    let x_bytes = sha256(&[salt.as_slice(), &inner].concat());
    let x = BigUint::from_bytes_be(&x_bytes);
    let v = g.modpow(&x, &n);
    let salt_hex: String = salt.iter().map(|b| format!("{:02x}", b)).collect();
    let verifier_hex = v.to_str_radix(16);

    println!("Inscription de {email} sur {base_url_serveur}...");
    let agent = ureq::AgentBuilder::new().build();
    let r = agent.post(&format!("{}/login/login", base_url_serveur.trim_end_matches('/')))
        .send_form(&[("action","signup"),("nom",&nom),("email",&email),("srp_salt",&salt_hex),("srp_verifier",&verifier_hex),("scales","1")]);
    let r = match r { Ok(x) => x, Err(ureq::Error::Status(_, x)) => x, Err(e) => { eprintln!("erreur inscription: {e}"); std::process::exit(1); } };
    let body: serde_json::Value = r.into_json().unwrap_or_default();
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("inscription echouee: {body}"); std::process::exit(1);
    }

    let dossier_test = std::env::temp_dir().join(format!("vex-guitest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dossier_test);
    fs::create_dir_all(&dossier_test).unwrap();
    fs::write(dossier_test.join("avant-connexion.txt"), b"depose avant la connexion via la page web").unwrap();
    println!("Fichier local pret dans {}", dossier_test.display());

    println!("POST {}/api/connecter ...", url_locale);
    let r = agent.post(&format!("{url_locale}/api/connecter"))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "base_url": base_url_serveur, "email": email, "password": password,
            "dossiers": [{"nom": "TestGUI", "chemin": dossier_test.to_string_lossy()}],
        }));
    let r = match r { Ok(x) => x, Err(e) => { eprintln!("POST /api/connecter a echoue : {e}"); std::process::exit(1); } };
    let body: serde_json::Value = r.into_json().unwrap_or_default();
    println!("Reponse /api/connecter : {body}");
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("ECHEC : /api/connecter n'a pas reussi"); std::process::exit(1);
    }
    println!("OK : /api/connecter a reussi.");

    let statut: serde_json::Value = agent.get(&format!("{url_locale}/api/statut")).call().unwrap().into_json().unwrap();
    println!("Statut apres connexion : {statut}");
    let a_testgui = statut["dossiers"].as_array().map(|a| a.iter().any(|d| d["nom"] == "TestGUI")).unwrap_or(false);
    if !a_testgui { eprintln!("ECHEC : le dossier TestGUI n'apparait pas dans /api/statut"); std::process::exit(1); }
    println!("OK : TestGUI present dans /api/statut.");

    println!("Attente de la premiere synchronisation automatique (jusqu'a 10s)...");
    std::thread::sleep(std::time::Duration::from_secs(6));

    // Verification independante : reconnexion via VexClient (pas via l'appli en tache de fond).
    let client = VexClient::login(&base_url_serveur, &email, password).expect("relogin de verification");
    let (dossiers, _) = client.lister_dossier(0).expect("liste racine distante");
    let d = dossiers.iter().find(|d| d.nom == "TestGUI");
    if d.is_none() { eprintln!("ECHEC : dossier distant TestGUI introuvable"); std::process::exit(1); }
    println!("OK : dossier distant TestGUI cree.");
    let (_, fichiers) = client.lister_dossier(d.unwrap().id).unwrap();
    if !fichiers.iter().any(|f| f.nom == "avant-connexion.txt") {
        eprintln!("ECHEC : avant-connexion.txt non present a distance apres la premiere synchro automatique");
        std::process::exit(1);
    }
    println!("OK : avant-connexion.txt uploade automatiquement apres la connexion via la page web.");
    println!("\n=== TEST GUI (page de config + handoff + 1ere synchro auto) : SUCCES ===");
}
