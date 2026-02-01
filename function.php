
<?php

function getPrivilegeDetails($user_privilege) {
    $privileges = [
        12 => ['nom_privilege' => 'ban', 'couleur_privilege' => '#000000'],
        11 => ['nom_privilege' => 'ban', 'couleur_privilege' => '#000000'],
        10 => ['nom_privilege' => 'aucun', 'couleur_privilege' => '#000000'],
        9  => ['nom_privilege' => 'beta-testeur', 'couleur_privilege' => '#20012a'],
        8  => ['nom_privilege' => 'utilisateur certifie', 'couleur_privilege' => '#4b4b4b'],
        7  => ['nom_privilege' => 'Moderateur', 'couleur_privilege' => '#32cd32'],
        6  => ['nom_privilege' => 'Super-moderateur', 'couleur_privilege' => '#006400'],
        5  => ['nom_privilege' => 'Developpeur', 'couleur_privilege' => '#4169e1'],
        4  => ['nom_privilege' => 'Developpeur Principal', 'couleur_privilege' => '#4169e1'],
        3  => ['nom_privilege' => 'admin', 'couleur_privilege' => '#d30000'],
        2  => ['nom_privilege' => 'super admin', 'couleur_privilege' => '#6d0000'],
        1  => ['nom_privilege' => 'fondateur', 'couleur_privilege' => 'fona'],
    ];

    return $privileges[$user_privilege] ?? ['nom_privilege' => 'inconnu', 'couleur_privilege' => '#ffffff'];
}
function getnameDetails($fuser_id) {
    $servername = "localhost";
    $username = "orsql";
    $password = "iDq]25F0u8v*z[1d";
    $dbname = "user";

    // Create a new MySQLi connection
    $conn = new mysqli($servername, $username, $password, $dbname);

    // Check connection
    if ($conn->connect_error) {
        die("La connexion a échoué : " . $conn->connect_error);
    }

    // Prepare the SQL query
    $stmt = $conn->prepare("SELECT * FROM `login` WHERE id = ?");
    if (!$stmt) {
        die("Échec de préparation de la requête : " . $conn->error);
    }

    // Bind parameters and execute the statement
    $stmt->bind_param("i", $fuser_id);
    $stmt->execute();
    $result = $stmt->get_result();

    // Check if a row was returned
    if ($result->num_rows === 1) {
        $row = $result->fetch_assoc();
        $fuser_nom = $row['nom']; // Assuming 'nom' is the correct field
    } else {
        $fuser_nom = null; // No user found
    }

    // Free result and close connections
    $stmt->close();
  


    return $fuser_nom;
}
//regalge()
function regalge() {

    echo 
   
    '<style>.vertical-menu {
    display: flex;
    flex-direction: column;
}

.vertical-menu a {
    background-color: #eee;
    color: black;
    padding: 8px;
    text-decoration: none;
    text-align: center;
    border-bottom: 1px solid #ddd;
    font-size: 14px;
}

.vertical-menu a:hover {
    background-color: #ccc;
}

.vertical-menu a.active {
    background-color: #4caf50;
    color: white;
}
.menu-container {
  display: none;
}
.container {
  background-color: #fff;
  padding: 10px;
  border-radius: 8px;
  box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
  width: 200px;
  position: absolute;
  top: 60px;
  right: 0;
  display: none;
  z-index: 1;
}

.profile-img {
           margin-right: 10px; 
			width: 30px;
			height: 30px;
			border-radius: 50%;
			cursor: pointer;
			margin-top: 3px; 
		}
        .profile-container {
           margin-left: auto;
            
            justify-content: flex-end;
            align-items: flex-start;
            display: flex;
            font-family: Arial, sans-serif;
        }

</style>
   <div class="profile-container">
        <img src="/login/regalge.png" alt="Profil" class="profile-img" id="profile-img">
        <div class="container" id="menu-container" style="display: none;">
            <div class="vertical-menu">
                <a href="/login/account.php" id="account-link">Mon Compte</a>
                <a href="/login/account.php" id="settings-link">Parametres</a>
                <a href="/login/logout.php">Deconnexion</a>
            </div> 
            <div id="settings-form" class="form-container">
            </div>
        </div>
    </div>
    ';echo "  
 <script>
        document.getElementById('profile-img').addEventListener('click', function() {
            var menuContainer = document.getElementById('menu-container');
            if (menuContainer.style.display === 'none' || menuContainer.style.display === '') {
                menuContainer.style.display = 'block';
            } else {
                menuContainer.style.display = 'none';
            }
        });
        document.getElementById('account-link').addEventListener('click', function() {
            document.getElementById('account-form').style.display = 'flex';
            document.getElementById('settings-form').style.display = 'none';
        });
        document.getElementById('settings-link').addEventListener('click', function() {
            document.getElementById('settings-form').style.display = 'flex';
            document.getElementById('account-form').style.display = 'none';
        });
    </script>

    ";
}

function footer() {
include "/var/www/html/c.php"
;
}
/*$tagUserColumn =  	'vAutologin';
$tagUserValue =verifierrr($userId, $tagUserColumn, $conn);
 */
