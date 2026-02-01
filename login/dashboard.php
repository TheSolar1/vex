<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Dashboard - Vex</title>
    <link rel="icon" type="image/png" href="/vex.png">
    <script src="/fa-local.js" defer></script>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        :root {
            --green-primary: #4caf50;
            --green-dark: #45a049;
            --bg-light: #f0f2f5;
            --text-dark: #1c1e21;
            --text-secondary: #65676b;
            --border-color: #e4e6eb;
            --white: #ffffff;
            --red-notification: #ff3d00;
            --success-bg: #e8f5e9;
            --hover-bg: #f0f2f5;
        }

        [data-theme="dark"] {
            --green-primary: #4caf50;
            --green-dark: #2d5f2e;
            --bg-light: #18191a;
            --text-dark: #e4e6eb;
            --text-secondary: #b0b3b8;
            --border-color: #3a3b3c;
            --white: #242526;
            --hover-bg: #3a3b3c;
            --success-bg: #2d5f2e;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-light);
            color: var(--text-dark);
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }

        /* Header */
        .header {
            background: linear-gradient(135deg, var(--green-primary) 0%, var(--green-dark) 100%);
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            padding: 15px 20px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            position: sticky;
            top: 0;
            z-index: 1000;
            height: 60px;
        }

        .header-left {
            display: flex;
            align-items: center;
            gap: 15px;
        }

        .header-right {
            display: flex;
            align-items: center;
            gap: 15px;
        }

        .logo {
            color: white;
            font-size: 24px;
            font-weight: bold;
            display: flex;
            align-items: center;
            gap: 10px;
            text-decoration: none;
        }

        .sidebar-toggle {
            width: 36px;
            height: 36px;
            border-radius: 50%;
            background: rgba(255,255,255,0.2);
            border: none;
            color: white;
            font-size: 18px;
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.3s;
        }

        .sidebar-toggle:hover {
            background: rgba(255,255,255,0.3);
        }

        .user-info-header {
           display: flex;
            align-items: center;
            gap: 12px;
            cursor: pointer;
            padding: 6px 12px;
            border-radius: 6px;
            transition: background-color 0.2s ease;
        }

        .user-info-header:hover {
            background: rgba(255,255,255,0.1);
        }

        .user-name-header {
            color: white;
            font-weight: 600;
            font-size: 15px;
            max-width: 200px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }

        .user-name-header.fona {
            color: transparent;
            -webkit-background-clip: text;
            background-clip: text;
            background-image: url("/admin/image2.png"), url("/admin/image3.png");
            background-size: 100% 200%;
            background-position: 100% 100%, 100% 200%;
            background-repeat: no-repeat, no-repeat;
            animation: slideImages 6s linear infinite;
            font-weight: 700;
        }

        @keyframes slideImages {
            0%, 100% { background-position: 100% 100%, 100% 200%; }
            50% { background-position: 100% 0%, 100% 100%; }
        }

        .profile-avatar {
            width: 32px;
            height: 32px;
            border-radius: 50%;
            background: rgba(255,255,255,0.2);
            display: flex;
            align-items: center;
            justify-content: center;
            color: white;
            font-size: 14px;
            font-weight: 600;
            overflow: hidden;
            
        }

        .profile-avatar img {
            width: 100%;
            height: 100%;
            object-fit: cover;
        }

        /* Main Content Area */
        .main-container {
            display: flex;
            flex: 1;
            overflow: hidden;
        }

        /* Sidebar */
        .sidebar {
            width: 260px;
            background: var(--white);
            border-right: 1px solid var(--border-color);
            padding: 20px 10px;
            overflow-y: auto;
            transition: all 0.3s;
            
        }

        .sidebar-aintieur{
             position: fixed;
        }

        .sidebar.collapsed {
            width: 0;
            padding: 0;
            opacity: 0;
        }

        .sidebar::-webkit-scrollbar {
            width: 6px;
        }

        .sidebar::-webkit-scrollbar-thumb {
            background: var(--border-color);
            border-radius: 3px;
        }

        .folder-item {
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 12px 16px;
            margin: 4px 0;
            border-radius: 8px;
            color: var(--text-dark);
            text-decoration: none;
            font-size: 15px;
            font-weight: 500;
            cursor: pointer;
            transition: all 0.2s;
           
        }

        .folder-item:hover {
            background: var(--hover-bg);
        }

        .folder-item.active {
            background: var(--green-primary);
            color: white;
            font-weight: 600;
        }

        .folder-item i {
            font-size: 18px;
            width: 20px;
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .folder-item.admin-item {
            color: #d32f2f;
        }

        .folder-item.admin-item:hover {
            background: rgba(211, 47, 47, 0.1);
        }

        .folder-item.admin-item.active {
            background: #d32f2f;
            color: white;
        }

        .nav-divider {
            height: 1px;
            background: var(--border-color);
            margin: 12px 8px;
        }

        /* Content Area */
        .content-area {
            flex: 1;
            background: var(--white);
            padding: 30px;
            overflow-y: auto;
        }

        .welcome-section {
            background: linear-gradient(135deg, var(--green-primary) 0%, var(--green-dark) 100%);
            border-radius: 12px;
            padding: 40px;
            margin-bottom: 30px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.1);
            color: white;
        }

        .welcome-title {
            font-size: 32px;
            font-weight: 300;
            margin-bottom: 10px;
        }

        .welcome-name {
            font-size: 42px;
            font-weight: 700;
        }

        .welcome-name.fona {
            color: transparent;
            -webkit-background-clip: text;
            background-clip: text;
            background-image: url("/admin/image2.png"), url("/admin/image3.png");
            background-size: 100% 200%;
            background-position: 100% 100%, 100% 200%;
            background-repeat: no-repeat, no-repeat;
            animation: slideImages 6s linear infinite;
        }

        .content-grid {
           display: flex;
            
            gap: 20px;
            margin-top: 30px;
            align-items: stretch;
            flex-wrap: wrap;
        }

        .card {
            background: var(--white);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 24px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.05);
            transition: all 0.3s;
            width: 49%;
        }

        .card:hover {
            box-shadow: 0 4px 16px rgba(0,0,0,0.1);
        }

        .card-header {
            display: flex;
            align-items: center;
            gap: 15px;
            margin-bottom: 20px;
            padding-bottom: 15px;
            border-bottom: 2px solid var(--border-color);
        }

        .card-icon {
            width: 48px;
            height: 48px;
            border-radius: 12px;
            background: var(--success-bg);
            display: flex;
            align-items: center;
            justify-content: center;
            color: var(--green-primary);
            font-size: 24px;
            flex-shrink: 0;
        }

        .card-header-text {
            flex: 1;
        }

        .card-title {
            font-size: 20px;
            font-weight: 600;
            margin-bottom: 4px;
            color: var(--text-dark);
        }

        .card-subtitle {
            font-size: 14px;
            color: var(--text-secondary);
        }

        .card-content {
            display: flex;
            flex-direction: column;
            gap: 12px;
        }

        .log-item, .file-item, .mail-item {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 12px 16px;
            background: var(--bg-light);
            border-radius: 8px;
            transition: all 0.2s;
            cursor: pointer;
            border-left: 3px solid transparent;
        }

        .log-item:hover, .file-item:hover, .mail-item:hover {
            background: var(--hover-bg);
            border-left-color: var(--green-primary);
            transform: translateX(5px);
        }

        .item-left {
            display: flex;
            align-items: center;
            gap: 12px;
            flex: 1;
            min-width: 0;
        }

        .item-icon {
            font-size: 18px;
            color: var(--green-primary);
            flex-shrink: 0;
        }

        .item-info {
            flex: 1;
            min-width: 0;
        }

        .item-title {
            font-size: 14px;
            font-weight: 600;
            color: var(--text-dark);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 250px;
        }

        .item-subtitle {
            font-size: 12px;
            color: var(--text-secondary);
            margin-top: 2px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 250px;
        }

        .item-right {
            display: flex;
            align-items: center;
            gap: 10px;
            flex-shrink: 0;
        }

        .item-badge {
            padding: 4px 10px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 600;
        }

        .badge-success {
            background: #e8f5e9;
            color: #2e7d32;
        }

        .badge-warning {
            background: #fff3e0;
            color: #ef6c00;
        }

        .badge-error {
            background: #ffebee;
            color: #c62828;
        }

        .badge-info {
            background: #e3f2fd;
            color: #1565c0;
        }

        .item-time {
            font-size: 12px;
            color: var(--text-secondary);
            white-space: nowrap;
        }

        .btn-action {
            padding: 6px 12px;
            background: var(--green-primary);
            color: white;
            border: none;
            border-radius: 6px;
            font-size: 12px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.2s;
            text-decoration: none;
            display: inline-flex;
            align-items: center;
            gap: 6px;
        }

        .btn-action:hover {
            background: var(--green-dark);
        }

        .empty-state {
            text-align: center;
            padding: 40px 20px;
            color: var(--text-secondary);
        }

        .empty-state i {
            font-size: 48px;
            color: var(--border-color);
            margin-bottom: 15px;
        }

        .show-more {
            text-align: center;
            padding-top: 15px;
        }

        .show-more a {
            color: var(--green-primary);
            text-decoration: none;
            font-weight: 600;
            font-size: 14px;
            transition: all 0.2s;
        }

        .show-more a:hover {
            color: var(--green-dark);
        }

        /* Profile Menu */
        .profile-menu {
            position: fixed;
            top: 70px;
            right: 20px;
            width: 200px;
            background: var(--white);
            border: 1px solid var(--border-color);
            border-radius: 10px;
            box-shadow: 0 8px 30px rgba(0,0,0,0.18);
            padding: 8px;
            display: none;
            z-index: 1002;
        }

        .profile-menu.active {
            display: block;
        }

        .profile-menu a {
            display: block;
            text-decoration: none;
            color: var(--text-dark);
            padding: 10px 12px;
            border-radius: 8px;
            font-weight: 600;
            margin-bottom: 4px;
            transition: background 0.2s;
        }

        .profile-menu a:hover {
            background: var(--hover-bg);
        }

        /* Overlay */
        .overlay {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0,0,0,0.3);
            display: none;
            z-index: 1001;
        }

        .overlay.active {
            display: block;
        }

        /* Responsive */
        @media (max-width: 768px) {
            .sidebar {
                position: fixed;
                left: 0;
                top: 0;
                bottom: 0;
                z-index: 1100;
                transform: translateX(-100%);
            }

            .sidebar.show {
                transform: translateX(0);
            }

            .sidebar.collapsed {
                transform: translateX(-100%);
                width: 260px;
                opacity: 1;
            }

            .user-name-header {
                display: none;
            }

            .welcome-section {
                padding: 30px 20px;
            }

            .welcome-title {
                font-size: 24px;
            }

            .welcome-name {
                font-size: 32px;
            }

            .content-grid {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
<?php


 include '/var/www/html/access_control.php';
 include '/var/www/html/function.php';
 footer();
 ini_set('display_errors', 1);
ini_set('display_startup_errors', 1);
error_reporting(E_ALL);
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("La connexion a échoué : " . $conn->connect_error);
}

$user_nom = "";
$user_privilege = 10;
$couler_privilege = "#000000";
$user_id = 0;

if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    
    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=?");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        
        if ($row['pc'] === $_SERVER['REMOTE_ADDR'] && $row['navi'] === $_SERVER['HTTP_USER_AGENT']) {
            $one_hour_ago = strtotime('-1 hour');
            $datecra_timestamp = strtotime($row['datecra']);
            
            if ($datecra_timestamp > $one_hour_ago) {
                $user_email = $row['email'];
                $user_nom = $row['nom'];
                
                $stmt = $conn->prepare("SELECT * FROM login WHERE email=?");
                $stmt->bind_param("s", $user_email);
                $stmt->execute();
                $result = $stmt->get_result();
                
                if ($result->num_rows == 1) {
                    $row = $result->fetch_assoc();
                    $user_id = (int)$row['id'];
                    $user_privilege = (int)$row['privilege'];
                    
                    $privilege_details = getPrivilegeDetails($user_privilege);
                    $couler_privilege = $privilege_details['couleur_privilege'];
                }
            } else {
                header("Location: login.php");
                echo "<script>window.location.href='login.php';</script>";
                exit();
            }
        } else {
            header("Location: login.php");
            echo "<script>window.location.href='login.php';</script>";
            exit();
        }
    }
    $stmt->close();
} else {
    header("Location: login.php");
    echo "<script>window.location.href='login.php';</script>";
    exit();
}
    function getFileIcon($filename) {
        $icon_map = [
            'jpg' => 'fa-file-image',
            'jpeg' => 'fa-file-image',
            'png' => 'fa-file-image',
            'svg' => 'fa-file-image',
            'gif' => 'fa-file-image',
            'tiff' => 'fa-file-image',
            'tif' => 'fa-file-image',
            'mp4' => 'fa-file-video',
            'webm' => 'fa-file-video',
            'pdf' => 'fa-file-pdf',
            'doc' => 'fa-file-word',
            'docx' => 'fa-file-word',
            'xls' => 'fa-file-excel',
            'xlsx' => 'fa-file-excel',
            'ppt' => 'fa-file-powerpoint',
            'pptx' => 'fa-file-powerpoint',
            'txt' => 'fa-file-lines',
            'csv' => 'fa-file-csv',
            'zip' => 'fa-file-zipper',
            'gz' => 'fa-file-zipper',
            'rar' => 'fa-file-zipper',
            '7z' => 'fa-file-zipper',
            'sql' => 'fa-database',
            'php' => 'fa-file-code',
            'html' => 'fa-file-code',
            'css' => 'fa-file-code',
            'js' => 'fa-file-code',
            'json' => 'fa-file-code',
            'xml' => 'fa-file-code',
            'mtl' => 'fa-solid fa-cube',
            'obj' => 'fa-solid fa-cube',
            'fbx' => 'fa-solid fa-cube',
            'stl' => 'fa-solid fa-cube',
            'default' => 'fa-file'
        ];
        
        $extension = strtolower(pathinfo($filename, PATHINFO_EXTENSION));
        return isset($icon_map[$extension]) ? $icon_map[$extension] : $icon_map['default'];
    }
