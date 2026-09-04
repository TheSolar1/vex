// Integre l'icone VEX directement dans la ressource de l'executable, pour
// que le fichier .exe lui-meme (dans l'Explorateur, pas seulement la racine
// de synchro ou le raccourci) affiche l'icone au lieu de l'icone generique
// Windows par defaut d'un binaire non decore.
fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("vex-folder-icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Impossible d'integrer l'icone dans l'exe : {e}");
        }
    }
}
