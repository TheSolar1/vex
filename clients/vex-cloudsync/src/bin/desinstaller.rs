// ══════════════════════════════════════════════════════════════════
// desinstaller.rs — exe autonome, inclus dans le zip de telechargement (voir
// src/login/appareil.rs::telecharger_bundle cote serveur), pour retirer VEX
// Cloud Sync de la machine sans avoir a relancer vex-cloudsync.exe. Fait
// exactement ce que fait main.rs::desinstaller() (meme fichiers de config,
// meme deregistrement Cloud Filter) -- duplique volontairement cette petite
// logique plutot que de faire dependre les deux binaires d'un crate lib
// partage : ~30 lignes, la duplication est moins risquee qu'un refactor de
// vex-cloudsync.exe (deja teste) pour un second exe secondaire.
//
// NE SUPPRIME PAS les fichiers synchronises de l'utilisateur -- seulement la
// config locale de l'app (meme principe que main.rs::desinstaller()).
// ══════════════════════════════════════════════════════════════════
#![windows_subsystem = "windows"]

use cloud_filter::root::{SecurityId, SyncRootIdBuilder};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO,
};

const PROVIDER_NAME: &str = "VEX";

fn vers_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn boite(titre: &str, texte: &str, style: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE) -> i32 {
    unsafe {
        let t = vers_wide(titre);
        let m = vers_wide(texte);
        MessageBoxW(None, PCWSTR(m.as_ptr()), PCWSTR(t.as_ptr()), style).0
    }
}

/// Langue deja choisie dans vex-cloudsync.exe (voir i18n.rs::sauvegarder_langue_ui)
/// -- meme fichier, pour que la confirmation/le message final de cet exe
/// distinct restent dans la langue que l'utilisateur a deja choisie. Repli
/// sur le francais si vex-cloudsync.exe n'a jamais tourne sur cette machine.
fn langue_actuelle() -> String {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let chemin = std::path::PathBuf::from(base).join("VexCloudSync").join("langue_ui.txt");
    std::fs::read_to_string(chemin).map(|s| s.trim().to_string()).unwrap_or_else(|_| "fr".to_string())
}

