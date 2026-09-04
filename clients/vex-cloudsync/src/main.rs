// ══════════════════════════════════════════════════════════════════
// vex-cloudsync — premiere ebauche de synchro VEX via l'API Windows
// Cloud Files (fichiers-fantomes, colonne "Statut" automatique dans
// l'Explorateur, telechargement a la demande) -- voir
// vex-sync-client/PLAN-CLOUD-FILES-API.md pour le contexte complet.
//
// ETAT : premiere version testable, PAS complete. Simplifications
// assumees pour ce premier jet (documentees en ligne) :
//   - rename/delete de DOSSIER renvoient NotSupported (VexClient n'a
//     pas encore ces methodes cote client -- a ajouter)
//   - pas de mark_in_sync au demarrage (suppose un dossier local VIDE
//     au premier lancement, donc uniquement pour un dossier de test
//     jetable, jamais un dossier avec du contenu existant)
//   - fichier entier dechiffre en memoire dans fetch_data avant d'etre
//     decoupe en tranches (voir le plan, option (a) : simple mais pas
//     un vrai flux pour les tres gros fichiers)
//
// A NE JAMAIS POINTER SUR UN DOSSIER CONTENANT DES DONNEES IMPORTANTES
// tant que ce n'est pas plus mature -- utiliser un dossier de test
// jetable, exactement comme pour vex-overlay.
// ══════════════════════════════════════════════════════════════════

mod device_auth;

use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cloud_filter::{
    error::{CResult, CloudErrorKind},
    filter::{info, ticket, Request, SyncFilter},
    metadata::Metadata,
    placeholder::{ConvertOptions, Placeholder},
    placeholder_file::PlaceholderFile,
    root::{HydrationType, PopulationType, SecurityId, Session, SyncRootIdBuilder, SyncRootInfo},
    utility::WriteAt,
};

use vex_sync_client::api::VexClient;

const PROVIDER_NAME: &str = "VEX";
const DISPLAY_NAME: &str = "VEX";

// ══════════════════════════════════════════════════════════════════
// DETECTION ADAPTATIVE DE L'URL DU SERVEUR
// L'exe est un fichier STATIQUE, signe une fois pour toutes en local (la
// cle de signature ne quitte jamais le PC de dev -- voir conversation :
// patcher un fichier signe casse sa signature, et signer a la volee cote
// serveur exigerait d'y mettre la cle, refuse pour des raisons de
// securite). L'"adaptatif" se fait donc cote CLIENT : au demarrage, on
// essaie chaque URL candidate et on garde la premiere qui repond -- utile
// tant que l'acces public (vex.hopto.org) et l'acces local (IP du Pi) ne
// sont pas garantis fonctionner en meme temps (probleme de routeur en
// cours, voir conversation).
const BASE_URL_CANDIDATS: &[&str] = &["https://vex.hopto.org", "http://192.168.1.14:8080"];

/// Essaie VEX_BASE_URL en priorite (utile pour les tests/dev), sinon
/// interroge chaque candidat (timeout court) et garde le premier qui
/// repond. Un endpoint sans auth et toujours 200 (meme pour un code
/// inconnu) sert de "ping".
fn detecter_base_url() -> Option<String> {
    if let Ok(v) = env::var("VEX_BASE_URL") {
        return Some(v);
    }
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(4)).build();
    for candidat in BASE_URL_CANDIDATS {
        println!("Test de connexion a {candidat}...");
        if agent.get(&format!("{candidat}/api/appareil/statut?code=ping")).call().is_ok() {
            println!("-> {candidat} repond, utilise pour cette session.");
            return Some(candidat.to_string());
        }
    }
    None
}
/// Windows 10 version 1709 (Fall Creators Update) -- premiere version a
/// exposer l'API Cloud Files. En dessous, l'enregistrement de la racine
/// echoue de facon peu comprehensible ; on prefere le detecter avant et
/// donner un message clair.
const BUILD_MINIMUM: u32 = 16299;

/// Numero de build Windows installe, via le registre (pas de dependance
/// supplementaire juste pour ca).
fn build_windows() -> Result<u32, String> {
    let sortie = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "/v", "CurrentBuildNumber"])
        .output()
        .map_err(|e| format!("impossible d'executer 'reg query' : {e}"))?;
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let mots: Vec<&str> = texte.split_whitespace().collect();
    mots.windows(3)
        .find(|w| w[0] == "CurrentBuildNumber")
        .and_then(|w| w[2].parse().ok())
        .ok_or_else(|| "impossible de lire le numero de build Windows dans le registre".to_string())
}

