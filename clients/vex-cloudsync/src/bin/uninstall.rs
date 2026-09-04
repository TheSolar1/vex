// Retire proprement tout ce que vex-cloudsync a pu installer.
//
// DECOUVERTE IMPORTANTE (testee et confirmee) : desinscrire une racine de
// synchro (CfUnregisterSyncRoot) SUPPRIME REELLEMENT les fichiers locaux,
// meme ceux deja completement telecharges (coche verte). Ce n'est PAS
// documente de facon evidente et contredit l'intuition "un fichier deja
// hydrate devrait juste redevenir un fichier normal". Verifie par un test
// dedie : fichiers presents avant `taskkill`, toujours presents apres
// `taskkill` seul, DISPARUS apres l'appel a `unregister()`.
//
// Consequence : cet outil exige maintenant le chemin du dossier synchronise
// en argument, et fait une COPIE DE SECOURS complete avant de desinscrire
// quoi que ce soit. Sans backup reussi (ou dossier deja vide), on refuse de
// continuer plutot que de risquer une perte de donnees silencieuse.
use cloud_filter::root::{SecurityId, SyncRootIdBuilder};
use std::path::{Path, PathBuf};

const PROVIDER_NAME: &str = "VEX";

fn copier_recursif(source: &Path, dest: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(dest)?;
    let mut copies = 0u64;
    for entree in std::fs::read_dir(source)? {
        let entree = entree?;
        let dest_item = dest.join(entree.file_name());
        if entree.file_type()?.is_dir() {
            copies += copier_recursif(&entree.path(), &dest_item)?;
        } else {
            std::fs::copy(entree.path(), &dest_item)?;
            copies += 1;
        }
    }
    Ok(copies)
}

fn sauvegarder(chemin_local: &Path) -> Result<u64, String> {
    if !chemin_local.exists() {
        println!("  Dossier local introuvable ({}), rien à sauvegarder.", chemin_local.display());
        return Ok(0);
    }
    let vide = chemin_local
        .read_dir()
        .map(|mut it| it.next().is_none())
        .unwrap_or(true);
    if vide {
        println!("  Dossier local vide, rien à sauvegarder.");
        return Ok(0);
    }

    let horodatage = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = PathBuf::from(format!("{}-backup-{horodatage}", chemin_local.display()));

    println!("Sauvegarde de secours avant désinscription : {} -> {}", chemin_local.display(), dest.display());
    let n = copier_recursif(chemin_local, &dest).map_err(|e| format!("{e}"))?;
    println!("✓ {n} fichier(s) copié(s) dans {}", dest.display());
    Ok(n)
}

fn desinscrire_racine() -> Result<(), String> {
    let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME)
        .user_security_id(SecurityId::current_user().unwrap())
        .build();

    match sync_root_id.is_registered() {
        Ok(true) => sync_root_id.unregister().map_err(|e| format!("{e:?}")),
        Ok(false) => { println!("  Racine de synchro : déjà absente, rien à faire."); Ok(()) }
        Err(e) => Err(format!("{e:?}")),
    }
}

fn retirer_raccourci() {
    let bureau = match std::env::var("USERPROFILE") {
        Ok(p) => format!("{p}\\Desktop\\VEX.lnk"),
        Err(_) => { println!("? USERPROFILE introuvable, raccourci non traité."); return; }
    };
    if !Path::new(&bureau).exists() {
        println!("  Raccourci Bureau : déjà absent, rien à faire.");
        return;
    }
    match std::fs::remove_file(&bureau) {
        Ok(_) => println!("✓ Raccourci Bureau supprimé ({bureau})."),
        Err(e) => println!("✗ Échec de suppression du raccourci Bureau : {e}"),
    }
}

fn main() {
    println!("── Désinstallation de vex-cloudsync ──\n");

    let chemin_arg = std::env::args().nth(1);
    let chemin_local = chemin_arg
        .or_else(|| std::env::var("VEX_LOCAL_PATH").ok())
        .map(PathBuf::from);

    let Some(chemin_local) = chemin_local else {
        eprintln!(
            "ERREUR : chemin du dossier synchronisé requis (argument, ou VEX_LOCAL_PATH).\n\
             Raison : désinscrire la racine de synchro SUPPRIME les fichiers locaux\n\
             (voir commentaire en tête de ce fichier) -- on refuse de continuer sans\n\
             savoir quoi sauvegarder avant. Exemple :\n\
             \u{20}\u{20}uninstall.exe \"C:\\Users\\toi\\Documents\\MonDossierVex\""
        );
        std::process::exit(1);
    };

    match sauvegarder(&chemin_local) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("ERREUR : sauvegarde de secours échouée ({e}) -- désinscription ANNULÉE par sécurité.");
            std::process::exit(1);
        }
    }

    match desinscrire_racine() {
        Ok(()) => println!("✓ Racine de synchro désinscrite."),
        Err(e) => println!("✗ Échec de désinscription de la racine de synchro : {e}"),
    }
    retirer_raccourci();

    println!("\nTerminé. ATTENTION : les fichiers locaux du dossier synchronisé ont pu");
    println!("être supprimés par la désinscription (comportement confirmé de l'API");
    println!("Cloud Files) -- une copie de secours a été faite avant si le dossier");
    println!("contenait quelque chose (voir chemin affiché ci-dessus).");
}
