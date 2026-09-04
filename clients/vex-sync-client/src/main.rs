// Pas de fenetre de commande : l'appli tourne en tache de fond, toute
// l'interaction passe par la page web locale (voir assets/setup.html)
// et l'icone dans la barre des taches.
#![windows_subsystem = "windows"]

// ══════════════════════════════════════════════════════════════════
// vex-sync-client — synchronisation de dossiers locaux (Windows) avec
// fchier sur un serveur VEX.
//
// Pas de console : au demarrage, une page web locale stylee s'ouvre
// dans le navigateur par defaut pour se connecter et choisir les
// dossiers a synchroniser (Documents, Images, Videos... ou un dossier
// personnalise via un vrai selecteur natif). Une fois connecte, tout
// se passe en tache de fond avec une icone dans la barre des taches.
//
// Le moteur de synchronisation (recursion, conflits, suppressions
// bidirectionnelles) est dans sync.rs.
// ══════════════════════════════════════════════════════════════════

use vex_sync_client::api::VexClient;
use vex_sync_client::sync;

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONFIG_FILE: &str = "vex-sync-client.json";
const LOG_FILE: &str = "vex-sync-client.log";
const POLL_REMOTE_SECS: u64 = 60;
const JOURNAL_MAX: usize = 300;

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct DossierMapping {
    nom: String,
    chemin: String,
    remote_id: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct Config {
    base_url: String,
    email: String,
    dossiers: Vec<DossierMapping>,
}

fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "vex", "vex-sync-client")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
fn config_path() -> PathBuf { config_dir().join(CONFIG_FILE) }
fn log_path() -> PathBuf { config_dir().join(LOG_FILE) }

fn charger_config() -> Config {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn sauver_config(cfg: &Config) {
    let p = config_path();
    if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(s) = serde_json::to_string_pretty(cfg) { let _ = std::fs::write(p, s); }
}

// ── Journal partage (fichier + memoire pour la page de statut) ──────
struct Etat {
    config: Config,
    journal: VecDeque<String>,
}
type EtatPartage = Arc<Mutex<Etat>>;

fn horodatage() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn journaliser(etat: &EtatPartage, msg: &str) {
    let ligne = format!("[{}] {}", horodatage(), msg);
    if let Ok(mut e) = etat.lock() {
        e.journal.push_back(ligne.clone());
        if e.journal.len() > JOURNAL_MAX { e.journal.pop_front(); }
    }
    let p = log_path();
    if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{ligne}");
    }
}

// ── Dossiers standards Windows (Documents, Images, ...) ─────────────
fn dossiers_standards() -> Vec<serde_json::Value> {
    let mut out = vec![];
    if let Some(ud) = directories::UserDirs::new() {
        let mut ajouter = |nom: &str, chemin: Option<&Path>, recommande: bool| {
            if let Some(p) = chemin {
                out.push(serde_json::json!({
                    "nom": nom, "chemin": p.to_string_lossy(), "recommande": recommande,
                }));
            }
        };
        ajouter("Documents", ud.document_dir(), true);
        ajouter("Images", ud.picture_dir(), true);
        ajouter("Videos", ud.video_dir(), false);
        ajouter("Musique", ud.audio_dir(), false);
        ajouter("Bureau", ud.desktop_dir(), false);
        ajouter("Telechargements", ud.download_dir(), false);
    }
    out
}

// ── Page de configuration locale (tiny_http) ─────────────────────────
fn reponse_html(s: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(s.to_string())
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap())
}
fn reponse_json(v: serde_json::Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(v.to_string())
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap())
}

fn json_statut(etat: &EtatPartage) -> serde_json::Value {
    let e = etat.lock().unwrap();
    serde_json::json!({
        "base_url": if e.config.base_url.is_empty() { "https://vex.hopto.org".to_string() } else { e.config.base_url.clone() },
        "email": e.config.email,
        "dossiers": e.config.dossiers.iter().map(|d| serde_json::json!({"nom": d.nom, "chemin": d.chemin})).collect::<Vec<_>>(),
    })
}

fn api_parcourir() -> serde_json::Value {
    match rfd::FileDialog::new().pick_folder() {
        Some(p) => serde_json::json!({"chemin": p.to_string_lossy()}),
        None => serde_json::json!({"chemin": null}),
    }
}

#[derive(serde::Deserialize)]
struct DossierEntree { nom: String, chemin: String }
#[derive(serde::Deserialize)]
struct EntreeConnexion { base_url: String, email: String, password: String, dossiers: Vec<DossierEntree> }