fn verifier_compatibilite() {
    match build_windows() {
        Ok(build) if build >= BUILD_MINIMUM => {
            println!("Windows build {build} : compatible (>= {BUILD_MINIMUM} requis).");
        }
        Ok(build) => {
            eprintln!(
                "Windows build {build} detecte -- l'API Cloud Files necessite au moins le \
                 build {BUILD_MINIMUM} (Windows 10 version 1709 / \"Fall Creators Update\" ou \
                 plus recent). Impossible de continuer sur ce systeme."
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Avertissement : verification de la version Windows impossible ({e}) -- on continue quand meme.");
        }
    }
}

/// Decode le blob opaque stocke sur chaque placeholder : "f:<id>" pour un
/// fichier, "d:<id>" pour un dossier distant. Vide = racine (dossier 0).
enum Cible {
    Fichier(i64),
    Dossier(i64),
}

fn decoder_blob(blob: &[u8]) -> Option<Cible> {
    let s = std::str::from_utf8(blob).ok()?;
    let (prefixe, id) = s.split_once(':')?;
    let id: i64 = id.parse().ok()?;
    match prefixe {
        "f" => Some(Cible::Fichier(id)),
        "d" => Some(Cible::Dossier(id)),
        _ => None,
    }
}

fn encoder_blob_fichier(id: i64) -> Vec<u8> { format!("f:{id}").into_bytes() }
fn encoder_blob_dossier(id: i64) -> Vec<u8> { format!("d:{id}").into_bytes() }

fn get_client_path() -> String {
    env::var("VEX_LOCAL_PATH").expect("variable d'environnement VEX_LOCAL_PATH requise")
}

pub struct Filter {
    client: VexClient,
}

impl SyncFilter for Filter {
    fn fetch_data(
        &self,
        request: Request,
        ticket: ticket::FetchData,
        info: info::FetchData,
    ) -> CResult<()> {
        let Some(Cible::Fichier(id)) = decoder_blob(request.file_blob()) else {
            return Err(CloudErrorKind::InvalidRequest);
        };

        // Simplification V1 (voir en-tete de fichier) : on telecharge et
        // dechiffre tout le fichier d'un coup (VexClient::telecharger fait
        // deja le dechiffrement AES-256-GCM), puis on sert des tranches du
        // buffer dechiffre selon l'intervalle demande par Windows.
        let contenu = self.client.telecharger(id).map_err(|e| {
            println!("fetch_data: erreur telechargement id={id} : {e}");
            CloudErrorKind::InvalidRequest
        })?;

        let range = info.required_file_range();
        let end = (range.end as usize).min(contenu.len()) as u64;
        let start = range.start;

        println!("fetch_data id={id} range={}..{}", start, end);

        ticket
            .write_at(&contenu[start as usize..end as usize], start)
            .map_err(|_| CloudErrorKind::InvalidRequest)?;

        Ok(())
    }

    fn fetch_placeholders(
        &self,
        request: Request,
        ticket: ticket::FetchPlaceholders,
        _info: info::FetchPlaceholders,
    ) -> CResult<()> {
        // Racine du sync root -> blob vide -> dossier distant 0.
        let dossier_distant_id = match decoder_blob(request.file_blob()) {
            Some(Cible::Dossier(id)) => id,
            _ => 0,
        };

        println!("fetch_placeholders dossier distant id={dossier_distant_id}");

        let (dossiers, fichiers) = self
            .client
            .lister_dossier(dossier_distant_id)
            .map_err(|e| {
                println!("fetch_placeholders: erreur liste : {e}");
                CloudErrorKind::InvalidRequest
            })?;

        let mut placeholders: Vec<PlaceholderFile> = Vec::new();

        for d in &dossiers {
            placeholders.push(
                PlaceholderFile::new(&d.nom)
                    .metadata(Metadata::directory())
                    .mark_in_sync()
                    .overwrite()
                    .blob(encoder_blob_dossier(d.id)),
            );
        }
        for f in &fichiers {
            placeholders.push(
                PlaceholderFile::new(&f.nom)
                    .metadata(Metadata::file().size(f.taille.max(0) as u64))
                    .mark_in_sync()
                    .overwrite()
                    .blob(encoder_blob_fichier(f.id)),
            );
        }

        ticket.pass_with_placeholder(&mut placeholders).map_err(|e| {
            println!("fetch_placeholders: pass_with_placeholder a echoue : {e:?}");
            CloudErrorKind::InvalidRequest
        })?;

        Ok(())
    }

    fn delete(&self, request: Request, ticket: ticket::Delete, info: info::Delete) -> CResult<()> {
        if info.is_undelete() {
            // Restauration depuis la corbeille -- pas gere en V1.
            return Err(CloudErrorKind::NotSupported);
        }
        match decoder_blob(request.file_blob()) {
            Some(Cible::Fichier(id)) => {
                self.client.supprimer_fichier(id).map_err(|e| {
                    println!("delete: erreur suppression distante id={id} : {e}");
                    CloudErrorKind::InvalidRequest
                })?;
            }
            // Suppression de dossier distant : pas encore expose cote
            // VexClient (a ajouter -- endpoint serveur deja existant).
            Some(Cible::Dossier(_)) => return Err(CloudErrorKind::NotSupported),
            None => return Err(CloudErrorKind::InvalidRequest),
        }
        ticket.pass().map_err(|_| CloudErrorKind::InvalidRequest)?;
        Ok(())
    }

    fn deleted(&self, _request: Request, _info: info::Deleted) {
        println!("deleted (confirme)");
    }

    fn rename(&self, _request: Request, _ticket: ticket::Rename, _info: info::Rename) -> CResult<()> {
        // Pas encore expose cote VexClient -- V1 refuse proprement plutot
        // que de silencieusement desynchroniser.
        Err(CloudErrorKind::NotSupported)
    }

    fn renamed(&self, _request: Request, _info: info::Renamed) {}

    fn opened(&self, request: Request, _info: info::Opened) {
        println!("opened: {:?}", request.path());
    }

    fn closed(&self, request: Request, info: info::Closed) {
        println!("closed {:?}, deleted={}", request.path(), info.deleted());
    }

    fn cancel_fetch_data(&self, _request: Request, _info: info::CancelFetchData) {
        println!("cancel_fetch_data");
    }

    fn validate_data(
        &self,
        _request: Request,
        _ticket: ticket::ValidateData,
        _info: info::ValidateData,
    ) -> CResult<()> {
        Err(CloudErrorKind::NotSupported)
    }

    fn cancel_fetch_placeholders(&self, _request: Request, _info: info::CancelFetchPlaceholders) {
        println!("cancel_fetch_placeholders");
    }

    fn dehydrate(&self, _request: Request, _ticket: ticket::Dehydrate, _info: info::Dehydrate) -> CResult<()> {
        Err(CloudErrorKind::NotSupported)
    }

    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) {
        println!("dehydrated");
    }

    fn state_changed(&self, changes: Vec<std::path::PathBuf>) {
        println!("state_changed: {:?}", changes);
    }
}

