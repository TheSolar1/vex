// ══════════════════════════════════════════════════════════════════
// fenetre_mdp.rs — petite fenetre Win32 native pour saisir le mot de
// passe de chiffrement (remplace l'ancienne page web locale servie par
// tiny_http). Ecrit en Win32 brut via le crate `windows` deja utilise
// pour ShellExecuteW (device_auth.rs) -- volontairement PAS via
// native-windows-gui/-derive : ce crate, une fois lie dans le meme
// binaire que `windows`, fait planter l'exe des le lancement
// (STATUS_DLL_NOT_FOUND, constate et reproduit de façon fiable en test,
// meme sans jamais executer le code nwg -- cause exacte non identifiee,
// mais un seul crate d'API Win32 dans tout le binaire evite le probleme).
// ══════════════════════════════════════════════════════════════════

use std::cell::RefCell;
use windows::core::PCWSTR;

/// Journal de diagnostic temporaire pour le menu de langue (voir
/// ouvrir_menu_langue) -- "ca ne marche pas" signale a plusieurs reprises
/// sans qu'aucune hypothese de code seule n'ait suffi a le confirmer/corriger ;
/// ecrit dans %LOCALAPPDATA%\VexCloudSync\debug.log pour voir precisement ou
/// le flux s'arrete, plutot que de continuer a corriger a l'aveugle.
fn journal_debug(msg: &str) {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let chemin = std::path::PathBuf::from(base).join("VexCloudSync").join("debug.log");
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&chemin) {
        let _ = writeln!(f, "{msg}");
    }
}
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush,
    DeleteDC, DeleteObject, DrawTextW, Ellipse, FillRect, GetDC, GetStockObject, InvalidateRect,
    ReleaseDC, SetBkColor, SetBkMode, SetTextColor, ANTIALIASED_QUALITY, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC,
    HFONT, HGDIOBJ, NULL_PEN, OUT_DEFAULT_PRECIS, SelectObject, TRANSPARENT,
};

// Style de controle STATIC pour afficher une icone (SS_ICON), et attribut
// DWM pour le mode sombre de la barre de titre -- ni l'un ni l'autre n'est
// expose par la version du module windows-rs utilisee ici, valeurs Win32
// standard (documentees par Microsoft).
const SS_ICON: u32 = 0x0000_0003;
const SS_NOTIFY: u32 = 0x0000_0100;
// Constantes owner-draw pour le menu de langue (voir ouvrir_menu_langue) --
// non exposees par ce module de windows-rs (seuls les OD*_ pour controles
// standards le sont), valeurs Win32 documentees standard.
const ODS_SELECTED: u32 = 0x0001;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

