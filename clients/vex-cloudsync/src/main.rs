// Pas de fenetre de commande : plus aucune interaction en cmd (jugee
// "moche" et peu rassurante -- retour utilisateur direct). Toute
// l'interaction passe par une page web locale stylee (assets/setup.html,
// meme pattern que vex-sync-client) + une icone dans la barre des taches.
#![windows_subsystem = "windows"]

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
mod fenetre_mdp;
mod i18n;

use std::collections::VecDeque;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use cloud_filter::{
    error::{CResult, CloudErrorKind},
    filter::{info, ticket, Request, SyncFilter},
    metadata::Metadata,
    placeholder::{ConvertOptions, Placeholder},
    placeholder_file::{BatchCreate, PlaceholderFile},
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
// essaie chaque URL candidate et on garde la premiere qui repond.
//
// HTTPS FORCE (voir conversation, suite a un audit) : l'ancien candidat
// "http://192.168.1.14:8080" (acces reseau local direct, sans certificat)
// a ete retire, et exiger_https() ci-dessous rejette explicitement toute
// adresse non-https, y compris celle tapee a la main par l'utilisateur --
// le mot de passe n'est jamais envoye en clair (SRP-6a, voir
// vex-sync-client::api), mais l'echange SRP (email, valeur publique,
// preuve, cookie de session) circulait lui bel et bien en clair sur ce
// candidat HTTP, exploitable par quiconque ecoute le reseau local.
const BASE_URL_CANDIDATS: &[&str] = &["https://vex.hopto.org"];

fn exiger_https(url: &str) -> Option<&str> {
    let url = url.trim().trim_end_matches('/');
    if url.starts_with("https://") {
        Some(url)
    } else {
        if !url.is_empty() {
            println!("Adresse ignoree (pas en https) : {url}");
        }
        None
    }
}

/// Essaie VEX_BASE_URL en priorite (utile pour les tests/dev -- seul
/// echappatoire non soumis a exiger_https, deja protege par le fait qu'il
/// faut un acces shell a la machine pour le positionner), puis l'adresse
/// choisie par l'utilisateur dans la fenetre de connexion (voir
/// fenetre_mdp.rs -- configurable au lieu d'etre limitee au candidat code
/// en dur ci-dessous, utile pour qui heberge sa propre instance VEX en
/// https), enfin les candidats par defaut. Un endpoint sans auth et
/// toujours 200 (meme pour un code inconnu) sert de "ping".
/// FIX (retour utilisateur : "a chaque redemarrage du PC on doit
/// reconfigurer l'app") -- assurer_demarrage_auto() relance l'app des la
/// session Windows ouverte, souvent AVANT que le reseau (Wi-Fi, VPN,
/// obtention d'une adresse IP) ne soit pleinement pret -- la tentative de
/// connexion echouait alors systematiquement au tout premier essai apres
/// un redemarrage, meme si le serveur est parfaitement joignable une
/// poignee de secondes plus tard. Reessaie desormais plusieurs fois avec
/// une pause avant d'abandonner, au lieu d'un seul essai sec.
const TENTATIVES_DETECTION_URL: u32 = 5;
const PAUSE_ENTRE_TENTATIVES: std::time::Duration = std::time::Duration::from_secs(3);

fn detecter_base_url(url_utilisateur: &str) -> Option<String> {
    if let Ok(v) = env::var("VEX_BASE_URL") {
        return Some(v);
    }
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(4)).build();
    let mut candidats: Vec<&str> = Vec::new();
    if let Some(u) = exiger_https(url_utilisateur) {
        candidats.push(u);
    }
    candidats.extend(BASE_URL_CANDIDATS.iter().copied());

    for tentative in 1..=TENTATIVES_DETECTION_URL {
        for &candidat in &candidats {
            println!("Test de connexion a {candidat}... (tentative {tentative}/{TENTATIVES_DETECTION_URL})");
            if agent.get(&format!("{candidat}/api/appareil/statut?code=ping")).call().is_ok() {
                println!("-> {candidat} repond, utilise pour cette session.");
                if tentative > 1 {
                    fenetre_mdp::diag(&format!(
                        "connexion reseau reussie a la tentative {tentative}/{TENTATIVES_DETECTION_URL} (reseau probablement pas encore pret juste apres le demarrage)"
                    ));
                }
                return Some(candidat.to_string());
            }
        }
        if tentative < TENTATIVES_DETECTION_URL {
            std::thread::sleep(PAUSE_ENTRE_TENTATIVES);
        }
    }
    fenetre_mdp::diag(&format!(
        "aucun serveur joignable apres {TENTATIVES_DETECTION_URL} tentatives ({candidats:?})"
    ));
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

/// Retourne Err si Windows est trop ancien pour l'API Cloud Files -- pas de
/// process::exit ici, plus de console pour l'afficher (voir `main`, le
/// message doit remonter sur la page locale a la place).
fn verifier_compatibilite(etat: &EtatPartage) -> Result<(), String> {
    let langue = i18n::langue_courante();
    match build_windows() {
        Ok(build) if build >= BUILD_MINIMUM => {
            journaliser(etat, i18n::t(&langue, i18n::Cle::JournalWindowsCompatible).replace("{build}", &build.to_string()));
            Ok(())
        }
        Ok(build) => Err(i18n::t(&langue, i18n::Cle::ErreurWindowsIncompatible)
            .replace("{build}", &build.to_string())
            .replace("{minimum}", &BUILD_MINIMUM.to_string())),
        Err(e) => {
            journaliser(etat, i18n::t(&langue, i18n::Cle::JournalWindowsVerifImpossible).replace("{erreur}", &e));
            Ok(())
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

/// BUG CORRIGE : cette fonction exigeait la variable d'environnement
/// VEX_LOCAL_PATH via .expect() -- jamais definie nulle part dans
/// l'installation normale (pas de script d'installation qui la pose), donc
/// l'app plantait systematiquement ici (panic), juste apres avoir trouve le
/// serveur mais avant meme de demander un code d'autorisation. Constate en
/// pratique : aucune requete cote serveur, aucun appareil jamais autorise,
/// et rien de visible pour l'utilisateur (fenetre sans console). Dossier
/// par defaut maintenant, meme principe que OneDrive/Dropbox -- cree s'il
/// n'existe pas encore. VEX_LOCAL_PATH reste utilisable pour forcer un
/// autre emplacement (tests, utilisateur avance).
/// Chemin propose par defaut dans la fenetre de premier lancement (voir
/// `main`) -- separe de `get_client_path` pour ne pas creer le dossier avant
/// que l'utilisateur ait confirme (ou change) cet emplacement.
fn chemin_par_defaut() -> String {
    let base = env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("VEX Cloud").to_string_lossy().to_string()
}

fn get_client_path() -> String {
    if let Ok(chemin) = env::var("VEX_LOCAL_PATH") {
        return chemin;
    }
    let chemin = fenetre_mdp::charger_dossier_choisi().unwrap_or_else(chemin_par_defaut);
    let _ = std::fs::create_dir_all(&chemin);
    chemin
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
        let range = info.required_file_range();
        let start = range.start;
        let longueur = range.end.saturating_sub(range.start) as usize;

        // Simplification V1 (voir en-tete de fichier) : on telecharge et
        // dechiffre tout le fichier d'un coup (VexClient::telecharger fait
        // deja le dechiffrement AES-256-GCM), puis on sert des tranches du
        // buffer dechiffre selon l'intervalle demande par Windows.
        let id = match decoder_blob(request.file_blob()) {
            Some(Cible::Fichier(id)) => Some(id),
            _ => None,
        };
        let contenu = id.and_then(|id| match self.client.telecharger(id) {
            Ok(c) => Some(c),
            Err(e) => {
                println!("fetch_data: erreur telechargement id={id} : {e} -- ecrit du contenu vide plutot que d'echouer (voir FIX crash 0xc0000409)");
                None
            }
        });

        // FIX (crash reproductible constate en pratique -- exception
        // Windows 0xc0000409 dans ucrtbase.dll, voir Application event log
        // + %LOCALAPPDATA%\CrashDumps) -- avec un fichier local dont
        // l'original a ete supprime cote serveur (telecharger() renvoie
        // une 404), retourner Err ici declenchait, A L'INTERIEUR MEME du
        // crate cloud-filter (proxy.rs::fetch_data, hors de notre
        // controle, appelee directement par Windows dans un thread qu'
        // AUCUN catch_unwind de notre cote ne peut intercepter), un
        // command::Write::fail(...).unwrap() qui peut lui-meme echouer
        // (HRESULT 0x8007017C, "operation cloud invalide") -- panique NON
        // RATTRAPABLE qui plante TOUT LE PROCESS, coupant la synchro de
        // TOUS les dossiers, pas seulement du fichier fautif. On
        // n'emprunte donc plus jamais ce chemin : on ecrit TOUJOURS
        // quelque chose (le vrai contenu si le telechargement a reussi,
        // sinon des zeros de la longueur demandee) et on retourne
        // TOUJOURS Ok. Un fichier reellement supprime cote serveur finit
        // hydrate avec du faux contenu localement -- imparfait, mais
        // infiniment mieux qu'un crash qui coupe tout le monde ; nettoye
        // des que la reconciliation peut a nouveau lire ses metadonnees
        // (voir le fix catch_unwind sur nettoyer_placeholders_orphelines).
        let zeros;
        let source: &[u8] = match &contenu {
            Some(c) => c,
            None => {
                zeros = vec![0u8; longueur];
                &zeros
            }
        };
        let fin = (start as usize + longueur).min(source.len());
        let debut = (start as usize).min(fin);

        if let Err(e) = ticket.write_at(&source[debut..fin], start) {
            println!("fetch_data: write_at a echoue ({e:?}) -- ignore plutot que de renvoyer Err (voir FIX crash 0xc0000409)");
        }

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

/// FIX (demande utilisateur : "l'app affiche des fichiers qui ne sont pas
/// dans le cloud") : l'API Cloud Filter de Windows ne supprime JAMAIS
/// automatiquement une placeholder locale quand l'element correspondant a
/// ete supprime cote serveur (par un autre appareil, ou depuis l'admin) --
/// c'est a l'appli de le detecter et de le faire explicitement.
///
/// FIX (bug constate en pratique, orphan cleanup ne se declenchait
/// JAMAIS) : la version precedente comparait chaque placeholder locale a
/// l'ensemble des fichiers/dossiers distants par BLOB (l'id distant
/// stocke dans les metadonnees, lu via Placeholder::info()) -- mais cette
/// lecture PANIQUE de facon systematique dans le crate cloud-filter
/// (0.0.6, bug confirme en pratique : meme une placeholder tout juste
/// creee par CE process, format garanti correct, ne peut pas etre relue
/// -- "range end index 64 out of range for slice of length 63"). Sans
/// protection ca plantait tout le process (voir le fix catch_unwind sur
/// l'appel a .info(), garde ailleurs pour les rares cas ou cette fonction
/// serait encore appelee sur une entree illisible) ; avec la protection,
/// ca ignorait juste TOUJOURS l'entree -- aucun fichier supprime cote
/// serveur ne disparaissait donc jamais localement, silencieusement.
/// Compare desormais par NOM uniquement, exactement comme
/// creer_placeholders_manquants juste en dessous (meme logique, meme
/// limite acceptee : un renommage cote serveur ressemble a une
/// suppression+creation plutot qu'un vrai renommage local -- acceptable
/// pour cet usage) -- n'a plus besoin de lire quoi que ce soit dans le
/// reparse point, contourne entierement l'API du crate qui panique.
fn nettoyer_placeholders_orphelines(local_dir: &Path, client: &VexClient, dossier_distant_id: i64) {
    let Ok((dossiers, fichiers)) = client.lister_dossier(dossier_distant_id) else { return };
    let noms_distants: std::collections::HashSet<String> = dossiers
        .iter()
        .map(|d| d.nom.clone())
        .chain(fichiers.iter().map(|f| f.nom.clone()))
        .collect();

    let Ok(entries) = local_dir.read_dir() else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let chemin = entry.path();
        let nom = entry.file_name().to_string_lossy().to_string();
        let est_dossier = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        // Ne touche qu'une vraie placeholder deja synchronisee -- un
        // fichier local pas encore uploade (pas une placeholder cloud
        // filter) n'est jamais concerne par ce nettoyage.
        if Placeholder::open(&chemin).is_err() {
            continue;
        }
        if !noms_distants.contains(&nom) {
            println!("nettoyer_placeholders_orphelines: suppression locale de {chemin:?} (plus present cote serveur)");
            let res = if est_dossier {
                std::fs::remove_dir_all(&chemin)
            } else {
                std::fs::remove_file(&chemin)
            };
            if let Err(e) = res {
                println!("nettoyer_placeholders_orphelines: suppression {chemin:?} echouee : {e}");
            }
            continue;
        }
        if est_dossier {
            if let Some(d) = dossiers.iter().find(|d| d.nom == nom) {
                nettoyer_placeholders_orphelines(&chemin, client, d.id);
            }
        }
    }
}

/// FIX (demande utilisateur : "ça ne s'actualise pas [avec] les
/// modifications effectuées dans le cloud") : avant, un fichier/dossier
/// ajoute cote serveur (autre appareil, admin) n'apparaissait localement
/// que si l'Explorateur redemandait le contenu du dossier a Windows
/// (fetch_placeholders, declenche par exemple en rouvrant le dossier) --
/// aucun mecanisme ne le forçait pendant que l'app tournait deja. Cree
/// directement les placeholders manquantes sur le disque, avec la meme
/// API que fetch_placeholders (PlaceholderFile::create au lieu de
/// ticket.pass_with_placeholder, voir cloud-filter::placeholder_file --
/// recommande par la doc du crate pour un usage hors requete Explorateur).
/// Recursif : parcourt aussi les sous-dossiers, y compris ceux tout juste
/// crees a cet appel.
fn creer_placeholders_manquants(local_dir: &Path, client: &VexClient, dossier_distant_id: i64) {
    let Ok((dossiers, fichiers)) = client.lister_dossier(dossier_distant_id) else { return };
    let noms_locaux: std::collections::HashSet<String> = std::fs::read_dir(local_dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut a_creer: Vec<PlaceholderFile> = Vec::new();
    for d in &dossiers {
        if !noms_locaux.contains(&d.nom) {
            a_creer.push(
                PlaceholderFile::new(&d.nom)
                    .metadata(Metadata::directory())
                    .mark_in_sync()
                    .blob(encoder_blob_dossier(d.id)),
            );
        }
    }
    for f in &fichiers {
        if !noms_locaux.contains(&f.nom) {
            a_creer.push(
                PlaceholderFile::new(&f.nom)
                    .metadata(Metadata::file().size(f.taille.max(0) as u64))
                    .mark_in_sync()
                    .blob(encoder_blob_fichier(f.id)),
            );
        }
    }
    if !a_creer.is_empty() {
        if let Err(e) = a_creer.create(local_dir) {
            println!("creer_placeholders_manquants: creation echouee dans {local_dir:?} : {e:?}");
        }
    }

    for d in &dossiers {
        creer_placeholders_manquants(&local_dir.join(&d.nom), client, d.id);
    }
}

fn reconcilier(local_dir: &Path, client: &VexClient) {
    nettoyer_placeholders_orphelines(local_dir, client, 0);
    creer_placeholders_manquants(local_dir, client, 0);
}

/// FIX (retour utilisateur : "je veux pas de requete quand il se passe
/// rien, une requete seulement quand il se passe quelque chose") -- trois
/// versions successives de cette boucle avant celle-ci :
///   1. Reconciliation complete toutes les 3 minutes (poll lourd, rare).
///   2. Reconciliation complete toutes les 20 secondes (poll lourd,
///      frequent -- mieux ressenti, mais chaque tick fait un appel HTTP
///      RECURSIF par dossier cote serveur, meme quand rien n'a change).
///   3. Poll ADAPTATIF de /api/fchier/version (2s en cas d'activite
///      recente, ralentit jusqu'a 30s en cas d'inactivite) -- bien
///      moins lourd, mais fait encore une requete a intervalle regulier
///      MEME a l'arret complet (retour utilisateur : "je veux pas de
///      requete quand il se passe rien").
/// Remplace par un vrai LONG-POLL : /api/fchier/attendre bloque cote
/// SERVEUR (jusqu'a ~25s) jusqu'a ce qu'une modification survienne,
/// PUIS repond -- ce client relance immediatement un nouvel appel. Zero
/// requete supplementaire tant que rien ne change, reveil quasi
/// instantane (le temps d'un aller-retour reseau) des qu'un changement a
/// vraiment lieu. Le serveur ne bloque PAS son thread principal pour ca :
/// cette route est traitee sur un thread dedie cote serveur (voir
/// main.rs du serveur, /api/fchier/attendre), jamais sur la boucle
/// d'acceptation qui traite toutes les autres requetes -- aucun autre
/// utilisateur n'est retarde par cette attente.
fn lancer_reconciliation_periodique(client_path: String, client: VexClient) -> mpsc::Sender<()> {
    let (tx_stop, rx_stop) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        // Version au moment de la connexion : la reconciliation initiale
        // (lancee ailleurs, juste apres la connexion) est deja a jour avec
        // cette version, donc la premiere attente ne doit PAS redeclencher
        // un travail complet pour rien tant que rien n'a change depuis.
        let mut derniere_version = client.version().unwrap_or(-1);
        loop {
            // Verifie le signal d'arret AVANT de repartir sur un appel
            // bloquant (jusqu'a ~40s) -- pas d'attente ici, juste un
            // sondage instantane.
            if rx_stop.try_recv().is_ok() {
                break;
            }
            match client.attendre(derniere_version) {
                Ok(v) if v != derniere_version => {
                    derniere_version = v;
                    reconcilier(Path::new(&client_path), &client);
                    // Relance tout de suite une nouvelle attente -- pas de
                    // pause, le long-poll lui-meme fait deja office
                    // d'attente passive cote serveur.
                }
                Ok(_) => {
                    // Timeout cote serveur (~25s), rien de nouveau : on
                    // relance simplement une nouvelle attente, sans aucun
                    // travail supplementaire.
                }
                Err(_) => {
                    // Reseau coupe/serveur injoignable : pas la peine de
                    // marteler dans une boucle serree, courte pause avant
                    // de reessayer (et sondable via rx_stop pour rester
                    // reactif a "Quitter" meme en cas de panne reseau).
                    if rx_stop.recv_timeout(std::time::Duration::from_secs(5)).is_ok() {
                        break;
                    }
                }
            }
        }
    });
    tx_stop
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

// ══════════════════════════════════════════════════════════════════
// ICONES — integrees dans l'executable (include_bytes!, resolu a la
// COMPILATION sur la machine du developpeur -- inoffensif, ca ne fait
// qu'embarquer les octets dans le binaire) puis extraites sur le disque
// de l'UTILISATEUR au premier lancement.
//
// BUG CORRIGE : le code precedent construisait le chemin des icones via
// `env!("CARGO_MANIFEST_DIR")`, qui pointe vers le dossier du projet sur
// la machine ou l'exe a ete compile -- fige dans le binaire distribue.
// Sur le PC de n'importe quel utilisateur (le seul qui execute vraiment
// cet exe), ce chemin n'existe pas -- Windows ne trouve pas l'icone et
// retombe sur l'icone de dossier generique. D'ou "il n'y a pas d'icone"
// constate en pratique par un utilisateur autre que le developpeur.
const ICONE_DOSSIER_OCTETS: &[u8] = include_bytes!("../vex-folder-icon.ico");
const ICONE_RACCOURCI_OCTETS: &[u8] = include_bytes!("../vex-icon.ico");

fn dossier_vex_local() -> PathBuf {
    let base = env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("VexCloudSync")
}

/// Ecrit les icones embarquees sur le disque local (ecrase a chaque lancement
/// pour rester synchronise avec la version du binaire) et retourne leurs
/// chemins absolus, valides sur CETTE machine.
fn extraire_icones_locales() -> (String, String) {
    let dossier = dossier_vex_local();
    let _ = std::fs::create_dir_all(&dossier);

    let chemin_dossier_icone = dossier.join("vex-folder-icon.ico");
    let chemin_raccourci_icone = dossier.join("vex-icon.ico");

    let _ = std::fs::write(&chemin_dossier_icone, ICONE_DOSSIER_OCTETS);
    let _ = std::fs::write(&chemin_raccourci_icone, ICONE_RACCOURCI_OCTETS);

    (
        chemin_dossier_icone.to_string_lossy().to_string(),
        chemin_raccourci_icone.to_string_lossy().to_string(),
    )
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

/// Signal "deja installe" : soit un dossier de synchro a deja ete choisi
/// lors d'un lancement precedent (voir `main`), soit la racine Cloud Files
/// est deja enregistree pour cet utilisateur. Le deuxieme cas couvre une
/// installation faite avec une version anterieure a l'ajout de la fenetre
/// de premier lancement (donc sans dossier.txt) -- BUG CONSTATE EN PRATIQUE :
/// sans cette verification, l'app ne detectait pas une synchro deja active
/// et redemandait tout comme si de rien n'etait.
fn deja_installe() -> bool {
    if fenetre_mdp::charger_dossier_choisi().is_some() {
        return true;
    }
    if let Ok(sid) = SecurityId::current_user() {
        if SyncRootIdBuilder::new(PROVIDER_NAME)
            .user_security_id(sid)
            .build()
            .is_registered()
            .unwrap_or(false)
        {
            return true;
        }
    }
    // BUG CONSTATE EN PRATIQUE : les deux verifications ci-dessus peuvent
    // toutes les deux repasser a "non installe" apres un "Reinstaller" (qui
    // efface dossier.txt sans desenregistrer la racine) si la racine n'avait
    // en fait jamais ete correctement enregistree (ex. connexion jamais
    // menee a bien) -- dernier filet : le dossier par defaut existe deja et
    // contient quelque chose, signe qu'un lancement precedent y a deja mis
    // des fichiers, meme si les deux verifications precedentes disent non.
    std::fs::read_dir(chemin_par_defaut()).map(|mut e| e.next().is_some()).unwrap_or(false)
}

/// Retire la synchro et la configuration locale (PAS les fichiers
/// synchronises de l'utilisateur -- voir `fenetre_mdp::demander_action_installation`).
/// Desenregistre aussi la racine Cloud Files si elle est encore active,
/// sinon Windows la considererait toujours comme "en cours de synchro" apres
/// suppression de la config.
fn desinstaller() {
    if let Ok(sid) = SecurityId::current_user() {
        let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME).user_security_id(sid).build();
        if sync_root_id.is_registered().unwrap_or(false) {
            let _ = sync_root_id.unregister();
        }
    }
    fenetre_mdp::effacer_configuration_locale();
    let _ = std::fs::remove_file(chemin_jeton());
    if let Ok(profil) = env::var("USERPROFILE") {
        let _ = std::fs::remove_file(format!("{profil}\\Desktop\\VEX.lnk"));
    }
    let langue = i18n::langue_courante();
    fenetre_mdp::afficher_message("VEX Cloud Client", i18n::t(&langue, i18n::Cle::Desinstallee));
}

// ══════════════════════════════════════════════════════════════════
// ETAT PARTAGE + PAGE LOCALE (remplace le cmd)
// ══════════════════════════════════════════════════════════════════
#[derive(Default)]
struct EtatUi {
    lignes: VecDeque<String>,
    erreur: Option<String>,
    termine: bool,
    dossier: Option<String>,
}
type EtatPartage = Arc<Mutex<EtatUi>>;

fn journaliser(etat: &EtatPartage, msg: impl Into<String>) {
    let mut e = etat.lock().unwrap();
    e.lignes.push_back(msg.into());
    if e.lignes.len() > 200 {
        e.lignes.pop_front();
    }
}

fn signaler_erreur(etat: &EtatPartage, msg: impl Into<String>) {
    etat.lock().unwrap().erreur = Some(msg.into());
}

use fenetre_mdp::{afficher_message, demander_mot_de_passe};

/// Affiche l'etat courant dans une boite de dialogue native (declenche par
/// le clic sur "Ouvrir VEX Cloud Sync" dans la barre des taches). Remplace
/// l'ancienne page web locale de suivi.
fn afficher_statut(etat: &EtatPartage) {
    use i18n::Cle;
    let langue = i18n::langue_courante();
    let e = etat.lock().unwrap();
    let mut texte = String::new();
    if let Some(err) = &e.erreur {
        texte.push_str(&format!("{}{err}\n\n", i18n::t(&langue, Cle::StatutErreurPrefixe)));
    }
    texte.push_str(&format!(
        "{}{}\n",
        i18n::t(&langue, Cle::StatutLabel),
        if e.termine { i18n::t(&langue, Cle::StatutConnecte) } else { i18n::t(&langue, Cle::StatutEnCours) }
    ));
    if let Some(dossier) = &e.dossier {
        texte.push_str(&format!("{}{dossier}\n", i18n::t(&langue, Cle::StatutDossierLabel)));
    }
    // "Derniere etape" seulement en cas d'erreur (utile pour comprendre ce
    // qui bloquait) -- sur un succes, c'est juste la derniere ligne de log
    // interne (ex. "Marquage des fichiers locaux..."), qui ne veut rien dire
    // pour l'utilisateur et donne l'impression a tort que ce n'est pas fini.
    if e.erreur.is_some() {
        if let Some(derniere) = e.lignes.back() {
            texte.push_str(&format!("\n{}{derniere}", i18n::t(&langue, Cle::StatutDerniereEtapeLabel)));
        }
    }
    drop(e);
    afficher_message("VEX Cloud Client", &texte);
}

/// Deroule tout le flux (compatibilite, detection serveur, autorisation,
/// enregistrement de la racine de synchro, connexion Cloud Filter) en
/// journalisant chaque etape sur `etat` (affiche sur la page locale, plus
/// de console). Bloque en fin de fonction jusqu'a reception d'un signal
/// d'arret (clic "Quitter" dans la barre des taches), pour garder la
/// session Cloud Filter vivante tout du long.
fn executer_synchro(password: String, url_serveur: String, etat: EtatPartage, rx_quitter: mpsc::Receiver<()>) {
    let langue = i18n::langue_courante();
    if let Err(e) = verifier_compatibilite(&etat) {
        signaler_erreur(&etat, e);
        return;
    }

    journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalRechercheServeur));
    let base_url = match detecter_base_url(&url_serveur) {
        Some(u) => u.trim_end_matches('/').to_string(),
        None => {
            signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurServeurInjoignable));
            return;
        }
    };
    journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalServeurTrouve).replace("{url}", &base_url));

    let client_path = get_client_path();
    let (icone_dossier_locale, _) = extraire_icones_locales();
    let icone = env::var("VEX_ICON_PATH").unwrap_or_else(|_| format!("{},0", icone_dossier_locale));

    let jeton = match charger_jeton(&base_url) {
        Some(j) => {
            journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalAppareilDejaAutorise));
            j
        }
        None => {
            journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalAucunAppareilAutorise));
            match device_auth::attendre_approbation(&base_url, &nom_appareil()) {
                Ok(j) => {
                    sauver_jeton(&base_url, &j);
                    j
                }
                Err(e) => {
                    signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurAutorisationEchouee).replace("{erreur}", &e.to_string()));
                    return;
                }
            }
        }
    };
    journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalAppareilAutorise));

    let client = VexClient::depuis_jeton(&base_url, &jeton, &password);

    if let Err(e) = std::fs::create_dir_all(&client_path) {
        signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurDossierLocal).replace("{erreur}", &e.to_string()));
        return;
    }

    let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME)
        .user_security_id(SecurityId::current_user().unwrap())
        .build();

    let deja_enregistree = sync_root_id.is_registered().unwrap_or(false);
    if !deja_enregistree {
        let enregistrement = SyncRootInfo::default()
            .with_display_name(DISPLAY_NAME)
            .with_hydration_type(HydrationType::Full)
            .with_population_type(PopulationType::Full)
            .with_icon(&icone)
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_path(Path::new(&client_path));
        let enregistrement = match enregistrement {
            Ok(e) => e,
            Err(e) => {
                signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurCheminInvalide).replace("{erreur}", &format!("{e:?}")));
                return;
            }
        };
        if let Err(e) = sync_root_id.register(enregistrement) {
            signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurEnregistrementRacine).replace("{erreur}", &format!("{e:?}")));
            return;
        }
        journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalRacineEnregistree));
    } else {
        journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalRacineDejaEnregistree));
    }

    journaliser(&etat, i18n::t(&langue, i18n::Cle::JournalMarquageFichiers));
    mark_in_sync(Path::new(&client_path), &client, 0);
    // FIX (demande utilisateur : "le synchronisateur ne marche toujours
    // pas, statut connexion en cours [bloque]") -- reconcilier() parcourt
    // recursivement TOUT l'arbre de dossiers cote serveur (un appel reseau
    // par dossier, via creer_placeholders_manquants) -- avec une
    // arborescence un peu grosse ou une connexion lente, la faire de facon
    // SYNCHRONE avant la connexion Cloud Filter bloquait l'ecran sur
    // "Connexion en cours..." indefiniment. Lancee en tache de fond.
    //
    // FIX (crash reproductible constate en pratique, 0xc0000409 dans
    // ucrtbase.dll -- voir Application event log + %LOCALAPPDATA%\
    // CrashDumps) -- MAIS lancer reconcilier() en fond ET connecter la
    // session Cloud Filter dans la foulee (comme avant ce fix) cree une
    // COURSE : si un fichier local est une placeholder ORPHELINE (original
    // supprime cote serveur depuis) et que Windows/l'antivirus/l'indexeur
    // tente de l'hydrater (fetch_data) AVANT que nettoyer_placeholders_
    // orphelines() (dans reconcilier(), toujours en cours en tache de
    // fond) ait eu le temps de la retirer, le telechargement echoue (404)
    // -- et le crate cloud-filter appelle en interne un .unwrap() sur le
    // signalement d'echec a Windows, qui panique/plante TOUT le process
    // (aucune parade cote appelant : ce n'est pas notre code qui panique).
    // Reproduit en pratique avec un fichier supprime cote serveur entre
    // deux lancements. Attend desormais la fin de reconcilier() (avec un
    // delai maximum : ne bloque plus indefiniment sur une grosse
    // arborescence, voir le fix precedent) AVANT de connecter la session
    // -- aucune placeholder orpheline ne peut donc plus etre presente
    // quand Windows commence a envoyer des requetes de hydratation.
    let client_pour_reconciliation = client.clone();
    let client_pour_thread_init = client.clone();
    let chemin_pour_thread_init = client_path.clone();
    let (tx_reconciliation_initiale, rx_reconciliation_initiale) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        reconcilier(Path::new(&chemin_pour_thread_init), &client_pour_thread_init);
        let _ = tx_reconciliation_initiale.send(());
    });
    const ATTENTE_MAX_RECONCILIATION_INITIALE: std::time::Duration = std::time::Duration::from_secs(15);
    if rx_reconciliation_initiale.recv_timeout(ATTENTE_MAX_RECONCILIATION_INITIALE).is_err() {
        // Arborescence inhabituellement grosse/connexion lente : on
        // continue quand meme (mieux vaut se connecter avec un (tres)
        // faible risque residuel de course que rester bloque
        // indefiniment) -- reconcilier() continue son travail en tache de
        // fond independamment de ce timeout.
        journaliser(&etat, "Nettoyage initial toujours en cours apres 15s, connexion sans attendre plus longtemps...");
    }

    // FIX (HRESULT 0x8007017A, "la racine de synchronisation du cloud est
    // deja connectee a un autre fournisseur") : si un lancement precedent
    // n'a pas correctement libere sa connexion Cloud Filter (kill brutal
    // du process, plantage), Windows peut garder la racine marquee
    // "connectee" alors que plus rien n'est reellement connecte dessus --
    // aucune API de ce crate ne permet de forcer la deconnexion d'ailleurs
    // que le process qui l'a ouverte. Seul un desenregistrement +
    // reenregistrement de la racine remet les choses d'aplomb. On le tente
    // automatiquement une fois avant d'abandonner, plutot que de forcer
    // l'utilisateur a desinstaller/reinstaller a la main.
    let connection = match Session::new().connect(&client_path, Filter { client: client_pour_reconciliation.clone() }) {
        Ok(c) => c,
        Err(_) => {
            journaliser(&etat, "Connexion Cloud Filter refusee (racine deja marquee connectee) -- nouvelle tentative apres reinitialisation...");
            let _ = sync_root_id.unregister();
            let info = match SyncRootInfo::default()
                .with_display_name(DISPLAY_NAME)
                .with_hydration_type(HydrationType::Full)
                .with_population_type(PopulationType::Full)
                .with_icon(&icone)
                .with_version(env!("CARGO_PKG_VERSION"))
                .with_path(Path::new(&client_path))
            {
                Ok(info) => info,
                Err(e) => {
                    signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurCheminInvalide).replace("{erreur}", &format!("{e:?}")));
                    return;
                }
            };
            if let Err(e) = sync_root_id.register(info) {
                signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurEnregistrementRacine).replace("{erreur}", &format!("{e:?}")));
                return;
            }
            match Session::new().connect(&client_path, Filter { client: client_pour_reconciliation.clone() }) {
                Ok(c) => c,
                Err(e) => {
                    signaler_erreur(&etat, i18n::t(&langue, i18n::Cle::ErreurConnexionCloudFilter).replace("{erreur}", &format!("{e:?}")));
                    return;
                }
            }
        }
    };

    let arreter_reconciliation = lancer_reconciliation_periodique(client_path.clone(), client_pour_reconciliation);

    {
        let mut e = etat.lock().unwrap();
        e.termine = true;
        e.dossier = Some(client_path.clone());
    }

    // Garde la session vivante jusqu'au signal "Quitter" (barre des taches).
    let _ = rx_quitter.recv();

    let _ = arreter_reconciliation.send(());
    drop(connection);
    let _ = sync_root_id.unregister();
}