fn textes(langue: &str) -> (&'static str, &'static str, &'static str) {
    // (titre_confirmation, question, message_final)
    match langue {
        "en" => (
            "Uninstall VEX Cloud Sync",
            "Remove VEX Cloud Sync from this computer?\n\nYour synced files will not be deleted.",
            "Uninstall complete.\n\nYour synced files were not deleted -- you can keep, move, or delete them yourself. You can also delete vex-cloudsync.exe if you no longer plan to use it.",
        ),
        "es" => (
            "Desinstalar VEX Cloud Sync",
            "Quitar VEX Cloud Sync de este ordenador?\n\nTus archivos sincronizados no se eliminaran.",
            "Desinstalacion completada.\n\nTus archivos sincronizados no se han eliminado -- puedes conservarlos, moverlos o borrarlos tu mismo. Tambien puedes eliminar vex-cloudsync.exe si ya no lo vas a usar.",
        ),
        "de" => (
            "VEX Cloud Sync deinstallieren",
            "VEX Cloud Sync von diesem Computer entfernen?\n\nDeine synchronisierten Dateien werden nicht geloescht.",
            "Deinstallation abgeschlossen.\n\nDeine synchronisierten Dateien wurden nicht geloescht -- du kannst sie behalten, verschieben oder selbst loeschen. Du kannst auch vex-cloudsync.exe loeschen, wenn du es nicht mehr benutzen willst.",
        ),
        "it" => (
            "Disinstalla VEX Cloud Sync",
            "Rimuovere VEX Cloud Sync da questo computer?\n\nI tuoi file sincronizzati non verranno eliminati.",
            "Disinstallazione completata.\n\nI tuoi file sincronizzati non sono stati eliminati -- puoi tenerli, spostarli o eliminarli tu stesso. Puoi anche eliminare vex-cloudsync.exe se non intendi piu usarlo.",
        ),
        "pt" => (
            "Desinstalar VEX Cloud Sync",
            "Remover o VEX Cloud Sync deste computador?\n\nOs teus ficheiros sincronizados nao serao eliminados.",
            "Desinstalacao concluida.\n\nOs teus ficheiros sincronizados nao foram eliminados -- podes guarda-los, move-los ou elimina-los tu mesmo. Tambem podes eliminar o vex-cloudsync.exe se ja nao o fores usar.",
        ),
        "ru" => (
            "Удалить VEX Cloud Sync",
            "Удалить VEX Cloud Sync с этого компьютера?\n\nВаши синхронизированные файлы не будут удалены.",
            "Удаление завершено.\n\nВаши синхронизированные файлы не были удалены -- вы можете оставить, переместить или удалить их сами. Вы также можете удалить vex-cloudsync.exe, если больше не планируете его использовать.",
        ),
        "zh" => (
            "卸载 VEX Cloud Sync",
            "要从此电脑上移除 VEX Cloud Sync 吗？\n\n你的同步文件不会被删除。",
            "卸载完成。\n\n你的同步文件并未被删除——你可以自行保留、移动或删除它们。如果不再需要，也可以删除 vex-cloudsync.exe。",
        ),
        "ja" => (
            "VEX Cloud Sync のアンインストール",
            "このパソコンから VEX Cloud Sync を削除しますか？\n\n同期済みのファイルは削除されません。",
            "アンインストールが完了しました。\n\n同期されていたファイルは削除されていません -- そのまま残す、移動する、削除するのは自由です。今後使わないなら vex-cloudsync.exe を削除しても構いません。",
        ),
        "ar" => (
            "إلغاء تثبيت VEX Cloud Sync",
            "هل تريد إزالة VEX Cloud Sync من هذا الجهاز؟\n\nلن يتم حذف ملفاتك المُزامَنة.",
            "اكتمل إلغاء التثبيت.\n\nلم يتم حذف ملفاتك المُزامَنة -- يمكنك الاحتفاظ بها أو نقلها أو حذفها بنفسك. يمكنك أيضًا حذف vex-cloudsync.exe إذا لم تعد تنوي استخدامه.",
        ),
        _ => (
            "Desinstaller VEX Cloud Sync",
            "Retirer VEX Cloud Sync de cet ordinateur ?\n\nTes fichiers synchronises ne seront pas supprimes.",
            "Desinstallation terminee.\n\nTes fichiers synchronises n'ont pas ete supprimes -- tu peux les garder, les deplacer ou les supprimer toi-meme. Tu peux egalement supprimer vex-cloudsync.exe si tu ne comptes plus l'utiliser.",
        ),
    }
}

fn dossier_config() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("VexCloudSync")
}

fn main() {
    let langue = langue_actuelle();
    let (titre, question, message_final) = textes(&langue);

    if boite(titre, question, MB_YESNO | MB_ICONQUESTION) != IDYES.0 {
        return;
    }

    if let Ok(sid) = SecurityId::current_user() {
        let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME).user_security_id(sid).build();
        if sync_root_id.is_registered().unwrap_or(false) {
            let _ = sync_root_id.unregister();
        }
    }

    // Memes noms de fichiers que fenetre_mdp.rs (serveur.txt, dossier.txt),
    // i18n.rs (langue_ui.txt) et main.rs (device.json) -- voir le
    // commentaire en tete de fichier sur le choix de dupliquer plutot que
    // partager un crate lib entre les deux exe.
    let dossier = dossier_config();
    let _ = std::fs::remove_file(dossier.join("serveur.txt"));
    let _ = std::fs::remove_file(dossier.join("dossier.txt"));
    let _ = std::fs::remove_file(dossier.join("langue_ui.txt"));
    let _ = std::fs::remove_file(dossier.join("device.json"));
    if let Ok(profil) = std::env::var("USERPROFILE") {
        let _ = std::fs::remove_file(format!("{profil}\\Desktop\\VEX.lnk"));
    }

    boite("VEX Cloud Client", message_final, MB_OK | MB_ICONINFORMATION);
}