fn api_connecter(
    req: &mut tiny_http::Request,
    etat: &EtatPartage,
    tx: &Sender<(VexClient, Config)>,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let mut corps = String::new();
    if req.as_reader().read_to_string(&mut corps).is_err() {
        return reponse_json(serde_json::json!({"success": false, "error": "Corps de requete illisible."}));
    }
    let entree: EntreeConnexion = match serde_json::from_str(&corps) {
        Ok(e) => e,
        Err(_) => return reponse_json(serde_json::json!({"success": false, "error": "Requete invalide."})),
    };
    if entree.dossiers.is_empty() {
        return reponse_json(serde_json::json!({"success": false, "error": "Choisis au moins un dossier."}));
    }

    let client = match VexClient::login(&entree.base_url, &entree.email, &entree.password) {
        Ok(c) => c,
        Err(e) => return reponse_json(serde_json::json!({"success": false, "error": e})),
    };

    let dossiers_distants = match client.lister_dossier(0) {
        Ok((d, _)) => d,
        Err(e) => return reponse_json(serde_json::json!({"success": false, "error": format!("Liste distante impossible : {e}")})),
    };

    let mut mappings = Vec::new();
    for d in &entree.dossiers {
        if let Err(e) = std::fs::create_dir_all(&d.chemin) {
            return reponse_json(serde_json::json!({"success": false, "error": format!("Dossier local {} : {e}", d.chemin)}));
        }
        let remote_id = dossiers_distants.iter().find(|x| x.nom == d.nom).map(|x| x.id);
        let remote_id = match remote_id {
            Some(id) => id,
            None => match client.creer_dossier(&d.nom, 0) {
                Ok(id) => id,
                Err(e) => return reponse_json(serde_json::json!({"success": false, "error": format!("Creation du dossier distant {} : {e}", d.nom)})),
            },
        };
        mappings.push(DossierMapping { nom: d.nom.clone(), chemin: d.chemin.clone(), remote_id });
    }

    for m in &mappings { epingler_dossier_favoris(&m.chemin); }

    let config = Config { base_url: entree.base_url.clone(), email: entree.email.clone(), dossiers: mappings };
    sauver_config(&config);
    if let Ok(mut e) = etat.lock() { e.config = config.clone(); }
    let _ = tx.send((client, config));
    reponse_json(serde_json::json!({"success": true}))
}

fn lancer_serveur_local(etat: EtatPartage, tx_config: Sender<(VexClient, Config)>) -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("impossible de demarrer le serveur local");
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
    };
    let url = format!("http://127.0.0.1:{port}/");
    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let methode = req.method().as_str().to_string();
            let chemin = req.url().split('?').next().unwrap_or("").to_string();
            let reponse = match (methode.as_str(), chemin.as_str()) {
                ("GET", "/") => reponse_html(include_str!("../assets/setup.html")),
                ("GET", "/api/statut") => reponse_json(json_statut(&etat)),
                ("GET", "/api/dossiers-standards") => reponse_json(serde_json::Value::Array(dossiers_standards())),
                ("POST", "/api/parcourir") => reponse_json(api_parcourir()),
                ("POST", "/api/connecter") => api_connecter(&mut req, &etat, &tx_config),
                _ => reponse_json(serde_json::json!({"error": "introuvable"})),
            };
            let _ = req.respond(reponse);
        }
    });
    url
}