function migrant(){
  echo '<script>
        function migrant() {
            window.location.href = "/login/login.php";
        }
    
    </script>';
     header('Location: /login/login.php');
    echo '<script>migrant()</script>';
};
function fcichier() {
        
      $icon_map = [
                                'jpg' => 'fa-file-image',//img
                                'tiff' => '"><img src="/img/fill-image-etoile.svg" class="icone-fichier',
                                'tif' =>  '"><img src="/img/fill-image-etoile.svg" class="icone-fichier',
                                'jpeg' => 'fa-file-image',
                                'svg' => 'fa-file-image',
                                'gif' => 'fa-file-image',
                                'psd' => '"><img src="/img/fill-ps.svg" class="icone-fichier',


                                'mp4' => 'fa-file-video',//video
                                'webm' => 'fa-file-video',

                                'pdf' => 'fa-file-pdf',//doc
                                'doc' => 'fa-file-word',
                                'docx' => 'fa-file-word',
                                'xls' => 'fa-file-excel',
                                'xlsx' => 'fa-file-excel',
                                'ppt' => 'fa-file-powerpoint',
                                'pptx' => 'fa-file-powerpoint',
                                'txt' => 'fa-file-lines',
                                'csv' => 'fa-file-csv',

                                'zip' => 'fa-file-zipper',//archive
                                'gz' => 'fa-file-zipper',
                                'rar' => 'fa-file-zipper',
                                '7z' => 'fa-file-zipper',

                                'sql' => 'fa-database', //fichier code
                                'php' => 'fa-file-code',
                                'html' => 'fa-file-code',
                                'css' => 'fa-file-code',
                                'js' => 'fa-file-code',
                                'json' => 'fa-file-code',
                                'xml' => 'fa-file-code',
                                'exe' => '"><img src="/img/fill-exe-bat.svg" class="icone-fichier',
                                'bat' => '"><img src="/img/fill-exe-bat.svg" class="icone-fichier',

                                'Mtl' => 'fa-solid fa-cube',//3D files
                                'Obj' => 'fa-solid fa-cube',
                                'Fbxl' => 'fa-solid fa-cube',
                                'Fbx' => 'fa-solid fa-cube',
                                'stl' => 'fa-solid fa-cube',
                                'gcode' =>  '"><img src="/img/fill-gcode.svg" class="icone-fichier',
                                'mtl' =>   'fa-solid fa-cube',

                     
                                'default' => 'fa-file',

                            ];
  return $icon_map;
}
function verifierrr($iduserav, $tagUserColumn, $conn) {
    $tagUserValue = 0;
    
    // Vérifier la valeur de $tagUserColumn dans `tag-user`
    $tagUserQuery = "SELECT $tagUserColumn FROM `tag-user` WHERE `user-id` = ?";
    $stmt = $conn->prepare($tagUserQuery);
    $stmt->bind_param("i", $iduserav);
    $stmt->execute();
    $stmt->bind_result($tagUserValue);
    $stmt->fetch();
    $stmt->close();

    $tagUserValuetout = 'non';
    $tagUserQuery = "SELECT tout FROM `tag-user` WHERE `user-id` = ?";
    $stmt = $conn->prepare($tagUserQuery);
    $stmt->bind_param("i", $iduserav);
    $stmt->execute();
    $stmt->bind_result($tagUserValuetout);
    $stmt->fetch();
    $stmt->close();

    // Vérifier la valeur de `tout`
    if ($tagUserValuetout === 'v') {
        $tagUserValue = 'non';
    } elseif ($tagUserValuetout !== 'non') {  // Corrigé `=` en `===`

        $chrechedb = 'login';
        $weredb = 'id';
        $vefier = '';  // Initialiser correctement cette variable
        $naisairedecopar = 1;

        if ($tagUserColumn === 'VMotdePasse') {
            $loginvaleur = 'motdepass';
        } elseif ($tagUserColumn === 'VPrivilege') {
            $loginvaleur = 'privilege';
        } elseif ($tagUserColumn === 'VVIP') {
            $loginvaleur = 'vip';
        } elseif ($tagUserColumn === 'vcreAutologin') {
            $chrechedb = 'autologin';
            $loginvaleur = 'nombre';
            $weredb = 'compteid';
        } elseif ($tagUserColumn === 'vAutologin') {
            $naisairedecopar = 0;
        } elseif ($tagUserColumn === 'VEmail') {
            $loginvaleur = 'Email';
        }

        if ($naisairedecopar === 1) {  // Corrigé `"1"` en `1` pour comparaison stricte
            $tagUserQuery = "SELECT $loginvaleur FROM $chrechedb WHERE $weredb = ?";
            $stmt = $conn->prepare($tagUserQuery);
            $stmt->bind_param("i", $iduserav);
            $stmt->execute();
            $stmt->bind_result($vefier);
            $stmt->fetch();
            $stmt->close();
        } else {
            $vefier = "nonononoon"; // Correction d'affectation
        }

        // Debug
        

        if ($tagUserValue === "v" || $tagUserValue === $vefier) {
            $tagUserValue = 'non';
        } else {
            $tagUserValue = 'oui';
        }
    }

    return $tagUserValue;
}






/**
 * Récupère les préférences d'un utilisateur
 * @param int $userId ID de l'utilisateur
 * @param mysqli $conn Connexion à la base de données
 * @return array|null Tableau des préférences ou null si erreur
 */
function getUserPreferences($userId, $conn = null) {
    $shouldCloseConn = false;
    
    if ($conn === null) {
        $servername = "localhost";
        $username = "orsql";
        $password = "iDq]25F0u8v*z[1d";
        $dbname = "user";
        $conn = new mysqli($servername, $username, $password, $dbname);
        
        if ($conn->connect_error) {
            error_log("Erreur de connexion getUserPreferences: " . $conn->connect_error);
            return null;
        }
        $shouldCloseConn = true;
    }
    
    // Vérifier si l'utilisateur a des préférences
    $stmt = $conn->prepare("SELECT `teme`, `langue`, `notifications_meet`, `auto_record`, `mic_default`, `camera_default`, `quality_video`, `profile_icon_type`, `profile_icon_url` FROM `pref` WHERE `id-user` = ?");
    if (!$stmt) {
        error_log("Erreur de préparation getUserPreferences: " . $conn->error);
        if ($shouldCloseConn) $conn->close();
        return null;
    }
    
    $stmt->bind_param("i", $userId);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows === 0) {
        // Créer les préférences par défaut si elles n'existent pas
        $stmt->close();
        $stmt = $conn->prepare("INSERT INTO `pref` (`id-user`, `teme`, `langue`, `profile_icon_type`) VALUES (?, 0, 'fr', 'initials')");
        $stmt->bind_param("i", $userId);
        $stmt->execute();
        $stmt->close();
        
        if ($shouldCloseConn) $conn->close();
        return [
            'teme' => 0,
            'langue' => 'fr',
            'notifications_meet' => 1,
            'auto_record' => 0,
            'mic_default' => 0,
            'camera_default' => 0,
            'quality_video' => 'auto',
            'profile_icon_type' => 'initials',
            'profile_icon_url' => null
        ];
    }
    
    $row = $result->fetch_assoc();
    $stmt->close();
    
    if ($shouldCloseConn) $conn->close();
    
    return [
        'teme' => (int)$row['teme'],
        'langue' => $row['langue'] ?? 'fr',
        'notifications_meet' => (int)($row['notifications_meet'] ?? 1),
        'auto_record' => (int)($row['auto_record'] ?? 0),
        'mic_default' => (int)($row['mic_default'] ?? 0),
        'camera_default' => (int)($row['camera_default'] ?? 0),
        'quality_video' => $row['quality_video'] ?? 'auto',
        'profile_icon_type' => $row['profile_icon_type'] ?? 'initials',
        'profile_icon_url' => $row['profile_icon_url'] ?? null
    ];
}

