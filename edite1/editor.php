<?php
ini_set('display_errors', 1);
error_reporting(E_ALL);
require_once __DIR__ . '/config.php';
 include '/var/www/html/access_control.php';
 include '/var/www/html/function.php';
 
$user_id = $_SESSION['user_id'] ?? 'guest_' . uniqid();
$user_name = $_SESSION['user_name'] ?? 'Invité';

$doc_id = isset($_GET['id']) ? (int)$_GET['id'] : 0;
if ($doc_id === 0) die("Document non spécifié");

$conn = new mysqli("localhost", "orsql", "iDq]25F0u8v*z[1d", "user");
$conn->set_charset("utf8mb4");

$user_data_nav = null;
if ($conn->connect_error) {
    die("La connexion a echoue : " . $conn->connect_error);
}
  $user_privilege = 10;
// Verification si le cookie de session existe
if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];

    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=? ");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        $user_idcokier = $row['idcokier'];
        $datecra = $row['datecra'];
        $pc = $row['pc'];
        $navi = $row['navi'];
        $user_email = $row['email'];
        $user_nom = $row['nom'];
        
        if ($pc === $_SERVER['REMOTE_ADDR'] && $navi === $_SERVER['HTTP_USER_AGENT']) {
            $one_hour_ago = strtotime('-1 hour');
            $datecra_timestamp = strtotime($datecra);
            $is_recent = $datecra_timestamp > $one_hour_ago;

            if ($is_recent) {
                $current_path = $_SERVER['REQUEST_URI'];
                 
                $stmt = $conn->prepare("SELECT * FROM login WHERE email=?");
                $stmt->bind_param("s", $user_email);
                $stmt->execute();
                $result = $stmt->get_result();
                     
                if ($result->num_rows == 1) {
                    if ($row) {
                        $row = $result->fetch_assoc();
                    }
                    $user_vip = $row['vip'];
                    $user_id = (int)$row['id'];
                    $user_privilege = (int)$row['privilege'];
                    $user_data_nav = [
                        'id' => $user_id,
                        'nom' => $row['nom'],
                        'email' => $row['email'],
                        'vip' => $user_vip,
                        'privilege' => $user_privilege,
                                        
                    ];
            
                }
            }
        }
    }
}

$stmt = $conn->prepare("SELECT nom, fichier, type_fichier, visble, id_utilisateur, partage FROM fichiers WHERE id=?");
$stmt->bind_param("i", $doc_id);
$stmt->execute();

$result = $stmt->get_result();
if ($result->num_rows !== 1) die("Document introuvable");
$doc = $result->fetch_assoc();
$stmt->close();

$permission = 0;
$droits = $doc['partage'];

if ($droits !== null) {
    if (strpos($droits, '@') !== false) {
        die("Erreur : les groupes (@) ne sont pas autorisés dans la liste de droits !");
    }
    
    $entries = explode(',', $droits);
    $permission = 0;
    
    foreach ($entries as $entry) {
        $entry = trim($entry);
        if (preg_match('/^#(\d+)-(\d+)$/', $entry, $matches)) {
            $id = (int)$matches[1];
            $niveau = (int)$matches[2];
            if ($id === $user_id) {
                $permission = $niveau;
                break;
            }
        }
    }
} elseif ((int)$doc['id_utilisateur'] === $user_id) {
    $permission = 3;
} elseif ($doc['visble'] === '0' or $doc['visble'] === '3') {
    $permission = 1;
} else {
   // die("Vous n'avez pas la permission d'accéder à ce document.");
}

$safe_filename = preg_replace('/[^a-zA-Z0-9._-]/', '_', $doc['nom']);
$content_hash = substr(md5($doc['fichier']), 0, 8);
$document_key = "doc_" . $doc_id . "_" . $content_hash;
$temp_file = DOCUMENTS_DIR . $document_key . '_' . $safe_filename;

if (!file_exists($temp_file)) {
    file_put_contents($temp_file, $doc['fichier']);
    chmod($temp_file, 0644);
}

$documentUrl = BASE_URL . '/documents/' . basename($temp_file);