/// Construit une Command qui ne flashe jamais de fenetre console --
/// indispensable : sans ca, chaque appel a "cmd"/"powershell" depuis
/// cette appli sans console (windows_subsystem="windows") ferait
/// brievement apparaitre une fenetre noire (le processus enfant n'herite
/// d'aucune console, Windows lui en cree une par defaut).
fn commande_invisible(programme: &str) -> std::process::Command {
    let mut c = std::process::Command::new(programme);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Ouvre l'URL dans le navigateur par defaut. Passe par "explorer.exe
/// <url>" plutot que "cmd /C start" : verifie experimentalement (via
/// capture d'ecran + inspection des connexions TCP) que "cmd /C start"
/// combine a CREATE_NO_WINDOW echoue SILENCIEUSEMENT a lancer quoi que
/// ce soit (le "start" interne de cmd.exe a besoin d'une console pour
/// fonctionner). explorer.exe est un programme GUI normal : il fait le
/// meme relais vers le gestionnaire d'URL par defaut, sans avoir besoin
/// de console, donc compatible avec CREATE_NO_WINDOW.
fn ouvrir_navigateur(url: &str) {
    let _ = commande_invisible("explorer").arg(url).spawn();
}

/// Epingle un dossier aux "Acces rapides" de l'Explorateur -- pas une
/// vraie extension d'espace de noms (ça, c'est hors de portee : COM,
/// enregistrement systeme, semaines de travail C++), mais l'approche
/// realiste et sans risque : verbe Shell.Application deja utilise par
/// Explorer lui-meme pour "Epingler aux acces rapides" au clic droit.
/// Best-effort : une erreur ici n'empeche jamais la synchronisation.
fn epingler_dossier_favoris(chemin: &str) {
    let script = format!(
        "(New-Object -ComObject Shell.Application).Namespace('{}').Self.InvokeVerb('pintohome')",
        chemin.replace('\'', "''")
    );
    let _ = commande_invisible("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .status();
}

fn synchroniser_tout(client: &VexClient, config: &Config, etat: &EtatPartage) {
    for d in &config.dossiers {
        let etat2 = etat.clone();
        let log = move |msg: &str| journaliser(&etat2, msg);
        // Namespace par compte + nom : voir le commentaire de
        // synchroniser_mapping() sur le risque de baseline perimee entre
        // deux comptes differents partageant le meme nom de dossier.
        let cle_espace = format!("{}::{}", config.email, d.nom);
        sync::synchroniser_mapping(client, &cle_espace, &d.nom, Path::new(&d.chemin), d.remote_id, &log);
    }
}

enum Evenement { OuvrirPage, SyncMaintenant, Quitter }

fn main() {
    let etat: EtatPartage = Arc::new(Mutex::new(Etat { config: charger_config(), journal: VecDeque::new() }));
    let (tx_config, rx_config): (Sender<(VexClient, Config)>, Receiver<(VexClient, Config)>) = channel();
    let url_locale = lancer_serveur_local(etat.clone(), tx_config);
    journaliser(&etat, &format!("Page de configuration disponible sur {url_locale}"));

    // ── Icone dans la barre des taches (visible des le demarrage) ────
    let (tx_evt, rx_evt): (Sender<Evenement>, Receiver<Evenement>) = channel();
    let mut tray = tray_item::TrayItem::new("VEX Sync", tray_item::IconSource::Resource("")).ok();
    if let Some(t) = tray.as_mut() {
        let tx1 = tx_evt.clone();
        let _ = t.add_menu_item("Ouvrir VEX Sync", move || { let _ = tx1.send(Evenement::OuvrirPage); });
        let tx2 = tx_evt.clone();
        let _ = t.add_menu_item("Synchroniser maintenant", move || { let _ = tx2.send(Evenement::SyncMaintenant); });
        let tx3 = tx_evt.clone();
        let _ = t.add_menu_item("Quitter", move || { let _ = tx3.send(Evenement::Quitter); });
    }

    ouvrir_navigateur(&url_locale);

    let mut client: Option<VexClient> = None;
    let mut config = charger_config();
    let mut watcher: Option<notify::RecommendedWatcher> = None;
    let pending = Arc::new(Mutex::new(false));
    let mut dernier_poll = std::time::Instant::now();

    loop {
        // Connexion (initiale ou reconfiguration depuis la page web).
        if let Ok((c, cfg)) = rx_config.try_recv() {
            client = Some(c);
            config = cfg;
            let noms: Vec<String> = config.dossiers.iter().map(|d| d.nom.clone()).collect();
            journaliser(&etat, &format!("Connecte. Dossiers synchronises : {}", noms.join(", ")));

            let pending2 = pending.clone();
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok() { *pending2.lock().unwrap() = true; }
            }) {
                Ok(mut w) => {
                    use notify::Watcher;
                    for d in &config.dossiers {
                        if let Err(e) = w.watch(Path::new(&d.chemin), notify::RecursiveMode::Recursive) {
                            journaliser(&etat, &format!("Surveillance impossible pour {} : {e}", d.nom));
                        }
                    }
                    watcher = Some(w);
                }
                Err(e) => journaliser(&etat, &format!("Surveillance des dossiers indisponible : {e}")),
            }

            if let Some(cl) = &client { synchroniser_tout(cl, &config, &etat); }
            dernier_poll = std::time::Instant::now();
        }

        match rx_evt.try_recv() {
            Ok(Evenement::OuvrirPage) => ouvrir_navigateur(&url_locale),
            Ok(Evenement::SyncMaintenant) => { if let Some(cl) = &client { synchroniser_tout(cl, &config, &etat); } }
            Ok(Evenement::Quitter) => { journaliser(&etat, "Arret demande depuis la barre des taches."); break; }
            Err(_) => {}
        }

        {
            let mut p = pending.lock().unwrap();
            if *p {
                *p = false;
                drop(p);
                std::thread::sleep(Duration::from_millis(800)); // laisse le temps a l'ecriture de finir
                if let Some(cl) = &client { synchroniser_tout(cl, &config, &etat); }
            }
        }

        if client.is_some() && dernier_poll.elapsed() >= Duration::from_secs(POLL_REMOTE_SECS) {
            if let Some(cl) = &client { synchroniser_tout(cl, &config, &etat); }
            dernier_poll = std::time::Instant::now();
        }

        std::thread::sleep(Duration::from_millis(300));
    }

    let _ = watcher.take();
}