/**
 * Met à jour les préférences d'un utilisateur
 * @param int $userId ID de l'utilisateur
 * @param string $prefName Nom de la préférence ('teme', etc.)
 * @param mixed $prefValue Valeur de la préférence
 * @param mysqli $conn Connexion à la base de données
 * @return bool True si succès, false sinon
 */
function updateUserPreference($userId, $prefName, $prefValue, $conn = null) {
    $shouldCloseConn = false;
    
    if ($conn === null) {
        $servername = "localhost";
        $username = "orsql";
        $password = "iDq]25F0u8v*z[1d";
        $dbname = "user";
        $conn = new mysqli($servername, $username, $password, $dbname);
        
        if ($conn->connect_error) {
            error_log("Erreur de connexion updateUserPreference: " . $conn->connect_error);
            return false;
        }
        $shouldCloseConn = true;
    }
    
    // Valider le nom de la préférence
    $allowedPrefs = ['teme', 'langue', 'notifications_meet', 'auto_record', 'mic_default', 'camera_default', 'quality_video'];
    if (!in_array($prefName, $allowedPrefs)) {
        error_log("Préférence non valide: " . $prefName);
        if ($shouldCloseConn) $conn->close();
        return false;
    }
    
    // Vérifier si l'utilisateur existe déjà dans pref
    $stmt = $conn->prepare("SELECT `id-user` FROM `pref` WHERE `id-user` = ?");
    $stmt->bind_param("i", $userId);
    $stmt->execute();
    $result = $stmt->get_result();
    $exists = $result->num_rows > 0;
    $stmt->close();
    
    if ($exists) {
        // Mise à jour
        $stmt = $conn->prepare("UPDATE `pref` SET `$prefName` = ? WHERE `id-user` = ?");
        if ($prefName === 'langue' || $prefName === 'quality_video') {
            $stmt->bind_param("si", $prefValue, $userId);
        } else {
            $stmt->bind_param("ii", $prefValue, $userId);
        }
    } else {
        // Insertion
        $stmt = $conn->prepare("INSERT INTO `pref` (`id-user`, `$prefName`) VALUES (?, ?)");
        if ($prefName === 'langue' || $prefName === 'quality_video') {
            $stmt->bind_param("is", $userId, $prefValue);
        } else {
            $stmt->bind_param("ii", $userId, $prefValue);
        }
    }
    
    $success = $stmt->execute();
    $stmt->close();
    
    if ($shouldCloseConn) $conn->close();
    
    return $success;
}
function loadLanguage($lang = 'fr', $section = 'all') {
    $langFile = __DIR__ . "/lang/{$lang}.php";
    
    // Langue par défaut si fichier introuvable
    if (!file_exists($langFile)) {
        error_log("Fichier langue introuvable: {$lang}.php, utilisation de fr.php");
        $lang = 'fr';
        $langFile = __DIR__ . "/lang/fr.php";
    }
    
    if (!file_exists($langFile)) {
        error_log("ERREUR CRITIQUE: Aucun fichier de langue trouvé");
        return [];
    }
    
    $translations = include $langFile;
    
    // Retourner une section spécifique ou tout
    if ($section !== 'all' && isset($translations[$section])) {
        return $translations[$section];
    }
    
    return $translations;
}
function __($key, $vars = [], $lang = null) {
    static $translations = [];
    
    // Détecter la langue de l'utilisateur si non spécifiée
    if ($lang === null) {
        $lang = getUserLanguage();
    }
    
    // Charger les traductions si pas déjà en cache
    if (!isset($translations[$lang])) {
        $translations[$lang] = loadLanguage($lang);
    }
    
    // Séparer la section et la clé (ex: 'meet.join_room' -> ['meet', 'join_room'])
    $parts = explode('.', $key, 2);
    $section = $parts[0];
    $subkey = $parts[1] ?? $key;
    
    // Récupérer la traduction
    $text = $translations[$lang][$section][$subkey] ?? $key;
    
    // Remplacer les variables {var}
    if (!empty($vars)) {
        foreach ($vars as $varKey => $varValue) {
            $text = str_replace('{' . $varKey . '}', $varValue, $text);
        }
    }
    
    return $text;
}

/**
 * Récupère la langue de l'utilisateur connecté
 * @return string Code langue (fr, en, etc.)
 */
function getUserLanguage() {
    static $userLang = null;
    
    if ($userLang !== null) {
        return $userLang;
    }
    
    // 1. Vérifier si l'utilisateur est connecté
    if (isset($_SESSION['user_id'])) {
        $prefs = getUserPreferences($_SESSION['user_id']);
        if ($prefs && isset($prefs['langue'])) {
            $userLang = $prefs['langue'];
            return $userLang;
        }
    }
    
    // 2. Vérifier cookie de langue
    if (isset($_COOKIE['vex_lang'])) {
        $userLang = $_COOKIE['vex_lang'];
        return $userLang;
    }
    
    // 3. Détecter depuis navigateur
    if (isset($_SERVER['HTTP_ACCEPT_LANGUAGE'])) {
        $browserLang = substr($_SERVER['HTTP_ACCEPT_LANGUAGE'], 0, 2);
        $supportedLangs = ['fr', 'en', 'es', 'de', 'it', 'pt', 'ar', 'zh', 'ja', 'ru'];
        if (in_array($browserLang, $supportedLangs)) {
            $userLang = $browserLang;
            return $userLang;
        }
    }
    
    // 4. Langue par défaut
    $userLang = 'fr';
    return $userLang;
}

/**
 * Change la langue de l'utilisateur
 * @param int $userId ID utilisateur
 * @param string $lang Code langue
 * @return bool Succès
 */
function setUserLanguage($userId, $lang) {
    // Valider la langue
    $supportedLangs = ['fr', 'en', 'es', 'de', 'it', 'pt', 'ar', 'zh', 'ja', 'ru'];
    if (!in_array($lang, $supportedLangs)) {
        return false;
    }
    
    // Mettre à jour en base
    $success = updateUserPreference($userId, 'langue', $lang);
    
    // Mettre à jour le cookie
    if ($success) {
        setcookie('vex_lang', $lang, time() + (365 * 24 * 60 * 60), '/');
    }
    
    return $success;
}

/**
 * Retourne les langues supportées
 * @return array [code => nom natif]
 */