$ext = strtolower(pathinfo($doc['nom'], PATHINFO_EXTENSION));
$doc_types = [
    'doc' => 'word', 'docx' => 'word', 'odt' => 'word', 'txt' => 'word', 'rtf' => 'word', 'pdf' => 'word',
    'xls' => 'cell', 'xlsx' => 'cell', 'ods' => 'cell', 'csv' => 'cell',
    'ppt' => 'slide', 'pptx' => 'slide', 'odp' => 'slide'
];
$documentType = $doc_types[$ext] ?? 'word';

$custom_title = " - " . $doc['nom'];

$config = [
    "documentType" => $documentType,
    "document" => [
        "fileType" => $ext,
        "key" => $document_key,
        "title" => $doc['nom'],
        "url" => $documentUrl,
        "permissions" => [
            "edit" => true,
            "download" => true,
            "print" => true,
            "comment" => true,
            "review" => true,
            "chat" => false
        ]
    ],
    "editorConfig" => [
        "mode" => "edit",
        "lang" => "fr",
        "user" => [
            "id" => (string)$user_id,
            "name" => $user_name
        ],
        "customization" => [
            "autosave" => true,
            "forcesave" => true,
            "comments" => true,
            "compactHeader" => false,
            "compactToolbar" => false,
            "feedback" => false,
            "help" => false,
            "goback" => [
                "url" => BASE_URL . '/index.php',
                "text" => "← Retour",
                "blank" => false
            ],
            "uiTheme" => "theme-light",
            "logo" => [
                "image" => "/img/fill-all-app.png",
                "imageEmbedded" => "/img/fill-all-app.png",
                "url" => "/"
            ]
        ],
        "callbackUrl" => BASE_URL . '/callback-db.php?id=' . $doc_id
    ]
];

if (!function_exists('jwt_encode_editor')) {
    function jwt_encode_editor($payload, $secret) {
        $header = ['alg'=>'HS256','typ'=>'JWT'];
        $base64UrlHeader = str_replace(['+', '/', '='], ['-', '_', ''], base64_encode(json_encode($header)));
        $base64UrlPayload = str_replace(['+', '/', '='], ['-', '_', ''], base64_encode(json_encode($payload)));
        $signature = hash_hmac('sha256', $base64UrlHeader . "." . $base64UrlPayload, $secret, true);
        $base64UrlSignature = str_replace(['+', '/', '='], ['-', '_', ''], base64_encode($signature));
        return $base64UrlHeader . "." . $base64UrlPayload . "." . $base64UrlSignature;
    }
}

$token = jwt_encode_editor($config, ONLYOFFICE_SECRET);
$config['token'] = $token;

$conn->close();
?>
<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title><?php echo htmlspecialchars($custom_title); ?></title>
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
<script src="<?php echo ONLYOFFICE_URL_PUBLIC; ?>/web-apps/apps/api/documents/api.js"></script>
<link rel="icon" type="image/png" href="/vex.png">
<script src="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.0/js/all.min.js"></script>

<style>
html, body { 
    height: 100%; 
    width: 100%; 
    margin: 0; 
    padding: 0; 
    overflow: hidden; 
}

#placeholder { 
    height: 100%; 
    width: 100%; 
}
</style>
</head>
<body>
<div id="vex-overlay"></div>
<div id="placeholder"></div>

<script>
// DÉFINIR userDataNav AVANT TOUT
var userDataNav = <?php echo json_encode($user_data_nav); ?>;

// Configuration avec thème personnalisé VEX
var config = <?php echo json_encode($config, JSON_UNESCAPED_SLASHES); ?>;

