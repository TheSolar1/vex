<?php
 include '/var/www/html/access_control.php';
 include '/var/www/html/function.php';

session_start();

// Connexion à la base de données
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("La connexion a echoue : " . $conn->connect_error);
}

// Vérification du cookie de session
$user_authenticated = false;
$user_data = null;

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
                $user_authenticated = true;
                $user_email = $row['email'];
                $user_nom = $row['nom'];
                
                $stmt2 = $conn->prepare("SELECT id, privilege, vip FROM login WHERE email=?");
                $stmt2->bind_param("s", $user_email);
                $stmt2->execute();
                $result2 = $stmt2->get_result();
                if ($result2->num_rows == 1) {
                    $user_row = $result2->fetch_assoc();
                    $user_id = $user_row['id'];
                    $user_privilege = $user_row['privilege'];
                    $user_vip = $user_row['vip'];
                }
                $stmt2->close();
            }
        }
    }
    $stmt->close();
}

if (!$user_authenticated) {
    header("Location: login.php");
    exit();
}

// Traitement des modifications de préférences
if ($_SERVER["REQUEST_METHOD"] == "POST" && isset($_POST['update_theme'])) {
    $new_theme = isset($_POST['theme']) ? (int)$_POST['theme'] : 0;
    if (updateUserPreference($user_id, 'teme', $new_theme, $conn)) {
        $success_message = "Préférences mises à jour avec succès!";
        header("Refresh:0");
    }
}

// Traitement de la modification du compte
if ($_SERVER["REQUEST_METHOD"] == "POST" && isset($_POST['modifier'])) {
    $enmotdepass = $_POST['enmotdepass'] ?? '';
    $nouveau_motdepasse = $_POST['modifier_motdepasse'] ?? '';
    
    if (!empty($nouveau_motdepasse)) {
        $stmt = $conn->prepare("SELECT motdepass FROM login WHERE email=?");
        $stmt->bind_param("s", $user_email);
        $stmt->execute();
        $stmt->store_result();
        
        if ($stmt->num_rows == 1) {
            $stmt->bind_result($motdepass_bd);
            $stmt->fetch();
            
            if (password_verify($enmotdepass, $motdepass_bd)) {
                $nouveau_motdepasse_hash = password_hash($nouveau_motdepasse, PASSWORD_DEFAULT);
                $stmt_update = $conn->prepare("UPDATE login SET motdepass=? WHERE email=?");
                $stmt_update->bind_param("ss", $nouveau_motdepasse_hash, $user_email);
                
                if ($stmt_update->execute()) {
                    $success_message = "Mot de passe mis à jour avec succès.";
                }
                $stmt_update->close();
            } else {
                $error_message = "Le mot de passe actuel est incorrect.";
            }
        }
        $stmt->close();
    }
}

// Récupérer les préférences actuelles
$user_prefs = getUserPreferences($user_id, $conn);
$current_theme = $user_prefs['teme'] ?? 0;
$isDarkMode = ($current_theme === 1);

// Définir le jeu de couleurs pour la navigation - Thème vert avec adaptation
$nav_colors = [
    'nav_bg' => '#4caf50',
    'nav_gradient' => 'linear-gradient(135deg, #4caf50 0%, #45a049 100%)',
    'shadow' => '0 2px 8px rgba(0,0,0,0.15)',
    'hover_bg' => 'rgba(255,255,255,0.15)',
    'active_color' => '#ffffff',
    'text_primary' => '#ffffff',
    'muted' => 'rgba(255,255,255,0.9)',
    'admin_sep' => 'rgba(255,255,255,0.1)',
    'popup_bg' => $isDarkMode ? '#242526' : '#ffffff',
    'popup_border' => $isDarkMode ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.1)',
    'app_item_bg' => $isDarkMode ? '#2a2a2a' : '#f7f7f7',
    'app_item_border' => $isDarkMode ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.05)',
    'app_item_hover' => $isDarkMode ? '#333333' : '#e8f5e9',
    'btn_bg' => 'rgba(255,255,255,0.2)',
    'btn_border' => 'rgba(255,255,255,0.3)',
    'profile_icon_bg' => 'rgba(255,255,255,0.2)'
];