function getSupportedLanguages() {
    return [
        'fr' => 'Français',
        'en' => 'English',
        'es' => 'Español',
        'de' => 'Deutsch',
        'it' => 'Italiano',
        'pt' => 'Português',
        'ar' => 'العربية',
        'zh' => '中文',
        'ja' => '日本語',
        'ru' => 'Русский'
    ];
}

/**
 * Vérifie si une langue utilise l'écriture RTL (Right-to-Left)
 * @param string $lang Code langue
 * @return bool True si RTL
 */
function isRTL($lang) {
    $rtlLangs = ['ar', 'he', 'fa', 'ur'];
    return in_array($lang, $rtlLangs);
}

// =====================================================
// Fonctions Meet spécifiques
// =====================================================

/**
 * Génère un code de salle unique
 * @return string Code de 10 caractères
 */

/**
 * Bascule le thème de l'utilisateur (clair/sombre)
 * @param int $userId ID de l'utilisateur
 * @param mysqli $conn Connexion à la base de données
 * @return int|null Nouveau thème (0=clair, 1=sombre) ou null si erreur
 */
function toggleUserTheme($userId, $conn = null) {
    $prefs = getUserPreferences($userId, $conn);
    if ($prefs === null) {
        return null;
    }
    
    $newTheme = $prefs['teme'] === 0 ? 1 : 0;
    $success = updateUserPreference($userId, 'teme', $newTheme, $conn);
    
    return $success ? $newTheme : null;
}

/**
 * Applique le thème de l'utilisateur au CSS
 * @param int $userId ID de l'utilisateur
 * @param mysqli $conn Connexion à la base de données
 */