// CRÉATION DE LA SECTION UTILISATEUR EXTERNE (hors iframe)
function createMinimalUserSection() {
    if (document.getElementById('vex-user-minimal')) return;
    
    var userNameClass = 'user-name-top';
    var userStyle = '';
    
    // Nom d'utilisateur à afficher
    var userName = userDataNav ? userDataNav.nom : 'Invité';
    
    // Déterminer le style selon le privilège
    if (userDataNav && userDataNav.privilege === 1) {
        userNameClass += ' fona';
    } else if (userDataNav && userDataNav.privilege) {
        <?php
        $privilege = getPrivilegeDetails($user_privilege);
        $color = $privilege['couleur_privilege'] ?? '#ffffff';
        ?>
        var color = '<?php echo $color; ?>';
        if (color && color !== '#ffffff' && color !== 'fona') {
            userStyle = 'color: ' + color + ' !important;';
        }
    }
    
    // CSS pour la section utilisateur EXTERNE
    var style = document.createElement('style');
    style.textContent = `
        /* OVERLAY INVISIBLE POUR FERMER LES POP-UPS */
        #vex-overlay {
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: transparent;
            display: none;
            z-index: 999997;
        }
        #vex-overlay.active {
            display: block !important;
        }
        
        #vex-user-minimal {
            position: fixed;
            top: -2px;
            right: 0px;
            z-index: 999999;
        }
        .user-info-top {
            display: flex !important;
            align-items: center;
            gap: 8px;
            padding: 6px 14px;
            border-radius: 8px;
            cursor: pointer;
            transition: all 0.2s;
            background: transparent;
        }
        .user-name-top {
            font-size: 14px !important;
            font-weight: 600 !important;
            font-family: Arial, sans-serif !important;
            color: #212121 !important;
        }
        .user-name-top.fona {
            background-image: url("/admin/image2.png"), url("/admin/image3.png") !important;
            background-size: 100% 200% !important;
            background-clip: text !important;
            -webkit-background-clip: text !important;
            color: transparent !important;
            animation: slidefona 6s linear infinite !important;
        }
        @keyframes slidefona {
            0%, 100% { background-position: 100% 100%, 100% 200%; }
            50% { background-position: 100% 0%, 100% 100%; }
        }
        .profile-icon-top {
            width: 26px;
            height: 26px;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #fff;
        }
        .profile-icon-top i {
            font-size: 14px;
        }
        
        /* MENU PROFIL - EXTERNE */
        .profile-menu-external {
            position: fixed;
            top: 70px;
            right: 20px;
            width: 220px;
            padding: 14px;
            background: #ffffff;
            border: 1px solid rgba(0,0,0,0.1);
            border-radius: 12px;
            box-shadow: 0 8px 30px rgba(0,0,0,0.18);
            display: none;
            z-index: 999998;
            font-family: Arial, sans-serif !important;
        }
        .profile-menu-external.active {
            display: block !important;
        }
        .profile-menu-external a {
            display: block !important;
            padding: 12px 16px !important;
            text-decoration: none !important;
            color: #212121 !important;
            font-weight: 600 !important;
            font-family: Arial, sans-serif !important;
            border-radius: 8px !important;
            margin-bottom: 4px !important;
            transition: all 0.2s !important;
        }
        .profile-menu-external a:hover {
            background: rgba(76,175,80,0.08) !important;
            color: #4caf50 !important;
        }
        
        /* POP-UP VEX APPS - EXTERNE À GAUCHE */
       .vex-apps-popup-external {
            position: fixed;
            top: 60px;
            left: 20px;
            width: 380px;
            padding: 16px;
            background: white;
            border-radius: 12px;
            box-shadow: 0 8px 32px rgba(0,0,0,0.15);
            display: none;
            z-index: 999998;
            font-family: Arial, sans-serif !important;
        }

        .vex-apps-popup-external.active {
            display: block !important;
        }
        .vex-apps-grid {
            display: grid;
            grid-template-columns: repeat(3, 1fr);
            gap: 12px;
            margin: 18px;
        }
        .vex-app-item {
            width: 96px;
            height: 96px;
            border-radius: 12px;
            background: #f7f7f7;
            display: flex;
            flex-direction: column;
            gap: 6px;
            align-items: center;
            justify-content: center;
            text-decoration: none;
            color: #212121;
            transition: transform 0.12s ease, background 0.12s ease;
            border: 1px solid rgba(0,0,0,0.05);
        }
        .vex-app-item:hover {
            transform: translateY(-6px);
            background: #4caf50 !important;
            color: white !important;
        }
        .vex-app-item:hover i {
            color: white !important;
        }
        .vex-app-item svg {
            color: #4caf50 !important;
            width: 22px;
            height: 22px;
        }
        .vex-app-item i {
            font-size: 30px;
            color: #4caf50;
        }
        .vex-app-item span {
            font-size: 13px;
            font-weight: 600;
            color: inherit;
            font-family: Arial, sans-serif !important;
        }
    `;
    document.head.appendChild(style);
    
    // Créer le conteneur utilisateur
    var userContainer = document.createElement('div');
    userContainer.id = 'vex-user-minimal';
    
    userContainer.innerHTML = `
        <div class="user-info-top" id="user-info-top-external" title="Mon compte VEX">
            <span class="${userNameClass}" style="${userStyle}">${userName}</span>
            <div class="profile-icon-top">
                <i class="fas fa-user"></i>
            </div>
        </div>
    `;
    
    document.body.appendChild(userContainer);

    // MENU PROFIL
    var profileMenu = document.createElement('div');
    profileMenu.className = 'profile-menu-external';
    profileMenu.id = 'profile-menu-external';
    
    if (userDataNav) {
        profileMenu.innerHTML = `
            <a href="/login/account.php">Mon Compte</a>
            <a href="/login/account.php">Paramètres</a>
            <a href="/login/logout.php">Déconnexion</a>
        `;
    }
    
    document.body.appendChild(profileMenu);
    
    // POP-UP APPS (à gauche)
    var adminApps = '';
   
    
    var appsPopup = document.createElement('div');
    appsPopup.className = 'vex-apps-popup-external';
    appsPopup.id = 'vex-apps-popup-external';
    appsPopup.innerHTML = `
        <div class="vex-apps-grid">
            <a class="vex-app-item" href="/">
                <i class="fas fa-home"></i><span>Accueil</span>
            </a>
            <a class="vex-app-item" href="/tel/">
                <i class="fas fa-hard-drive"></i><span>Exodrive</span>
            </a>
            <a class="vex-app-item" href="/edite1/">
                <i class="fas fa-folder-open"></i><span>Éditeur</span>
            </a>
            <a class="vex-app-item" href="/sitec/">
                <i class="fas fa-globe"></i><span>Sitec</span>
            </a>
            <a class="vex-app-item" href="/cloud/">
                <i class="fas fa-cloud"></i><span>Cloud</span>
            </a>
            <a class="vex-app-item" href="/mail/">
                <i class="fas fa-envelope"></i><span>Mail</span>
            </a>
            ${adminApps}
        </div>
    `;
    
    document.body.appendChild(appsPopup);
    
    console.log('✅ Section utilisateur minimaliste créée avec menu profil et overlay');
}

