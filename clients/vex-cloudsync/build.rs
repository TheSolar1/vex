// Integre l'icone VEX directement dans la ressource de l'executable, pour
// que le fichier .exe lui-meme (dans l'Explorateur, pas seulement la racine
// de synchro ou le raccourci) affiche l'icone au lieu de l'icone generique
// Windows par defaut d'un binaire non decore.
fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("vex-folder-icon.ico");
        // Metadonnees visibles dans Windows (Explorateur -> clic droit sur
        // l'exe -> Proprietes -> onglet Details).
        res.set("CompanyName", "VEX Corp by TheSolar");
        res.set("ProductName", "VEX Cloud Client");
        res.set("FileDescription", "VEX Cloud Client");
        res.set("LegalCopyright", "VEX Corp by TheSolar");
        // Sans ca, les controles Win32 (bouton, champ texte) de fenetre_mdp.rs
        // s'affichent avec le rendu "classique" gris de Windows 98, meme sur
        // Windows 11 -- ce manifeste declare la dependance a ComCtl32 v6, qui
        // active le theme visuel moderne (coins arrondis, focus visuel, etc.)
        // pour ces memes controles sans changer une ligne de code Win32.
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <asmv3:application xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
    <asmv3:windowsSettings xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
      <dpiAware>true</dpiAware>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>"#,
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=Impossible d'integrer l'icone/manifeste dans l'exe : {e}");
        }
    }
}
