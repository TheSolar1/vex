// ══════════════════════════════════════════════════════════════════
// i18n.rs — traductions des textes de l'interface (fenetres natives,
// menu de la barre des taches, messages). Pas de crate d'internationalisation
// externe : une vingtaine de chaines, une table statique suffit et reste
// simple a auditer/completer. Memes 10 langues que le site (voir
// src/function.rs::SUPPORTED_LANGS cote serveur).
//
// La langue est determinee au demarrage (voir `langue_courante`) : fichier
// de preference locale si deja choisie -- sinon `langue.txt` inclus dans le
// zip (voir appareil.rs::telecharger_bundle, reflete la langue du compte VEX
// au moment du telechargement) -- sinon francais. Modifiable ensuite via le
// menu de langue (icone globe) de la fenetre de connexion (voir fenetre_mdp.rs).
// ══════════════════════════════════════════════════════════════════

pub const LANGUES: &[(&str, &str)] = &[
    ("fr", "Français"),
    ("en", "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("zh", "中文"),
    ("ja", "日本語"),
    ("ar", "العربية"),
];

fn est_langue_connue(l: &str) -> bool {
    LANGUES.iter().any(|(c, _)| *c == l)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cle {
    AdresseServeur,
    MotDePasse,
    AfficherMdp,
    Connecter,
    TitreDossier,
    OuStocker,
    Parcourir,
    Continuer,
    TitreParcourirDialogue,
    StatutErreurPrefixe,
    StatutLabel,
    StatutConnecte,
    StatutEnCours,
    StatutDossierLabel,
    StatutDerniereEtapeLabel,
    TrayOuvrir,
    TrayQuitter,
    DejaInstalleTitre,
    /// Contient le marqueur litteral "{chemin}" -- a remplacer par l'appelant
    /// (voir fenetre_mdp::demander_action_installation) avec le dossier reel.
    DejaInstalleTexte,
    ActionReinstaller,
    ActionDesinstaller,
    Desinstallee,
    DejaEnCoursExecution,

    // Messages de statut/erreur pendant la synchronisation (executer_synchro
    // dans main.rs) -- affiches dans la fenetre de statut (voir
    // afficher_statut). Les gabarits contiennent des marqueurs litteraux
    // ("{build}", "{minimum}", "{url}", "{erreur}") remplaces par l'appelant.
    JournalWindowsCompatible,
    ErreurWindowsIncompatible,
    JournalWindowsVerifImpossible,
    JournalRechercheServeur,
    ErreurServeurInjoignable,
    JournalServeurTrouve,
    JournalAppareilDejaAutorise,
    JournalAucunAppareilAutorise,
    ErreurAutorisationEchouee,
    JournalAppareilAutorise,
    ErreurDossierLocal,
    ErreurCheminInvalide,
    ErreurEnregistrementRacine,
    JournalRacineEnregistree,
    JournalRacineDejaEnregistree,
    JournalMarquageFichiers,
    ErreurConnexionCloudFilter,
    ErreurInterneInattendue,
}

pub fn t(langue: &str, cle: Cle) -> &'static str {
    use Cle::*;
    match (langue, cle) {
        // ────────────────────────────── Français (aussi le repli) ──
        (_, AdresseServeur) if langue == "fr" || !est_langue_connue(langue) => "Adresse du serveur :",
        (_, MotDePasse) if langue == "fr" || !est_langue_connue(langue) => "Mot de passe VEX :",
        (_, AfficherMdp) if langue == "fr" || !est_langue_connue(langue) => "Afficher le mot de passe",
        (_, Connecter) if langue == "fr" || !est_langue_connue(langue) => "Connecter",
        (_, TitreDossier) if langue == "fr" || !est_langue_connue(langue) => "Dossier de synchronisation",
        (_, OuStocker) if langue == "fr" || !est_langue_connue(langue) => "Où stocker tes fichiers VEX :",
        (_, Parcourir) if langue == "fr" || !est_langue_connue(langue) => "Parcourir...",
        (_, Continuer) if langue == "fr" || !est_langue_connue(langue) => "Continuer",
        (_, TitreParcourirDialogue) if langue == "fr" || !est_langue_connue(langue) => {
            "Choisis le dossier de synchronisation VEX"
        }
        (_, StatutErreurPrefixe) if langue == "fr" || !est_langue_connue(langue) => "Erreur : ",
        (_, StatutLabel) if langue == "fr" || !est_langue_connue(langue) => "Statut : ",
        (_, StatutConnecte) if langue == "fr" || !est_langue_connue(langue) => "Connecté",
        (_, StatutEnCours) if langue == "fr" || !est_langue_connue(langue) => "Connexion en cours...",
        (_, StatutDossierLabel) if langue == "fr" || !est_langue_connue(langue) => "Dossier synchronisé : ",
        (_, StatutDerniereEtapeLabel) if langue == "fr" || !est_langue_connue(langue) => "Dernière étape : ",
        (_, TrayOuvrir) if langue == "fr" || !est_langue_connue(langue) => "Ouvrir VEX Cloud Client",
        (_, TrayQuitter) if langue == "fr" || !est_langue_connue(langue) => "Quitter",
        (_, DejaInstalleTitre) if langue == "fr" || !est_langue_connue(langue) => "Déjà installé",
        (_, DejaInstalleTexte) if langue == "fr" || !est_langue_connue(langue) => {
            "VEX Cloud Sync est déjà configuré sur cet ordinateur.\r\n\
             Dossier synchronisé actuel : {chemin}\r\n\r\n\
             • Réinstaller : choisir un nouveau dossier et se reconnecter.\r\n\
             • Désinstaller : retirer la synchro et la configuration (tes fichiers ne sont pas supprimés).\r\n\r\n\
             Ferme cette fenêtre (croix) pour quitter, sans rien changer."
        }
        (_, ActionReinstaller) if langue == "fr" || !est_langue_connue(langue) => "Réinstaller",
        (_, ActionDesinstaller) if langue == "fr" || !est_langue_connue(langue) => "Désinstaller",
        (_, Desinstallee) if langue == "fr" || !est_langue_connue(langue) => {
            "Désinstallation terminée.\n\nTes fichiers synchronisés n'ont pas été supprimés -- tu peux les garder, les déplacer ou les supprimer toi-même. Tu peux également supprimer vex-cloudsync.exe si tu ne comptes plus l'utiliser."
        }
        (_, DejaEnCoursExecution) if langue == "fr" || !est_langue_connue(langue) => {
            "VEX Cloud Client est déjà lancé (regarde la barre des tâches, en bas à droite) -- une seule instance à la fois peut se connecter à la synchronisation."
        }
        (_, JournalWindowsCompatible) if langue == "fr" || !est_langue_connue(langue) => "Windows build {build} : compatible.",
        (_, ErreurWindowsIncompatible) if langue == "fr" || !est_langue_connue(langue) => {
            "Windows build {build} détecté -- l'API Cloud Files nécessite au moins le build {minimum} (Windows 10 version 1709 ou plus récent)."
        }
        (_, JournalWindowsVerifImpossible) if langue == "fr" || !est_langue_connue(langue) => {
            "Avertissement : vérification de version Windows impossible ({erreur}) -- on continue quand même."
        }
        (_, JournalRechercheServeur) if langue == "fr" || !est_langue_connue(langue) => "Recherche du serveur VEX...",
        (_, ErreurServeurInjoignable) if langue == "fr" || !est_langue_connue(langue) => {
            "Impossible de joindre le serveur VEX (aucune adresse connue ne répond -- vérifie ta connexion)."
        }
        (_, JournalServeurTrouve) if langue == "fr" || !est_langue_connue(langue) => "Serveur trouvé : {url}",
        (_, JournalAppareilDejaAutorise) if langue == "fr" || !est_langue_connue(langue) => {
            "Appareil déjà autorisé, réutilisation du jeton local."
        }
        (_, JournalAucunAppareilAutorise) if langue == "fr" || !est_langue_connue(langue) => {
            "Aucun appareil autorisé -- vérifie ton navigateur pour autoriser cet appareil..."
        }
        (_, ErreurAutorisationEchouee) if langue == "fr" || !est_langue_connue(langue) => "Autorisation échouée : {erreur}",
        (_, JournalAppareilAutorise) if langue == "fr" || !est_langue_connue(langue) => "Appareil autorisé.",
        (_, ErreurDossierLocal) if langue == "fr" || !est_langue_connue(langue) => "Impossible de créer le dossier local : {erreur}",
        (_, ErreurCheminInvalide) if langue == "fr" || !est_langue_connue(langue) => "Chemin de synchro invalide : {erreur}",
        (_, ErreurEnregistrementRacine) if langue == "fr" || !est_langue_connue(langue) => {
            "Échec d'enregistrement de la racine de synchro : {erreur}"
        }
        (_, JournalRacineEnregistree) if langue == "fr" || !est_langue_connue(langue) => "Racine de synchro enregistrée.",
        (_, JournalRacineDejaEnregistree) if langue == "fr" || !est_langue_connue(langue) => "Racine de synchro déjà enregistrée.",
        (_, JournalMarquageFichiers) if langue == "fr" || !est_langue_connue(langue) => {
            "Marquage des fichiers locaux déjà présents comme synchronisés..."
        }
        (_, ErreurConnexionCloudFilter) if langue == "fr" || !est_langue_connue(langue) => {
            "Échec de connexion de la session Cloud Filter : {erreur}"
        }
        (_, ErreurInterneInattendue) if langue == "fr" || !est_langue_connue(langue) => {
            "Erreur interne inattendue -- relance l'application."
        }

        // ────────────────────────────────────────────── English ──
        ("en", AdresseServeur) => "Server address:",
        ("en", MotDePasse) => "VEX password:",
        ("en", AfficherMdp) => "Show password",
        ("en", Connecter) => "Connect",
        ("en", TitreDossier) => "Sync folder",
        ("en", OuStocker) => "Where to store your VEX files:",
        ("en", Parcourir) => "Browse...",
        ("en", Continuer) => "Continue",
        ("en", TitreParcourirDialogue) => "Choose the VEX sync folder",
        ("en", StatutErreurPrefixe) => "Error: ",
        ("en", StatutLabel) => "Status: ",
        ("en", StatutConnecte) => "Connected",
        ("en", StatutEnCours) => "Connecting...",
        ("en", StatutDossierLabel) => "Synced folder: ",
        ("en", StatutDerniereEtapeLabel) => "Last step: ",
        ("en", TrayOuvrir) => "Open VEX Cloud Client",
        ("en", TrayQuitter) => "Quit",
        ("en", DejaInstalleTitre) => "Already installed",
        ("en", DejaInstalleTexte) => {
            "VEX Cloud Sync is already set up on this computer.\r\n\
             Current synced folder: {chemin}\r\n\r\n\
             • Reinstall: choose a new folder and reconnect.\r\n\
             • Uninstall: remove sync and configuration (your files are not deleted).\r\n\r\n\
             Close this window (X) to quit, without changing anything."
        }
        ("en", ActionReinstaller) => "Reinstall",
        ("en", ActionDesinstaller) => "Uninstall",
        ("en", Desinstallee) => {
            "Uninstall complete.\n\nYour synced files were not deleted -- you can keep, move, or delete them yourself. You can also delete vex-cloudsync.exe if you no longer plan to use it."
        }
        ("en", JournalWindowsCompatible) => "Windows build {build}: compatible.",
        ("en", ErreurWindowsIncompatible) => {
            "Windows build {build} detected -- the Cloud Files API requires at least build {minimum} (Windows 10 version 1709 or later)."
        }
        ("en", JournalWindowsVerifImpossible) => "Warning: could not check Windows version ({erreur}) -- continuing anyway.",
        ("en", JournalRechercheServeur) => "Looking for the VEX server...",
        ("en", ErreurServeurInjoignable) => "Could not reach the VEX server (no known address responded -- check your connection).",
        ("en", JournalServeurTrouve) => "Server found: {url}",
        ("en", JournalAppareilDejaAutorise) => "Device already authorized, reusing local token.",
        ("en", JournalAucunAppareilAutorise) => "No authorized device -- check your browser to authorize this device...",
        ("en", ErreurAutorisationEchouee) => "Authorization failed: {erreur}",
        ("en", JournalAppareilAutorise) => "Device authorized.",
        ("en", ErreurDossierLocal) => "Could not create the local folder: {erreur}",
        ("en", ErreurCheminInvalide) => "Invalid sync path: {erreur}",
        ("en", ErreurEnregistrementRacine) => "Failed to register the sync root: {erreur}",
        ("en", JournalRacineEnregistree) => "Sync root registered.",
        ("en", JournalRacineDejaEnregistree) => "Sync root already registered.",
        ("en", JournalMarquageFichiers) => "Marking existing local files as synced...",
        ("en", ErreurConnexionCloudFilter) => "Failed to connect the Cloud Filter session: {erreur}",
        ("en", ErreurInterneInattendue) => "Unexpected internal error -- restart the app.",

        // ────────────────────────────────────────────── Español ──
        ("es", AdresseServeur) => "Direccion del servidor:",
        ("es", MotDePasse) => "Contrasena VEX:",
        ("es", AfficherMdp) => "Mostrar contrasena",
        ("es", Connecter) => "Conectar",
        ("es", TitreDossier) => "Carpeta de sincronizacion",
        ("es", OuStocker) => "Donde guardar tus archivos VEX:",
        ("es", Parcourir) => "Examinar...",
        ("es", Continuer) => "Continuar",
        ("es", TitreParcourirDialogue) => "Elige la carpeta de sincronizacion de VEX",
        ("es", StatutErreurPrefixe) => "Error: ",
        ("es", StatutLabel) => "Estado: ",
        ("es", StatutConnecte) => "Conectado",
        ("es", StatutEnCours) => "Conectando...",
        ("es", StatutDossierLabel) => "Carpeta sincronizada: ",
        ("es", StatutDerniereEtapeLabel) => "Ultimo paso: ",
        ("es", TrayOuvrir) => "Abrir VEX Cloud Client",
        ("es", TrayQuitter) => "Salir",
        ("es", DejaInstalleTitre) => "Ya instalado",
        ("es", DejaInstalleTexte) => {
            "VEX Cloud Sync ya esta configurado en este ordenador.\r\n\
             Carpeta sincronizada actual: {chemin}\r\n\r\n\
             • Reinstalar: elegir una nueva carpeta y reconectar.\r\n\
             • Desinstalar: quitar la sincronizacion y la configuracion (tus archivos no se eliminan).\r\n\r\n\
             Cierra esta ventana (X) para salir, sin cambiar nada."
        }
        ("es", ActionReinstaller) => "Reinstalar",
        ("es", ActionDesinstaller) => "Desinstalar",
        ("es", Desinstallee) => {
            "Desinstalacion completada.\n\nTus archivos sincronizados no se han eliminado -- puedes conservarlos, moverlos o borrarlos tu mismo. Tambien puedes eliminar vex-cloudsync.exe si ya no lo vas a usar."
        }
        ("es", JournalWindowsCompatible) => "Compilacion de Windows {build}: compatible.",
        ("es", ErreurWindowsIncompatible) => {
            "Compilacion de Windows {build} detectada -- la API Cloud Files requiere al menos la compilacion {minimum} (Windows 10 version 1709 o posterior)."
        }
        ("es", JournalWindowsVerifImpossible) => "Aviso: no se pudo comprobar la version de Windows ({erreur}) -- se continua de todos modos.",
        ("es", JournalRechercheServeur) => "Buscando el servidor VEX...",
        ("es", ErreurServeurInjoignable) => "No se pudo contactar con el servidor VEX (ninguna direccion conocida responde -- comprueba tu conexion).",
        ("es", JournalServeurTrouve) => "Servidor encontrado: {url}",
        ("es", JournalAppareilDejaAutorise) => "Dispositivo ya autorizado, reutilizando el token local.",
        ("es", JournalAucunAppareilAutorise) => "Ningun dispositivo autorizado -- revisa tu navegador para autorizar este dispositivo...",
        ("es", ErreurAutorisationEchouee) => "Autorizacion fallida: {erreur}",
        ("es", JournalAppareilAutorise) => "Dispositivo autorizado.",
        ("es", ErreurDossierLocal) => "No se pudo crear la carpeta local: {erreur}",
        ("es", ErreurCheminInvalide) => "Ruta de sincronizacion no valida: {erreur}",
        ("es", ErreurEnregistrementRacine) => "Error al registrar la raiz de sincronizacion: {erreur}",
        ("es", JournalRacineEnregistree) => "Raiz de sincronizacion registrada.",
        ("es", JournalRacineDejaEnregistree) => "Raiz de sincronizacion ya registrada.",
        ("es", JournalMarquageFichiers) => "Marcando los archivos locales ya existentes como sincronizados...",
        ("es", ErreurConnexionCloudFilter) => "Error al conectar la sesion de Cloud Filter: {erreur}",
        ("es", ErreurInterneInattendue) => "Error interno inesperado -- reinicia la aplicacion.",

        // ─────────────────────────────────────────────── Deutsch ──
        ("de", AdresseServeur) => "Serveradresse:",
        ("de", MotDePasse) => "VEX-Passwort:",
        ("de", AfficherMdp) => "Passwort anzeigen",
        ("de", Connecter) => "Verbinden",
        ("de", TitreDossier) => "Synchronisierungsordner",
        ("de", OuStocker) => "Wo deine VEX-Dateien speichern:",
        ("de", Parcourir) => "Durchsuchen...",
        ("de", Continuer) => "Weiter",
        ("de", TitreParcourirDialogue) => "Waehle den VEX-Synchronisierungsordner",
        ("de", StatutErreurPrefixe) => "Fehler: ",
        ("de", StatutLabel) => "Status: ",
        ("de", StatutConnecte) => "Verbunden",
        ("de", StatutEnCours) => "Verbindung wird hergestellt...",
        ("de", StatutDossierLabel) => "Synchronisierter Ordner: ",
        ("de", StatutDerniereEtapeLabel) => "Letzter Schritt: ",
        ("de", TrayOuvrir) => "VEX Cloud Client oeffnen",
        ("de", TrayQuitter) => "Beenden",
        ("de", DejaInstalleTitre) => "Bereits installiert",
        ("de", DejaInstalleTexte) => {
            "VEX Cloud Sync ist auf diesem Computer bereits eingerichtet.\r\n\
             Aktueller synchronisierter Ordner: {chemin}\r\n\r\n\
             • Neu installieren: neuen Ordner waehlen und neu verbinden.\r\n\
             • Deinstallieren: Synchronisierung und Konfiguration entfernen (deine Dateien werden nicht geloescht).\r\n\r\n\
             Schliesse dieses Fenster (X), um zu beenden, ohne etwas zu aendern."
        }
        ("de", ActionReinstaller) => "Neu installieren",
        ("de", ActionDesinstaller) => "Deinstallieren",
        ("de", Desinstallee) => {
            "Deinstallation abgeschlossen.\n\nDeine synchronisierten Dateien wurden nicht geloescht -- du kannst sie behalten, verschieben oder selbst loeschen. Du kannst auch vex-cloudsync.exe loeschen, wenn du es nicht mehr benutzen willst."
        }
        ("de", JournalWindowsCompatible) => "Windows-Build {build}: kompatibel.",
        ("de", ErreurWindowsIncompatible) => {
            "Windows-Build {build} erkannt -- die Cloud Files API benoetigt mindestens Build {minimum} (Windows 10 Version 1709 oder neuer)."
        }
        ("de", JournalWindowsVerifImpossible) => "Warnung: Windows-Version konnte nicht gepruft werden ({erreur}) -- wird trotzdem fortgesetzt.",
        ("de", JournalRechercheServeur) => "Suche nach dem VEX-Server...",
        ("de", ErreurServeurInjoignable) => "VEX-Server nicht erreichbar (keine bekannte Adresse antwortet -- prufe deine Verbindung).",
        ("de", JournalServeurTrouve) => "Server gefunden: {url}",
        ("de", JournalAppareilDejaAutorise) => "Geraet bereits autorisiert, lokales Token wird wiederverwendet.",
        ("de", JournalAucunAppareilAutorise) => "Kein autorisiertes Geraet -- prufe deinen Browser, um dieses Geraet zu autorisieren...",
        ("de", ErreurAutorisationEchouee) => "Autorisierung fehlgeschlagen: {erreur}",
        ("de", JournalAppareilAutorise) => "Geraet autorisiert.",
        ("de", ErreurDossierLocal) => "Lokaler Ordner konnte nicht erstellt werden: {erreur}",
        ("de", ErreurCheminInvalide) => "Ungueltiger Synchronisierungspfad: {erreur}",
        ("de", ErreurEnregistrementRacine) => "Registrierung der Synchronisierungswurzel fehlgeschlagen: {erreur}",
        ("de", JournalRacineEnregistree) => "Synchronisierungswurzel registriert.",
        ("de", JournalRacineDejaEnregistree) => "Synchronisierungswurzel bereits registriert.",
        ("de", JournalMarquageFichiers) => "Vorhandene lokale Dateien werden als synchronisiert markiert...",
        ("de", ErreurConnexionCloudFilter) => "Verbindung der Cloud Filter-Sitzung fehlgeschlagen: {erreur}",
        ("de", ErreurInterneInattendue) => "Unerwarteter interner Fehler -- Anwendung neu starten.",

        // ─────────────────────────────────────────────── Italiano ──
        ("it", AdresseServeur) => "Indirizzo del server:",
        ("it", MotDePasse) => "Password VEX:",
        ("it", AfficherMdp) => "Mostra password",
        ("it", Connecter) => "Connetti",
        ("it", TitreDossier) => "Cartella di sincronizzazione",
        ("it", OuStocker) => "Dove salvare i tuoi file VEX:",
        ("it", Parcourir) => "Sfoglia...",
        ("it", Continuer) => "Continua",
        ("it", TitreParcourirDialogue) => "Scegli la cartella di sincronizzazione VEX",
        ("it", StatutErreurPrefixe) => "Errore: ",
        ("it", StatutLabel) => "Stato: ",
        ("it", StatutConnecte) => "Connesso",
        ("it", StatutEnCours) => "Connessione in corso...",
        ("it", StatutDossierLabel) => "Cartella sincronizzata: ",
        ("it", StatutDerniereEtapeLabel) => "Ultimo passaggio: ",
        ("it", TrayOuvrir) => "Apri VEX Cloud Client",
        ("it", TrayQuitter) => "Esci",
        ("it", DejaInstalleTitre) => "Già installato",
        ("it", DejaInstalleTexte) => {
            "VEX Cloud Sync e gia configurato su questo computer.\r\n\
             Cartella sincronizzata attuale: {chemin}\r\n\r\n\
             • Reinstalla: scegliere una nuova cartella e riconnettersi.\r\n\
             • Disinstalla: rimuovere sincronizzazione e configurazione (i tuoi file non vengono eliminati).\r\n\r\n\
             Chiudi questa finestra (X) per uscire, senza cambiare nulla."
        }
        ("it", ActionReinstaller) => "Reinstalla",
        ("it", ActionDesinstaller) => "Disinstalla",
        ("it", Desinstallee) => {
            "Disinstallazione completata.\n\nI tuoi file sincronizzati non sono stati eliminati -- puoi tenerli, spostarli o eliminarli tu stesso. Puoi anche eliminare vex-cloudsync.exe se non intendi piu usarlo."
        }
        ("it", JournalWindowsCompatible) => "Build di Windows {build}: compatibile.",
        ("it", ErreurWindowsIncompatible) => {
            "Build di Windows {build} rilevata -- l'API Cloud Files richiede almeno la build {minimum} (Windows 10 versione 1709 o successiva)."
        }
        ("it", JournalWindowsVerifImpossible) => "Avviso: impossibile verificare la versione di Windows ({erreur}) -- si continua comunque.",
        ("it", JournalRechercheServeur) => "Ricerca del server VEX...",
        ("it", ErreurServeurInjoignable) => "Impossibile raggiungere il server VEX (nessun indirizzo noto risponde -- controlla la connessione).",
        ("it", JournalServeurTrouve) => "Server trovato: {url}",
        ("it", JournalAppareilDejaAutorise) => "Dispositivo gia autorizzato, riutilizzo del token locale.",
        ("it", JournalAucunAppareilAutorise) => "Nessun dispositivo autorizzato -- controlla il browser per autorizzare questo dispositivo...",
        ("it", ErreurAutorisationEchouee) => "Autorizzazione fallita: {erreur}",
        ("it", JournalAppareilAutorise) => "Dispositivo autorizzato.",
        ("it", ErreurDossierLocal) => "Impossibile creare la cartella locale: {erreur}",
        ("it", ErreurCheminInvalide) => "Percorso di sincronizzazione non valido: {erreur}",
        ("it", ErreurEnregistrementRacine) => "Registrazione della radice di sincronizzazione fallita: {erreur}",
        ("it", JournalRacineEnregistree) => "Radice di sincronizzazione registrata.",
        ("it", JournalRacineDejaEnregistree) => "Radice di sincronizzazione gia registrata.",
        ("it", JournalMarquageFichiers) => "Contrassegno dei file locali gia presenti come sincronizzati...",
        ("it", ErreurConnexionCloudFilter) => "Connessione della sessione Cloud Filter fallita: {erreur}",
        ("it", ErreurInterneInattendue) => "Errore interno imprevisto -- riavvia l'applicazione.",

        // ──────────────────────────────────────────── Português ──
        ("pt", AdresseServeur) => "Endereco do servidor:",
        ("pt", MotDePasse) => "Palavra-passe VEX:",
        ("pt", AfficherMdp) => "Mostrar palavra-passe",
        ("pt", Connecter) => "Ligar",
        ("pt", TitreDossier) => "Pasta de sincronizacao",
        ("pt", OuStocker) => "Onde guardar os teus ficheiros VEX:",
        ("pt", Parcourir) => "Procurar...",
        ("pt", Continuer) => "Continuar",
        ("pt", TitreParcourirDialogue) => "Escolhe a pasta de sincronizacao VEX",
        ("pt", StatutErreurPrefixe) => "Erro: ",
        ("pt", StatutLabel) => "Estado: ",
        ("pt", StatutConnecte) => "Ligado",
        ("pt", StatutEnCours) => "A ligar...",
        ("pt", StatutDossierLabel) => "Pasta sincronizada: ",
        ("pt", StatutDerniereEtapeLabel) => "Ultimo passo: ",
        ("pt", TrayOuvrir) => "Abrir VEX Cloud Client",
        ("pt", TrayQuitter) => "Sair",
        ("pt", DejaInstalleTitre) => "Já instalado",
        ("pt", DejaInstalleTexte) => {
            "O VEX Cloud Sync ja esta configurado neste computador.\r\n\
             Pasta sincronizada atual: {chemin}\r\n\r\n\
             • Reinstalar: escolher uma nova pasta e ligar novamente.\r\n\
             • Desinstalar: remover a sincronizacao e a configuracao (os teus ficheiros nao sao eliminados).\r\n\r\n\
             Fecha esta janela (X) para sair, sem mudar nada."
        }
        ("pt", ActionReinstaller) => "Reinstalar",
        ("pt", ActionDesinstaller) => "Desinstalar",
        ("pt", Desinstallee) => {
            "Desinstalacao concluida.\n\nOs teus ficheiros sincronizados nao foram eliminados -- podes guarda-los, move-los ou elimina-los tu mesmo. Tambem podes eliminar o vex-cloudsync.exe se ja nao o fores usar."
        }
        ("pt", JournalWindowsCompatible) => "Build do Windows {build}: compativel.",
        ("pt", ErreurWindowsIncompatible) => {
            "Build do Windows {build} detetada -- a API Cloud Files requer pelo menos a build {minimum} (Windows 10 versao 1709 ou posterior)."
        }
        ("pt", JournalWindowsVerifImpossible) => "Aviso: nao foi possivel verificar a versao do Windows ({erreur}) -- a continuar mesmo assim.",
        ("pt", JournalRechercheServeur) => "A procurar o servidor VEX...",
        ("pt", ErreurServeurInjoignable) => "Nao foi possivel contactar o servidor VEX (nenhum endereco conhecido respondeu -- verifica a tua ligacao).",
        ("pt", JournalServeurTrouve) => "Servidor encontrado: {url}",
        ("pt", JournalAppareilDejaAutorise) => "Dispositivo ja autorizado, a reutilizar o token local.",
        ("pt", JournalAucunAppareilAutorise) => "Nenhum dispositivo autorizado -- verifica o teu navegador para autorizar este dispositivo...",
        ("pt", ErreurAutorisationEchouee) => "Autorizacao falhou: {erreur}",
        ("pt", JournalAppareilAutorise) => "Dispositivo autorizado.",
        ("pt", ErreurDossierLocal) => "Nao foi possivel criar a pasta local: {erreur}",
        ("pt", ErreurCheminInvalide) => "Caminho de sincronizacao invalido: {erreur}",
        ("pt", ErreurEnregistrementRacine) => "Falha ao registar a raiz de sincronizacao: {erreur}",
        ("pt", JournalRacineEnregistree) => "Raiz de sincronizacao registada.",
        ("pt", JournalRacineDejaEnregistree) => "Raiz de sincronizacao ja registada.",
        ("pt", JournalMarquageFichiers) => "A marcar os ficheiros locais ja existentes como sincronizados...",
        ("pt", ErreurConnexionCloudFilter) => "Falha ao ligar a sessao Cloud Filter: {erreur}",
        ("pt", ErreurInterneInattendue) => "Erro interno inesperado -- reinicia a aplicacao.",

        // ──────────────────────────────────────────────── Русский ──
        ("ru", AdresseServeur) => "Адрес сервера:",
        ("ru", MotDePasse) => "Пароль VEX:",
        ("ru", AfficherMdp) => "Показать пароль",
        ("ru", Connecter) => "Подключиться",
        ("ru", TitreDossier) => "Папка синхронизации",
        ("ru", OuStocker) => "Куда сохранить файлы VEX:",
        ("ru", Parcourir) => "Обзор...",
        ("ru", Continuer) => "Продолжить",
        ("ru", TitreParcourirDialogue) => "Выберите папку синхронизации VEX",
        ("ru", StatutErreurPrefixe) => "Ошибка: ",
        ("ru", StatutLabel) => "Статус: ",
        ("ru", StatutConnecte) => "Подключено",
        ("ru", StatutEnCours) => "Подключение...",
        ("ru", StatutDossierLabel) => "Синхронизированная папка: ",
        ("ru", StatutDerniereEtapeLabel) => "Последний шаг: ",
        ("ru", TrayOuvrir) => "Открыть VEX Cloud Client",
        ("ru", TrayQuitter) => "Выход",
        ("ru", DejaInstalleTitre) => "Уже установлено",
        ("ru", DejaInstalleTexte) => {
            "VEX Cloud Sync уже настроен на этом компьютере.\r\n\
             Текущая синхронизированная папка: {chemin}\r\n\r\n\
             • Переустановить: выбрать новую папку и подключиться заново.\r\n\
             • Удалить: убрать синхронизацию и настройки (ваши файлы не удаляются).\r\n\r\n\
             Закройте это окно (X), чтобы выйти, ничего не меняя."
        }
        ("ru", ActionReinstaller) => "Переустановить",
        ("ru", ActionDesinstaller) => "Удалить",
        ("ru", Desinstallee) => {
            "Удаление завершено.\n\nВаши синхронизированные файлы не были удалены -- вы можете оставить, переместить или удалить их сами. Вы также можете удалить vex-cloudsync.exe, если больше не планируете его использовать."
        }
        ("ru", JournalWindowsCompatible) => "Сборка Windows {build}: совместима.",
        ("ru", ErreurWindowsIncompatible) => {
            "Обнаружена сборка Windows {build} -- для API Cloud Files требуется как минимум сборка {minimum} (Windows 10 версии 1709 или новее)."
        }
        ("ru", JournalWindowsVerifImpossible) => "Предупреждение: не удалось проверить версию Windows ({erreur}) -- продолжаем в любом случае.",
        ("ru", JournalRechercheServeur) => "Поиск сервера VEX...",
        ("ru", ErreurServeurInjoignable) => "Не удалось подключиться к серверу VEX (ни один известный адрес не отвечает -- проверьте подключение).",
        ("ru", JournalServeurTrouve) => "Сервер найден: {url}",
        ("ru", JournalAppareilDejaAutorise) => "Устройство уже авторизовано, используется локальный токен.",
        ("ru", JournalAucunAppareilAutorise) => "Нет авторизованного устройства -- проверьте браузер, чтобы авторизовать это устройство...",
        ("ru", ErreurAutorisationEchouee) => "Ошибка авторизации: {erreur}",
        ("ru", JournalAppareilAutorise) => "Устройство авторизовано.",
        ("ru", ErreurDossierLocal) => "Не удалось создать локальную папку: {erreur}",
        ("ru", ErreurCheminInvalide) => "Некорректный путь синхронизации: {erreur}",
        ("ru", ErreurEnregistrementRacine) => "Не удалось зарегистрировать корень синхронизации: {erreur}",
        ("ru", JournalRacineEnregistree) => "Корень синхронизации зарегистрирован.",
        ("ru", JournalRacineDejaEnregistree) => "Корень синхронизации уже зарегистрирован.",
        ("ru", JournalMarquageFichiers) => "Пометка уже имеющихся локальных файлов как синхронизированных...",
        ("ru", ErreurConnexionCloudFilter) => "Не удалось подключить сессию Cloud Filter: {erreur}",
        ("ru", ErreurInterneInattendue) => "Непредвиденная внутренняя ошибка -- перезапустите приложение.",

        // ────────────────────────────────────────────────── 中文 ──
        ("zh", AdresseServeur) => "服务器地址：",
        ("zh", MotDePasse) => "VEX 密码：",
        ("zh", AfficherMdp) => "显示密码",
        ("zh", Connecter) => "连接",
        ("zh", TitreDossier) => "同步文件夹",
        ("zh", OuStocker) => "选择存放 VEX 文件的位置：",
        ("zh", Parcourir) => "浏览...",
        ("zh", Continuer) => "继续",
        ("zh", TitreParcourirDialogue) => "选择 VEX 同步文件夹",
        ("zh", StatutErreurPrefixe) => "错误：",
        ("zh", StatutLabel) => "状态：",
        ("zh", StatutConnecte) => "已连接",
        ("zh", StatutEnCours) => "正在连接...",
        ("zh", StatutDossierLabel) => "已同步文件夹：",
        ("zh", StatutDerniereEtapeLabel) => "最近一步：",
        ("zh", TrayOuvrir) => "打开 VEX Cloud Client",
        ("zh", TrayQuitter) => "退出",
        ("zh", DejaInstalleTitre) => "已安装",
        ("zh", DejaInstalleTexte) => {
            "此电脑上已经配置过 VEX Cloud Sync。\r\n\
             当前同步文件夹：{chemin}\r\n\r\n\
             • 重新安装：选择新文件夹并重新连接。\r\n\
             • 卸载：移除同步和配置（不会删除你的文件）。\r\n\r\n\
             关闭此窗口（X）即可退出，不做任何更改。"
        }
        ("zh", ActionReinstaller) => "重新安装",
        ("zh", ActionDesinstaller) => "卸载",
        ("zh", Desinstallee) => {
            "卸载完成。\n\n你的同步文件并未被删除——你可以自行保留、移动或删除它们。如果不再需要，也可以删除 vex-cloudsync.exe。"
        }
        ("zh", JournalWindowsCompatible) => "Windows 内部版本 {build}：兼容。",
        ("zh", ErreurWindowsIncompatible) => {
            "检测到 Windows 内部版本 {build} —— Cloud Files API 至少需要版本 {minimum}（Windows 10 版本 1709 或更高）。"
        }
        ("zh", JournalWindowsVerifImpossible) => "警告：无法检查 Windows 版本（{erreur}）—— 仍将继续。",
        ("zh", JournalRechercheServeur) => "正在查找 VEX 服务器...",
        ("zh", ErreurServeurInjoignable) => "无法连接到 VEX 服务器（没有已知地址响应 —— 请检查你的网络连接）。",
        ("zh", JournalServeurTrouve) => "已找到服务器：{url}",
        ("zh", JournalAppareilDejaAutorise) => "设备已授权，正在复用本地令牌。",
        ("zh", JournalAucunAppareilAutorise) => "没有已授权的设备 —— 请在浏览器中授权此设备...",
        ("zh", ErreurAutorisationEchouee) => "授权失败：{erreur}",
        ("zh", JournalAppareilAutorise) => "设备已授权。",
        ("zh", ErreurDossierLocal) => "无法创建本地文件夹：{erreur}",
        ("zh", ErreurCheminInvalide) => "同步路径无效：{erreur}",
        ("zh", ErreurEnregistrementRacine) => "注册同步根目录失败：{erreur}",
        ("zh", JournalRacineEnregistree) => "同步根目录已注册。",
        ("zh", JournalRacineDejaEnregistree) => "同步根目录已存在注册。",
        ("zh", JournalMarquageFichiers) => "正在将已存在的本地文件标记为已同步...",
        ("zh", ErreurConnexionCloudFilter) => "连接 Cloud Filter 会话失败：{erreur}",
        ("zh", ErreurInterneInattendue) => "意外的内部错误 —— 请重启应用。",

        // ────────────────────────────────────────────────── 日本語 ──
        ("ja", AdresseServeur) => "サーバーアドレス：",
        ("ja", MotDePasse) => "VEX パスワード：",
        ("ja", AfficherMdp) => "パスワードを表示",
        ("ja", Connecter) => "接続",
        ("ja", TitreDossier) => "同期フォルダ",
        ("ja", OuStocker) => "VEX ファイルの保存先：",
        ("ja", Parcourir) => "参照...",
        ("ja", Continuer) => "続ける",
        ("ja", TitreParcourirDialogue) => "VEX の同期フォルダを選択",
        ("ja", StatutErreurPrefixe) => "エラー：",
        ("ja", StatutLabel) => "状態：",
        ("ja", StatutConnecte) => "接続済み",
        ("ja", StatutEnCours) => "接続中...",
        ("ja", StatutDossierLabel) => "同期フォルダ：",
        ("ja", StatutDerniereEtapeLabel) => "最新のステップ：",
        ("ja", TrayOuvrir) => "VEX Cloud Client を開く",
        ("ja", TrayQuitter) => "終了",
        ("ja", DejaInstalleTitre) => "インストール済み",
        ("ja", DejaInstalleTexte) => {
            "このパソコンには VEX Cloud Sync がすでに設定されています。\r\n\
             現在の同期フォルダ：{chemin}\r\n\r\n\
             • 再インストール：新しいフォルダを選び、再接続する。\r\n\
             • アンインストール：同期と設定を削除する（ファイルは削除されません）。\r\n\r\n\
             このウィンドウ（×）を閉じると、何も変更せず終了します。"
        }
        ("ja", ActionReinstaller) => "再インストール",
        ("ja", ActionDesinstaller) => "アンインストール",
        ("ja", Desinstallee) => {
            "アンインストールが完了しました。\n\n同期されていたファイルは削除されていません -- そのまま残す、移動する、削除するのは自由です。今後使わないなら vex-cloudsync.exe を削除しても構いません。"
        }
        ("ja", JournalWindowsCompatible) => "Windows ビルド {build}：互換性あり。",
        ("ja", ErreurWindowsIncompatible) => {
            "Windows ビルド {build} を検出しました —— Cloud Files API には少なくともビルド {minimum}（Windows 10 バージョン 1709 以降）が必要です。"
        }
        ("ja", JournalWindowsVerifImpossible) => "警告：Windows のバージョンを確認できませんでした（{erreur}）—— そのまま続行します。",
        ("ja", JournalRechercheServeur) => "VEX サーバーを検索中...",
        ("ja", ErreurServeurInjoignable) => "VEX サーバーに接続できません（既知のアドレスが応答しません —— 接続を確認してください）。",
        ("ja", JournalServeurTrouve) => "サーバーが見つかりました：{url}",
        ("ja", JournalAppareilDejaAutorise) => "デバイスはすでに認証済みです。ローカルのトークンを再利用します。",
        ("ja", JournalAucunAppareilAutorise) => "認証済みのデバイスがありません —— ブラウザでこのデバイスを認証してください...",
        ("ja", ErreurAutorisationEchouee) => "認証に失敗しました：{erreur}",
        ("ja", JournalAppareilAutorise) => "デバイスが認証されました。",
        ("ja", ErreurDossierLocal) => "ローカルフォルダを作成できませんでした：{erreur}",
        ("ja", ErreurCheminInvalide) => "同期パスが無効です：{erreur}",
        ("ja", ErreurEnregistrementRacine) => "同期ルートの登録に失敗しました：{erreur}",
        ("ja", JournalRacineEnregistree) => "同期ルートを登録しました。",
        ("ja", JournalRacineDejaEnregistree) => "同期ルートはすでに登録されています。",
        ("ja", JournalMarquageFichiers) => "既存のローカルファイルを同期済みとしてマーク中...",
        ("ja", ErreurConnexionCloudFilter) => "Cloud Filter セッションの接続に失敗しました：{erreur}",
        ("ja", ErreurInterneInattendue) => "予期しない内部エラー —— アプリを再起動してください。",

        // ──────────────────────────────────────────────── العربية ──
        ("ar", AdresseServeur) => "عنوان الخادم:",
        ("ar", MotDePasse) => "كلمة مرور VEX:",
        ("ar", AfficherMdp) => "إظهار كلمة المرور",
        ("ar", Connecter) => "اتصال",
        ("ar", TitreDossier) => "مجلد المزامنة",
        ("ar", OuStocker) => "أين تريد حفظ ملفات VEX:",
        ("ar", Parcourir) => "استعراض...",
        ("ar", Continuer) => "متابعة",
        ("ar", TitreParcourirDialogue) => "اختر مجلد مزامنة VEX",
        ("ar", StatutErreurPrefixe) => "خطأ: ",
        ("ar", StatutLabel) => "الحالة: ",
        ("ar", StatutConnecte) => "متصل",
        ("ar", StatutEnCours) => "جارٍ الاتصال...",
        ("ar", StatutDossierLabel) => "المجلد المُزامَن: ",
        ("ar", StatutDerniereEtapeLabel) => "آخر خطوة: ",
        ("ar", TrayOuvrir) => "فتح VEX Cloud Client",
        ("ar", TrayQuitter) => "إنهاء",
        ("ar", DejaInstalleTitre) => "مثبّت بالفعل",
        ("ar", DejaInstalleTexte) => {
            "تم إعداد VEX Cloud Sync بالفعل على هذا الجهاز.\r\n\
             المجلد المُزامَن حاليًا: {chemin}\r\n\r\n\
             • إعادة التثبيت: اختيار مجلد جديد وإعادة الاتصال.\r\n\
             • إلغاء التثبيت: إزالة المزامنة والإعدادات (لن يتم حذف ملفاتك).\r\n\r\n\
             أغلق هذه النافذة (X) للخروج، دون أي تغيير."
        }
        ("ar", ActionReinstaller) => "إعادة التثبيت",
        ("ar", ActionDesinstaller) => "إلغاء التثبيت",
        ("ar", Desinstallee) => {
            "اكتمل إلغاء التثبيت.\n\nلم يتم حذف ملفاتك المُزامَنة -- يمكنك الاحتفاظ بها أو نقلها أو حذفها بنفسك. يمكنك أيضًا حذف vex-cloudsync.exe إذا لم تعد تنوي استخدامه."
        }
        ("ar", JournalWindowsCompatible) => "إصدار Windows {build}: متوافق.",
        ("ar", ErreurWindowsIncompatible) => {
            "تم اكتشاف إصدار Windows {build} -- تتطلب واجهة Cloud Files على الأقل الإصدار {minimum} (Windows 10 الإصدار 1709 أو أحدث)."
        }
        ("ar", JournalWindowsVerifImpossible) => "تحذير: تعذر التحقق من إصدار Windows ({erreur}) -- سيتم المتابعة على أي حال.",
        ("ar", JournalRechercheServeur) => "جارٍ البحث عن خادم VEX...",
        ("ar", ErreurServeurInjoignable) => "تعذر الوصول إلى خادم VEX (لم يستجب أي عنوان معروف -- تحقق من اتصالك).",
        ("ar", JournalServeurTrouve) => "تم العثور على الخادم: {url}",
        ("ar", JournalAppareilDejaAutorise) => "الجهاز مصرّح له بالفعل، يُعاد استخدام الرمز المحلي.",
        ("ar", JournalAucunAppareilAutorise) => "لا يوجد جهاز مصرّح له -- تحقق من متصفحك لتصريح هذا الجهاز...",
        ("ar", ErreurAutorisationEchouee) => "فشل التصريح: {erreur}",
        ("ar", JournalAppareilAutorise) => "تم تصريح الجهاز.",
        ("ar", ErreurDossierLocal) => "تعذر إنشاء المجلد المحلي: {erreur}",
        ("ar", ErreurCheminInvalide) => "مسار مزامنة غير صالح: {erreur}",
        ("ar", ErreurEnregistrementRacine) => "فشل تسجيل جذر المزامنة: {erreur}",
        ("ar", JournalRacineEnregistree) => "تم تسجيل جذر المزامنة.",
        ("ar", JournalRacineDejaEnregistree) => "جذر المزامنة مسجَّل بالفعل.",
        ("ar", JournalMarquageFichiers) => "جارٍ تحديد الملفات المحلية الموجودة بالفعل كمُزامَنة...",
        ("ar", ErreurConnexionCloudFilter) => "فشل الاتصال بجلسة Cloud Filter: {erreur}",
        ("ar", ErreurInterneInattendue) => "خطأ داخلي غير متوقع -- أعد تشغيل التطبيق.",

        // Toute autre combinaison (langue reconnue mais bras non couvert
        // explicitement ci-dessus -- ne devrait pas arriver, tous les cas
        // sont listes) : repli sur le francais.
        (_, cle) => t("fr", cle),
    }
}

fn chemin_fichier_langue() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("VexCloudSync").join("langue_ui.txt")
}