/// Convertit en placeholders les fichiers/dossiers REELS deja presents en
/// local (pas encore des placeholders) qui correspondent (par nom) a une
/// entree distante -- indispensable pour pointer sur un dossier qui a deja
/// du contenu, pas seulement un dossier vide (cas teste jusqu'ici).
/// Recursif, best-effort : une erreur sur une entree n'empeche pas les autres.
fn mark_in_sync(local_dir: &Path, client: &VexClient, dossier_distant_id: i64) {
    let Ok((dossiers, fichiers)) = client.lister_dossier(dossier_distant_id) else { return };
    let Ok(entries) = local_dir.read_dir() else { return };

    for entry in entries.filter_map(|e| e.ok()) {
        let nom = entry.file_name();
        let nom_str = nom.to_string_lossy().to_string();
        let est_dossier = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if est_dossier {
            let Some(d) = dossiers.iter().find(|d| d.nom == nom_str) else { continue };
            let options = ConvertOptions::default()
                .mark_in_sync()
                .has_children()
                .blob(encoder_blob_dossier(d.id));
            if let Ok(mut placeholder) = Placeholder::open(entry.path()) {
                if let Err(e) = placeholder.convert_to_placeholder(options, None) {
                    println!("mark_in_sync: conversion dossier {nom_str} echouee : {e:?}");
                }
            }
            mark_in_sync(&entry.path(), client, d.id);
        } else {
            let Some(f) = fichiers.iter().find(|f| f.nom == nom_str) else { continue };
            let options = ConvertOptions::default()
                .mark_in_sync()
                .blob(encoder_blob_fichier(f.id));
            match std::fs::File::open(entry.path()) {
                Ok(fichier) => {
                    let mut placeholder: Placeholder = fichier.into();
                    if let Err(e) = placeholder.convert_to_placeholder(options, None) {
                        println!("mark_in_sync: conversion fichier {nom_str} echouee : {e:?}");
                    }
                }
                Err(e) => println!("mark_in_sync: ouverture {nom_str} impossible : {e}"),
            }
        }
    }
}