// Appeler immédiatement la fonction
createMinimalUserSection();

// GESTION DES ÉVÉNEMENTS AVEC OVERLAY
setTimeout(function() {
    var overlay = document.getElementById('vex-overlay');
    var userInfoExternal = document.getElementById('user-info-top-external');
    var profileMenuExternal = document.getElementById('profile-menu-external');
    var appsPopupExternal = document.getElementById('vex-apps-popup-external');
    
    console.log('overlay:', overlay);
    console.log('userInfoExternal:', userInfoExternal);
    console.log('profileMenuExternal:', profileMenuExternal);
    console.log('appsPopupExternal:', appsPopupExternal);
    
    // Clic sur l'utilisateur → ouvre menu profil
    if (userInfoExternal && profileMenuExternal && overlay) {
        userInfoExternal.addEventListener('click', function(e) {
            e.preventDefault();
            e.stopPropagation();
            console.log('Clic sur user-info-top !');
            var isActive = profileMenuExternal.classList.toggle('active');
            overlay.classList.toggle('active', isActive);
            appsPopupExternal.classList.remove('active');
        });
    }
    
    // Gestion du clic sur le logo OnlyOffice
    function setupLogoClick() {
        var iframe = document.querySelector('iframe');
        if (!iframe || !iframe.contentDocument) return;
        
        var logo = iframe.contentDocument.querySelector('.logo #header-logo, section.logo #header-logo');
        if (logo && !logo.dataset.vexListenerAdded) {
            logo.dataset.vexListenerAdded = 'true';
            logo.style.cursor = 'pointer';
            
            logo.addEventListener('click', function(e) {
                e.preventDefault();
                e.stopPropagation();
                console.log('Clic sur logo OnlyOffice !');
                var isActive = appsPopupExternal.classList.toggle('active');
                overlay.classList.toggle('active', isActive);
                profileMenuExternal.classList.remove('active');
            });
            console.log('✅ Événement clic ajouté au logo');
        }
    }
    
    var logoInterval = setInterval(setupLogoClick, 500);
    setTimeout(function() { clearInterval(logoInterval); }, 10000);
    
    // Clic sur l'overlay → ferme tout
    overlay.addEventListener('click', function() {
        profileMenuExternal.classList.remove('active');
        appsPopupExternal.classList.remove('active');
        overlay.classList.remove('active');
    });
    
    // Empêcher la fermeture en cliquant dans les pop-ups
    profileMenuExternal.addEventListener('click', function(e) {
        e.stopPropagation();
    });
    appsPopupExternal.addEventListener('click', function(e) {
        e.stopPropagation();
    });
    
    console.log('✅ Événements de clic avec overlay ajoutés');
}, 100);