// Marge commune a tous les elements, pour un alignement gauche/droite
// coherent sur toute la fenetre (logo+titre, champ, bouton).
const MARGE: i32 = 24;
const LARGEUR_CONTENU: i32 = 320;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{SetWindowTheme, DRAWITEMSTRUCT};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VIRTUAL_KEY, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::Shell::{
    ExtractIconExW, SHBrowseForFolderW, SHGetPathFromIDListW, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS,
    BROWSEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::i18n::{self, Cle};

const ID_CHAMP_MDP: i32 = 101;
const ID_BOUTON: i32 = 102;
const ID_CASE_AFFICHER: i32 = 103;
const ID_CHAMP_URL: i32 = 104;
const ID_ICONE_LANGUE: i32 = 105;
const STN_CLICKED: u32 = 0;
const CLASSE_MENU_LANGUE: &str = "VexMenuLangue";

// Fenetre "deja installe" (voir demander_action_installation).
const ID_BOUTON_REINSTALLER: i32 = 210;
const ID_BOUTON_DESINSTALLER: i32 = 211;
const CLASSE_FENETRE_INSTALL: &str = "VexFenetreDejaInstalle";
// EM_SETPASSWORDCHAR n'est pas expose par ce module -- valeur Win32 standard.
const EM_SETPASSWORDCHAR: u32 = 0x00CC;
const CLASSE_FENETRE: &str = "VexFenetreMotDePasse";

// Fenetre "dossier de synchronisation" (voir demander_dossier_destination) --
// IDs distincts de ceux ci-dessus pour que les deux wndproc restent lisibles
// independamment, meme si en pratique une seule fenetre est ouverte a la fois.
const ID_CHAMP_DOSSIER: i32 = 201;
const ID_BOUTON_PARCOURIR: i32 = 202;
const ID_BOUTON_CONTINUER: i32 = 203;
const CLASSE_FENETRE_DOSSIER: &str = "VexFenetreDossier";

// Charte du site (voir account.html) : fond sombre #171a21, champs #0f1115,
// texte clair #e7e9ee, vert d'action #4caf50 (bouton "Autoriser" de
// autoriser-appareil, voir login/appareil.rs) -- pour que la fenetre native
// ne jure pas a cote du reste de l'experience VEX.
const COULEUR_FOND: u32 = 0x211a17; // BGR de #171a21
const COULEUR_CHAMP: u32 = 0x15100f; // BGR de #0f1115
const COULEUR_TEXTE: u32 = 0xeee9e7; // BGR de #e7e9ee
const COULEUR_ACCENT: u32 = 0x47a043; // BGR de #43a047 (un cran plus fonce/sature que #4caf50, juge "trop clair")
const COULEUR_DANGER: u32 = 0x4d48e5; // BGR de #e5484d (plus sature que #ff6b6b, juge "trop fade")

thread_local! {
    static HWND_CHAMP: RefCell<Option<HWND>> = RefCell::new(None);
    static HWND_URL: RefCell<Option<HWND>> = RefCell::new(None);
    static MOT_DE_PASSE: RefCell<Option<String>> = RefCell::new(None);
    static URL_SAISIE: RefCell<Option<String>> = RefCell::new(None);
    static BROSSE_FOND: RefCell<Option<HBRUSH>> = RefCell::new(None);
    static BROSSE_CHAMP: RefCell<Option<HBRUSH>> = RefCell::new(None);
    static BROSSE_ACCENT: RefCell<Option<HBRUSH>> = RefCell::new(None);
    static BROSSE_DANGER: RefCell<Option<HBRUSH>> = RefCell::new(None);
    static ACTION_INSTALLATION: RefCell<ActionInstallation> = RefCell::new(ActionInstallation::Continuer);
    static POLICE_BOUTON: RefCell<Option<HFONT>> = RefCell::new(None);
    // Etat de la case "Afficher le mot de passe" -- en BS_OWNERDRAW (voir
    // plus bas), Windows ne suit plus cet etat tout seul comme il le fait
    // pour une vraie BS_AUTOCHECKBOX : on le gere nous-memes.
    static CASE_COCHEE: RefCell<bool> = RefCell::new(false);
    static HWND_DOSSIER: RefCell<Option<HWND>> = RefCell::new(None);
    static DOSSIER_CHOISI: RefCell<Option<String>> = RefCell::new(None);
    // Mis a true quand l'utilisateur choisit une langue dans le menu ouvert
    // par l'icone globe (voir ouvrir_menu_langue) -- demander_mot_de_passe
    // relit ce drapeau apres la boucle de messages pour savoir s'il doit
    // rouvrir la fenetre avec les nouveaux textes plutot que retourner un
    // resultat.
    static CHANGEMENT_LANGUE: RefCell<bool> = RefCell::new(false);
    // Langue choisie dans le menu ouvert par ouvrir_menu_langue -- relu par
    // ouvrir_menu_langue elle-meme apres la fin de sa propre boucle de
    // messages (voir wndproc_menu_langue::WM_COMMAND / LBN_SELCHANGE).
    static LANGUE_CHOISIE_MENU: RefCell<Option<usize>> = RefCell::new(None);
    // Garde-fou pour wndproc_menu_langue::WM_ACTIVATE -- voir ouvrir_menu_langue.
    static MENU_LANGUE_PRET: RefCell<bool> = RefCell::new(false);
    // Police normale (pas grasse) pour les elements du menu de langue --
    // POLICE_BOUTON (grasse) est deja prise pour les boutons ; un menu
    // de choix simple n'a pas besoin de gras.
    static POLICE_MENU_LANGUE: RefCell<Option<HFONT>> = RefCell::new(None);
}

fn vers_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn lire_texte(hwnd: HWND) -> String {
    unsafe {
        let longueur = GetWindowTextLengthW(hwnd);
        if longueur <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; longueur as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

/// Lit le mot de passe saisi et ferme la fenetre -- partage entre le clic
/// sur le bouton (BN_CLICKED) et la touche Entree (geree manuellement, voir
/// boucle de messages : un bouton BS_OWNERDRAW n'est plus reconnu comme
/// bouton "par defaut" par IsDialogMessageW).
unsafe extern "system" fn wndproc_menu_langue(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let notif = ((wparam.0 >> 16) & 0xFFFF) as u32;
            journal_debug(&format!("[menu] wndproc_menu_langue WM_COMMAND notif={notif}"));
            if notif == LBN_SELCHANGE {
                let liste = HWND(lparam.0 as *mut _);
                let index = SendMessageW(liste, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                if index >= 0 {
                    LANGUE_CHOISIE_MENU.with(|c| *c.borrow_mut() = Some(index as usize));
                }
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        // Clic ailleurs (sur la fenetre de connexion ou en dehors) : le menu
        // perd l'activation et se ferme tout seul, comme un vrai menu.
        // BUG CONSTATE EN PRATIQUE sans le garde-fou MENU_LANGUE_PRET : selon
        // l'ordre des messages d'activation Windows a la creation (avant meme
        // que ouvrir_menu_langue ait fini d'appeler SetForegroundWindow), ce
        // WM_ACTIVATE(WA_INACTIVE) pouvait arriver en tout premier et
        // detruire le menu instantanement -- il paraissait "ne rien faire"
        // au clic, alors qu'il s'ouvrait puis se refermait aussitot.
        WM_ACTIVATE => {
            let pret = MENU_LANGUE_PRET.with(|p| *p.borrow());
            let etat = (wparam.0 & 0xFFFF) as u32;
            journal_debug(&format!("[menu] WM_ACTIVATE etat={etat} pret={pret}"));
            if pret && etat == WA_INACTIVE {
                journal_debug("[menu] WM_ACTIVATE : fermeture (perte d'activation)");
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let selectionne = (dis.itemState.0 & ODS_SELECTED) != 0;
            let brosse = if selectionne {
                BROSSE_ACCENT.with(|b| *b.borrow())
            } else {
                BROSSE_FOND.with(|b| *b.borrow())
            }
            .unwrap_or_default();
            FillRect(dis.hDC, &dis.rcItem, brosse);
            SetBkMode(dis.hDC, TRANSPARENT);
            SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
            if let Some(police) = POLICE_MENU_LANGUE.with(|p| *p.borrow()) {
                SelectObject(dis.hDC, HGDIOBJ(police.0));
            }
            if let Some((_, nom)) = i18n::LANGUES.get(dis.itemID as usize) {
                let mut texte = vers_wide(nom);
                texte.pop();
                let mut rect: RECT = dis.rcItem;
                rect.left += 14;
                DrawTextW(dis.hDC, &mut texte, &mut rect, DT_VCENTER | DT_SINGLELINE);
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            journal_debug("[menu] wndproc_menu_langue WM_DESTROY");
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Ouvre le choix de langue -- une vraie petite fenetre a nous (WS_POPUP +
/// CS_DROPSHADOW pour une ombre portee), pas TrackPopupMenuEx. BUG CONSTATE
/// EN PRATIQUE avec un vrai menu Win32 : meme avec des elements MF_OWNERDRAW
/// entierement sombres, Windows dessine quand meme un cadre clair autour du
/// menu lui-meme, impossible a retirer sans re-themer tout Windows. Une
/// fenetre normale, elle, n'a strictement que le style qu'on lui donne.
unsafe fn ouvrir_menu_langue(hwnd_parent: HWND, icone: HWND) {
    journal_debug("[menu] ouvrir_menu_langue: entree");
    LANGUE_CHOISIE_MENU.with(|c| *c.borrow_mut() = None);
    MENU_LANGUE_PRET.with(|p| *p.borrow_mut() = false);

    let Ok(hmodule) = GetModuleHandleW(None) else {
        journal_debug("[menu] GetModuleHandleW a echoue, abandon");
        return;
    };
    let hinstance: HINSTANCE = hmodule.into();
    let nom_classe = vers_wide(CLASSE_MENU_LANGUE);
    let brosse_fond = BROSSE_FOND.with(|b| *b.borrow()).unwrap_or_default();
    let classe = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DROPSHADOW,
        lpfnWndProc: Some(wndproc_menu_langue),
        hInstance: hinstance,
        lpszClassName: PCWSTR(nom_classe.as_ptr()),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: brosse_fond,
        ..Default::default()
    };
    let atome = RegisterClassExW(&classe);
    journal_debug(&format!("[menu] RegisterClassExW atome={atome} (0 = deja enregistree ou echec, voir GetLastError non journalise ici)"));

    const LARGEUR: i32 = 170;
    const HAUTEUR_ITEM: i32 = 28;
    let hauteur = HAUTEUR_ITEM * i18n::LANGUES.len() as i32;

    let mut rect_icone = RECT::default();
    let ok_rect = GetWindowRect(icone, &mut rect_icone);
    journal_debug(&format!(
        "[menu] GetWindowRect(icone) ok={} rect=({},{},{},{})",
        ok_rect.is_ok(), rect_icone.left, rect_icone.top, rect_icone.right, rect_icone.bottom
    ));
    let x = rect_icone.right - LARGEUR;
    let y = rect_icone.bottom + 4;

    let resultat_fenetre = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(nom_classe.as_ptr()),
        PCWSTR::null(),
        WS_POPUP | WS_VISIBLE,
        x,
        y,
        LARGEUR,
        hauteur,
        hwnd_parent,
        HMENU::default(),
        hinstance,
        None,
    );
    journal_debug(&format!("[menu] CreateWindowExW(popup) ok={} x={x} y={y} largeur={LARGEUR} hauteur={hauteur}", resultat_fenetre.is_ok()));
    let Ok(fenetre) = resultat_fenetre else {
        journal_debug("[menu] echec creation fenetre popup, abandon");
        return;
    };

    let nom_police = vers_wide("Segoe UI");
    let police_normale = CreateFontW(
        15, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        PCWSTR(nom_police.as_ptr()),
    );
    POLICE_MENU_LANGUE.with(|p| *p.borrow_mut() = Some(police_normale));

    let classe_liste = vers_wide("LISTBOX");
    let liste = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(classe_liste.as_ptr()),
        PCWSTR::null(),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE((LBS_NOTIFY | LBS_OWNERDRAWFIXED | LBS_NOINTEGRALHEIGHT) as u32),
        0,
        0,
        LARGEUR,
        hauteur,
        fenetre,
        HMENU::default(),
        hinstance,
        None,
    );
    journal_debug(&format!("[menu] CreateWindowExW(listbox) ok={}", liste.is_ok()));
    if let Ok(liste) = liste {
        SendMessageW(liste, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(0));
        SendMessageW(liste, LB_SETITEMHEIGHT, WPARAM(0), LPARAM(HAUTEUR_ITEM as isize));
        for (_, nom) in i18n::LANGUES.iter() {
            let nom_w = vers_wide(nom);
            SendMessageW(liste, LB_ADDSTRING, WPARAM(0), LPARAM(nom_w.as_ptr() as isize));
        }
        let _ = SetFocus(liste);
    }

    let ok_fg = SetForegroundWindow(fenetre);
    journal_debug(&format!("[menu] SetForegroundWindow ok={}", ok_fg.as_bool()));
    // A partir d'ici seulement : un WM_ACTIVATE(WA_INACTIVE) recu avant ce
    // point (pendant la creation/le focus/l'activation ci-dessus) est ignore
    // au lieu de fermer le menu avant meme qu'il soit visible.
    MENU_LANGUE_PRET.with(|p| *p.borrow_mut() = true);
    journal_debug("[menu] entree boucle de messages");

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        // Echap interceptee ici, au niveau de la boucle -- pas dans
        // wndproc_menu_langue : le clavier suit le focus, qui est sur la
        // LISTBOX (voir SetFocus plus haut), donc WM_KEYDOWN va a son propre
        // wndproc systeme, jamais a celui de `fenetre`.
        if msg.message == WM_KEYDOWN && VIRTUAL_KEY(msg.wParam.0 as u16) == VK_ESCAPE {
            let _ = DestroyWindow(fenetre);
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    journal_debug("[menu] sortie boucle de messages (fenetre detruite)");

    if let Some(index) = LANGUE_CHOISIE_MENU.with(|c| c.borrow_mut().take()) {
        journal_debug(&format!("[menu] langue choisie index={index}"));
        if let Some((code, _)) = i18n::LANGUES.get(index) {
            i18n::sauvegarder_langue_ui(code);
            CHANGEMENT_LANGUE.with(|c| *c.borrow_mut() = true);
            let _ = DestroyWindow(hwnd_parent);
        }
    } else {
        journal_debug("[menu] aucune langue choisie (ferme sans selection)");
    }
}

unsafe fn valider(hwnd: HWND) {
    let mdp = HWND_CHAMP.with(|c| c.borrow().map(lire_texte)).unwrap_or_default();
    let url = HWND_URL.with(|c| c.borrow().map(lire_texte)).unwrap_or_default();
    MOT_DE_PASSE.with(|m| *m.borrow_mut() = Some(mdp));
    URL_SAISIE.with(|u| *u.borrow_mut() = Some(url));
    let _ = DestroyWindow(hwnd);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let notif = ((wparam.0 >> 16) & 0xFFFF) as u32;
            journal_debug(&format!("[mdp] WM_COMMAND id={id} notif={notif}"));
            if id == ID_BOUTON && notif == BN_CLICKED as u32 {
                valider(hwnd);
            } else if id == ID_CASE_AFFICHER && notif == BN_CLICKED as u32 {
                // BS_OWNERDRAW (voir plus bas) : Windows ne suit plus l'etat
                // coche/decoche tout seul, on le bascule nous-memes puis on
                // applique le caractere de masquage correspondant (0 = pas
                // de masquage) sur le champ.
                let coche = CASE_COCHEE.with(|c| {
                    let nouveau = !*c.borrow();
                    *c.borrow_mut() = nouveau;
                    nouveau
                });
                let case = HWND(lparam.0 as *mut _);
                let _ = InvalidateRect(case, None, true);
                if let Some(champ) = HWND_CHAMP.with(|c| *c.borrow()) {
                    // Rond plein (comme le masquage par defaut de Windows),
                    // pas un asterisque -- plus discret visuellement.
                    let caractere = if coche { 0 } else { '\u{25CF}' as usize };
                    SendMessageW(champ, EM_SETPASSWORDCHAR, WPARAM(caractere), LPARAM(0));
                    let _ = InvalidateRect(champ, None, true);
                }
            } else if id == ID_ICONE_LANGUE && notif == STN_CLICKED {
                let icone = HWND(lparam.0 as *mut _);
                ouvrir_menu_langue(hwnd, icone);
            } else if id == IDCANCEL.0 {
                // Echap, relaye par IsDialogMessageW -- ferme sans valider.
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            // Texte du label en clair sur fond sombre (sinon Windows dessine
            // le texte noir par defaut, illisible sur notre fond fonce).
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(COULEUR_TEXTE));
            SetBkColor(hdc, COLORREF(COULEUR_FOND));
            let brosse = BROSSE_FOND.with(|b| *b.borrow());
            LRESULT(brosse.map(|b| b.0 as isize).unwrap_or(0))
        }
        WM_CTLCOLOREDIT => {
            // Champ mot de passe : fond legerement plus fonce que la
            // fenetre (meme nuance que les inputs du site), texte clair.
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(COULEUR_TEXTE));
            SetBkColor(hdc, COLORREF(COULEUR_CHAMP));
            let brosse = BROSSE_CHAMP.with(|b| *b.borrow());
            LRESULT(brosse.map(|b| b.0 as isize).unwrap_or(0))
        }
        WM_DRAWITEM => {
            // Bouton "Connecter" en BS_OWNERDRAW (voir plus bas) : les
            // boutons standards de Windows ignorent toute couleur de fond
            // personnalisee, meme sans thème visuel -- seul le dessin
            // manuel garantit le rendu voulu (vert, coherent avec le bouton
            // "Autoriser" de la page web d'approbation).
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let langue = i18n::langue_courante();
            if dis.CtlID == ID_BOUTON as u32 {
                let brosse = BROSSE_ACCENT.with(|b| *b.borrow()).unwrap_or_default();
                FillRect(dis.hDC, &dis.rcItem, brosse);
                SetBkMode(dis.hDC, TRANSPARENT);
                SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                if let Some(police) = POLICE_BOUTON.with(|p| *p.borrow()) {
                    SelectObject(dis.hDC, HGDIOBJ(police.0));
                }
                let mut texte = vers_wide(i18n::t(&langue, Cle::Connecter));
                texte.pop(); // DrawTextW veut la longueur sans le \0 final
                let mut rect: RECT = dis.rcItem;
                DrawTextW(dis.hDC, &mut texte, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            } else if dis.CtlID == ID_CASE_AFFICHER as u32 {
                // Case "Afficher le mot de passe" : petit carre dessine a
                // la main (coche = vert plein + coche blanche, decoche =
                // simple contour) plutot que le carre blanc generique de
                // Windows, pour rester coherent avec le reste de la fenetre.
                let brosse_fond = BROSSE_FOND.with(|b| *b.borrow()).unwrap_or_default();
                FillRect(dis.hDC, &dis.rcItem, brosse_fond);

                let taille_case = 16;
                let y_centre = (dis.rcItem.top + dis.rcItem.bottom) / 2;
                let mut case_rect = RECT {
                    left: dis.rcItem.left,
                    top: y_centre - taille_case / 2,
                    right: dis.rcItem.left + taille_case,
                    bottom: y_centre + taille_case / 2,
                };

                let coche = CASE_COCHEE.with(|c| *c.borrow());
                if coche {
                    let brosse_accent = BROSSE_ACCENT.with(|b| *b.borrow()).unwrap_or_default();
                    FillRect(dis.hDC, &case_rect, brosse_accent);
                    SetBkMode(dis.hDC, TRANSPARENT);
                    SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                    let mut coche_txt = vers_wide("\u{2713}");
                    coche_txt.pop();
                    DrawTextW(dis.hDC, &mut coche_txt, &mut case_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                } else {
                    let brosse_champ = BROSSE_CHAMP.with(|b| *b.borrow()).unwrap_or_default();
                    FillRect(dis.hDC, &case_rect, brosse_champ);
                }

                SetBkMode(dis.hDC, TRANSPARENT);
                SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                let mut texte = vers_wide(i18n::t(&langue, Cle::AfficherMdp));
                texte.pop();
                let mut texte_rect = RECT {
                    left: dis.rcItem.left + taille_case + 10,
                    top: dis.rcItem.top,
                    right: dis.rcItem.right,
                    bottom: dis.rcItem.bottom,
                };
                DrawTextW(dis.hDC, &mut texte, &mut texte_rect, DT_VCENTER | DT_SINGLELINE);
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Ouvre le selecteur de dossier natif de Windows (meme boite que "Parcourir"
/// dans l'Explorateur). BIF_NEWDIALOGSTYLE : version moderne, redimensionnable
/// (sans ce flag, SHBrowseForFolderW affiche l'ancienne boite figee de
/// Windows 2000). Retourne None si l'utilisateur annule.
unsafe fn parcourir_dossier(parent: HWND, titre: &str) -> Option<String> {
    let titre_w = vers_wide(titre);
    let mut buf_affichage = [0u16; 260]; // MAX_PATH
    let mut bi = BROWSEINFOW {
        hwndOwner: parent,
        pidlRoot: std::mem::zeroed(),
        pszDisplayName: windows::core::PWSTR(buf_affichage.as_mut_ptr()),
        lpszTitle: PCWSTR(titre_w.as_ptr()),
        ulFlags: BIF_NEWDIALOGSTYLE | BIF_RETURNONLYFSDIRS,
        lpfn: None,
        lParam: LPARAM(0),
        iImage: 0,
    };
    let pidl = SHBrowseForFolderW(&mut bi);
    if pidl.is_null() {
        return None; // annule par l'utilisateur
    }
    let mut chemin_buf = [0u16; 260];
    let ok = SHGetPathFromIDListW(pidl, &mut chemin_buf);
    CoTaskMemFree(Some(pidl as *const _));
    if !ok.as_bool() {
        return None;
    }
    let fin = chemin_buf.iter().position(|&c| c == 0).unwrap_or(chemin_buf.len());
    Some(String::from_utf16_lossy(&chemin_buf[..fin]))
}

/// Icone "globe" pour le selecteur de langue -- extraite directement de
/// shell32.dll (index 13, l'icone reseau/globe classique de l'Explorateur),
/// pas un emoji : pas de fichier .ico supplementaire a embarquer, et un
/// vrai rendu icone quel que soit le theme/police du systeme.
unsafe fn charger_icone_globe() -> HICON {
    let chemin = vers_wide(r"C:\Windows\System32\shell32.dll");
    let mut petite = HICON::default();
    let n = ExtractIconExW(PCWSTR(chemin.as_ptr()), 13, None, Some(&mut petite), 1);
    if n == 0 {
        HICON::default()
    } else {
        petite
    }
}

unsafe fn valider_dossier(hwnd: HWND) {
    let chemin = HWND_DOSSIER.with(|c| c.borrow().map(lire_texte)).unwrap_or_default();
    let chemin = chemin.trim().to_string();
    if !chemin.is_empty() {
        DOSSIER_CHOISI.with(|d| *d.borrow_mut() = Some(chemin));
    }
    let _ = DestroyWindow(hwnd);
}

unsafe extern "system" fn wndproc_dossier(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let notif = ((wparam.0 >> 16) & 0xFFFF) as u32;
            if id == ID_BOUTON_CONTINUER && notif == BN_CLICKED as u32 {
                valider_dossier(hwnd);
            } else if id == ID_BOUTON_PARCOURIR && notif == BN_CLICKED as u32 {
                let langue = i18n::langue_courante();
                if let Some(chemin) = parcourir_dossier(hwnd, i18n::t(&langue, Cle::TitreParcourirDialogue)) {
                    if let Some(champ) = HWND_DOSSIER.with(|c| *c.borrow()) {
                        let chemin_w = vers_wide(&chemin);
                        SendMessageW(champ, WM_SETTEXT, WPARAM(0), LPARAM(chemin_w.as_ptr() as isize));
                    }
                }
            } else if id == IDCANCEL.0 {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(COULEUR_TEXTE));
            SetBkColor(hdc, COLORREF(COULEUR_FOND));
            let brosse = BROSSE_FOND.with(|b| *b.borrow());
            LRESULT(brosse.map(|b| b.0 as isize).unwrap_or(0))
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(COULEUR_TEXTE));
            SetBkColor(hdc, COLORREF(COULEUR_CHAMP));
            let brosse = BROSSE_CHAMP.with(|b| *b.borrow());
            LRESULT(brosse.map(|b| b.0 as isize).unwrap_or(0))
        }
        WM_DRAWITEM => {
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let langue = i18n::langue_courante();
            if dis.CtlID == ID_BOUTON_CONTINUER as u32 {
                let brosse = BROSSE_ACCENT.with(|b| *b.borrow()).unwrap_or_default();
                FillRect(dis.hDC, &dis.rcItem, brosse);
                SetBkMode(dis.hDC, TRANSPARENT);
                SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                if let Some(police) = POLICE_BOUTON.with(|p| *p.borrow()) {
                    SelectObject(dis.hDC, HGDIOBJ(police.0));
                }
                let mut texte = vers_wide(i18n::t(&langue, Cle::Continuer));
                texte.pop();
                let mut rect: RECT = dis.rcItem;
                DrawTextW(dis.hDC, &mut texte, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            } else if dis.CtlID == ID_BOUTON_PARCOURIR as u32 {
                // Bouton secondaire : fond assorti aux champs (COULEUR_CHAMP)
                // plutot que l'accent vert de "Continuer", pour marquer que
                // c'est une action secondaire -- pas grise/blanche comme le
                // rendu Windows par defaut.
                let brosse = BROSSE_CHAMP.with(|b| *b.borrow()).unwrap_or_default();
                FillRect(dis.hDC, &dis.rcItem, brosse);
                SetBkMode(dis.hDC, TRANSPARENT);
                SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                let mut texte = vers_wide(i18n::t(&langue, Cle::Parcourir));
                texte.pop();
                let mut rect: RECT = dis.rcItem;
                DrawTextW(dis.hDC, &mut texte, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Premiere fenetre affichee au tout premier lancement (avant meme
/// `demander_mot_de_passe`) : ou stocker le dossier synchronise, avec un
/// champ pre-rempli par un chemin par defaut et un bouton "Parcourir..." vers
/// le selecteur natif. Meme cycle de vie que `demander_mot_de_passe`
/// (une fenetre, une boucle de messages, une fermeture) pour rester coherent
/// et eviter tout effet d'ouverture/fermeture parasite. Retourne None si
/// l'utilisateur ferme sans valider -- l'appelant garde alors le chemin par
/// defaut plutot que de bloquer le lancement.
pub fn demander_dossier_destination(chemin_icone: &str, defaut: &str) -> Option<String> {
    unsafe {
        let langue = i18n::langue_courante();
        let hinstance = GetModuleHandleW(None).ok()?.into();
        let nom_classe = vers_wide(CLASSE_FENETRE_DOSSIER);

        let chemin_icone_w = vers_wide(chemin_icone);
        let charger_icone = |taille: i32| -> HICON {
            LoadImageW(None, PCWSTR(chemin_icone_w.as_ptr()), IMAGE_ICON, taille, taille, LR_LOADFROMFILE)
                .map(|h| HICON(h.0))
                .unwrap_or_default()
        };
        let hicone = charger_icone(0);
        let hicone_logo = charger_icone(64);

        let nom_police = vers_wide("Segoe UI");
        let police_titre = CreateFontW(
            22, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_normale = CreateFontW(
            16, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_bouton = CreateFontW(
            17, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        POLICE_BOUTON.with(|p| *p.borrow_mut() = Some(police_bouton));

        let brosse_fond = CreateSolidBrush(COLORREF(COULEUR_FOND));
        let brosse_champ = CreateSolidBrush(COLORREF(COULEUR_CHAMP));
        let brosse_accent = CreateSolidBrush(COLORREF(COULEUR_ACCENT));
        BROSSE_FOND.with(|b| *b.borrow_mut() = Some(brosse_fond));
        BROSSE_CHAMP.with(|b| *b.borrow_mut() = Some(brosse_champ));
        BROSSE_ACCENT.with(|b| *b.borrow_mut() = Some(brosse_accent));

        let classe = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc_dossier),
            hInstance: hinstance,
            lpszClassName: PCWSTR(nom_classe.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: hicone,
            hIconSm: hicone,
            hbrBackground: brosse_fond,
            ..Default::default()
        };
        RegisterClassExW(&classe);

        let titre = vers_wide("VEX Cloud Client");
        let fenetre = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(nom_classe.as_ptr()),
            PCWSTR(titre.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            392,
            300,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
        .ok()?;

        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            fenetre,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        );
        let sombre: i32 = 1;
        let _ = DwmSetWindowAttribute(
            fenetre,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(DWMWA_USE_IMMERSIVE_DARK_MODE as i32),
            &sombre as *const _ as *const _,
            std::mem::size_of_val(&sombre) as u32,
        );

        let classe_static = vers_wide("STATIC");
        const TAILLE_LOGO: i32 = 56;
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_ICON as u32),
            MARGE,
            24,
            TAILLE_LOGO,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        let logo = FindWindowExW(fenetre, HWND::default(), PCWSTR(classe_static.as_ptr()), PCWSTR::null());
        if let Ok(logo) = logo {
            SendMessageW(logo, STM_SETICON, WPARAM(hicone_logo.0 as usize), LPARAM(0));
        }

        let titre_x = MARGE + TAILLE_LOGO + 16;
        let titre_texte = vers_wide(i18n::t(&langue, Cle::TitreDossier));
        let titre_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(titre_texte.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            titre_x,
            24,
            MARGE + LARGEUR_CONTENU - titre_x,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(t) = titre_ctrl {
            SendMessageW(t, WM_SETFONT, WPARAM(police_titre.0 as usize), LPARAM(1));
        }

        let label = vers_wide(i18n::t(&langue, Cle::OuStocker));
        let label_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGE,
            24 + TAILLE_LOGO + 20,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(l) = label_ctrl {
            SendMessageW(l, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        }

        let classe_edit = vers_wide("EDIT");
        let champ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_edit.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            MARGE,
            24 + TAILLE_LOGO + 46,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU(ID_CHAMP_DOSSIER as *mut _),
            hinstance,
            None,
        )
        .ok()?;
        let vide = vers_wide("");
        let _ = SetWindowTheme(champ, PCWSTR(vide.as_ptr()), PCWSTR(vide.as_ptr()));
        SendMessageW(champ, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        let defaut_w = vers_wide(defaut);
        SendMessageW(champ, WM_SETTEXT, WPARAM(0), LPARAM(defaut_w.as_ptr() as isize));
        HWND_DOSSIER.with(|c| *c.borrow_mut() = Some(champ));

        let classe_bouton = vers_wide("BUTTON");
        let texte_parcourir = vers_wide(i18n::t(&langue, Cle::Parcourir));
        // BS_OWNERDRAW (voir WM_DRAWITEM ci-dessus, meme principe que
        // "Continuer") -- sans ca, ce bouton reste dessine en gris/blanc
        // par Windows et jure a cote du reste de la fenetre sombre.
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_bouton.as_ptr()),
            PCWSTR(texte_parcourir.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            24 + TAILLE_LOGO + 46 + 32,
            140,
            26,
            fenetre,
            HMENU(ID_BOUTON_PARCOURIR as *mut _),
            hinstance,
            None,
        );

        let texte_continuer = vers_wide(i18n::t(&langue, Cle::Continuer));
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_bouton.as_ptr()),
            PCWSTR(texte_continuer.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            24 + TAILLE_LOGO + 46 + 32 + 44,
            LARGEUR_CONTENU,
            36,
            fenetre,
            HMENU(ID_BOUTON_CONTINUER as *mut _),
            hinstance,
            None,
        );

        let _ = SetFocus(champ);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_KEYDOWN && VIRTUAL_KEY(msg.wParam.0 as u16) == VK_RETURN {
                valider_dossier(fenetre);
                continue;
            }
            if !IsDialogMessageW(fenetre, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        DOSSIER_CHOISI.with(|d| d.borrow_mut().take())
    }
}

const NOM_FICHIER_DOSSIER: &str = "dossier.txt";

fn chemin_fichier_dossier() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("VexCloudSync").join(NOM_FICHIER_DOSSIER)
}

/// Dossier choisi lors du tout premier lancement (voir `demander_dossier_destination`),
/// ou None si l'utilisateur n'a encore rien choisi -- dans ce cas l'appelant
/// (main.rs) doit encore afficher la fenetre de choix.
pub fn charger_dossier_choisi() -> Option<String> {
    std::fs::read_to_string(chemin_fichier_dossier())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn sauvegarder_dossier_choisi(chemin: &str) {
    let fichier = chemin_fichier_dossier();
    if let Some(parent) = fichier.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(fichier, chemin);
}

/// Retire les fichiers de configuration locale (dossier choisi + adresse
/// serveur) -- utilise pour "Reinstaller" (on repart de zero, la fenetre de
/// premier lancement redemandera tout) et pour "Desinstaller" (voir
/// `proposer_reinstallation` cote appelant dans main.rs). Ne touche PAS aux
/// fichiers synchronises de l'utilisateur -- seulement a la config de l'app.
pub fn effacer_configuration_locale() {
    let _ = std::fs::remove_file(chemin_fichier_url());
    let _ = std::fs::remove_file(chemin_fichier_dossier());
}

#[derive(Clone, Copy)]
pub enum ActionInstallation {
    Continuer,
    Reinstaller,
    Desinstaller,
}

unsafe fn choisir_action(hwnd: HWND, action: ActionInstallation) {
    ACTION_INSTALLATION.with(|c| *c.borrow_mut() = action);
    let _ = DestroyWindow(hwnd);
}

unsafe extern "system" fn wndproc_install(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let notif = ((wparam.0 >> 16) & 0xFFFF) as u32;
            // BUG CONSTATE EN PRATIQUE : BN_CLICKED et STN_CLICKED valent
            // tous les deux 0 en Win32. Un `if notif == BN_CLICKED { ... }
            // else if id == ID_ICONE_LANGUE && notif == STN_CLICKED { ... }`
            // fait donc tomber le clic sur l'icone dans la branche "bouton"
            // (le notif seul ne distingue rien), qui ne correspond a aucun
            // ID connu -- l'icone semblait ne rien faire. Chaque branche
            // verifie maintenant id ET notif ensemble, comme dans wndproc
            // (fenetre de connexion), qui n'avait pas ce bug.
            if id == ID_BOUTON_REINSTALLER && notif == BN_CLICKED as u32 {
                choisir_action(hwnd, ActionInstallation::Reinstaller);
            } else if id == ID_BOUTON_DESINSTALLER && notif == BN_CLICKED as u32 {
                choisir_action(hwnd, ActionInstallation::Desinstaller);
            } else if id == ID_ICONE_LANGUE && notif == STN_CLICKED {
                let icone = HWND(lparam.0 as *mut _);
                ouvrir_menu_langue(hwnd, icone);
            } else if id == IDCANCEL.0 {
                choisir_action(hwnd, ActionInstallation::Continuer);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            choisir_action(hwnd, ActionInstallation::Continuer);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            SetTextColor(hdc, COLORREF(COULEUR_TEXTE));
            SetBkColor(hdc, COLORREF(COULEUR_FOND));
            let brosse = BROSSE_FOND.with(|b| *b.borrow());
            LRESULT(brosse.map(|b| b.0 as isize).unwrap_or(0))
        }
        WM_DRAWITEM => {
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let langue = i18n::langue_courante();
            let details = if dis.CtlID == ID_BOUTON_REINSTALLER as u32 {
                Some((BROSSE_ACCENT.with(|b| *b.borrow()), Cle::ActionReinstaller))
            } else if dis.CtlID == ID_BOUTON_DESINSTALLER as u32 {
                Some((BROSSE_DANGER.with(|b| *b.borrow()), Cle::ActionDesinstaller))
            } else {
                None
            };
            if let Some((brosse, cle_texte)) = details {
                let brosse = brosse.unwrap_or_default();
                FillRect(dis.hDC, &dis.rcItem, brosse);
                SetBkMode(dis.hDC, TRANSPARENT);
                SetTextColor(dis.hDC, COLORREF(COULEUR_TEXTE));
                if let Some(police) = POLICE_BOUTON.with(|p| *p.borrow()) {
                    SelectObject(dis.hDC, HGDIOBJ(police.0));
                }
                let mut texte = vers_wide(i18n::t(&langue, cle_texte));
                texte.pop();
                let mut rect: RECT = dis.rcItem;
                DrawTextW(dis.hDC, &mut texte, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Fenetre stylee (remplace l'ancienne MessageBoxW Oui/Non/Annuler, qui
/// ressortait en gris/blanc natif Windows au milieu du reste de
/// l'application, entierement sombre -- retour utilisateur direct) demandant
/// quoi faire quand l'app detecte qu'elle est deja configuree sur cette
/// machine (voir `main` dans main.rs). Trois vrais boutons plutot que des
/// libelles Oui/Non/Annuler non personnalisables.
pub fn demander_action_installation(chemin_icone: &str, chemin_actuel: &str) -> ActionInstallation {
    unsafe {
        // Boucle : comme demander_mot_de_passe, choisir une langue dans le
        // menu (voir ouvrir_menu_langue) ferme cette fenetre et la rouvre
        // avec les nouveaux textes.
        loop {
        let langue = i18n::langue_courante();
        CHANGEMENT_LANGUE.with(|c| *c.borrow_mut() = false);
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return ActionInstallation::Continuer,
        };
        let nom_classe = vers_wide(CLASSE_FENETRE_INSTALL);

        let chemin_icone_w = vers_wide(chemin_icone);
        let charger_icone = |taille: i32| -> HICON {
            LoadImageW(None, PCWSTR(chemin_icone_w.as_ptr()), IMAGE_ICON, taille, taille, LR_LOADFROMFILE)
                .map(|h| HICON(h.0))
                .unwrap_or_default()
        };
        let hicone = charger_icone(0);
        let hicone_logo = charger_icone(64);

        let nom_police = vers_wide("Segoe UI");
        let police_titre = CreateFontW(
            22, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_normale = CreateFontW(
            15, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_bouton = CreateFontW(
            16, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        POLICE_BOUTON.with(|p| *p.borrow_mut() = Some(police_bouton));

        let brosse_fond = CreateSolidBrush(COLORREF(COULEUR_FOND));
        let brosse_champ = CreateSolidBrush(COLORREF(COULEUR_CHAMP));
        let brosse_accent = CreateSolidBrush(COLORREF(COULEUR_ACCENT));
        let brosse_danger = CreateSolidBrush(COLORREF(COULEUR_DANGER));
        BROSSE_FOND.with(|b| *b.borrow_mut() = Some(brosse_fond));
        BROSSE_CHAMP.with(|b| *b.borrow_mut() = Some(brosse_champ));
        BROSSE_ACCENT.with(|b| *b.borrow_mut() = Some(brosse_accent));
        BROSSE_DANGER.with(|b| *b.borrow_mut() = Some(brosse_danger));
        ACTION_INSTALLATION.with(|c| *c.borrow_mut() = ActionInstallation::Continuer);

        let classe = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc_install),
            hInstance: hinstance,
            lpszClassName: PCWSTR(nom_classe.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: hicone,
            hIconSm: hicone,
            hbrBackground: brosse_fond,
            ..Default::default()
        };
        RegisterClassExW(&classe);

        // Taille de fenetre calculee a partir de la taille de CLIENT voulue
        // (via AdjustWindowRectEx) plutot que devinee a la main -- BUG
        // CONSTATE EN PRATIQUE avec une taille totale fixe choisie au juger :
        // le troisieme bouton se retrouvait partiellement hors de la zone
        // client (masque par le bas de la fenetre), la barre de titre
        // rognant plus de hauteur que prevu.
        // BUG CONSTATE EN PRATIQUE : 420 ne correspondait a rien -- le
        // contenu (marge + LARGEUR_CONTENU + marge) ne va que jusqu'a 368,
        // laissant 52px de vide a droite et donnant l'impression que tout le
        // contenu est plaque a gauche au lieu d'etre centre dans la fenetre.
        const CLIENT_LARGEUR: i32 = MARGE + LARGEUR_CONTENU + MARGE;
        const CLIENT_HAUTEUR: i32 = 24 + 56 + 20 + 210 + 12 + 44 + 36 + 24;
        let style_fenetre = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let mut rect_fenetre = RECT { left: 0, top: 0, right: CLIENT_LARGEUR, bottom: CLIENT_HAUTEUR };
        let _ = AdjustWindowRectEx(&mut rect_fenetre, style_fenetre, false, WINDOW_EX_STYLE(0));

        let titre = vers_wide("VEX Cloud Client");
        let Ok(fenetre) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(nom_classe.as_ptr()),
            PCWSTR(titre.as_ptr()),
            style_fenetre | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect_fenetre.right - rect_fenetre.left,
            rect_fenetre.bottom - rect_fenetre.top,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        ) else {
            return ActionInstallation::Continuer;
        };

        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            fenetre,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        );
        let sombre: i32 = 1;
        let _ = DwmSetWindowAttribute(
            fenetre,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(DWMWA_USE_IMMERSIVE_DARK_MODE as i32),
            &sombre as *const _ as *const _,
            std::mem::size_of_val(&sombre) as u32,
        );

        let classe_static = vers_wide("STATIC");
        const TAILLE_LOGO: i32 = 56;
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_ICON as u32),
            MARGE,
            24,
            TAILLE_LOGO,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        let logo = FindWindowExW(fenetre, HWND::default(), PCWSTR(classe_static.as_ptr()), PCWSTR::null());
        if let Ok(logo) = logo {
            SendMessageW(logo, STM_SETICON, WPARAM(hicone_logo.0 as usize), LPARAM(0));
        }

        let titre_x = MARGE + TAILLE_LOGO + 16;
        let titre_texte = vers_wide(i18n::t(&langue, Cle::DejaInstalleTitre));
        let titre_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(titre_texte.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            titre_x,
            24,
            // -32 : laisse la place a l'icone de langue en haut a droite
            // (24px + marge) -- BUG CONSTATE EN PRATIQUE sans ca : le
            // rectangle du titre s'etendait par-dessus l'icone, et son
            // repeint (fond uni via WM_CTLCOLORSTATIC) la recouvrait par
            // moments, la rendant invisible selon l'ordre de dessin.
            MARGE + LARGEUR_CONTENU - titre_x - 32,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(t) = titre_ctrl {
            SendMessageW(t, WM_SETFONT, WPARAM(police_titre.0 as usize), LPARAM(1));
        }

        // Icone de langue, en haut a droite -- meme mecanisme que dans
        // demander_mot_de_passe (voir ouvrir_menu_langue).
        let icone_globe = charger_icone_globe();
        let icone_globe_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE((SS_ICON | SS_NOTIFY) as u32),
            MARGE + LARGEUR_CONTENU - 24,
            30,
            24,
            24,
            fenetre,
            HMENU(ID_ICONE_LANGUE as *mut _),
            hinstance,
            None,
        );
        if let Ok(ctrl) = icone_globe_ctrl {
            SendMessageW(ctrl, STM_SETICON, WPARAM(icone_globe.0 as usize), LPARAM(0));
        }

        // Corps explicatif -- multi-lignes, un STATIC standard fait deja le
        // retour a la ligne automatique tant que sa hauteur est suffisante.
        let corps_texte = i18n::t(&langue, Cle::DejaInstalleTexte).replace("{chemin}", chemin_actuel);
        let corps_w = vers_wide(&corps_texte);
        let corps_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(corps_w.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGE,
            24 + TAILLE_LOGO + 20,
            LARGEUR_CONTENU,
            210,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(c) = corps_ctrl {
            SendMessageW(c, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        }

        const Y_BOUTONS: i32 = 24 + 56 + 20 + 210 + 12;
        let classe_bouton = vers_wide("BUTTON");

        let texte_reinstaller = vers_wide(i18n::t(&langue, Cle::ActionReinstaller));
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_bouton.as_ptr()),
            PCWSTR(texte_reinstaller.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            Y_BOUTONS,
            LARGEUR_CONTENU,
            36,
            fenetre,
            HMENU(ID_BOUTON_REINSTALLER as *mut _),
            hinstance,
            None,
        );
        let texte_desinstaller = vers_wide(i18n::t(&langue, Cle::ActionDesinstaller));
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_bouton.as_ptr()),
            PCWSTR(texte_desinstaller.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            Y_BOUTONS + 44,
            LARGEUR_CONTENU,
            36,
            fenetre,
            HMENU(ID_BOUTON_DESINSTALLER as *mut _),
            hinstance,
            None,
        );
        // Pas de troisieme bouton "Continuer" : fermer la fenetre (Echap,
        // croix, ou simplement l'ignorer) a deja exactement cet effet --
        // superflu, remonte par l'utilisateur.

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if !IsDialogMessageW(fenetre, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if CHANGEMENT_LANGUE.with(|c| *c.borrow()) {
            continue;
        }
        return ACTION_INSTALLATION.with(|c| *c.borrow());
        } // loop
    }
}

/// Affiche une petite fenetre native avec un champ mot de passe et un
/// bouton "Connecter". Bloque jusqu'a la validation ou la fermeture de la
/// fenetre. Retourne None si l'utilisateur ferme sans rien saisir.
/// `chemin_icone` : fichier .ico a afficher dans la barre de titre et la
/// barre des taches (extrait par `extraire_icones_locales` dans main.rs).
/// Retourne (mot_de_passe, url_serveur).
pub fn demander_mot_de_passe(chemin_icone: &str) -> Option<(String, String)> {
    unsafe {
        // Boucle : choisir une langue dans le menu deroulant (voir plus bas)
        // ferme cette fenetre et la rouvre avec les nouveaux textes, plutot
        // que d'essayer de re-appliquer les textes en direct sur les
        // controles existants -- meme cycle de vie propre (une fenetre, une
        // boucle de messages, une fermeture) que le reste de ce fichier.
        loop {
        let langue = i18n::langue_courante();
        CHANGEMENT_LANGUE.with(|c| *c.borrow_mut() = false);

        let hinstance = GetModuleHandleW(None).ok()?.into();
        let nom_classe = vers_wide(CLASSE_FENETRE);

        // LoadImageW depuis un fichier plutot qu'une ressource de l'exe :
        // coherent avec extraire_icones_locales, qui ecrit deja les .ico sur
        // le disque de l'utilisateur pour l'icone du dossier de synchro.
        // Deux tailles : petite pour la barre de titre/taches, grande pour le
        // logo affiche dans le corps de la fenetre.
        let chemin_icone_w = vers_wide(chemin_icone);
        let charger_icone = |taille: i32| -> HICON {
            LoadImageW(None, PCWSTR(chemin_icone_w.as_ptr()), IMAGE_ICON, taille, taille, LR_LOADFROMFILE)
                .map(|h| HICON(h.0))
                .unwrap_or_default()
        };
        let hicone = charger_icone(0);
        let hicone_logo = charger_icone(64);

        // Polices Segoe UI (au lieu de la police systeme "MS Shell Dlg" par
        // defaut, datee) -- titre en gras, reste en normal.
        let nom_police = vers_wide("Segoe UI");
        let police_titre = CreateFontW(
            22, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_normale = CreateFontW(
            16, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        let police_bouton = CreateFontW(
            17, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32, ANTIALIASED_QUALITY.0 as u32, (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(nom_police.as_ptr()),
        );
        POLICE_BOUTON.with(|p| *p.borrow_mut() = Some(police_bouton));

        let brosse_fond = CreateSolidBrush(COLORREF(COULEUR_FOND));
        let brosse_champ = CreateSolidBrush(COLORREF(COULEUR_CHAMP));
        let brosse_accent = CreateSolidBrush(COLORREF(COULEUR_ACCENT));
        BROSSE_FOND.with(|b| *b.borrow_mut() = Some(brosse_fond));
        BROSSE_CHAMP.with(|b| *b.borrow_mut() = Some(brosse_champ));
        BROSSE_ACCENT.with(|b| *b.borrow_mut() = Some(brosse_accent));

        let classe = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: PCWSTR(nom_classe.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: hicone,
            hIconSm: hicone,
            hbrBackground: brosse_fond,
            ..Default::default()
        };
        RegisterClassExW(&classe);

        let titre = vers_wide("VEX Cloud Client");
        let fenetre = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(nom_classe.as_ptr()),
            PCWSTR(titre.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            392,
            446,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
        .ok()?;

        // Windows 11 : coins arrondis natifs, comme toutes les fenetres
        // modernes du systeme (sans ca, une fenetre creee "a la main" reste
        // carree meme sur Windows 11). Ignore silencieusement sur Windows 10
        // (attribut inconnu, sans consequence).
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            fenetre,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        );

        // Barre de titre en mode sombre (dessinee par Windows, pas par
        // nous) -- sans ca, elle reste blanche par defaut et jure a cote du
        // contenu sombre de la fenetre, meme si tout le reste est theme.
        let sombre: i32 = 1;
        let _ = DwmSetWindowAttribute(
            fenetre,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(DWMWA_USE_IMMERSIVE_DARK_MODE as i32),
            &sombre as *const _ as *const _,
            std::mem::size_of_val(&sombre) as u32,
        );

        // En-tete : logo VEX en grand a gauche, titre + sous-titre a droite
        // -- meme composition qu'un ecran d'accueil d'installeur.
        let classe_static = vers_wide("STATIC");
        const TAILLE_LOGO: i32 = 56;
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SS_ICON as u32),
            MARGE,
            24,
            TAILLE_LOGO,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        // SS_ICON lit l'icone a afficher via STM_SETICON, pas via le texte
        // de creation -- on l'envoie juste apres.
        let logo = FindWindowExW(fenetre, HWND::default(), PCWSTR(classe_static.as_ptr()), PCWSTR::null());
        if let Ok(logo) = logo {
            SendMessageW(logo, STM_SETICON, WPARAM(hicone_logo.0 as usize), LPARAM(0));
        }

        // Titre a droite du logo, aligne sur le meme bord droit que le
        // champ et le bouton plus bas -- hauteur genereuse (36px) pour que
        // la police de 22px n'ait aucune chance d'etre rognee en bas.
        let titre_x = MARGE + TAILLE_LOGO + 16;
        let titre_texte = vers_wide("VEX Cloud Client");
        let titre_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(titre_texte.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            titre_x,
            24,
            // -32 : laisse la place a l'icone de langue en haut a droite,
            // voir le meme correctif dans demander_action_installation.
            MARGE + LARGEUR_CONTENU - titre_x - 32,
            TAILLE_LOGO,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(t) = titre_ctrl {
            SendMessageW(t, WM_SETFONT, WPARAM(police_titre.0 as usize), LPARAM(1));
        }

        // Adresse du serveur -- configurable par utilisateur au lieu de se
        // limiter aux deux adresses codees en dur (BASE_URL_CANDIDATS dans
        // main.rs), utile pour qui heberge sa propre instance VEX. Pre-
        // remplie avec la derniere valeur utilisee (voir charger_url_preferee
        // / sauvegarder_url_preferee) ou le premier candidat par defaut.
        let label_url = vers_wide(i18n::t(&langue, Cle::AdresseServeur));
        let label_url_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(label_url.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGE,
            24 + TAILLE_LOGO + 20,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(l) = label_url_ctrl {
            SendMessageW(l, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        }

        let classe_edit = vers_wide("EDIT");
        let champ_url = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_edit.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            MARGE,
            24 + TAILLE_LOGO + 46,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU(ID_CHAMP_URL as *mut _),
            hinstance,
            None,
        )
        .ok()?;
        let _ = SetWindowTheme(champ_url, PCWSTR(vers_wide("").as_ptr()), PCWSTR(vers_wide("").as_ptr()));
        SendMessageW(champ_url, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        let url_defaut = vers_wide(&charger_url_preferee());
        SendMessageW(champ_url, WM_SETTEXT, WPARAM(0), LPARAM(url_defaut.as_ptr() as isize));
        HWND_URL.with(|c| *c.borrow_mut() = Some(champ_url));

        const DECALAGE: i32 = 56;

        let label = vers_wide(i18n::t(&langue, Cle::MotDePasse));
        let label_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR(label.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            MARGE,
            24 + TAILLE_LOGO + 20 + DECALAGE,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU::default(),
            hinstance,
            None,
        );
        if let Ok(l) = label_ctrl {
            SendMessageW(l, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        }

        // Cadre plat (WS_BORDER) plutot que le rebord 3D classique
        // (WS_EX_CLIENTEDGE) : plus coherent avec le theme sombre, et son
        // biseau asymetrique deformait legerement le centrage vertical du
        // texte/des ronds de masquage a l'interieur du champ.
        let champ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_edit.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32),
            MARGE,
            24 + TAILLE_LOGO + 46 + DECALAGE,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU(ID_CHAMP_MDP as *mut _),
            hinstance,
            None,
        )
        .ok()?;
        // Comme le bouton "Connecter" : un controle "theme" par Windows
        // ignore la couleur de fond qu'on lui donne via WM_CTLCOLOREDIT --
        // d'ou le champ qui restait blanc malgre le code plus haut. On
        // desactive le theme AVANT d'appliquer la police (l'ordre compte :
        // appliquer la police sur un controle encore theme, puis desactiver
        // le theme apres, laissait le texte colle en haut du champ au lieu
        // d'etre centre verticalement).
        let vide = vers_wide("");
        let _ = SetWindowTheme(champ, PCWSTR(vide.as_ptr()), PCWSTR(vide.as_ptr()));
        SendMessageW(champ, WM_SETFONT, WPARAM(police_normale.0 as usize), LPARAM(1));
        HWND_CHAMP.with(|c| *c.borrow_mut() = Some(champ));

        // Case a cocher "Afficher le mot de passe" en BS_OWNERDRAW -- comme
        // pour "Connecter", une BS_AUTOCHECKBOX standard reste dessinee par
        // Windows (petit carre blanc generique) et jure a cote du reste de
        // la fenetre. Dessin manuel (voir WM_DRAWITEM) pour un rendu assorti
        // a la charte du site.
        let classe_case = vers_wide("BUTTON");
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_case.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            24 + TAILLE_LOGO + 46 + 36 + DECALAGE,
            LARGEUR_CONTENU,
            22,
            fenetre,
            HMENU(ID_CASE_AFFICHER as *mut _),
            hinstance,
            None,
        );

        let classe_bouton = vers_wide("BUTTON");
        let texte_bouton = vers_wide(i18n::t(&langue, Cle::Connecter));
        // BS_OWNERDRAW (voir WM_DRAWITEM ci-dessus) : seul moyen fiable
        // d'obtenir un bouton entierement colore -- BS_DEFPUSHBUTTON est
        // ignore par Windows des qu'un style de dessin personnalise est
        // demande, donc la touche Entree est reprise a la main plus bas.
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_bouton.as_ptr()),
            PCWSTR(texte_bouton.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            MARGE,
            24 + TAILLE_LOGO + 46 + 36 + 32 + DECALAGE,
            LARGEUR_CONTENU,
            36,
            fenetre,
            HMENU(ID_BOUTON as *mut _),
            hinstance,
            None,
        );

        // Selecteur de langue -- juste l'icone (SS_NOTIFY : un STATIC peut
        // recevoir des clics et les relayer en STN_CLICKED, voir WM_COMMAND
        // ci-dessus), pas de combobox ni de libelle texte. En haut a droite
        // de la fenetre, alignee sur la meme rangee que le logo/titre. Un
        // clic ouvre un vrai petit menu sombre juste en-dessous (voir
        // ouvrir_menu_langue) -- pas la combobox blanche/theme clair de
        // Windows d'avant.
        let icone_globe = charger_icone_globe();
        let icone_globe_ctrl = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(classe_static.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE((SS_ICON | SS_NOTIFY) as u32),
            MARGE + LARGEUR_CONTENU - 24,
            30,
            24,
            24,
            fenetre,
            HMENU(ID_ICONE_LANGUE as *mut _),
            hinstance,
            None,
        );
        if let Ok(ctrl) = icone_globe_ctrl {
            SendMessageW(ctrl, STM_SETICON, WPARAM(icone_globe.0 as usize), LPARAM(0));
        }

        let _ = SetFocus(champ);

        // IsDialogMessageW donne a cette fenetre normale le comportement
        // standard d'une boite de dialogue : Tab pour changer de champ,
        // Echap pour fermer. Entree est interceptee ici directement (le
        // bouton BS_OWNERDRAW n'est plus reconnu comme bouton "par defaut"
        // par IsDialogMessageW, qui ne la relaierait donc pas).
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_KEYDOWN && VIRTUAL_KEY(msg.wParam.0 as u16) == VK_RETURN {
                valider(fenetre);
                continue;
            }
            if !IsDialogMessageW(fenetre, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if CHANGEMENT_LANGUE.with(|c| *c.borrow()) {
            continue; // rouvre la fenetre au tout debut de la boucle, langue mise a jour
        }

        let mdp = MOT_DE_PASSE.with(|m| m.borrow_mut().take()).filter(|s| !s.is_empty())?;
        let url = URL_SAISIE.with(|u| u.borrow_mut().take()).unwrap_or_default();
        let url = if url.trim().is_empty() { charger_url_preferee() } else { url.trim().to_string() };
        sauvegarder_url_preferee(&url);
        return Some((mdp, url));
        } // loop
    }
}

/// Boite de dialogue native simple (remplace l'ancienne page de statut
/// web). Utilisee pour afficher l'etat courant depuis le menu de la
/// barre des taches.
pub fn afficher_message(titre: &str, texte: &str) {
    unsafe {
        let t = vers_wide(titre);
        let m = vers_wide(texte);
        MessageBoxW(None, PCWSTR(m.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONINFORMATION);
    }
}

/// Charge un .ico depuis le disque et retourne le handle brut (valeur
/// numerique HICON) pour tray_item::IconSource::RawIcon -- ce crate attend
/// un HICON de `windows_sys` (simple isize), pas le type `windows::...::HICON`
/// (struct) utilise partout ailleurs dans ce fichier ; meme valeur, juste
/// une autre bibliotheque de bindings Win32. Retourne 0 en cas d'echec
/// (tray_item traite un HICON nul comme une erreur, gere par l'appelant).
pub fn charger_icone_brute(chemin: &str, taille: i32) -> isize {
    unsafe {
        let chemin_w = vers_wide(chemin);
        LoadImageW(None, PCWSTR(chemin_w.as_ptr()), IMAGE_ICON, taille, taille, LR_LOADFROMFILE)
            .map(|h| h.0 as isize)
            .unwrap_or(0)
    }
}

/// Meme icone que `charger_icone_brute`, avec un badge rond vert + coche
/// blanche ajoute en bas a droite -- utilisee dans la barre des taches une
/// fois la synchro etablie avec succes, pour distinguer visuellement
/// "connecte" de "en cours de connexion".
pub fn charger_icone_connectee(chemin: &str, taille: i32) -> isize {
    unsafe {
        let chemin_w = vers_wide(chemin);
        let Ok(hicone) = LoadImageW(None, PCWSTR(chemin_w.as_ptr()), IMAGE_ICON, taille, taille, LR_LOADFROMFILE)
        else {
            return 0;
        };
        let hicone = HICON(hicone.0);

        let hdc_ecran = GetDC(None);
        let hdc_mem = CreateCompatibleDC(hdc_ecran);

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: taille,
                biHeight: -taille, // top-down, plus simple pour DrawIconEx
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(hdc_mem, &mut bmi, DIB_RGB_COLORS, &mut pixels, None, 0)
        else {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_ecran);
            return hicone.0 as isize;
        };
        let ancien = SelectObject(hdc_mem, HGDIOBJ(bitmap.0));

        // Fond transparent (alpha=0 partout), puis l'icone de base dessinee
        // par-dessus avec son propre canal alpha preserve.
        std::ptr::write_bytes(pixels as *mut u8, 0, (taille * taille * 4) as usize);
        let _ = DrawIconEx(hdc_mem, 0, 0, hicone, taille, taille, 0, None, DI_NORMAL);

        // Badge : petit disque vert avec une coche blanche, coin bas-droit,
        // taille proportionnelle a l'icone (environ 45%).
        let taille_badge = (taille as f32 * 0.5) as i32;
        let x0 = taille - taille_badge;
        let y0 = taille - taille_badge;
        let brosse_badge = CreateSolidBrush(COLORREF(COULEUR_ACCENT));
        let stylo_nul = GetStockObject(NULL_PEN);
        let ancien_stylo = SelectObject(hdc_mem, stylo_nul);
        let ancienne_brosse = SelectObject(hdc_mem, HGDIOBJ(brosse_badge.0));
        let _ = Ellipse(hdc_mem, x0, y0, taille, taille);
        SelectObject(hdc_mem, ancienne_brosse);
        SelectObject(hdc_mem, ancien_stylo);
        let _ = DeleteObject(HGDIOBJ(brosse_badge.0));

        SetBkMode(hdc_mem, TRANSPARENT);
        SetTextColor(hdc_mem, COLORREF(0x00FFFFFF));
        let mut coche = vers_wide("\u{2713}");
        coche.pop();
        let mut rect_badge = RECT { left: x0, top: y0, right: taille, bottom: taille };
        DrawTextW(hdc_mem, &mut coche, &mut rect_badge, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

        // Re-forcer l'alpha du badge a 255 : les fonctions GDI classiques
        // (Ellipse, DrawTextW) n'ecrivent pas le canal alpha d'un DIB 32bpp,
        // qui resterait donc transparent malgre les pixels de couleur.
        let pixels_u32 = pixels as *mut u32;
        for y in y0.max(0)..taille {
            for x in x0.max(0)..taille {
                let idx = (y * taille + x) as usize;
                let p = *pixels_u32.add(idx);
                if p != 0 {
                    *pixels_u32.add(idx) = p | 0xFF000000;
                }
            }
        }

        SelectObject(hdc_mem, ancien);
        let masque = CreateBitmap(taille, taille, 1, 1, None);
        let mut icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: masque,
            hbmColor: bitmap,
        };
        let nouvelle_icone = CreateIconIndirect(&mut icon_info);

        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteObject(HGDIOBJ(masque.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_ecran);
        let _ = DestroyIcon(hicone);

        nouvelle_icone.map(|h| h.0 as isize).unwrap_or(0)
    }
}

const NOM_FICHIER_URL: &str = "serveur.txt";

fn chemin_fichier_url() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("VexCloudSync").join(NOM_FICHIER_URL)
}

/// Derniere adresse de serveur saisie par l'utilisateur, ou l'adresse VEX
/// par defaut si rien n'a encore ete sauvegarde.
pub fn charger_url_preferee() -> String {
    std::fs::read_to_string(chemin_fichier_url())
        .ok()
        .map(|s| s.trim().to_string())
        // HTTPS force (voir main.rs::exiger_https) : une adresse http://
        // sauvegardee avant ce durcissement (ou modifiee a la main dans le
        // fichier) ne doit pas revenir se pre-remplir dans le champ comme si
        // de rien n'etait -- filtree ici, retombe sur le defaut https.
        .filter(|s| !s.is_empty() && s.starts_with("https://"))
        .unwrap_or_else(|| "https://vex.hopto.org".to_string())
}

fn sauvegarder_url_preferee(url: &str) {
    let chemin = chemin_fichier_url();
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(chemin, url);
}