/// Dossier ou l'exe a ete lance (avant que l'utilisateur ne le deplace) --
/// c'est la ou `langue.txt` se trouve si l'utilisateur a extrait le zip en
/// entier sans deplacer seulement l'exe.
fn chemin_langue_bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("langue.txt"))
}

/// Langue de l'interface pour ce lancement : preference deja choisie (menu
/// deroulant, voir fenetre_mdp.rs) -- sinon `langue.txt` du zip au tout
/// premier lancement (reflete la langue du compte VEX au moment du
/// telechargement, voir appareil.rs::telecharger_bundle), alors sauvegardee
/// comme preference -- sinon francais.
pub fn langue_courante() -> String {
    if let Ok(l) = std::fs::read_to_string(chemin_fichier_langue()) {
        let l = l.trim();
        if est_langue_connue(l) {
            return l.to_string();
        }
    }
    if let Some(chemin) = chemin_langue_bundle() {
        if let Ok(l) = std::fs::read_to_string(&chemin) {
            let l = l.trim();
            if est_langue_connue(l) {
                sauvegarder_langue_ui(l);
                return l.to_string();
            }
        }
    }
    "fr".to_string()
}

pub fn sauvegarder_langue_ui(langue: &str) {
    let chemin = chemin_fichier_langue();
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(chemin, langue);
}