function applyUserTheme($userId, $conn = null) {
    $prefs = getUserPreferences($userId, $conn);
    if ($prefs === null) {
        return;
    }
    
    $isDark = $prefs['teme'] === 1;
    
    echo '<style>';
    if ($isDark) {
        echo '
        :root {
          --nav-bg: #1a1a1a;
          --nav-gradient: linear-gradient(to bottom, #1a1a1a, #0f0f0f);
          --shadow: 0 1px 4px rgba(0,0,0,0.5);
          --hover-bg: rgba(255,255,255,0.08);
          --active-color: rgba(76, 175, 80, 0.9);
          --text-primary: #e0e0e0;
          --muted: #999999;
          --admin-sep: rgba(255,255,255,0.06);
        }
        .apps-popup-7844, .profile-menu-7844 {
          background: #222 !important;
          border: 1px solid rgba(255,255,255,0.1);
        }
        .app-item-7844 {
          background: #2a2a2a !important;
          border-color: rgba(255,255,255,0.08) !important;
        }
        .app-item-7844:hover {
          background: #333 !important;
        }
        .apps-btn-nav-7844 {
          background: #2a2a2a !important;
          border-color: rgba(255,255,255,0.1) !important;
        }
        .profile-icon-top-7844 {
          background: rgba(255,255,255,0.08) !important;
        }
        ';
    }
    echo '</style>';
}

function displayNavigation($conn = null, $apps = [], $admin_apps = [], $colorScheme = null) {
    // --------------------------
    // Vérification session utilisateur
    // --------------------------
    $user_data = null;
    $userId = null;
    
    if ($conn && isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
        $cookie_value = $_COOKIE['connexion_cookie'];

        $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=?");
        $stmt->bind_param("s", $cookie_value);
        $stmt->execute();
        $result = $stmt->get_result();

        if ($result && $result->num_rows == 1) {
            $row = $result->fetch_assoc();
            if ($row['pc'] === $_SERVER['REMOTE_ADDR'] && $row['navi'] === $_SERVER['HTTP_USER_AGENT']) {
                if (strtotime($row['datecra']) > strtotime('-1 hour')) {
                    $stmt = $conn->prepare("SELECT id, email, nom, privilege, vip FROM login WHERE email=?");
                    $stmt->bind_param("s", $row['email']);
                    $stmt->execute();
                    $result = $stmt->get_result();
                    if ($result && $result->num_rows == 1) {
                        $user_row = $result->fetch_assoc();
                        $user_privilege = (int)$user_row['privilege'];
                        $privilege_details = function_exists('getPrivilegeDetails')
                            ? getPrivilegeDetails($user_privilege)
                            : ['nom_privilege' => 'utilisateur', 'couleur_privilege' => '#000000'];

                        $user_data = [
                            'id' => (int)$user_row['id'],
                            'email' => $user_row['email'],
                            'nom' => $user_row['nom'],
                            'privilege' => $user_privilege,
                            'vip' => $user_row['vip'],
                            'privilege_details' => $privilege_details
                        ];
                        $userId = (int)$user_row['id'];
                    }
                }
            }
        }
    }

    $is_admin = $user_data && $user_data['privilege'] <= 6;

    // --------------------------
    // Détermination du jeu de couleurs
    // --------------------------
    $colors = [];
    
    // Si un jeu de couleurs personnalisé est fourni, l'utiliser
    if ($colorScheme !== null && is_array($colorScheme)) {
        $colors = $colorScheme;
    } 
    // Sinon, utiliser le thème de l'utilisateur (clair/sombre)
    else if ($userId !== null && function_exists('getUserPreferences')) {
        $prefs = getUserPreferences($userId, $conn);
        $isDark = ($prefs && isset($prefs['teme']) && $prefs['teme'] === 1);
        
        if ($isDark) {
            // Thème sombre
            $colors = [
                'nav_bg' => '#1a1a1a',
                'nav_gradient' => 'linear-gradient(to bottom, #1a1a1a, #0f0f0f)',
                'shadow' => '0 1px 4px rgba(0,0,0,0.5)',
                'hover_bg' => 'rgba(255,255,255,0.08)',
                'active_color' => 'rgba(76, 175, 80, 0.9)',
                'text_primary' => '#e0e0e0',
                'muted' => '#999999',
                'admin_sep' => 'rgba(255,255,255,0.06)',
                'popup_bg' => '#222222',
                'popup_border' => 'rgba(255,255,255,0.1)',
                'app_item_bg' => '#2a2a2a',
                'app_item_border' => 'rgba(255,255,255,0.08)',
                'app_item_hover' => '#333333',
                'btn_bg' => '#2a2a2a',
                'btn_border' => 'rgba(255,255,255,0.1)',
                'profile_icon_bg' => 'rgba(255,255,255,0.08)'
            ];
        } else {
            // Thème clair (par défaut)
            $colors = [
                'nav_bg' => '#ffffff',
                'nav_gradient' => 'linear-gradient(to bottom, #ffffff, #f0f0f0)',
                'shadow' => '0 1px 4px rgba(0,0,0,0.15)',
                'hover_bg' => 'rgba(0,0,0,0.05)',
                'active_color' => 'rgba(76, 175, 80, 0.9)',
                'text_primary' => '#111111',
                'muted' => '#666666',
                'admin_sep' => 'rgba(0,0,0,0.06)',
                'popup_bg' => '#ffffff',
                'popup_border' => 'rgba(0,0,0,0.08)',
                'app_item_bg' => '#f7f7f7',
                'app_item_border' => 'rgba(0,0,0,0.04)',
                'app_item_hover' => '#f0f0f0',
                'btn_bg' => '#ffffff',
                'btn_border' => 'rgba(0,0,0,0.06)',
                'profile_icon_bg' => 'rgba(0,0,0,0.05)'
            ];
        }
    }
    // Valeurs par défaut si aucune préférence n'est trouvée
    else {
        $colors = [
            'nav_bg' => '#ffffff',
            'nav_gradient' => 'linear-gradient(to bottom, #ffffff, #f0f0f0)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.15)',
            'hover_bg' => 'rgba(0,0,0,0.05)',
            'active_color' => 'rgba(76, 175, 80, 0.9)',
            'text_primary' => '#111111',
            'muted' => '#666666',
            'admin_sep' => 'rgba(0,0,0,0.06)',
            'popup_bg' => '#ffffff',
            'popup_border' => 'rgba(0,0,0,0.08)',
            'app_item_bg' => '#f7f7f7',
            'app_item_border' => 'rgba(0,0,0,0.04)',
            'app_item_hover' => '#f0f0f0',
            'btn_bg' => '#ffffff',
            'btn_border' => 'rgba(0,0,0,0.06)',
            'profile_icon_bg' => 'rgba(0,0,0,0.05)'
        ];
    }

    // --------------------------
    // Applications par défaut
    // --------------------------
    if (empty($apps)) {
        $apps = [
            ['icon' => 'fas fa-home', 'label' => 'Accueil', 'url' => '/login/dashboard.php'],
            ['icon' => 'fa-envelope', 'label' => 'Mail', 'url' => '/mess/vexmail.php'],
            ['icon' => 'fa-hard-drive', 'label' => 'Exodrive', 'url' => '/tel/'],
            ['icon' => 'fa-folder-open', 'label' => 'Éditeur de fichiers', 'url' => '/edite1/'],
            ['icon' => 'fa-video', 'label' => 'Vidéos', 'url' => '#'],
            ['icon' => 'fa-globe', 'label' => 'Sitec', 'url' => '/sitec/']
           
        ];
    }

    if (empty($admin_apps) && $is_admin) {
        $admin_apps = [
         
            ['icon' => 'fa-shield-alt', 'label' => 'Administration', 'url' => '/admin/admin.php']
        ];
    }

    // --------------------------
    // CSS avec variables dynamiques
    // --------------------------
    echo '<style>
    /* Reset simple */
    * { margin:0; padding:0; box-sizing:border-box; }

    /* Variables générales avec valeurs dynamiques */
    :root {
      --nav-bg: ' . $colors['nav_bg'] . ';
      --nav-gradient: ' . $colors['nav_gradient'] . ';
      --shadow: ' . $colors['shadow'] . ';
      --hover-bg: ' . $colors['hover_bg'] . ';
      --active-color: ' . $colors['active_color'] . ';
      --text-primary: ' . $colors['text_primary'] . ';
      --muted: ' . $colors['muted'] . ';
      --admin-sep: ' . $colors['admin_sep'] . ';
    }

    /* Barre unique */
    .top-nav-bar-haut7844 {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      height: 60px;
      background: var(--nav-gradient);
      box-shadow: var(--shadow);
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 20px;
      z-index: 1000;
      transition: background-color 0.2s ease;
    }

    .top-nav-left-7844, .top-nav-right-7844 {
      display:flex;
      align-items:center;
      gap:12px;
    }

    /* Bouton apps */
    .apps-btn-nav-7844 {
      width:40px; height:40px; border-radius:8px;
      background: ' . $colors['btn_bg'] . ';
      border:1px solid ' . $colors['btn_border'] . ';
      display:flex; align-items:center; justify-content:center;
      cursor:pointer; color: var(--active-color); font-size:18px;
      transition: transform 0.15s ease, background 0.15s ease;
    }
    .apps-btn-nav-7844:hover { 
      transform: translateY(-2px); 
      background: ' . $colors['app_item_hover'] . '; 
    }

    /* User info */
    .user-info-top-7844 {
      display:flex; align-items:center; gap:12px;
      cursor:pointer; padding:6px 12px; border-radius:6px;
      transition: background-color 0.2s ease;
      color: var(--text-primary);
    }
    .user-info-top-7844:hover { background: var(--hover-bg); }

    .user-name-top-7844 {
      font-size:14px; font-weight:600; color: var(--text-primary);
      max-width: 180px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
      font-family: "Arial", sans-serif;

    }

    /* fona gradient text */
    .user-name-top-7844.fona {
      color: transparent;
      -webkit-background-clip: text;
      background-clip: text;
      background-image: url("/admin/image2.png"), url("/admin/image3.png");
      background-size: 100% 200%;
      background-position: 100% 100%, 100% 200%;
      background-repeat: no-repeat, no-repeat;
      animation: slideImages7844 6s linear infinite;
      font-weight:700;
    }
    @keyframes slideImages7844 {
      0%,100% { background-position:100% 100%,100% 200%; }
      50% { background-position:100% 0%,100% 100%; }
    }

    /* Profile icon circle */
    .profile-icon-top-7844 {
      width:32px; height:32px; border-radius:50%;
      background: ' . $colors['profile_icon_bg'] . ';
      display:flex; align-items:center; justify-content:center;
      color: var(--muted); font-size:14px;
    }

    /* Popup apps */
    .apps-popup-7844 {
      position: fixed; top:70px; left:20px;
      width: 360px; max-height: 520px; overflow:auto;
      background: ' . $colors['popup_bg'] . ';
      border: 1px solid ' . $colors['popup_border'] . ';
      border-radius:12px; box-shadow:0 8px 30px rgba(0,0,0,0.18);
      padding:14px; display:none; z-index:1001;
    }
    .apps-grid-7844 {
      display:grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 12px;
      justify-items:center;
    }
    .app-item-7844 {
      width:96px; height:96px; border-radius:12px;
      background: ' . $colors['app_item_bg'] . ';
      display:flex; flex-direction:column; gap:6px;
      align-items:center; justify-content:center; text-decoration:none; 
      color:var(--text-primary);
      transition: transform 0.12s ease, background 0.12s ease;
      border:1px solid ' . $colors['app_item_border'] . ';
    }
    .app-item-7844 i { font-size:22px; color: var(--active-color); }
    .app-item-7844 span { font-size:12px; font-weight:600; text-align:center; }
    .app-item-7844:hover { 
      transform: translateY(-6px); 
      background: ' . $colors['app_item_hover'] . '; 
    }

    /* admin separation */
    .apps-admin-sep-7844 { 
      margin-top:12px; 
      padding-top:10px; 
      border-top:1px solid var(--admin-sep); 
    }

    /* Profile menu */
    .profile-menu-7844 {
      position: fixed; top:70px; right:20px; width:200px;
      background: ' . $colors['popup_bg'] . ';
      border: 1px solid ' . $colors['popup_border'] . ';
      border-radius:10px; box-shadow:0 8px 30px rgba(0,0,0,0.18);
      padding:8px; display:none; z-index:1002;
    }
    .profile-menu-7844 a {
      display:block; text-decoration:none; color:var(--text-primary);
      padding:8px 10px; border-radius:8px; font-weight:600; margin-bottom:6px;
    }
    .profile-menu-7844 a:hover { background: var(--hover-bg); }

    /* responsive */
    @media (max-width:768px) {
      .apps-popup-7844 { left:10px; width: calc(100% - 20px); }
      .profile-menu-7844 { right:10px; width: calc(100% - 20px); }
    }
    </style>';

    // --------------------------
    // HTML
    // --------------------------
    echo '<div class="top-nav-bar-haut7844" role="navigation" aria-label="Top navigation">';
      echo '<div class="top-nav-left-7844">';
        echo '<button class="apps-btn-nav-7844" id="apps-btn-nav" aria-label="Applications"><i class="fas fa-th" aria-hidden="true"></i></button>';
      echo '</div>';

      echo '<div class="top-nav-right-7844">';
        if ($user_data) {
            $name = htmlspecialchars($user_data["nom"]);
            $couleur = isset($user_data["privilege_details"]["couleur_privilege"]) ? $user_data["privilege_details"]["couleur_privilege"] : null;
            $inline_color = $couleur ? 'style="color:'.htmlspecialchars($couleur).'"' : '';
            echo '<div class="user-info-top-7844" id="user-info-top" tabindex="0" role="button" aria-haspopup="true" aria-expanded="false" '.$inline_color.'>';
           
           
           $privilegeInfo = getPrivilegeDetails($user_data['privilege']);

            if ($user_data['privilege'] === "1") {
                echo '<span class="user-name-top-7844 fona">'.$name.'</span>';
            } else {
                echo '<span class="user-name-top-7844" style="color: '.$privilegeInfo['couleur_privilege'].';">'.$name.'</span>';
            }
              echo '<div class="profile-icon-top-7844" id="profile-icon-top" title="Profil"><i class="fas fa-user" aria-hidden="true"></i></div>';
            echo '</div>';
        } else {
            echo '<button class="apps-btn-nav-7844" onclick="window.location.href=\'/login/login.php\'" aria-label="Connexion"><i class="fas fa-sign-in-alt" aria-hidden="true"></i></button>';
        }
      echo '</div>';
    echo '</div>';

    // Apps popup
    echo '<div class="apps-popup-7844" id="apps-popup">';
      echo '<div class="apps-grid-7844">';
        foreach ($apps as $app) {
            $url = htmlspecialchars($app['url']);
            $icon = htmlspecialchars($app['icon']);
            $label = htmlspecialchars($app['label']);
            echo '<a class="app-item-7844" href="'.$url.'" role="button">';
              echo '<i class="fas '.$icon.'" aria-hidden="true"></i>';
              echo '<span>'.$label.'</span>';
            echo '</a>';
        }
      echo '</div>';
      if ($is_admin && !empty($admin_apps)) {
          echo '<div class="apps-admin-sep-7844">';
            echo '<div class="apps-grid-7844">';
              foreach ($admin_apps as $app) {
                  $url = htmlspecialchars($app['url']);
                  $icon = htmlspecialchars($app['icon']);
                  $label = htmlspecialchars($app['label']);
                  echo '<a class="app-item-7844" href="'.$url.'" role="button">';
                    echo '<i class="fas '.$icon.'" aria-hidden="true"></i>';
                    echo '<span>'.$label.'</span>';
                  echo '</a>';
              }
            echo '</div>';
          echo '</div>';
      }
    echo '</div>';

    // Profile menu
    if ($user_data) {
      echo '<div class="profile-menu-7844" id="profile-menu" role="menu" aria-hidden="true">';
        echo '<a href="/login/account.php">Mon Compte</a>';
        echo '<a href="/login/account.php">Paramètres</a>';
        echo '<a href="/login/logout.php">Déconnexion</a>';
      echo '</div>';
    }

    // --------------------------
    // JavaScript
    // --------------------------
    echo "<script>
    (function(){
      const appsBtn = document.getElementById('apps-btn-nav');
      const appsPopup = document.getElementById('apps-popup');
      const userInfo = document.getElementById('user-info-top');
      const profileMenu = document.getElementById('profile-menu');

      if (appsBtn && appsPopup) {
        appsBtn.addEventListener('click', function(e){
          e.stopPropagation();
          appsPopup.style.display = (appsPopup.style.display === 'flex' || appsPopup.style.display === 'block') ? 'none' : 'flex';
        });
      }

      if (userInfo && profileMenu) {
        userInfo.addEventListener('click', function(e){
          e.stopPropagation();
          const expanded = this.getAttribute('aria-expanded') === 'true';
          this.setAttribute('aria-expanded', expanded ? 'false' : 'true');
          profileMenu.style.display = (profileMenu.style.display === 'block') ? 'none' : 'block';
        });
        userInfo.addEventListener('keydown', function(e){
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            this.click();
          }
        });
      }

      document.addEventListener('click', function(e){
        if (appsPopup && !appsPopup.contains(e.target) && appsBtn && !appsBtn.contains(e.target)) {
          appsPopup.style.display = 'none';
        }
        if (profileMenu && !profileMenu.contains(e.target) && userInfo && !userInfo.contains(e.target)) {
          profileMenu.style.display = 'none';
          if (userInfo) userInfo.setAttribute('aria-expanded','false');
        }
      });

      document.addEventListener('keydown', function(e){
        if (e.key === 'Escape') {
          if (appsPopup) appsPopup.style.display = 'none';
          if (profileMenu) profileMenu.style.display = 'none';
          if (userInfo) userInfo.setAttribute('aria-expanded','false');
        }
      });

      if (appsPopup) {
        appsPopup.style.display = 'none';
        appsPopup.style.flexDirection = 'column';
        appsPopup.style.alignItems = 'center';
      }
    })();
    </script>";
}