/// Cree (ou met a jour) un raccourci "VEX.lnk" sur le Bureau, pointant vers
/// le dossier de synchro, avec l'icone VEX. Mecanisme standard et sans
/// risque (celui qu'utilise n'importe quel logiciel qui pose une icone sur
/// le Bureau a l'installation) -- pas de registre systeme, pas de droits
/// admin, rien a voir avec les "dossiers connus" de la barre laterale.
/// Windows range lui-meme la position de l'icone ; pas de controle possible
/// sur "a cote de Ce PC" precisement.
fn creer_raccourci_bureau(client_path: &str, icone: &str) {
    let bureau = match env::var("USERPROFILE") {
        Ok(p) => format!("{p}\\Desktop\\VEX.lnk"),
        Err(_) => return,
    };
    // L'icone COM (IShellLink.IconLocation) attend "chemin,index" separement.
    let (icone_fichier, icone_index) = icone.rsplit_once(',').unwrap_or((icone, "0"));

    let script = format!(
        r#"$s = New-Object -ComObject WScript.Shell; $l = $s.CreateShortcut('{bureau}'); $l.TargetPath = '{client_path}'; $l.IconLocation = '{icone_fichier},{icone_index}'; $l.Description = 'VEX Cloud Sync'; $l.Save()"#
    );
    let resultat = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output();
    match resultat {
        Ok(sortie) if sortie.status.success() => println!("Raccourci Bureau cree/mis a jour : {bureau}"),
        Ok(sortie) => println!(
            "Raccourci Bureau : echec ({}) -- {}",
            sortie.status,
            String::from_utf8_lossy(&sortie.stderr)
        ),
        Err(e) => println!("Raccourci Bureau : impossible de lancer powershell ({e})"),
    }
}

/// Fichier local (hors depot, propre a la machine) ou le jeton d'appareil
/// approuve est mis en cache pour eviter de refaire le flux d'autorisation
/// a chaque lancement. Protection : permissions par defaut du profil
/// utilisateur Windows (dossier non partage) -- pas de chiffrement au
/// repos pour cette premiere version (meme niveau de risque que la
/// plupart des jetons OAuth de CLI stockes en local, ex. gh/docker).
fn chemin_jeton() -> PathBuf {
    let base = env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("VexCloudSync").join("device.json")
}

fn charger_jeton(base_url: &str) -> Option<String> {
    let contenu = std::fs::read_to_string(chemin_jeton()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&contenu).ok()?;
    if v.get("base_url").and_then(|x| x.as_str()) != Some(base_url) {
        // Jeton enregistre pour un autre serveur -- on redemande une
        // autorisation plutot que d'envoyer ce jeton au mauvais endroit.
        return None;
    }
    v.get("jeton").and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn sauver_jeton(base_url: &str, jeton: &str) {
    let chemin = chemin_jeton();
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let v = serde_json::json!({"base_url": base_url, "jeton": jeton});
    if let Err(e) = std::fs::write(&chemin, v.to_string()) {
        println!("Avertissement : impossible d'enregistrer le jeton localement ({e}) -- le flux d'autorisation devra etre refait au prochain lancement.");
    }
}

fn nom_appareil() -> String {
    env::var("COMPUTERNAME").unwrap_or_else(|_| "Appareil Windows".to_string())
}

/// Attend que l'utilisateur appuie sur Entree avant de fermer la fenetre --
/// sans ca, un lancement par double-clic depuis l'Explorateur ouvre une
/// console qui se referme instantanement des qu'une erreur survient (ou
/// meme a la fin normale), impossible a lire ("un cmd qui flashe").
fn attendre_avant_fermeture() {
    println!("\nAppuie sur Entree pour fermer cette fenetre...");
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}

/// Demande le mot de passe de facon interactive si VEX_PASSWORD n'est pas
/// deja fourni. Indispensable pour un lancement par double-clic (aucun
/// moyen pour l'utilisateur de positionner une variable d'environnement
/// dans ce cas) -- jamais envoye au serveur, voir device_auth.rs.
fn demander_mot_de_passe() -> String {
    println!("Mot de passe VEX (sert uniquement a chiffrer/dechiffrer tes fichiers en local, jamais envoye au serveur) :");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).expect("lecture du mot de passe impossible");
    buf.trim().to_string()
}