enum EvenementTray {
    Ouvrir,
    Quitter,
}

/// Verrou mono-instance : Windows Cloud Filter n'autorise qu'UNE seule
/// session connectee a la fois sur une racine de synchro donnee -- lancer
/// une deuxieme instance (double-clic accidentel, ancien process pas
/// encore ferme, raccourci Bureau + barre des taches) faisait echouer la
/// connexion avec un message HRESULT cryptique ("La racine de
/// synchronisation du cloud est deja connectee a un autre fournisseur").
/// Un mutex nomme global detecte ce cas des le demarrage et affiche un
/// message clair au lieu de laisser la connexion Cloud Filter echouer
/// plus loin dans le flux. Le HANDLE doit rester vivant jusqu'a la fin du
/// process (Windows le libere tout seul a la sortie), d'ou le retour ici
/// plutot qu'un drop immediat.
fn deja_en_cours() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::PCWSTR;
    let nom: Vec<u16> = "Global\\VEXCloudSyncSingleInstance\0".encode_utf16().collect();
    unsafe {
        match CreateMutexW(None, false, PCWSTR(nom.as_ptr())) {
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    None
                } else {
                    Some(h)
                }
            }
            Err(_) => None,
        }
    }
}

/// FIX (retour utilisateur : "l'app ne se relance pas toute seule") -- le
/// Run key (mecanisme d'origine) ne redemarre l'app qu'A LA CONNEXION
/// Windows : si le process crashe EN COURS DE SESSION (panique, tue par
/// un antivirus, plantage silencieux de l'API Cloud Filter), plus rien ne
/// le relance avant la prochaine connexion -- symptome constate : le
/// dossier VEX Cloud affiche "le fournisseur de fichiers cloud s'est
/// ferme de maniere inattendue" et le reste tant que personne ne relance
/// l'app a la main. Tente d'abord une TACHE PLANIFIEE (Task Scheduler) :
/// meme declenchement a la connexion (LogonTrigger), PLUS RestartOnFailure
/// qui relance automatiquement le process s'il se termine avec un code de
/// sortie non nul -- exactement ce que le Run key ne peut pas faire. Une
/// sortie normale (bouton "Quitter", code 0) N'EST PAS un "echec" pour
/// Task Scheduler, donc ne redeclenche rien : quitter volontairement reste
/// respecte.
///
/// TESTE EN PRATIQUE (pas juste suppose) : sur au moins une machine non-
/// admin reelle, `schtasks /Create` avec un declencheur "a la connexion"
/// echoue avec "Acces refuse" -- meme avec /RL LIMITED ou /RU explicite --
/// alors qu'un declencheur /SC ONCE reussit sans probleme sur le meme
/// compte. Cause probable : privilege "Ouvrir une session en tant que
/// tache par lots" (SeBatchLogonRight) restreint par une strategie locale/
/// de groupe sur cette machine, ou politique similaire sur d'autres PC
/// verrouilles. Comme schtasks echoue SILENCIEUSEMENT si on ignore son
/// code de sortie (l'ancienne version de ce commit faisait exactement
/// ça), un utilisateur sur une machine ainsi restreinte se serait retrouve
/// SANS AUCUN demarrage automatique (pire qu'avant, le Run key ayant ete
/// retire). On verifie donc le code de sortie : la tache planifiee n'est
/// gardee QUE si sa creation reussit reellement, avec repli sur le Run key
/// (mecanisme eprouve, fonctionne sans droits admin) dans le cas contraire.
fn assurer_demarrage_auto() {
    if tenter_tache_planifiee() {
        // Tache planifiee active : retire une eventuelle vieille entree Run
        // key d'une precedente execution -- sinon l'app se lancerait deux
        // fois a chaque connexion (Run key + tache), et le verrou mono-
        // instance (deja_en_cours) afficherait une popup inutile.
        retirer_ancienne_entree_run();
    } else {
        ecrire_entree_run();
    }
}