// --------------------------
// Fonctions helper pour créer des jeux de couleurs prédéfinis
// --------------------------

/**
 * Retourne un jeu de couleurs prédéfini
 * @param string $theme Nom du thème ('light', 'dark', 'blue', 'green', 'purple', 'ocean')
 * @return array Tableau associatif des couleurs
 */
function getColorScheme($theme = 'light') {
    $schemes = [
        'light' => [
            'nav_bg' => '#ffffff',
            'nav_gradient' => 'linear-gradient(to bottom, #ffffff, #f0f0f0)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.15)',
            'hover_bg' => 'rgba(0,0,0,0.05)',
            'active_color' => 'rgba(76, 175, 80, 0.9)',
            'text_primary' => '#111111',
            'muted' => '#666666',
            'admin_sep' => 'rgba(0,0,0,0.06)',
            'popup_bg' => '#ffffff',
            'popup_border' => 'rgba(0,0,0,0.08)',
            'app_item_bg' => '#f7f7f7',
            'app_item_border' => 'rgba(0,0,0,0.04)',
            'app_item_hover' => '#f0f0f0',
            'btn_bg' => '#ffffff',
            'btn_border' => 'rgba(0,0,0,0.06)',
            'profile_icon_bg' => 'rgba(0,0,0,0.05)'
        ],
        'dark' => [
            'nav_bg' => '#1a1a1a',
            'nav_gradient' => 'linear-gradient(to bottom, #1a1a1a, #0f0f0f)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.5)',
            'hover_bg' => 'rgba(255,255,255,0.08)',
            'active_color' => 'rgba(76, 175, 80, 0.9)',
            'text_primary' => '#e0e0e0',
            'muted' => '#999999',
            'admin_sep' => 'rgba(255,255,255,0.06)',
            'popup_bg' => '#222222',
            'popup_border' => 'rgba(255,255,255,0.1)',
            'app_item_bg' => '#2a2a2a',
            'app_item_border' => 'rgba(255,255,255,0.08)',
            'app_item_hover' => '#333333',
            'btn_bg' => '#2a2a2a',
            'btn_border' => 'rgba(255,255,255,0.1)',
            'profile_icon_bg' => 'rgba(255,255,255,0.08)'
        ],
        'blue' => [
            'nav_bg' => '#1e3a5f',
            'nav_gradient' => 'linear-gradient(to bottom, #1e3a5f, #152943)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.3)',
            'hover_bg' => 'rgba(255,255,255,0.1)',
            'active_color' => '#64b5f6',
            'text_primary' => '#e3f2fd',
            'muted' => '#90caf9',
            'admin_sep' => 'rgba(255,255,255,0.08)',
            'popup_bg' => '#1a2f4a',
            'popup_border' => 'rgba(100,181,246,0.2)',
            'app_item_bg' => '#234567',
            'app_item_border' => 'rgba(100,181,246,0.15)',
            'app_item_hover' => '#2a5280',
            'btn_bg' => '#234567',
            'btn_border' => 'rgba(100,181,246,0.2)',
            'profile_icon_bg' => 'rgba(100,181,246,0.15)'
        ],
        'green' => [
            'nav_bg' => '#1b3a2f',
            'nav_gradient' => 'linear-gradient(to bottom, #1b3a2f, #112920)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.3)',
            'hover_bg' => 'rgba(255,255,255,0.1)',
            'active_color' => '#66bb6a',
            'text_primary' => '#e8f5e9',
            'muted' => '#81c784',
            'admin_sep' => 'rgba(255,255,255,0.08)',
            'popup_bg' => '#1a3028',
            'popup_border' => 'rgba(102,187,106,0.2)',
            'app_item_bg' => '#234d3a',
            'app_item_border' => 'rgba(102,187,106,0.15)',
            'app_item_hover' => '#2a6047',
            'btn_bg' => '#234d3a',
            'btn_border' => 'rgba(102,187,106,0.2)',
            'profile_icon_bg' => 'rgba(102,187,106,0.15)'
        ],
        'purple' => [
            'nav_bg' => '#2d1b3d',
            'nav_gradient' => 'linear-gradient(to bottom, #2d1b3d, #1e122a)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.3)',
            'hover_bg' => 'rgba(255,255,255,0.1)',
            'active_color' => '#ba68c8',
            'text_primary' => '#f3e5f5',
            'muted' => '#ce93d8',
            'admin_sep' => 'rgba(255,255,255,0.08)',
            'popup_bg' => '#271834',
            'popup_border' => 'rgba(186,104,200,0.2)',
            'app_item_bg' => '#3d2555',
            'app_item_border' => 'rgba(186,104,200,0.15)',
            'app_item_hover' => '#4a2e66',
            'btn_bg' => '#3d2555',
            'btn_border' => 'rgba(186,104,200,0.2)',
            'profile_icon_bg' => 'rgba(186,104,200,0.15)'
        ],
        'ocean' => [
            'nav_bg' => '#0d3b4d',
            'nav_gradient' => 'linear-gradient(to bottom, #0d3b4d, #082838)',
            'shadow' => '0 1px 4px rgba(0,0,0,0.3)',
            'hover_bg' => 'rgba(255,255,255,0.1)',
            'active_color' => '#26c6da',
            'text_primary' => '#e0f7fa',
            'muted' => '#4dd0e1',
            'admin_sep' => 'rgba(255,255,255,0.08)',
            'popup_bg' => '#0a3242',
            'popup_border' => 'rgba(38,198,218,0.2)',
            'app_item_bg' => '#154a5e',
            'app_item_border' => 'rgba(38,198,218,0.15)',
            'app_item_hover' => '#1b5c72',
            'btn_bg' => '#154a5e',
            'btn_border' => 'rgba(38,198,218,0.2)',
            'profile_icon_bg' => 'rgba(38,198,218,0.15)'
        ]
    ];

    return $schemes[$theme] ?? $schemes['light'];
}
function generateRoomCode() {
    $characters = 'abcdefghijklmnopqrstuvwxyz0123456789';
    $code = '';
    for ($i = 0; $i < 10; $i++) {
        $code .= $characters[random_int(0, strlen($characters) - 1)];
    }
    return $code;
}