// Définir les applications
$apps = [
    ['icon' => 'fa-envelope', 'label' => 'Mail', 'url' => '/mess/vexmail.php'],
    ['icon' => 'fa-hard-drive', 'label' => 'Exodrive', 'url' => '/tel/'],
    ['icon' => 'fa-folder-open', 'label' => 'Éditeur de fichiers', 'url' => '/edite1/'],
    ['icon' => 'fa-video', 'label' => 'Vidéos', 'url' => '#'],
    ['icon' => 'fa-globe', 'label' => 'Sitec', 'url' => '/sitec/']
];
?>

<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Page de Réglages - .hopto.org</title>
    <link rel="icon" type="image/png" href="/vex.png">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #ffffff;
            min-height: 100vh;
            padding-top: 60px;
            overflow-x: hidden;
        }

        /* Style pour la navigation VexMail compatible */
        .top-nav-bar-haut7844 {
            background: linear-gradient(135deg, #4caf50 0%, #45a049 100%) !important;
        }
        .top-nav-bar-haut7844 .apps-btn-nav-7844,
        .top-nav-bar-haut7844 .user-info-top-7844 {
            color: white !important;
        }
        .apps-btn-nav-7844 {
            background: rgba(255,255,255,0.2) !important;
            border-color: rgba(255,255,255,0.3) !important;
            color: white !important;
        }
        .apps-btn-nav-7844:hover {
            background: rgba(255,255,255,0.3) !important;
        }
        .user-name-top-7844 {
            color: white !important;
        }
        .profile-icon-top-7844 {
            background: rgba(255,255,255,0.2) !important;
            color: white !important;
        }
        
        /* Popups adaptés au thème */
        .apps-popup-7844, .profile-menu-7844 {
            font-size: 15px !important;
        }
        .app-item-7844 {
            color: #1c1e21 !important;
        }
        .app-item-7844 i {
            color: #4caf50 !important;
        }
        .app-item-7844 span {
            color: #1c1e21 !important;
        }
        .app-item-7844:hover {
            background: #4caf50 !important;
            color: white !important;
        }
        .app-item-7844:hover i {
            color: white !important;
        }
        .app-item-7844:hover span {
            color: white !important;
        }
        .profile-menu-7844 a {
            color: #1c1e21 !important;
        }
        .profile-menu-7844 a:hover {
            background: #4caf50 !important;
            color: white !important;
        }

        .container-main {
            max-width: 1400px;
            margin: 20px auto;
            padding: 0 20px;
        }

        /* Messages */
        .message {
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
            animation: slideIn 0.3s ease;
        }

        .message.success {
            background: rgba(76, 175, 80, 0.2);
            color: #2e7d32;
            border-left: 4px solid #4caf50;
        }

        .message.error {
            background: rgba(255, 59, 48, 0.2);
            color: #c62828;
            border-left: 4px solid #ff3b30;
        }

        @keyframes slideIn {
            from {
                transform: translateY(-20px);
                opacity: 0;
            }
            to {
                transform: translateY(0);
                opacity: 1;
            }
        }

        /* Layout principal */
        .main-layout {
            display: flex;
            gap: 20px;
            margin-top: 20px;
        }

        /* Sidebar */
        .sidebar {
            width: 260px;
            background: rgba(255, 255, 255, 0.95);
            border-radius: 16px;
            padding: 20px 10px;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
            backdrop-filter: blur(10px);
            height: fit-content;
            position: sticky;
            top: 80px;
        }

        .sidebar-link {
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 12px 16px;
            color: #333;
            text-decoration: none;
            border-radius: 8px;
            margin-bottom: 8px;
            transition: all 0.3s ease;
            font-weight: 500;
        }

        .sidebar-link:hover {
            background: rgba(76, 175, 80, 0.1);
            color: #4caf50;
        }

        .sidebar-link.active {
            background: #4caf50;
            color: white;
        }

        .sidebar-link i {
            font-size: 18px;
            width: 20px;
            text-align: center;
        }

        /* Content Area */
        .content-area {
            flex: 1;
            min-width: 0;
        }

        /* Cards */
        .card {
            background: rgba(255, 255, 255, 0.95);
            border-radius: 16px;
            padding: 30px;
            margin-bottom: 20px;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
            backdrop-filter: blur(10px);
            transition: transform 0.3s ease, box-shadow 0.3s ease;
        }

        .card:hover {
            transform: translateY(-5px);
            box-shadow: 0 12px 48px rgba(0, 0, 0, 0.15);
        }

        .card h2 {
            color: #4caf50;
            margin-bottom: 20px;
            font-size: 24px;
            display: flex;
            align-items: center;
            gap: 10px;
        }

        .card h2 i {
            font-size: 28px;
        }

        .card h3 {
            color: #45a049;
            margin: 20px 0 10px 0;
            font-size: 18px;
        }

        .card p {
            color: #666;
            margin-bottom: 20px;
            line-height: 1.6;
        }

        /* Formulaires */
        .form-group {
            margin-bottom: 20px;
        }

        .form-group label {
            display: block;
            margin-bottom: 8px;
            color: #333;
            font-weight: 600;
        }

        .form-group input[type="text"],
        .form-group input[type="password"],
        .form-group select {
            width: 100%;
            padding: 12px 16px;
            border: 2px solid #e0e0e0;
            border-radius: 8px;
            font-size: 16px;
            transition: border-color 0.3s ease;
            background: white;
        }

        .form-group input[readonly] {
            background: #f5f5f5;
            cursor: not-allowed;
        }

        .form-group input:focus,
        .form-group select:focus {
            outline: none;
            border-color: #4caf50;
        }

        /* Boutons */
        .btn {
            padding: 12px 24px;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s ease;
            display: inline-flex;
            align-items: center;
            gap: 8px;
        }

        .btn-primary {
            background: #4caf50;
            color: white;
            box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
        }

        .btn-primary:hover {
            transform: translateY(-2px);
            box-shadow: 0 6px 20px rgba(76, 175, 80, 0.4);
            background: #45a049;
        }

        /* Theme Toggle */
        .theme-toggle {
            display: flex;
            gap: 15px;
            margin-top: 15px;
        }

        .theme-option {
            flex: 1;
            padding: 20px;
            border: 3px solid #e0e0e0;
            border-radius: 12px;
            cursor: pointer;
            transition: all 0.3s ease;
            text-align: center;
        }

        .theme-option:hover {
            border-color: #4caf50;
            transform: translateY(-3px);
        }

        .theme-option.selected {
            border-color: #4caf50;
            background: rgba(76, 175, 80, 0.1);
        }

        .theme-option input[type="radio"] {
            display: none;
        }

        .theme-preview {
            width: 100%;
            height: 80px;
            border-radius: 8px;
            margin-bottom: 10px;
        }

        .theme-preview.light {
            background: linear-gradient(to bottom, #ffffff, #f0f0f0);
        }

        .theme-preview.dark {
            background: linear-gradient(to bottom, #1a1a1a, #0f0f0f);
        }

        /* Autologin Section */
        .autologin-info {
            background: rgba(76, 175, 80, 0.1);
            border-left: 4px solid #4caf50;
            padding: 15px;
            border-radius: 8px;
            margin-top: 15px;
        }

        .autologin-info a {
            color: #4caf50;
            font-weight: 600;
            text-decoration: none;
        }

        .autologin-info a:hover {
            text-decoration: underline;
        }

        /* Section Content */
        .section-content {
            display: none;
        }

        .section-content.active {
            display: block;
        }

        /* Popup ID Utilisateur */
        .id-popup-overlay {
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.5);
            z-index: 9998;
            backdrop-filter: blur(5px);
        }

        .id-popup {
            display: none;
            position: fixed;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: white;
            padding: 30px;
            border-radius: 16px;
            box-shadow: 0 12px 48px rgba(0, 0, 0, 0.2);
            z-index: 9999;
            min-width: 320px;
            text-align: center;
        }

        .id-popup h3 {
            margin-bottom: 20px;
            color: #4caf50;
        }

        .id-popup .user-id-value {
            font-size: 32px;
            font-weight: 700;
            color: #333;
            margin: 20px 0;
            padding: 15px;
            background: #f5f5f5;
            border-radius: 8px;
        }

        .id-popup .close-popup-btn {
            background: #4caf50;
            color: white;
            border: none;
            padding: 10px 24px;
            border-radius: 8px;
            cursor: pointer;
            font-weight: 600;
            transition: all 0.3s ease;
        }

        .id-popup .close-popup-btn:hover {
            background: #45a049;
            transform: translateY(-2px);
        }

        .show-id-btn {
            display: inline-flex;
            align-items: center;
            gap: 8px;
            padding: 8px 16px;
            background: #4caf50;
            color: white;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-size: 14px;
            font-weight: 600;
            transition: all 0.3s ease;
        }

        .show-id-btn:hover {
            background: #45a049;
            transform: translateY(-2px);
        }

        /* Mode sombre */
        body.dark-mode {
            background: #000000;
        }

        body.dark-mode .sidebar,
        body.dark-mode .card {
            background: rgba(36, 37, 38, 0.95);
            color: #e4e6eb;
        }

        body.dark-mode .card h2 {
            color: #66bb6a;
        }

        body.dark-mode .card h3 {
            color: #81c784;
        }

        body.dark-mode .card p {
            color: #b0b3b8;
        }

        body.dark-mode .sidebar-link {
            color: #e4e6eb;
        }

        body.dark-mode .sidebar-link:hover {
            background: rgba(102, 187, 106, 0.1);
            color: #66bb6a;
        }

        body.dark-mode .sidebar-link.active {
            background: #66bb6a;
            color: white;
        }

        body.dark-mode .form-group label {
            color: #e4e6eb;
        }

        body.dark-mode .form-group input,
        body.dark-mode .form-group select {
            background: #3a3b3c;
            border-color: #3a3b3c;
            color: #e4e6eb;
        }

        body.dark-mode .form-group input[readonly] {
            background: #2a2b2c;
            color: #b0b3b8;
        }

        body.dark-mode .form-group input:focus,
        body.dark-mode .form-group select:focus {
            border-color: #66bb6a;
        }

        body.dark-mode .theme-option {
            border-color: #3a3b3c;
        }

        body.dark-mode .theme-option:hover,
        body.dark-mode .theme-option.selected {
            border-color: #66bb6a;
            background: rgba(102, 187, 106, 0.1);
        }

        body.dark-mode .theme-option p {
            color: #b0b3b8 !important;
        }

        body.dark-mode .theme-option strong {
            color: #e4e6eb;
        }

        body.dark-mode .autologin-info {
            background: rgba(102, 187, 106, 0.1);
            border-left-color: #66bb6a;
        }

        body.dark-mode .autologin-info a {
            color: #81c784;
        }

        body.dark-mode .message.success {
            background: rgba(102, 187, 106, 0.2);
            border-left-color: #66bb6a;
            color: #81c784;
        }

        body.dark-mode .message.error {
            background: rgba(255, 59, 48, 0.2);
            border-left-color: #ff3b30;
            color: #ff6b6b;
        }

        body.dark-mode .btn-primary {
            background: #66bb6a;
            color: #000;
            box-shadow: 0 4px 12px rgba(102, 187, 106, 0.3);
        }

        body.dark-mode .btn-primary:hover {
            background: #81c784;
            box-shadow: 0 6px 20px rgba(102, 187, 106, 0.4);
        }

        body.dark-mode .id-popup {
            background: #242526;
        }

        body.dark-mode .id-popup h3 {
            color: #66bb6a;
        }

        body.dark-mode .id-popup .user-id-value {
            color: #e4e6eb;
            background: #3a3b3c;
        }

        body.dark-mode .id-popup .close-popup-btn {
            background: #66bb6a;
            color: #000;
        }

        body.dark-mode .id-popup .close-popup-btn:hover {
            background: #81c784;
        }

        body.dark-mode .id-popup p {
            color: #b0b3b8 !important;
        }

        body.dark-mode .show-id-btn {
            background: #66bb6a;
            color: #000;
        }

        body.dark-mode .show-id-btn:hover {
            background: #81c784;
        }

        /* Statistiques en mode sombre */
        body.dark-mode [style*="background: linear-gradient(135deg, rgba(33, 150, 243"] {
            background: linear-gradient(135deg, rgba(33, 150, 243, 0.15), rgba(25, 118, 210, 0.08)) !important;
        }

        body.dark-mode [style*="background: linear-gradient(135deg, rgba(156, 39, 176"] {
            background: linear-gradient(135deg, rgba(156, 39, 176, 0.15), rgba(123, 31, 162, 0.08)) !important;
        }

        body.dark-mode [style*="background: linear-gradient(135deg, rgba(76, 175, 80"] {
            background: linear-gradient(135deg, rgba(76, 175, 80, 0.15), rgba(69, 160, 73, 0.08)) !important;
        }

        body.dark-mode [style*="font-size: 14px; color: #666"] {
            color: #b0b3b8 !important;
        }

        body.dark-mode [style*="font-size: 28px"][style*="color: #333"] {
            color: #e4e6eb !important;
        }

        body.dark-mode [style*="font-size: 16px"][style*="color: #666"] {
            color: #b0b3b8 !important;
        }

        body.dark-mode [style*="border-bottom: 2px solid #e0e0e0"] {
            border-bottom-color: #3a3b3c !important;
        }

        /* Responsive */
        @media (max-width: 768px) {
            .main-layout {
                flex-direction: column;
            }

            .sidebar {
                position: relative;
                top: 0;
                width: 100%;
            }

            body {
                padding-top: 70px;
            }
        }
    </style>
</head>
<body<?php if ($isDarkMode) echo ' class="dark-mode"'; ?>>

<?php 
// Afficher la navigation avec le thème adapté
displayNavigation($conn, $apps, [], $nav_colors);
?>

<div class="container-main">
    <?php if (isset($success_message)): ?>
        <div class="message success">
            <i class="fas fa-check-circle"></i> <?php echo $success_message; ?>
        </div>
    <?php endif; ?>

    <?php if (isset($error_message)): ?>
        <div class="message error">
            <i class="fas fa-exclamation-circle"></i> <?php echo $error_message; ?>
        </div>
    <?php endif; ?>

    <div class="main-layout">
        <aside class="sidebar">
            <a href="#compte" class="sidebar-link active" onclick="showSection(event, 'compte')">
                <i class="fas fa-user-circle"></i>
                <span>Profil</span>
            </a>
            <a href="#preferences" class="sidebar-link" onclick="showSection(event, 'preferences')">
                <i class="fas fa-palette"></i>
                <span>Préférences</span>
            </a>
            <a href="#securite" class="sidebar-link" onclick="showSection(event, 'securite')">
                <i class="fas fa-lock"></i>
                <span>Sécurité</span>
            </a>
            <a href="#notifications" class="sidebar-link" onclick="showSection(event, 'notifications')">
                <i class="fas fa-bell"></i>
                <span>Notifications</span>
            </a>
            <a href="/constiontu.html" class="sidebar-link">
                <i class="fas fa-shield-alt"></i>
                <span>Confidentialité</span>
            </a>
        </aside>

        <div class="content-area">
            <!-- Section Profil -->
            <div id="compte-section" class="section-content active">
                <div class="card">
                    <h2>
                        <i class="fas fa-user-circle"></i>
                        Mon Profil
                    </h2>
                    
                    <!-- Avatar et informations principales -->
                    <div style="display: flex; align-items: start; gap: 30px; margin-bottom: 30px; padding-bottom: 30px; border-bottom: 2px solid #e0e0e0;">
                        <div style="flex-shrink: 0;">
                            <div style="width: 120px; height: 120px; border-radius: 50%; background: linear-gradient(135deg, #4caf50, #45a049); display: flex; align-items: center; justify-content: center; color: white; font-size: 48px; font-weight: 700; box-shadow: 0 8px 24px rgba(76, 175, 80, 0.3);">
                                <?php echo strtoupper(substr($user_nom, 0, 1)); ?>
                            </div>
                        </div>
                        <div style="flex: 1;">
                            <h3 style="font-size: 28px; margin-bottom: 8px; color: #333;"><?php echo htmlspecialchars($user_nom); ?></h3>
                            <p style="color: #666; font-size: 16px; margin-bottom: 12px;">
                                <i class="fas fa-envelope" style="margin-right: 8px; color: #4caf50;"></i>
                                <?php echo htmlspecialchars($user_email); ?>
                            </p>
                            <div style="display: flex; gap: 15px; align-items: center;">
                                <?php
                                $privilege_info = getPrivilegeDetails($user_privilege);
                                $privilege_color = $privilege_info['couleur_privilege'];
                                $privilege_nom = $privilege_info['nom_privilege'];
                                ?>
                                <span style="display: inline-flex; align-items: center; gap: 6px; padding: 6px 14px; background: <?php echo $privilege_color; ?>; color: white; border-radius: 20px; font-size: 13px; font-weight: 600;">
                                    <i class="fas fa-shield-alt"></i>
                                    <?php echo htmlspecialchars($privilege_nom); ?>
                                </span>
                                <?php if ($user_vip == 1): ?>
                                <span style="display: inline-flex; align-items: center; gap: 6px; padding: 6px 14px; background: linear-gradient(135deg, #FFD700, #FFA500); color: white; border-radius: 20px; font-size: 13px; font-weight: 600;">
                                    <i class="fas fa-crown"></i>
                                    VIP
                                </span>
                                <?php endif; ?>
                            </div>
                        </div>
                    </div>

                    <!-- Statistiques du compte -->
                    <h3 style="margin-bottom: 20px;">
                        <i class="fas fa-chart-line"></i>
                        Statistiques du Compte
                    </h3>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 30px;">
                        <div style="background: linear-gradient(135deg, rgba(33, 150, 243, 0.1), rgba(25, 118, 210, 0.05)); padding: 20px; border-radius: 12px; border-left: 4px solid #2196f3;">
                            <div style="font-size: 14px; color: #666; margin-bottom: 8px;">Niveau de Privilège</div>
                            <div style="font-size: 24px; font-weight: 700; color: #2196f3;"><?php echo htmlspecialchars($privilege_nom); ?></div>
                        </div>
                        <div style="background: linear-gradient(135deg, rgba(156, 39, 176, 0.1), rgba(123, 31, 162, 0.05)); padding: 20px; border-radius: 12px; border-left: 4px solid #9c27b0;">
                            <div style="font-size: 14px; color: #666; margin-bottom: 8px;">Statut d'Abonnement</div>
                            <div style="font-size: 24px; font-weight: 700; color: #9c27b0;"><?php echo $user_vip == 1 ? 'Premium' : 'Gratuit'; ?></div>
                        </div>
                    </div>

                   

                    <!-- Informations détaillées -->
                    <h3 style="margin-bottom: 20px;">
                        <i class="fas fa-info-circle"></i>
                        Informations Détaillées
                    </h3>
                    <div class="form-group">
                        <label>Nom d'utilisateur</label>
                        <input type="text" value="<?php echo htmlspecialchars($user_nom); ?>" readonly>
                    </div>
                    <div class="form-group">
                        <label>Adresse Email</label>
                        <input type="text" value="<?php echo htmlspecialchars($user_email); ?>" readonly>
                    </div>
                    <div class="form-group">
                        <label>Identifiant Unique</label>
                        <input type="text" value="<?php echo $user_id; ?>" readonly>
                    </div>
                     <!-- Bouton ID Utilisateur -->
                    <div style="margin-bottom: 30px; text-align: center;">
                        <button class="show-id-btn" onclick="showIdPopup()">
                            <i class="fas fa-fingerprint"></i>
                            Afficher mon ID Utilisateur
                        </button>
                    </div>
                </div>
            </div>

            <!-- Section Préférences -->
            <div id="preferences-section" class="section-content">
                <div class="card">
                    <h2>
                        <i class="fas fa-palette"></i>
                        Préférences d'Affichage
                    </h2>
                    <p>Personnalisez l'apparence de votre interface sur tout le site</p>

                    <form method="post" action="">
                        <div class="form-group">
                            <label>Thème de l'interface</label>
                            <div class="theme-toggle">
                                <label class="theme-option <?php echo $current_theme == 0 ? 'selected' : ''; ?>">
                                    <input type="radio" name="theme" value="0" <?php echo $current_theme == 0 ? 'checked' : ''; ?>>
                                    <div class="theme-preview light"></div>
                                    <strong>Thème Clair</strong>
                                    <p style="font-size: 12px; color: #666;">Interface lumineuse</p>
                                </label>

                                <label class="theme-option <?php echo $current_theme == 1 ? 'selected' : ''; ?>">
                                    <input type="radio" name="theme" value="1" <?php echo $current_theme == 1 ? 'checked' : ''; ?>>
                                    <div class="theme-preview dark"></div>
                                    <strong>Thème Sombre</strong>
                                    <p style="font-size: 12px; color: #666;">Interface sombre</p>
                                </label>
                            </div>
                        </div>

                        <button type="submit" name="update_theme" class="btn btn-primary">
                            <i class="fas fa-save"></i>
                            Enregistrer les préférences
                        </button>
                    </form>
                </div>
            </div>

            <!-- Section Sécurité -->
            <div id="securite-section" class="section-content">
                <div class="card">
                    <h2>
                        <i class="fas fa-lock"></i>
                        Modifier le Compte de <?php echo htmlspecialchars($user_nom); ?>
                    </h2>
                    
                    <form method="post" action="">
                        <h3>Modifier le mot de passe</h3>
                        
                        <div class="form-group">
                            <label for="enmotdepass">Mot de passe actuel :</label>
                            <input type="password" id="enmotdepass" name="enmotdepass" required>
                        </div>

                        <div class="form-group">
                            <label for="modifier_motdepasse">Nouveau mot de passe :</label>
                            <input type="password" id="modifier_motdepasse" name="modifier_motdepasse" required>
                        </div>

                        <button type="submit" name="modifier" class="btn btn-primary">
                            <i class="fas fa-key"></i>
                            Modifier le mot de passe
                        </button>
                    </form>

                    <?php
                    // Section Autologin
                    $stmt = $conn->prepare("SELECT nombre FROM autologin WHERE compteid=?");
                    $stmt->bind_param("i", $user_id);
                    $stmt->execute();
                    $result = $stmt->get_result();
                    
                    if ($result->num_rows == 1) {
                        $row = $result->fetch_assoc();
                        $nombre2 = $row['nombre'];
                        
                        $stmt2 = $conn->prepare("SELECT compteid FROM autologin WHERE nombre=?");
                        $stmt2->bind_param("s", $nombre2);
                        $stmt2->execute();
                        $result2 = $stmt2->get_result();
                        
                        if ($result2->num_rows == 1) {
                            $row2 = $result2->fetch_assoc();
                            $compteid = $row2['compteid'];
                            
                            if ($compteid !== $user_id) {
                                echo '<div class="autologin-info">
                                    <h3><i class="fas fa-link"></i> Autologin</h3>
                                    <p>Créez une URL pour vous connecter automatiquement</p>
                                    <a href="Autologin.php">Créer un URL pour Autologin</a>
                                    <p style="font-size: 12px; margin-top: 10px; color: #666;">Note: Vous pouvez créer une seule URL</p>
                                </div>';
                            } else {
                                echo '<div class="autologin-info">
                                    <h3><i class="fas fa-link"></i> Autologin</h3>
                                    <p>Vous avez déjà créé une URL pour Autologin.</p>
                                </div>';
                            }
                        }
                        $stmt2->close();
                    }
                    $stmt->close();
                    ?>
                </div>
            </div>

            <!-- Section Notifications -->
            <div id="notifications-section" class="section-content">
                <div class="card">
                    <h2>
                        <i class="fas fa-bell"></i>
                        Notifications
                    </h2>
                    <p>Gérez vos préférences de notifications ici.</p>
                </div>
            </div>
        </div>
    </div>
</div>

<!-- Popup ID Utilisateur -->
<div class="id-popup-overlay" id="idPopupOverlay" onclick="closeIdPopup()"></div>
<div class="id-popup" id="idPopup">
    <h3><i class="fas fa-fingerprint"></i> Votre ID Utilisateur</h3>
    <div class="user-id-value">#<?php echo $user_id; ?></div>
    <p style="color: #666; font-size: 14px; margin-bottom: 20px;">Gardez cet identifiant confidentiel</p>
    <button class="close-popup-btn" onclick="closeIdPopup()">
        <i class="fas fa-times"></i> Fermer
    </button>
</div>

<script>
    // Afficher la section Profil par défaut au chargement
    document.addEventListener('DOMContentLoaded', function() {
        const compteSection = document.getElementById('compte-section');
        if (compteSection) {
            compteSection.classList.add('active');
        }
    });

    // Fonctions pour le popup ID Utilisateur
    function showIdPopup() {
        document.getElementById('idPopupOverlay').style.display = 'block';
        document.getElementById('idPopup').style.display = 'block';
    }

    function closeIdPopup() {
        document.getElementById('idPopupOverlay').style.display = 'none';
        document.getElementById('idPopup').style.display = 'none';
    }

    // Fermer avec la touche Échap
    document.addEventListener('keydown', function(event) {
        if (event.key === 'Escape') {
            closeIdPopup();
        }
    });

    // Gestion de la navigation entre sections
    function showSection(event, sectionName) {
        event.preventDefault();
        
        // Cacher toutes les sections
        document.querySelectorAll('.section-content').forEach(section => {
            section.classList.remove('active');
        });

        // Retirer la classe active de tous les liens
        document.querySelectorAll('.sidebar-link').forEach(link => {
            link.classList.remove('active');
        });

        // Afficher la section sélectionnée
        const targetSection = document.getElementById(sectionName + '-section');
        if (targetSection) {
            targetSection.classList.add('active');
        }

        // Ajouter la classe active au lien cliqué
        event.currentTarget.classList.add('active');
    }

    // Gestion de la sélection du thème
    document.querySelectorAll('.theme-option').forEach(option => {
        option.addEventListener('click', function() {
            document.querySelectorAll('.theme-option').forEach(opt => {
                opt.classList.remove('selected');
            });
            this.classList.add('selected');
            this.querySelector('input[type="radio"]').checked = true;
        });
    });
</script>

</body>
</html>

<?php
$conn->close();
?>