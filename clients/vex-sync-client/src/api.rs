// ══════════════════════════════════════════════════════════════════
// api.rs — Client HTTP VEX : login SRP-6a + operations fchier.
//
// Limitation assumee (v1) : ne synchronise que la RACINE de fchier
// (dossier=0), pas l'arborescence complete des sous-dossiers, et au
// plus 100 fichiers (meme limite que l'API serveur /api/fchier/data).
// ══════════════════════════════════════════════════════════════════

use crate::filecrypto;
use crate::srp;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Deux facons de s'authentifier aupres du serveur VEX :
/// - Cookie : session classique issue du login SRP-6a (email+mdp).
/// - Jeton  : jeton d'appareil issu du flux d'autorisation (voir
///   login/appareil.rs cote serveur) -- pas de mot de passe transmis au
///   serveur pour obtenir ce jeton, mais le mot de passe reste necessaire
///   EN LOCAL pour chiffrer/dechiffrer les fichiers (voir le champ
///   `password` de VexClient : la cle de chiffrement est derivee du mot
///   de passe en clair, jamais connue du serveur -- le jeton ne peut donc
///   pas s'y substituer).
enum Auth {
    Cookie(String),
    Jeton(String),
}

pub struct FileEntry {
    pub id: i64,
    pub nom: String,
    pub taille: i64,
    pub mime: String,
    pub date: String,
}

pub struct DossierEntry {
    pub id: i64,
    pub nom: String,
}

pub struct VexClient {
    base_url: String,
    agent: ureq::Agent,
    auth: Auth,
    password: String,
}

fn parse_form_response(resp: ureq::Response) -> Result<(Value, Option<String>), String> {
    let cookie = resp
        .header("Set-Cookie")
        .and_then(|c| c.split(';').next())
        .map(|s| s.to_string());
    let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
    Ok((body, cookie))
}

impl VexClient {
    /// Authentifie via SRP-6a (2 aller-retours) et retourne un client pret
    /// a l'emploi. Le mot de passe en clair est conserve en memoire pour
    /// le chiffrement/dechiffrement des fichiers (jamais ecrit sur disque).
    pub fn login(base_url: &str, email: &str, password: &str) -> Result<Self, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        let base_url = base_url.trim_end_matches('/').to_string();

