// ══════════════════════════════════════════════════════════════════
// device_auth.rs — Cote client du flux d'autorisation d'appareil (voir
// src/login/appareil.rs cote serveur VEX, et PLAN-INSTALLATION-1-CLIC.md).
//
// Remplace le login SRP interactif (email+mot de passe tapes a chaque
// lancement) par : demande d'un code -> ouverture du navigateur sur une
// page DU SERVEUR VEX ou l'utilisateur (deja connecte) approuve ou non
// -> recuperation d'un jeton d'acces via polling.
//
// IMPORTANT (voir api.rs::Auth) : ce jeton authentifie les appels HTTP,
// mais ne remplace PAS le mot de passe necessaire au chiffrement local
// des fichiers -- celui-ci reste fourni separement (VEX_PASSWORD).
// ══════════════════════════════════════════════════════════════════

use serde_json::Value;
use std::time::{Duration, Instant};

/// Le serveur expire un code non approuve au bout de 10 minutes
/// (voir EXPIRATION_MINUTES dans appareil.rs) -- on borne l'attente un
/// peu au-dela pour laisser une marge, plutot que d'attendre indefiniment.
const DELAI_MAX: Duration = Duration::from_secs(11 * 60);
const INTERVALLE_POLL: Duration = Duration::from_secs(3);

pub enum Statut {
    EnAttente,
    Approuve(String),
    Refuse,
    Expire,
    Introuvable,
}

fn demander_code(agent: &ureq::Agent, base_url: &str, nom_appareil: &str) -> Result<String, String> {
    let resp = agent
        .post(&format!("{base_url}/api/appareil/demander"))
        .send_form(&[("nom_appareil", nom_appareil)])
        .map_err(|e| format!("Demande de code impossible : {e}"))?;
    let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur inconnue").to_string());
    }
    body.get("code").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| "code manquant dans la reponse".into())
}

fn interroger_statut(agent: &ureq::Agent, base_url: &str, code: &str) -> Result<Statut, String> {
    let resp = agent
        .get(&format!("{base_url}/api/appareil/statut?code={code}"))
        .call()
        .map_err(|e| format!("Interrogation du statut impossible : {e}"))?;
    let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
    Ok(match body.get("statut").and_then(|v| v.as_str()) {
        Some("approuve") => match body.get("jeton").and_then(|v| v.as_str()) {
            Some(j) => Statut::Approuve(j.to_string()),
            // "approuve" sans jeton = deja recupere par un poll precedent
            // (ne devrait pas arriver dans une utilisation normale, un
            // seul appelant fait le polling pour un code donne).
            None => Statut::EnAttente,
        },
        Some("refuse") => Statut::Refuse,
        Some("expire") => Statut::Expire,
        Some("introuvable") => Statut::Introuvable,
        _ => Statut::EnAttente,
    })
}

pub fn ouvrir_navigateur(url: &str) {
    // ShellExecuteW (API Win32 officielle pour "ouvrir ce lien avec le
    // gestionnaire par defaut") -- plus fiable qu'un sous-processus
    // explorer.exe, qui peut reussir silencieusement en arriere-plan sans
    // jamais passer au premier plan (l'utilisateur croit alors que rien ne
    // s'est passe).
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let url_w = HSTRING::from(url);
    let verbe = HSTRING::from("open");
    unsafe {
        ShellExecuteW(None, &verbe, &url_w, None, None, SW_SHOWNORMAL);
    }
}

/// Lance le flux complet : demande un code, ouvre le navigateur sur la
/// page d'approbation du VRAI serveur VEX, puis attend (poll) que
/// l'utilisateur approuve. Retourne le jeton d'acces en cas de succes.
pub fn attendre_approbation(base_url: &str, nom_appareil: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let code = demander_code(&agent, base_url, nom_appareil)?;

    let url = format!("{base_url}/autoriser-appareil?code={code}");
    println!("Ouverture du navigateur pour autoriser cet appareil...");
    println!("(si rien ne s'ouvre, va sur : {url})");
    ouvrir_navigateur(&url);

    let debut = Instant::now();
    loop {
        if debut.elapsed() > DELAI_MAX {
            return Err("Delai d'attente depasse -- le code a du expirer. Relance.".into());
        }
        std::thread::sleep(INTERVALLE_POLL);
        match interroger_statut(&agent, base_url, &code)? {
            Statut::Approuve(jeton) => {
                println!("Appareil autorise.");
                return Ok(jeton);
            }
            Statut::Refuse => return Err("Autorisation refusee depuis le navigateur.".into()),
            Statut::Expire => return Err("Code expire avant d'etre approuve. Relance.".into()),
            Statut::Introuvable => return Err("Code introuvable cote serveur (erreur inattendue).".into()),
            Statut::EnAttente => {}
        }
    }
}
