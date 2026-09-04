// Bin de test UNIQUEMENT : simule ce que ferait le navigateur sur la page
// /autoriser-appareil (login SRP + clic "Autoriser"), pour tester le flux
// d'appareil de bout en bout sans devoir cliquer manuellement.
use num_bigint::BigUint;
use num_traits::Num;

const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

fn main() {
    let base_url = std::env::var("VEX_BASE_URL").expect("VEX_BASE_URL requis");
    let email = std::env::var("VEX_EMAIL").expect("VEX_EMAIL requis");
    let password = std::env::var("VEX_PASSWORD").expect("VEX_PASSWORD requis");
    let code = std::env::var("VEX_CODE").expect("VEX_CODE requis");
    let decision = std::env::var("VEX_DECISION").unwrap_or_else(|_| "oui".to_string());

    let agent = ureq::AgentBuilder::new().build();

    let r1 = agent.post(&format!("{base_url}/login/login"))
        .send_form(&[("action", "srp_step1"), ("email", &email)])
        .expect("etape 1");
    let d1: serde_json::Value = r1.into_json().unwrap();
    let salt_hex = d1["salt"].as_str().unwrap();
    let b_hex = d1["B"].as_str().unwrap();
    let token = d1["token"].as_str().unwrap();

    let n = BigUint::from_str_radix(N_HEX, 16).unwrap();
    let preuve = vex_sync_client::srp::calculer_preuve(&email, &password, salt_hex, b_hex).expect("calcul preuve");
    let _ = n; // juste pour verifier l'import, calculer_preuve fait le calcul en interne

    let r2 = agent.post(&format!("{base_url}/login/login"))
        .send_form(&[
            ("action", "srp_step2"),
            ("token", token),
            ("email", &email),
            ("A", &preuve.a_hex),
            ("M1", &preuve.m1_hex),
        ])
        .expect("etape 2");
    let cookie = r2.header("Set-Cookie").and_then(|c| c.split(';').next()).map(|s| s.to_string()).expect("cookie manquant");
    let d2: serde_json::Value = r2.into_json().unwrap();
    if d2.get("success").and_then(|v| v.as_bool()) != Some(true) {
        eprintln!("login echoue: {d2}");
        std::process::exit(1);
    }
    println!("Connecte en tant que {email}, cookie obtenu.");

    let r3 = agent.post(&format!("{base_url}/api/appareil/approuver"))
        .set("Cookie", &cookie)
        .send_form(&[("code", &code), ("decision", &decision)])
        .expect("approve request");
    let d3: serde_json::Value = r3.into_json().unwrap();
    println!("Reponse approbation ({decision}) pour code {code} : {d3}");
}