        // ── Etape 1 ──────────────────────────────────────────────
        let r1 = agent
            .post(&format!("{base_url}/login/login"))
            .send_form(&[("action", "srp_step1"), ("email", email)])
            .map_err(|e| format!("Connexion impossible (etape 1) : {e}"))?;
        let (d1, _) = parse_form_response(r1)?;
        if d1.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(d1.get("error").and_then(|v| v.as_str()).unwrap_or("Etape 1 echouee").to_string());
        }
        let salt_hex = d1.get("salt").and_then(|v| v.as_str()).ok_or("salt manquant")?;
        let b_hex = d1.get("B").and_then(|v| v.as_str()).ok_or("B manquant")?;
        let token = d1.get("token").and_then(|v| v.as_str()).ok_or("token manquant")?;

        // ── Calcul de la preuve (aucune donnee secrete n'est envoyee) ──
        let preuve = srp::calculer_preuve(email, password, salt_hex, b_hex)?;

        // ── Etape 2 ──────────────────────────────────────────────
        let r2 = agent
            .post(&format!("{base_url}/login/login"))
            .send_form(&[
                ("action", "srp_step2"),
                ("token", token),
                ("email", email),
                ("A", &preuve.a_hex),
                ("M1", &preuve.m1_hex),
            ])
            .map_err(|e| format!("Connexion impossible (etape 2) : {e}"))?;
        let (d2, cookie) = parse_form_response(r2)?;
        if d2.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(d2.get("error").and_then(|v| v.as_str()).unwrap_or("Email ou mot de passe incorrect").to_string());
        }
        let m2_hex = d2.get("M2").and_then(|v| v.as_str()).ok_or("M2 manquant")?;
        if !srp::verifier_m2(&preuve.a_hex, &preuve.m1_hex, &preuve.k_bytes, m2_hex) {
            // Le serveur n'a pas prouve connaitre le bon verifier : soit
            // usurpation, soit bug de protocole -- dans le doute, on refuse.
            return Err("Le serveur n'a pas pu prouver son identite (M2 invalide). Connexion refusee.".into());
        }
        let cookie = cookie.ok_or("Aucun cookie de session recu")?;

        Ok(Self { base_url, agent, auth: Auth::Cookie(cookie), password: password.to_string() })
    }

    /// Construit un client a partir d'un jeton d'appareil deja approuve
    /// (flux d'autorisation, voir device_auth cote vex-cloudsync) plutot
    /// que d'un login SRP interactif. Le mot de passe doit quand meme etre
    /// fourni EN LOCAL : il ne transite jamais vers le serveur via ce
    /// chemin, mais reste indispensable pour deriver la cle de chiffrement
    /// des fichiers (voir filecrypto.rs).
    pub fn depuis_jeton(base_url: &str, jeton: &str, password: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        let base_url = base_url.trim_end_matches('/').to_string();
        Self { base_url, agent, auth: Auth::Jeton(jeton.to_string()), password: password.to_string() }
    }

    fn appliquer_auth(&self, req: ureq::Request) -> ureq::Request {
        match &self.auth {
            Auth::Cookie(c) => req.set("Cookie", c),
            Auth::Jeton(j) => req.set("Authorization", &format!("Bearer {j}")),
        }
    }

    /// Requete GET authentifiee generique (debug/diagnostic).
    pub fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self.appliquer_auth(self.agent.get(&format!("{}{}", self.base_url, path)))
            .call().map_err(|e| format!("{e}"))?;
        resp.into_json().map_err(|e| format!("{e}"))
    }

    /// Requete POST JSON authentifiee generique (debug/diagnostic).
    pub fn post_json(&self, path: &str, body: Value) -> Result<Value, String> {
        let resp = self.appliquer_auth(self.agent.post(&format!("{}{}", self.base_url, path)))
            .set("Content-Type", "application/json")
            .send_json(body).map_err(|e| format!("{e}"))?;
        resp.into_json().map_err(|e| format!("{e}"))
    }

    /// Liste les fichiers a la racine de fchier (conservee pour selftest ;
    /// voir lister_dossier() pour la version recursive utilisee par la sync).
    pub fn lister_fichiers_racine(&self) -> Result<Vec<FileEntry>, String> {
        Ok(self.lister_dossier(0)?.1)
    }

    /// Liste le contenu d'un dossier fchier (sous-dossiers + fichiers).
    /// dossier_id=0 pour la racine ("Mes fichiers").
    ///
    /// Limitation cote serveur (voir src/fchier/fchier.rs::api_data) : la
    /// requete SQL sous-jacente plafonne a 100 fichiers pour le COMPTE
    /// ENTIER (tries par date desc), avant meme le filtrage par dossier --
    /// ce n'est donc pas 100 fichiers par dossier. Un compte avec plus de
    /// 100 fichiers au total peut voir des dossiers apparaitre incomplets
    /// ou vides s'ils contiennent des fichiers plus anciens que les 100
    /// plus recents du compte. Pas de pagination cote API pour contourner
    /// ca cote client.
    pub fn lister_dossier(&self, dossier_id: i64) -> Result<(Vec<DossierEntry>, Vec<FileEntry>), String> {
        let resp = self
            .appliquer_auth(self.agent.get(&format!("{}/api/fchier/data?dossier={}&shared=0", self.base_url, dossier_id)))
            .call()
            .map_err(|e| format!("Liste des fichiers impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;

        let dossiers = body.get("dossiers").and_then(|v| v.as_array()).cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|d| Some(DossierEntry {
                id: d.get("id")?.as_i64()?,
                nom: d.get("nom")?.as_str()?.to_string(),
            }))
            .collect();

        let fichiers = body.get("fichiers").and_then(|v| v.as_array()).cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|f| {
                Some(FileEntry {
                    id: f.get("id")?.as_i64()?,
                    nom: f.get("nom")?.as_str()?.to_string(),
                    taille: f.get("taille").and_then(|v| v.as_i64()).unwrap_or(0),
                    mime: f.get("type_fichier").and_then(|v| v.as_str()).unwrap_or("application/octet-stream").to_string(),
                    date: f.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .collect();

        Ok((dossiers, fichiers))
    }

    /// Cree un sous-dossier distant (parent_id=0 pour un dossier a la racine).
    pub fn creer_dossier(&self, nom: &str, parent_id: i64) -> Result<i64, String> {
        let resp = self
            .appliquer_auth(self.agent.post(&format!("{}/api/fchier/create_folder", self.base_url)))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({"folder_name": nom, "parent_id": parent_id}))
            .map_err(|e| format!("Creation du dossier impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur de creation de dossier").to_string());
        }
        body.get("id").and_then(|v| v.as_i64()).ok_or("id manquant dans la reponse".to_string())
    }

    /// Telecharge et dechiffre un fichier -> contenu en clair.
    pub fn telecharger(&self, id: i64) -> Result<Vec<u8>, String> {
        let resp = self
            .appliquer_auth(self.agent.get(&format!("{}/api/fchier/download?id={}", self.base_url, id)))
            .call()
            .map_err(|e| format!("Telechargement impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur de telechargement").to_string());
        }
        let contenu_b64 = body.get("contenu").and_then(|v| v.as_str()).ok_or("contenu manquant")?;
        let chiffre = B64.decode(contenu_b64).map_err(|e| format!("base64 invalide : {e}"))?;
        filecrypto::dechiffrer_fichier(&self.password, &chiffre)
    }

    /// Chiffre et uploade un fichier local a la racine de fchier.
    pub fn uploader(&self, nom: &str, plaintext: &[u8], mime: &str) -> Result<i64, String> {
        self.uploader_dans(0, nom, plaintext, mime)
    }

    /// Chiffre et uploade un fichier local dans le dossier distant donne
    /// (0 = racine "Mes fichiers").
    pub fn uploader_dans(&self, dossier_id: i64, nom: &str, plaintext: &[u8], mime: &str) -> Result<i64, String> {
        let chiffre = filecrypto::chiffrer_fichier(&self.password, plaintext);
        let file_b64 = B64.encode(&chiffre);
        let mut payload = HashMap::new();
        payload.insert("file_name", serde_json::json!(nom));
        payload.insert("file_b64", serde_json::json!(file_b64));
        payload.insert("mime_type", serde_json::json!(mime));
        payload.insert("taille", serde_json::json!(plaintext.len()));
        payload.insert("visble", serde_json::json!("1"));
        payload.insert("current_folder", serde_json::json!(dossier_id));

        let resp = self
            .appliquer_auth(self.agent.post(&format!("{}/api/fchier/upload", self.base_url)))
            .set("Content-Type", "application/json")
            .send_json(serde_json::to_value(&payload).unwrap())
            .map_err(|e| format!("Upload impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur d'upload").to_string());
        }
        body.get("id").and_then(|v| v.as_i64()).ok_or("id manquant dans la reponse".to_string())
    }

    /// Supprime un fichier distant (propagation d'une suppression locale).
    pub fn supprimer_fichier(&self, id: i64) -> Result<(), String> {
        let resp = self
            .appliquer_auth(self.agent.post(&format!("{}/api/fchier/delete", self.base_url)))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({"item_type": "file", "item_id": id}))
            .map_err(|e| format!("Suppression impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur de suppression").to_string());
        }
        Ok(())
    }

    /// Met a jour le contenu d'un fichier existant (chiffre + edit_content).
    pub fn remplacer_contenu(&self, id: i64, plaintext: &[u8]) -> Result<(), String> {
        let chiffre = filecrypto::chiffrer_fichier(&self.password, plaintext);
        let file_b64 = B64.encode(&chiffre);
        let resp = self
            .appliquer_auth(self.agent.post(&format!("{}/api/fchier/edit_content", self.base_url)))
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({"id": id, "contenu_b64": file_b64}))
            .map_err(|e| format!("Mise a jour impossible : {e}"))?;
        let body: Value = resp.into_json().map_err(|e| format!("Reponse illisible : {e}"))?;
        if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(body.get("error").and_then(|v| v.as_str()).unwrap_or("Erreur de mise a jour").to_string());
        }
        Ok(())
    }
}