// Ajouter la configuration du thème VEX
config.editorConfig = config.editorConfig || {};
config.editorConfig.customization = config.editorConfig.customization || {};

// THÈME VEX PERSONNALISÉ
config.editorConfig.customization.uiTheme = "theme-light";
config.editorConfig.customization.theme = {
    "type": "light",
    "main": {
        "primary": "#4caf50",
        "primaryDark": "#45a049",
        "primaryLight": "#66bb6a",
        "accent": "#4caf50",
        "background": "#f5f5f5",
        "backgroundLight": "#ffffff",
        "toolbar": "#4caf50",
        "toolbarDark": "#45a049"
    },
    "text": {
        "primary": "#212121",
        "secondary": "#757575",
        "tertiary": "#9e9e9e",
        "inverse": "#ffffff",
        "link": "#4caf50",
        "linkHover": "#45a049"
    },
    "icon": {
        "normal": "#616161",
        "hover": "#424242",
        "pressed": "#212121",
        "inverse": "#ffffff",
        "accent": "#4caf50"
    },
    "border": {
        "regular": "#e0e0e0",
        "hover": "#4caf50",
        "focus": "#4caf50",
        "error": "#f44336"
    },
    "button": {
        "primary": "#4caf50",
        "primaryHover": "#45a049",
        "primaryPressed": "#3d8b40",
        "secondary": "#f5f5f5",
        "secondaryHover": "#e8e8e8"
    },
    "canvas": {
        "background": "#f5f5f5",
        "contentBackground": "#ffffff",
        "pageBorder": "#e0e0e0",
        "ruler": "#fafafa",
        "scrollbar": "#bdbdbd"
    }
};

config.editorConfig.customization.logo = {
    "image": "/img/fill-all-app.png",
    "imageEmbedded": "/img/fill-all-app.png",
    "url": "/"
};

// Initialiser l'éditeur avec la config VEX
var docEditor = new DocsAPI.DocEditor("placeholder", config);

// Attendre que l'iframe soit chargée pour ajouter navigation
function setupVexNavigation() {
    var iframe = document.querySelector('iframe');
    if (!iframe || !iframe.contentDocument) return false;
    
    var doc = iframe.contentDocument;
    if (!doc.querySelector('#toolbar') && !doc.querySelector('#box-doc-name')) return false;
    
    // Marquer si déjà fait
    if (doc.getElementById('vex-nav-added')) return true;
    var marker = doc.createElement('div');
    marker.id = 'vex-nav-added';
    marker.style.display = 'none';
    doc.body.appendChild(marker);
    
    // Font Awesome
    if (!doc.querySelector('link[href*="font-awesome"]')) {
        var fa = doc.createElement('link');
        fa.rel = 'stylesheet';
        fa.href = 'https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.0/css/all.min.css';
        doc.head.appendChild(fa);
    }
    
    // CSS supplémentaire pour navigation VEX
    var style = doc.createElement('style');
    style.textContent = `
        .btn-current-user { display: none !important; }
        section.logo #header-logo {
            width: 20px !important; 
            height: 20px !important;
            background: url('/img/fill-all-app.png') center/contain no-repeat !important;
            cursor: pointer !important;
        }
        section.logo #header-logo * { display: none !important; }
        .extra { 
            margin-top: -6px  !important; 
        }
        section.logo { 
            margin-top: 5px !important; 
            padding-top: 5px !important; 
        }
    `;
    doc.head.appendChild(style);
    
    console.log('✅ Navigation VEX ajoutée dans iframe');
    return true;
}

// Initialiser navigation
var attempts = 0;
var navInterval = setInterval(function() {
    if (setupVexNavigation() || ++attempts >= 40) {
        clearInterval(navInterval);
        console.log(attempts < 40 ? '✅ Thème VEX via API OnlyOffice' : '⚠️ Timeout navigation');
    }
}, 500);

</script>
</body>
</html>