/// Tente de creer/mettre a jour la tache planifiee. Retourne true UNIQUEMENT
/// si `schtasks` a reellement reussi (code de sortie 0) -- jamais suppose.
fn tenter_tache_planifiee() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let exe_xml = exe
        .to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{exe_xml}"</Command>
    </Exec>
  </Actions>
</Task>"#
    );

    let chemin_xml = env::temp_dir().join("vex-cloudsync-task.xml");
    if ecrire_fichier_utf16(&chemin_xml, &xml).is_err() {
        return false;
    }

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let resultat = std::process::Command::new("schtasks")
        .args(["/Create", "/TN", "VEXCloudSync", "/XML"])
        .arg(&chemin_xml)
        .arg("/F")
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = std::fs::remove_file(&chemin_xml);

    matches!(resultat, Ok(sortie) if sortie.status.success())
}

/// Mecanisme de repli : entree dans HKCU\Software\Microsoft\Windows\
/// CurrentVersion\Run pointant vers l'executable courant -- ne relance
/// l'app qu'a la connexion (pas de recuperation apres crash en cours de
/// session, voir la doc de assurer_demarrage_auto), mais fonctionne sans
/// droits admin meme sur une machine qui refuse la creation de taches
/// planifiees a declenchement "connexion" pour un compte standard.
fn ecrire_entree_run() {
    use windows::Win32::System::Registry::{
        RegCreateKeyExW, RegSetValueExW, RegCloseKey, HKEY_CURRENT_USER,
        KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    use windows::core::PCWSTR;

    let Ok(exe) = std::env::current_exe() else { return };
    let valeur = format!("\"{}\"", exe.to_string_lossy());
    let sous_cle: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
    let nom_valeur: Vec<u16> = "VEXCloudSync\0".encode_utf16().collect();
    let mut data: Vec<u16> = valeur.encode_utf16().collect();
    data.push(0);
    let data_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2)
    };

    unsafe {
        let mut hkey = Default::default();
        let r = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sous_cle.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if r.is_ok() {
            let _ = RegSetValueExW(hkey, PCWSTR(nom_valeur.as_ptr()), 0, REG_SZ, Some(data_bytes));
            let _ = RegCloseKey(hkey);
        }
    }
}