/**
 * Crée une nouvelle salle de réunion
 * @param int $creatorId ID du créateur
 * @param array $options Options de la salle
 * @return array|false Données de la salle ou false
 */
function createMeetRoom($creatorId, $options = []) {
    $servername = "localhost";
    $username = "orsql";
    $password = "iDq]25F0u8v*z[1d";
    $dbname = "meet";
    
    $conn = new mysqli($servername, $username, $password, $dbname);
    
    if ($conn->connect_error) {
        error_log("Erreur connexion createMeetRoom: " . $conn->connect_error);
        return false;
    }
    
    // Générer un code unique
    $attempts = 0;
    $maxAttempts = 10;
    
    do {
        $roomCode = generateRoomCode();
        $stmt = $conn->prepare("SELECT id FROM meet_rooms WHERE room_code = ?");
        if (!$stmt) {
            error_log("Erreur vérification code: " . $conn->error);
            $conn->close();
            return false;
        }
        
        $stmt->bind_param("s", $roomCode);
        $stmt->execute();
        $exists = $stmt->get_result()->num_rows > 0;
        $stmt->close();
        
        $attempts++;
        
        if ($attempts >= $maxAttempts) {
            $conn->close();
            return false;
        }
        
    } while ($exists);
    
    // Valeurs par défaut
    $title = isset($options['title']) ? $options['title'] : 'Réunion sans titre';
    $isPublic = isset($options['is_public']) ? (int)$options['is_public'] : 1;
    $requirePassword = isset($options['require_password']) ? (int)$options['require_password'] : 0;
    $password = null;
    
    if ($requirePassword && isset($options['password'])) {
        $password = password_hash($options['password'], PASSWORD_BCRYPT);
    }
    
    $maxParticipants = isset($options['max_participants']) ? (int)$options['max_participants'] : 10;
    
    $stmt = $conn->prepare("INSERT INTO meet_rooms (room_code, creator_id, title, is_public, require_password, password_hash, max_participants) VALUES (?, ?, ?, ?, ?, ?, ?)");
    
    if (!$stmt) {
        error_log("Erreur préparation insertion: " . $conn->error);
        $conn->close();
        return false;
    }
    
    $stmt->bind_param("sisiisi", $roomCode, $creatorId, $title, $isPublic, $requirePassword, $password, $maxParticipants);
    
    if ($stmt->execute()) {
        $roomId = $conn->insert_id;
        $stmt->close();
        $conn->close();
        
        return [
            'id' => $roomId,
            'room_code' => $roomCode,
            'title' => $title,
            'is_public' => $isPublic,
            'creator_id' => $creatorId
        ];
    }
    
    $stmt->close();
    $conn->close();
    return false;
}

