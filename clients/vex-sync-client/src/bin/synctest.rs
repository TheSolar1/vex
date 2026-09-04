// Test d'integration du moteur de sync (sync.rs) : sous-dossiers et
// conflits, contre le serveur en production, avec un compte jetable
// (meme pattern que selftest.rs).

use num_bigint::BigUint;
use num_traits::Num;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use vex_sync_client::api::VexClient;
use vex_sync_client::sync::{reinitialiser_etat, synchroniser};

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
    let base_url = std::env::args().nth(1).unwrap_or_else(|| "https://vex.hopto.org".into());
    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let g = BigUint::from(2u32);

    let email = format!("synctest-{}@example.invalid", std::process::id());
    let nom = format!("synctest{}", std::process::id());
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

    // Dossier de travail local jetable, isole des vrais fichiers de l'utilisateur.
    let racine = std::env::temp_dir().join(format!("vex-synctest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&racine);
    fs::create_dir_all(&racine).unwrap();
    reinitialiser_etat(); // etat global au process -- on repart de zero pour ce test

    let log = |msg: &str| println!("  {msg}");

    // ── 1. Sous-dossiers : creer local puis synchroniser ────────────
    println!("── Test 1 : sous-dossiers ──");
    fs::write(racine.join("racine.txt"), b"contenu racine v1").unwrap();
    fs::create_dir_all(racine.join("sous")).unwrap();
    fs::write(racine.join("sous").join("profond.txt"), b"contenu profond v1").unwrap();
    synchroniser(&client, &racine, &log);

    let (dossiers, fichiers) = client.lister_dossier(0).expect("liste racine distante");
    assert_ok(fichiers.iter().any(|f| f.nom == "racine.txt"), "racine.txt uploade a la racine distante");
    let sous_id = dossiers.iter().find(|d| d.nom == "sous").map(|d| d.id);
    assert_ok(sous_id.is_some(), "sous-dossier 'sous' cree a distance");
    let (_, fichiers_sous) = client.lister_dossier(sous_id.unwrap()).expect("liste sous-dossier distant");
    assert_ok(fichiers_sous.iter().any(|f| f.nom == "profond.txt"), "profond.txt uploade dans le sous-dossier distant");

    // ── 2. Un "second PC" vide doit tout retelecharger, sous-dossiers inclus ──
    println!("\n── Test 2 : telechargement recursif sur un dossier local vide ──");
    let racine2 = std::env::temp_dir().join(format!("vex-synctest-{}-pc2", std::process::id()));
    let _ = fs::remove_dir_all(&racine2);
    fs::create_dir_all(&racine2).unwrap();
    reinitialiser_etat(); // simule un autre PC : aucun etat local connu
    synchroniser(&client, &racine2, &log);
    assert_ok(racine2.join("racine.txt").exists(), "racine.txt retelecharge sur le 'PC 2'");
    assert_ok(
        fs::read(racine2.join("racine.txt")).unwrap_or_default() == b"contenu racine v1",
        "contenu de racine.txt identique apres aller-retour",
    );
    assert_ok(racine2.join("sous").join("profond.txt").exists(), "sous/profond.txt recree recursivement sur le 'PC 2'");

    // ── 3. Conflit : modifier le fichier local ET son contenu distant, puis resynchroniser ──
    println!("\n── Test 3 : conflit (modifie des deux cotes) ──");
    reinitialiser_etat();
    let racine3 = std::env::temp_dir().join(format!("vex-synctest-{}-pc3", std::process::id()));
    let _ = fs::remove_dir_all(&racine3);
    fs::create_dir_all(&racine3).unwrap();
    fs::write(racine3.join("conflit.txt"), b"version initiale").unwrap();
    synchroniser(&client, &racine3, &log); // baseline connue des deux cotes

    let (_, fichiers_racine3) = client.lister_dossier(0).unwrap();
    let id_conflit = fichiers_racine3.iter().find(|f| f.nom == "conflit.txt").expect("fichier conflit.txt distant").id;

    // Cote "serveur" change (simule une autre machine) : contenu different.
    client.remplacer_contenu(id_conflit, b"modifie par une AUTRE machine").expect("remplacer contenu distant");
    // mtime doit changer pour que le fichier local soit vu comme modifie
    // (meme seconde que l'ecriture precedente possible sur un FS rapide).
    std::thread::sleep(Duration::from_secs(1));
    fs::write(racine3.join("conflit.txt"), b"modifie localement").unwrap();

    synchroniser(&client, &racine3, &log);

    assert_ok(
        fs::read(racine3.join("conflit.txt")).unwrap_or_default() == b"modifie localement",
        "fichier local NON ecrase malgre le conflit",
    );
    let a_une_copie_conflit = fs::read_dir(&racine3)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_str().unwrap_or("").contains("conflit-serveur"));
    assert_ok(a_une_copie_conflit, "copie de la version serveur creee a cote");

    // ── 4. Suppression locale -> propagee a distance ────────────────
    println!("\n── Test 4 : suppression locale propagee au serveur ──");
    reinitialiser_etat();
    let racine4 = std::env::temp_dir().join(format!("vex-synctest-{}-pc4", std::process::id()));
    let _ = fs::remove_dir_all(&racine4);
    fs::create_dir_all(&racine4).unwrap();
    fs::write(racine4.join("asupprimer.txt"), b"a supprimer").unwrap();
    synchroniser(&client, &racine4, &log); // baseline connue des deux cotes
    let (_, avant) = client.lister_dossier(0).unwrap();
    assert_ok(avant.iter().any(|f| f.nom == "asupprimer.txt"), "asupprimer.txt bien present a distance avant suppression");

    fs::remove_file(racine4.join("asupprimer.txt")).unwrap();
    synchroniser(&client, &racine4, &log); // doit propager la suppression au serveur

    let (_, apres) = client.lister_dossier(0).unwrap();
    assert_ok(!apres.iter().any(|f| f.nom == "asupprimer.txt"), "asupprimer.txt supprime a distance apres suppression locale");

    // Un 3e passage ne doit ni planter ni faire reapparaitre le fichier.
    synchroniser(&client, &racine4, &log);
    assert_ok(!racine4.join("asupprimer.txt").exists(), "asupprimer.txt ne reapparait pas localement apres un 3e passage");

    // ── 5. Suppression distante -> propagee en local ────────────────
    println!("\n── Test 5 : suppression distante propagee en local ──");
    reinitialiser_etat();
    let racine5 = std::env::temp_dir().join(format!("vex-synctest-{}-pc5", std::process::id()));
    let _ = fs::remove_dir_all(&racine5);
    fs::create_dir_all(&racine5).unwrap();
    fs::write(racine5.join("asupprimerdistant.txt"), b"a supprimer distant").unwrap();
    synchroniser(&client, &racine5, &log); // baseline connue des deux cotes

    let (_, fichiers5) = client.lister_dossier(0).unwrap();
    let id5 = fichiers5.iter().find(|f| f.nom == "asupprimerdistant.txt").expect("fichier distant introuvable").id;
    client.supprimer_fichier(id5).expect("suppression distante (simule le web)");

    synchroniser(&client, &racine5, &log); // doit propager la suppression en local
    assert_ok(!racine5.join("asupprimerdistant.txt").exists(), "fichier local supprime apres suppression distante");

    println!("\n=== TOUS LES TESTS DE SYNC SONT PASSES ===");
    println!("(compte de test {email} laisse en base -- purement jetable)");
    println!("(dossiers de test locaux conserves pour inspection : {})", racine.display());
    let _ = PathBuf::from(&racine2); // silence l'avertissement si jamais on retire l'assert plus tard
}