fn main() {
    // Toute panique (.expect() etc.) affiche son message normalement PUIS
    // attend une touche -- sinon la fenetre se ferme avant que quiconque
    // ait pu lire quoi que ce soit.
    let hook_defaut = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        hook_defaut(info);
        attendre_avant_fermeture();
    }));

    verifier_compatibilite();

    let base_url = detecter_base_url()
        .expect("Impossible de joindre le serveur VEX (aucune des adresses connues ne repond -- verifie ta connexion).")
        .trim_end_matches('/')
        .to_string();
    // Le mot de passe reste necessaire EN LOCAL uniquement : il ne transite
    // jamais vers le serveur par ce chemin (voir device_auth.rs et
    // api.rs::Auth), mais la cle de chiffrement des fichiers en depend.
    let password = match env::var("VEX_PASSWORD") {
        Ok(p) => p,
        Err(_) => demander_mot_de_passe(),
    };
    let client_path = get_client_path();
    // Deux icones distinctes (feedback utilisateur) : le dossier teinte VEX
    // pour la racine de synchro dans l'Explorateur (comme OneDrive/GDrive),
    // le logo officiel VEX pour le raccourci Bureau (identifie l'app, pas
    // un dossier).
    let icone = env::var("VEX_ICON_PATH").unwrap_or_else(|_| {
        format!("{}\\vex-folder-icon.ico,0", env!("CARGO_MANIFEST_DIR"))
    });
    let icone_raccourci = env::var("VEX_SHORTCUT_ICON_PATH").unwrap_or_else(|_| {
        format!("{}\\vex-icon.ico,0", env!("CARGO_MANIFEST_DIR"))
    });

    let jeton = match charger_jeton(&base_url) {
        Some(j) => {
            println!("Appareil deja autorise pour {base_url}, reutilisation du jeton local.");
            j
        }
        None => {
            println!("Aucun appareil autorise pour {base_url} -- lancement du flux d'autorisation...");
            let j = device_auth::attendre_approbation(&base_url, &nom_appareil())
                .expect("echec du flux d'autorisation d'appareil");
            sauver_jeton(&base_url, &j);
            j
        }
    };

    let client = VexClient::depuis_jeton(&base_url, &jeton, &password);

    std::fs::create_dir_all(&client_path).expect("impossible de creer le dossier local");

    let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME)
        .user_security_id(SecurityId::current_user().unwrap())
        .build();

    if !sync_root_id.is_registered().unwrap() {
        sync_root_id
            .register(
                SyncRootInfo::default()
                    .with_display_name(DISPLAY_NAME)
                    .with_hydration_type(HydrationType::Full)
                    .with_population_type(PopulationType::Full)
                    .with_icon(&icone)
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .with_path(Path::new(&client_path))
                    .unwrap(),
            )
            .expect("echec d'enregistrement de la racine de synchro");
        println!("Racine de synchro enregistree : {client_path}");
    } else {
        println!("Racine de synchro deja enregistree : {client_path}");
    }

    creer_raccourci_bureau(&client_path, &icone_raccourci);

    println!("Marquage des fichiers locaux deja presents comme synchronises...");
    mark_in_sync(Path::new(&client_path), &client, 0);

    let connection = Session::new()
        .connect(&client_path, Filter { client })
        .expect("echec de connexion de la session Cloud Filter");

    println!("En ligne. Ctrl+C pour arreter (desinscrit la racine de synchro a l'arret).");
    wait_for_ctrlc();

    drop(connection);
    sync_root_id.unregister().expect("echec de desinscription");
    println!("Racine de synchro desinscrite. Arret propre.");
}

fn wait_for_ctrlc() {
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .expect("erreur d'installation du gestionnaire Ctrl-C");
    rx.recv().unwrap();
}