// Récupérer les préférences de thème et icône profil
$user_prefs = getUserPreferences($user_id, $conn);
$current_theme = $user_prefs['teme'] ?? 0;
$isDarkMode = ($current_theme === 1);
$profile_icon_type = $user_prefs['profile_icon_type'] ?? 'initials';
$profile_icon_url = $user_prefs['profile_icon_url'] ?? null;

// Fonction helper pour obtenir l'icône de fichier


// Récupérer les statistiques et données récentes
$stats = [];
$recentData = [];

// 1. ADMIN - Derniers logs système (regroupés par utilisateur, connexions < 1h)
if ($user_privilege < 8) {
    try {
        $one_hour_ago = date('Y-m-d H:i:s', strtotime('-1 hour'));
        $stmt = $conn->prepare("
            SELECT email, nom, MAX(datecra) as derniere_connexion, pc, COUNT(*) as nb_connexions
            FROM loginc 
            WHERE datecra > ?
            GROUP BY email, nom, pc
            ORDER BY derniere_connexion DESC 
            LIMIT 5
        ");
        $stmt->bind_param("s", $one_hour_ago);
        $stmt->execute();
        $result = $stmt->get_result();
        $recentData['admin_logs'] = $result->fetch_all(MYSQLI_ASSOC);
        $stmt->close();
    } catch (Exception $e) {
        $recentData['admin_logs'] = [];
    }
}

// 2. EXODRIVE - Fichiers privés puis publics si pas assez
try {
    // Fichiers privés de l'utilisateur
    $stmt = $conn->prepare("SELECT id, nom, type_fichier, taille, date, visble FROM fichiers WHERE id_utilisateur = ? ORDER BY date DESC LIMIT 5");
    $stmt->bind_param("s", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();
    $private_files = $result->fetch_all(MYSQLI_ASSOC);
    $stmt->close();
    
    $recentData['exodrive_files'] = $private_files;
    
    // Si moins de 5 fichiers privés, compléter avec fichiers publics les plus vus
    if (count($private_files) < 5) {
        $remaining = 5 - count($private_files);
        $stmt = $conn->prepare("SELECT id, nom, type_fichier, taille, date, visble FROM fichiers WHERE visble = 'public' ORDER BY date DESC LIMIT ?");
        $stmt->bind_param("i", $remaining);
        $stmt->execute();
        $result = $stmt->get_result();
        $public_files = $result->fetch_all(MYSQLI_ASSOC);
        $stmt->close();
        
        $recentData['exodrive_files'] = array_merge($private_files, $public_files);
    }
    
    // Statistiques
    $stmt = $conn->prepare("SELECT COUNT(*) as total, SUM(taille) as total_size FROM fichiers WHERE id_utilisateur = ?");
    $stmt->bind_param("s", $user_id);
    $stmt->execute();
    $result = $stmt->get_result()->fetch_assoc();
    $stats['exodrive'] = [
        'total' => $result['total'] ?? 0,
        'total_size' => $result['total_size'] ?? 0
    ];
    $stmt->close();
} catch (Exception $e) {
    $recentData['exodrive_files'] = [];
    $stats['exodrive'] = ['total' => 0, 'total_size' => 0];
}

// 3. VEXMAIL - Emails récents
try {
    $user_email_clean = $user_email ?? '';
    $stmt = $conn->prepare("SELECT cd@, objet, date FROM mail WHERE a@ = ? AND folder = 'inbox' ORDER BY date DESC LIMIT 5");
    $stmt->bind_param("s", $user_email_clean);
    $stmt->execute();
    $result = $stmt->get_result();
    $recentData['mail_messages'] = $result->fetch_all(MYSQLI_ASSOC);
    $stmt->close();
    
    // Compteur non lus
    $stmt = $conn->prepare("SELECT COUNT(*) as unread FROM mail WHERE a@ = ? AND `read` = 0 AND folder = 'inbox'");
    $stmt->bind_param("s", $user_email_clean);
    $stmt->execute();
    $result = $stmt->get_result()->fetch_assoc();
    $stats['mail'] = ['unread' => $result['unread'] ?? 0];
    $stmt->close();
} catch (Exception $e) {
    $recentData['mail_messages'] = [];
    $stats['mail'] = ['unread' => 0];
}

// 4. SITEC - Pages privées puis publiques
try {
    // Pages privées de l'utilisateur
    $stmt = $conn->prepare("SELECT idpage, nompage, urlpage, popular FROM sitec WHERE user_id = ? ORDER BY popular DESC LIMIT 5");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();
    $private_sites = $result->fetch_all(MYSQLI_ASSOC);
    $stmt->close();
    
    $recentData['sitec_sites'] = $private_sites;
    
    // Si moins de 5 pages privées, compléter avec pages publiques
    if (count($private_sites) < 5) {
        $remaining = 5 - count($private_sites);
        $stmt = $conn->prepare("SELECT idpage, nompage, urlpage, popular FROM sitec WHERE porb = 1 ORDER BY popular DESC LIMIT ?");
        $stmt->bind_param("i", $remaining);
        $stmt->execute();
        $result = $stmt->get_result();
        $public_sites = $result->fetch_all(MYSQLI_ASSOC);
        $stmt->close();
        
        $recentData['sitec_sites'] = array_merge($private_sites, $public_sites);
    }
    
    $stmt = $conn->prepare("SELECT COUNT(*) as total FROM sitec WHERE user_id = ?");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result()->fetch_assoc();
    $stats['sitec'] = ['total' => $result['total'] ?? 0];
    $stmt->close();
} catch (Exception $e) {
    $recentData['sitec_sites'] = [];
    $stats['sitec'] = ['total' => 0];
}

$conn->close();

$initials = strtoupper(substr($user_nom, 0, 1));

// Fonction helper pour formater la taille
function formatSize($bytes) {
    if ($bytes >= 1073741824) {
        return number_format($bytes / 1073741824, 2) . ' GB';
    } elseif ($bytes >= 1048576) {
        return number_format($bytes / 1048576, 2) . ' MB';
    } elseif ($bytes >= 1024) {
        return number_format($bytes / 1024, 2) . ' KB';
    } else {
        return $bytes . ' octets';
    }
}

// Fonction helper pour le temps relatif
function timeAgo($datetime) {
    $timestamp = strtotime($datetime);
    $diff = time() - $timestamp;
    
    if ($diff < 60) return "À l'instant";
    if ($diff < 3600) return floor($diff / 60) . " min";
    if ($diff < 86400) return floor($diff / 3600) . " h";
    if ($diff < 604800) return floor($diff / 86400) . " j";
    return date('d/m/Y', $timestamp);
}
?>

<body data-theme="<?php echo $isDarkMode ? 'dark' : 'light'; ?>">

<!-- Header -->
<div class="header">
    <div class="header-left">
        <button class="sidebar-toggle" id="sidebar-toggle">
            <i class="fas fa-bars"></i>
        </button>
        <a href="#" class="logo">
            <img src="/vexB.png" alt="Vex Logo" style="width: 40px; height: 40px;">
     
        </a>
    </div>
    
    <div class="header-right">
        <div class="user-info-header" id="user-info-header">
            <?php if ($couler_privilege == "fona"): ?>
                <span class="user-name-header fona"><?php echo htmlspecialchars($user_nom); ?></span>
            <?php else: ?>
                <span class="user-name-header"><?php echo htmlspecialchars($user_nom); ?></span>
            <?php endif; ?>
           <div class="profile-avatar">
            <?php if ($profile_icon_type === 'image' && !empty($profile_icon_url)): ?>
                <img src="<?php echo htmlspecialchars($profile_icon_url); ?>" alt="Profile" style="width: 100%; height: 100%; object-fit: cover; border-radius: 50%;">
            <?php else: ?>
                <i class="fas fa-user"></i>

              
            <?php endif; ?>
        </div>
        </div>
    </div>
</div>

<!-- Main Container -->
<div class="main-container">
    <!-- Sidebar -->
    <nav class="sidebar" id="sidebar">
        <div class="sidebar-aintieur">
    <a href="/" class="folder-item active">
        <i class="fas fa-home"></i>
        <span>Accueil</span>
    </a>
    
    <a href="/tel/" class="folder-item">
        <i class="fas fa-hard-drive"></i>
        <span>Exodrive</span>
    </a>
        
        <a href="/mess/vexmail.php" class="folder-item">
            <i class="fas fa-envelope"></i>
            <span>Mail</span>
        </a>
        
        <a href="/sitec/" class="folder-item">
            <i class="fas fa-globe"></i>
            <span>Sitec</span>
        </a>
        
        <a href="#" class="folder-item">
            <i class="fas fa-video"></i>
            <span>Vidéos</span>
        </a>
        
        <a href="/edite1/" class="folder-item">
            <i class="fas fa-folder-open"></i>
            <span>Fichiers</span>
        </a>

        <?php if ($user_privilege < 8): ?>
        <div class="nav-divider"></div>
        
        <a href="/admin/admin.php" class="folder-item admin-item">
            <i class="fas fa-shield-alt"></i>
            <span>Administration</span>
        </a>
        
       
       
        <?php endif; ?>
        </div>
    </nav>

    <!-- Content Area -->
    <div class="content-area">
        <div class="welcome-section">
            <div class="welcome-title">Bienvenue,</div>
            <?php if ($couler_privilege == "fona"): ?>
                <div class="welcome-name fona"><?php echo htmlspecialchars($user_nom); ?></div>
            <?php else: ?>
                <div class="welcome-name"><?php echo htmlspecialchars($user_nom); ?></div>
            <?php endif; ?>
        </div>

        <div class="content-grid">
            <?php if ($user_privilege < 8): ?>
            <!-- Carte Administration -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon" style="background: rgba(211, 47, 47, 0.1); color: #d32f2f;">
                        <i class="fas fa-shield-alt"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">Administration</div>
                        <div class="card-subtitle">Dernières connexions au système</div>
                    </div>
                    <a href="/admin/admin.php" class="btn-action">
                        <i class="fas fa-cog"></i>
                        Gérer
                    </a>
                </div>
                <div class="card-content">
                    <?php if (!empty($recentData['admin_logs'])): ?>
                        <?php foreach ($recentData['admin_logs'] as $log): ?>
                            <div class="log-item" onclick="alert('Détails: <?php echo htmlspecialchars($log['nom']); ?>\nConnexions: <?php echo $log['nb_connexions']; ?>')">
                                <div class="item-left">
                                    <i class="fas fa-user-shield item-icon"></i>
                                    <div class="item-info">
                                        <div class="item-title"><?php echo htmlspecialchars($log['nom']); ?> (<?php echo $log['nb_connexions']; ?> connexion<?php echo $log['nb_connexions'] > 1 ? 's' : ''; ?>)</div>
                                        <div class="item-subtitle"><?php echo htmlspecialchars($log['email']); ?> - IP: <?php echo htmlspecialchars($log['pc']); ?></div>
                                    </div>
                                </div>
                                <div class="item-right">
                                    <span class="item-badge badge-success">Actif</span>
                                    <span class="item-time"><?php echo timeAgo($log['derniere_connexion']); ?></span>
                                </div>
                            </div>
                        <?php endforeach; ?>
                        <div class="show-more">
                            <a href="/admin/admin.php">
                                <i class="fas fa-chevron-right"></i> Voir tous les logs
                            </a>
                        </div>
                    <?php else: ?>
                        <div class="empty-state">
                            <i class="fas fa-clipboard-list"></i>
                            <p>Aucune connexion active</p>
                        </div>
                    <?php endif; ?>
                </div>
            </div>
            <?php endif; ?>

            <!-- Carte Exodrive -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon">
                        <i class="fas fa-hard-drive"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">Exodrive</div>
                        <div class="card-subtitle"><?php echo $stats['exodrive']['total']; ?> fichiers - <?php echo formatSize($stats['exodrive']['total_size']); ?></div>
                    </div>
                    <a href="/tel/" class="btn-action">
                        <i class="fas fa-folder-open"></i>
                        Ouvrir
                    </a>
                </div>
                <div class="card-content">
                    <?php if (!empty($recentData['exodrive_files'])): ?>
                        <?php foreach ($recentData['exodrive_files'] as $file): ?>
                            <?php 
                                $fileIcon = getFileIcon($file['nom']);
                                $fileUrl = '/tel/tel.php?id=' . $file['id'];
                                $downloadUrl = '/tel/tel.php?id=' . $file['id'] . '&download=1';
                                $isPublic = isset($file['visble']) && $file['visble'] === 'public';
                            ?>
                            <div class="file-item" onclick="window.location.href='<?php echo $fileUrl; ?>'">
                                <div class="item-left">
                                    <i class="fas <?php echo $fileIcon; ?> item-icon"></i>
                                    <div class="item-info">
                                        <div class="item-title">
                                            <?php echo htmlspecialchars($file['nom']); ?>
                                            <?php if ($isPublic): ?>
                                                <span class="item-badge badge-info" style="margin-left: 8px;">Public</span>
                                            <?php endif; ?>
                                        </div>
                                        <div class="item-subtitle"><?php echo formatSize($file['taille']); ?> - <?php echo htmlspecialchars($file['type_fichier']); ?></div>
                                    </div>
                                </div>
                                <div class="item-right">
                                    <span class="item-time"><?php echo timeAgo($file['date']); ?></span>
                                    <a href="<?php echo $downloadUrl; ?>" class="btn-action" onclick="event.stopPropagation();" title="Télécharger">
                                        <i class="fas fa-download"></i>
                                    </a>
                                    <a href="<?php echo $fileUrl; ?>" class="btn-action" onclick="event.stopPropagation();">
                                        <i class="fas fa-eye"></i>
                                        Voir
                                    </a>
                                </div>
                            </div>
                        <?php endforeach; ?>
                        <div class="show-more">
                            <a href="/tel/">
                                <i class="fas fa-chevron-right"></i> Voir tous les fichiers
                            </a>
                        </div>
                    <?php else: ?>
                        <div class="empty-state">
                            <i class="fas fa-folder-open"></i>
                            <p>Aucun fichier</p>
                        </div>
                    <?php endif; ?>
                </div>
            </div>

            <!-- Carte VexMail -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon">
                        <i class="fas fa-envelope"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">VexMail</div>
                        <div class="card-subtitle"><?php echo $stats['mail']['unread']; ?> message<?php echo $stats['mail']['unread'] > 1 ? 's' : ''; ?> non lu<?php echo $stats['mail']['unread'] > 1 ? 's' : ''; ?></div>
                    </div>
                    <a href="/mess/vexmail.php" class="btn-action">
                        <i class="fas fa-inbox"></i>
                        Ouvrir
                    </a>
                </div>
                <div class="card-content">
                    <?php if (!empty($recentData['mail_messages'])): ?>
                        <?php foreach ($recentData['mail_messages'] as $mail): ?>
                            <div class="mail-item" onclick="window.location.href='/mess/vexmail.php'">
                                <div class="item-left">
                                    <i class="fas fa-envelope item-icon"></i>
                                    <div class="item-info">
                                        <div class="item-title"><?php echo htmlspecialchars($mail['objet'] ?: '(Sans objet)'); ?></div>
                                        <div class="item-subtitle">De: <?php echo htmlspecialchars($mail['cd@']); ?></div>
                                    </div>
                                </div>
                                <div class="item-right">
                                    <span class="item-time"><?php echo timeAgo($mail['date']); ?></span>
                                    <a href="/mess/vexmail.php" class="btn-action" onclick="event.stopPropagation();">
                                        <i class="fas fa-eye"></i>
                                        Lire
                                    </a>
                                </div>
                            </div>
                        <?php endforeach; ?>
                        <div class="show-more">
                            <a href="/mess/vexmail.php">
                                <i class="fas fa-chevron-right"></i> Voir tous les emails
                            </a>
                        </div>
                    <?php else: ?>
                        <div class="empty-state">
                            <i class="fas fa-inbox"></i>
                            <p>Aucun email récent</p>
                        </div>
                    <?php endif; ?>
                </div>
            </div>

            <!-- Carte Sitec -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon">
                        <i class="fas fa-globe"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">Sitec</div>
                        <div class="card-subtitle"><?php echo $stats['sitec']['total']; ?> site<?php echo $stats['sitec']['total'] > 1 ? 's' : ''; ?> créé<?php echo $stats['sitec']['total'] > 1 ? 's' : ''; ?></div>
                    </div>
                    <a href="/sitec/" class="btn-action">
                        <i class="fas fa-plus"></i>
                        Créer
                    </a>
                </div>
                <div class="card-content">
                    <?php if (!empty($recentData['sitec_sites'])): ?>
                        <?php foreach ($recentData['sitec_sites'] as $site): ?>
                            <?php 
                                $pageUrl = '/sitec/pages/' . $site['urlpage'] . ".php";
                            ?>
                            <div class="file-item" onclick="window.open('<?php echo $pageUrl; ?>', '_blank')">
                                <div class="item-left">
                                    <i class="fas fa-globe item-icon"></i>
                                    <div class="item-info">
                                        <div class="item-title"><?php echo htmlspecialchars($site['nompage']); ?></div>
                                        <div class="item-subtitle"><?php echo htmlspecialchars(basename($site['urlpage'])); ?></div>
                                    </div>
                                </div>
                                <div class="item-right">
                                    <span class="item-badge badge-info"><?php echo $site['popular']; ?> vues</span>
                                    <a href="<?php echo $pageUrl; ?>" target="_blank" class="btn-action" onclick="event.stopPropagation();">
                                        <i class="fas fa-external-link-alt"></i>
                                        Visiter
                                    </a>
                                </div>
                            </div>
                        <?php endforeach; ?>
                        <div class="show-more">
                            <a href="/sitec/">
                                <i class="fas fa-chevron-right"></i> Voir tous les sites
                            </a>
                        </div>
                    <?php else: ?>
                        <div class="empty-state">
                            <i class="fas fa-globe"></i>
                            <p>Aucun site créé</p>
                        </div>
                    <?php endif; ?>
                </div>
            </div>

            <!-- Carte Éditeur -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon">
                        <i class="fas fa-folder-open"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">Éditeur de fichiers</div>
                        <div class="card-subtitle">Modifiez vos documents en ligne</div>
                    </div>
                    <a href="/edite1/" class="btn-action">
                        <i class="fas fa-edit"></i>
                        Ouvrir
                    </a>
                </div>
                <div class="card-content">
                    <div class="empty-state">
                        <i class="fas fa-edit"></i>
                        <p>Prêt à éditer vos fichiers</p>
                    </div>
                </div>
            </div>

            <!-- Carte Vidéos -->
            <div class="card">
                <div class="card-header">
                    <div class="card-icon">
                        <i class="fas fa-video"></i>
                    </div>
                    <div class="card-header-text">
                        <div class="card-title">Vidéos</div>
                        <div class="card-subtitle">Bientôt disponible</div>
                    </div>
                    <a href="#" class="btn-action" style="opacity: 0.5; cursor: not-allowed;">
                        <i class="fas fa-lock"></i>
                        Bientôt
                    </a>
                </div>
                <div class="card-content">
                    <div class="empty-state">
                        <i class="fas fa-video"></i>
                        <p>Cette fonctionnalité arrive bientôt</p>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>

<!-- Profile Menu -->
<div class="profile-menu" id="profile-menu">
    <a href="/login/account.php">Mon Compte</a>
    <a href="/login/account.php">Paramètres</a>
    <a href="/login/logout.php">Déconnexion</a>
</div>

<div class="overlay" id="overlay"></div>

<script>
// Toggle sidebar
const sidebarToggle = document.getElementById('sidebar-toggle');
const sidebar = document.getElementById('sidebar');

sidebarToggle.addEventListener('click', function() {
    sidebar.classList.toggle('collapsed');
    if (window.innerWidth <= 768) {
        sidebar.classList.toggle('show');
    }
    localStorage.setItem('sidebar', sidebar.classList.contains('collapsed') ? 'closed' : 'open');
});

// Charger l'état de la sidebar
const savedSidebar = localStorage.getItem('sidebar');
if (savedSidebar === 'closed') {
    sidebar.classList.add('collapsed');
}

// Toggle profile menu
const userInfoHeader = document.getElementById('user-info-header');
const profileMenu = document.getElementById('profile-menu');
const overlay = document.getElementById('overlay');

userInfoHeader.addEventListener('click', function() {
    profileMenu.classList.toggle('active');
    overlay.classList.toggle('active');
});

overlay.addEventListener('click', function() {
    profileMenu.classList.remove('active');
    overlay.classList.remove('active');
    if (window.innerWidth <= 768) {
        sidebar.classList.remove('show');
    }
});

// Échap pour fermer les menus
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        profileMenu.classList.remove('active');
        overlay.classList.remove('active');
        if (window.innerWidth <= 768) {
            sidebar.classList.remove('show');
        }
    }
});

// Fermer la sidebar mobile lors du clic sur un lien
if (window.innerWidth <= 768) {
    const folderItems = document.querySelectorAll('.folder-item');
    folderItems.forEach(item => {
        item.addEventListener('click', function() {
            sidebar.classList.remove('show');
        });
    });
}
</script>

</body>
</html>