/**
 * Récupère les informations d'une salle
 * @param string $roomCode Code de la salle
 * @return array|false Données de la salle ou false
 */
function getRoomInfo($roomCode) {
    $servername = "localhost";
    $username = "orsql";
    $password = "iDq]25F0u8v*z[1d";
    $dbname = "meet"; // CORRECTION: Utiliser la base 'meet'
    
    $conn = new mysqli($servername, $username, $password, $dbname);
    
    if ($conn->connect_error) {
        error_log("Erreur connexion getRoomInfo: " . $conn->connect_error);
        return false;
    }
    
    $stmt = $conn->prepare("SELECT * FROM meet_rooms WHERE room_code = ? AND is_active = 1");
    if (!$stmt) {
        error_log("Erreur préparation getRoomInfo: " . $conn->error);
        $conn->close();
        return false;
    }
    
    $stmt->bind_param("s", $roomCode);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows === 0) {
        $stmt->close();
        $conn->close();
        return false;
    }
    
    $room = $result->fetch_assoc();
    $stmt->close();
    $conn->close();
    
    return $room;
}
//encyptement ----------------------------------------------------------------
define('MIN_PLAINTEXT_LEN', 32);

function getNodeKey(): string {
    $file = '/var/www/html/node.key';

    if (!file_exists($file)) {
        $key = random_bytes(32);
        file_put_contents($file, $key);
        chmod($file, 0400);
        return $key;
    }

    return file_get_contents($file);
}
function encryptNode(string $plaintext): array {
    $key = getNodeKey();

    $len = strlen($plaintext);
    $header = pack('N', $len); // 4 octets longueur réelle

    if ($len < MIN_PLAINTEXT_LEN) {
        $paddingLen = MIN_PLAINTEXT_LEN - $len;
        $padding = random_bytes($paddingLen);
    } else {
        $padding = '';
    }

    $finalPlaintext = $header . $plaintext . $padding;

    $iv  = random_bytes(12);
    $tag = '';

    $cipher = openssl_encrypt(
        $finalPlaintext,
        'aes-256-gcm',
        $key,
        OPENSSL_RAW_DATA,
        $iv,
        $tag
    );

    return [
        'cipher' => base64_encode($cipher),
        'iv'     => base64_encode($iv),
        'tag'    => base64_encode($tag)
    ];
}
function decryptNode(array $blob): string|false {
    $key = getNodeKey();

    $data = openssl_decrypt(
        base64_decode($blob['cipher']),
        'aes-256-gcm',
        $key,
        OPENSSL_RAW_DATA,
        base64_decode($blob['iv']),
        base64_decode($blob['tag'])
    );

    if ($data === false || strlen($data) < 4) {
        return false;
    }

    $len = unpack('N', substr($data, 0, 4))[1];
    return substr($data, 4, $len);
}

function displayUserAvatar($nom, $pdp) {
    if (!empty($pdp)) {
        return '<img src="data:image/jpeg;base64,' . base64_encode($pdp) . '" alt="' . htmlspecialchars($nom) . '" class="user-avatar">';
    } else {
        $initiales = strtoupper(substr($nom, 0, 1));
        return '<div class="user-avatar-initials">' . $initiales . '</div>';
    }
}
?>
