// Verifie l'etat d'installation de vex-cloudsync : racine de synchro
// enregistree ? raccourci Bureau present et valide ? -- sans rien modifier.
use cloud_filter::root::{SecurityId, SyncRootIdBuilder};
use std::path::Path;

const PROVIDER_NAME: &str = "VEX";

fn verifier_raccourci() -> (bool, String, String) {
    let bureau = match std::env::var("USERPROFILE") {
        Ok(p) => format!("{p}\\Desktop\\VEX.lnk"),
        Err(_) => return (false, String::new(), "USERPROFILE introuvable".into()),
    };
    if !Path::new(&bureau).exists() {
        return (false, bureau, "fichier absent".into());
    }
    // Lit la cible via le meme COM que Windows utilise pour les .lnk.
    let script = format!(
        r#"$s = New-Object -ComObject WScript.Shell; $l = $s.CreateShortcut('{bureau}'); Write-Output $l.TargetPath"#
    );
    let sortie = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output();
    match sortie {
        Ok(o) if o.status.success() => {
            let cible = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let existe = Path::new(&cible).exists();
            (true, bureau, format!("cible = {cible} (dossier existe : {existe})"))
        }
        _ => (true, bureau, "impossible de lire la cible".into()),
    }
}

fn main() {
    println!("── État de l'installation vex-cloudsync ──\n");

    let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME)
        .user_security_id(SecurityId::current_user().unwrap())
        .build();

    match sync_root_id.is_registered() {
        Ok(true) => println!("✓ Racine de synchro : enregistrée"),
        Ok(false) => println!("  Racine de synchro : non enregistrée"),
        Err(e) => println!("? Racine de synchro : impossible à vérifier ({e:?})"),
    }

    let (present, chemin, detail) = verifier_raccourci();
    if present {
        println!("✓ Raccourci Bureau  : présent ({chemin})");
        println!("                      {detail}");
    } else {
        println!("  Raccourci Bureau  : absent ({chemin}) -- {detail}");
    }

    println!("\nPour tout retirer proprement : cargo run --release --bin uninstall");
}