/// schtasks /Create /XML exige un fichier encode dans l'encodage annonce
/// par son prologue (UTF-16 ci-dessus) -- un fichier UTF-8 est refuse a
/// l'import. Ecrit donc en UTF-16LE avec BOM.
fn ecrire_fichier_utf16(chemin: &Path, texte: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut octets: Vec<u8> = vec![0xFF, 0xFE];
    for unite in texte.encode_utf16() {
        octets.extend_from_slice(&unite.to_le_bytes());
    }
    std::fs::File::create(chemin)?.write_all(&octets)
}

/// Supprime l'entree HKCU\...\Run laissee par une version anterieure de
/// cette app (avant le passage a la tache planifiee ci-dessus). Best
/// effort : `reg delete` renvoie une erreur si la valeur n'existe deja
/// plus, ignoree silencieusement (cas normal apres la premiere migration).
fn retirer_ancienne_entree_run() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "VEXCloudSync",
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

fn main() {
    let _verrou_instance = match deja_en_cours() {
        Some(h) => h,
        None => {
            let langue = i18n::langue_courante();
            afficher_message("VEX Cloud Client", i18n::t(&langue, i18n::Cle::DejaEnCoursExecution));
            return;
        }
    };

    assurer_demarrage_auto();

    let etat: EtatPartage = Arc::new(Mutex::new(EtatUi::default()));

    let (icone_dossier_locale, _) = extraire_icones_locales();

    // Deja configure sur cette machine (relance apres un premier lancement
    // reussi) : on propose de reinstaller ou desinstaller plutot que de
    // redemander silencieusement dossier+mot de passe comme si de rien
    // n'etait. "Continuer" (Annuler dans la boite de dialogue) garde le
    // comportement normal.
    //
    // DEMANDE UTILISATEUR : ce dialogue s'affichait a CHAQUE lancement des
    // que deja_installe() est vrai -- donc a chaque redemarrage Windows via
    // assurer_demarrage_auto, bloquant la synchro tant que personne n'etait
    // devant l'ecran pour le fermer. Une fois le mot de passe mis en cache
    // (voir charger_mdp_sauvegarde / sauvegarder_mdp plus bas), un
    // relancement est par definition une reprise automatique et non une
    // reinstallation manuelle -- on saute directement ce dialogue dans ce
    // cas (equivalent a fermer sur "Continuer"). Sans mot de passe en
    // cache (premiere fois avec cette version, ou apres un "Reinstaller"),
    // le dialogue reste affiche comme avant.
    //
    // FIX (retour utilisateur : "a chaque redemarrage on doit reconfigurer")
    // -- charger_mdp_sauvegarde() n'est plus appelee deux fois separement
    // ici et plus bas (chaque appel journalise desormais un diagnostic
    // persistant, l'appeler deux fois aurait duplique la ligne de log pour
    // rien) : le resultat de CE premier appel est reutilise plus bas, sauf
    // si "Reinstaller" vient de l'effacer entre-temps.
    let deja_install = deja_installe();
    let mdp_en_cache = if deja_install { fenetre_mdp::charger_mdp_sauvegarde() } else { None };
    fenetre_mdp::diag(&format!(
        "demarrage : deja_installe={deja_install}, mdp_en_cache={}",
        mdp_en_cache.is_some()
    ));
    if deja_install && mdp_en_cache.is_none() {
        match fenetre_mdp::demander_action_installation(&icone_dossier_locale, &get_client_path()) {
            fenetre_mdp::ActionInstallation::Desinstaller => {
                desinstaller();
                return;
            }
            fenetre_mdp::ActionInstallation::Reinstaller => {
                fenetre_mdp::effacer_configuration_locale();
                let _ = std::fs::remove_file(chemin_jeton());
            }
            // Fermer cette fenetre (croix, Echap) = reconnecter normalement,
            // PAS quitter. BUG CONSTATE EN PRATIQUE : le `return` ici faisait
            // que tout relancement de l'app apres extinction (redemarrage
            // Windows, crash, fermeture manuelle) qui tombe sur ce dialogue
            // se terminait immediatement sans jamais rouvrir de session
            // Cloud Filter -- la racine de synchro restait enregistree aupres
            // de Windows mais sans fournisseur actif derriere, d'ou l'erreur
            // Explorateur "Le fournisseur de fichiers cloud s'est ferme de
            // maniere inattendue". Ici on ne rouvre PAS la fenetre de choix
            // de dossier (deja_installe()==true => elle est deja sautee plus
            // bas), seulement le mot de passe puis la reconnexion a la racine
            // deja enregistree (voir `deja_enregistree` dans
            // `executer_synchro`, qui ne re-enregistre pas si c'est deja fait).
            fenetre_mdp::ActionInstallation::Continuer => {}
        }
    }

    // Tout premier lancement (aucun dossier encore choisi) : on demande OU
    // synchroniser avant meme de demander le mot de passe -- une fenetre,
    // fermee proprement, puis la suivante s'ouvre (voir fenetre_mdp.rs pour
    // le detail du cycle de vie, identique a demander_mot_de_passe). Si
    // l'utilisateur ferme sans choisir, on garde simplement le dossier par
    // defaut plutot que de bloquer le lancement.
    if fenetre_mdp::charger_dossier_choisi().is_none() {
        if let Some(chemin) =
            fenetre_mdp::demander_dossier_destination(&icone_dossier_locale, &chemin_par_defaut())
        {
            fenetre_mdp::sauvegarder_dossier_choisi(&chemin);
        }
    }

    // DEMANDE UTILISATEUR : sur une machine deja configuree (jeton + dossier
    // deja enregistres, voir deja_installe()), ne plus redemander le mot de
    // passe de chiffrement a chaque lancement -- avant ce correctif, un
    // redemarrage Windows (assurer_demarrage_auto relance l'app) laissait la
    // synchro bloquee sur cette fenetre tant que quelqu'un n'etait pas
    // physiquement devant la machine pour le retaper. Le mot de passe est
    // desormais mis en cache via DPAPI (voir fenetre_mdp::sauvegarder_mdp) --
    // lisible uniquement par ce compte Windows sur cette machine, pas en
    // clair sur le disque. Si le cache est absent/invalide (premiere
    // installation, ou "Reinstaller"), la fenetre s'affiche normalement.
    let (mdp, url_serveur) = match mdp_en_cache {
        Some(mdp) => (mdp, fenetre_mdp::charger_url_preferee()),
        None => {
            let Some((mdp, url_serveur)) = demander_mot_de_passe(&icone_dossier_locale) else {
                return;
            };
            fenetre_mdp::sauvegarder_mdp(&mdp);
            (mdp, url_serveur)
        }
    };

    let (tx_evt, rx_evt) = mpsc::channel::<EvenementTray>();
    // BUG CORRIGE : IconSource::Resource("") cherchait une ressource nommee
    // par une chaine VIDE dans l'exe -- echoue toujours (LoadImageW renvoie
    // NULL), TrayItem::new() remontait une erreur silencieusement avalee
    // par le .ok() juste en dessous. Resultat en pratique : AUCUNE icone
    // dans la barre des taches, jamais, meme quand tout le reste (jeton,
    // synchro) fonctionnait correctement. IconSource::RawIcon avec un vrai
    // handle charge depuis le fichier .ico evite ce probleme de resolution
    // de ressource par nom.
    let icone_brute = fenetre_mdp::charger_icone_brute(&icone_dossier_locale, 32);
    let mut tray = tray_item::TrayItem::new(
        "VEX Cloud Client",
        tray_item::IconSource::RawIcon(icone_brute),
    )
    .ok();
    if let Some(t) = tray.as_mut() {
        let langue_tray = i18n::langue_courante();
        let tx1 = tx_evt.clone();
        let _ = t.add_menu_item(i18n::t(&langue_tray, i18n::Cle::TrayOuvrir), move || {
            let _ = tx1.send(EvenementTray::Ouvrir);
        });
        let tx2 = tx_evt.clone();
        let _ = t.add_menu_item(i18n::t(&langue_tray, i18n::Cle::TrayQuitter), move || {
            let _ = tx2.send(EvenementTray::Quitter);
        });
    }

    let (tx_quitter_worker, rx_quitter_worker) = mpsc::channel::<()>();
    let (tx_worker_termine, rx_worker_termine) = mpsc::channel::<()>();
    {
        let etat2 = etat.clone();
        let tx_t = tx_worker_termine.clone();
        std::thread::spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                executer_synchro(mdp, url_serveur, etat2.clone(), rx_quitter_worker);
            }))
            .is_err()
            {
                let langue = i18n::langue_courante();
                signaler_erreur(&etat2, i18n::t(&langue, i18n::Cle::ErreurInterneInattendue));
            }
            let _ = tx_t.send(());
        });
    }

    // Icone "connecte" (badge vert + coche) une fois la synchro etablie
    // avec succes -- distingue visuellement "en cours de connexion" de
    // "tout fonctionne", demande explicitement par l'utilisateur.
    let mut deja_signale_connecte = false;
    // Avant : rien n'informait l'utilisateur du resultat de la connexion --
    // il fallait cliquer "Ouvrir VEX Cloud Client" dans la barre des taches
    // pour le decouvrir. On affiche maintenant `afficher_statut` tout seul,
    // une fois, des que l'issue (succes OU erreur) est connue.
    let mut deja_affiche_resultat = false;

    loop {
        let (termine, a_erreur) = {
            let e = etat.lock().unwrap();
            (e.termine, e.erreur.is_some())
        };

        if !deja_signale_connecte && termine {
            deja_signale_connecte = true;
            if let Some(t) = tray.as_mut() {
                let icone_connectee = fenetre_mdp::charger_icone_connectee(&icone_dossier_locale, 32);
                let _ = t.set_icon(tray_item::IconSource::RawIcon(icone_connectee));
            }
        }

        if !deja_affiche_resultat && (termine || a_erreur) {
            deja_affiche_resultat = true;
            afficher_statut(&etat);
        }

        match rx_evt.try_recv() {
            Ok(EvenementTray::Ouvrir) => afficher_statut(&etat),
            Ok(EvenementTray::Quitter) => {
                let _ = tx_quitter_worker.send(());
                let _ = rx_worker_termine.recv_timeout(std::time::Duration::from_secs(10));
                break;
            }
            Err(_) => {}
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
