<?php

include '/var/www/html/function.php';
include '/var/www/html/access_control.php';
error_reporting(E_ALL);
ini_set('display_errors', 1);

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) die("Erreur de connexion : " . $conn->connect_error);

// ========================================
// VÉRIFICATION UTILISATEUR
// ========================================
$user_id = 0;
$user_nom = "Invité";
$is_connected = false;

if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    $stmt = $conn->prepare("SELECT datecra, pc, navi, email FROM loginc WHERE idcokier=?");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        if ($row['pc'] === $_SERVER['REMOTE_ADDR'] && 
            $row['navi'] === $_SERVER['HTTP_USER_AGENT'] && 
            strtotime($row['datecra']) > strtotime('-1 hour')) {
            
            $stmt2 = $conn->prepare("SELECT id, nom FROM login WHERE email=?");
            $stmt2->bind_param("s", $row['email']);
            $stmt2->execute();
            $result2 = $stmt2->get_result();
            
            if ($result2->num_rows == 1) {
                $row2 = $result2->fetch_assoc();
                $user_id = (int)$row2['id'];
                $user_nom = $row2['nom'];
                $is_connected = true;
            }
            $stmt2->close();
        }
    }
    $stmt->close();
}

if (!$is_connected) { 
    migrant(); 
    exit; 
}

// ========================================
// ACTIONS POST
// ========================================
if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['action'])) {
    $action = $_POST['action'];
    // UPLOAD FICHIER
    if (isset($_POST['upload']) && isset($_FILES['screenshot'])) {
        $current_folder_upload = isset($_POST['current_folder']) ? intval($_POST['current_folder']) : 0;
        $file = $_FILES['screenshot'];
        $visibilite = isset($_POST['visble']) ? intval($_POST['visble']) : 1;
        
        // Traitement upload (à adapter selon votre logique existante)
        $upload_dir = '/var/www/html/uploads/';
        $file_name = uniqid() . '_' . basename($file['name']);
        $target = $upload_dir . $file_name;
        
        if (move_uploaded_file($file['tmp_name'], $target)) {
            $stmt = $conn->prepare("INSERT INTO fichiers (nom, type_fichier, id_utilisateur, date, visble) VALUES (?, ?, ?, NOW(), ?)");
            $stmt->bind_param("ssii", $file['name'], $target, $user_id, $visibilite);
            $stmt->execute();
            $new_file_id = $conn->insert_id;
            $stmt->close();
            
            // Lier au dossier actuel
            if ($current_folder_upload > 0) {
                $stmt = $conn->prepare("SELECT idpage FROM sitecdos WHERE iddosier = ?");
                $stmt->bind_param("i", $current_folder_upload);
                $stmt->execute();
                $result = $stmt->get_result();
                
                if ($row = $result->fetch_assoc()) {
                    $idpage_current = $row['idpage'];
                    $new_idpage = $idpage_current . ',fich:' . $new_file_id;
                    
                    $stmt2 = $conn->prepare("UPDATE sitecdos SET idpage = ? WHERE iddosier = ?");
                    $stmt2->bind_param("si", $new_idpage, $current_folder_upload);
                    $stmt2->execute();
                    $stmt2->close();
                }
                $stmt->close();
            }
            
            header("Location: " . $_SERVER['PHP_SELF'] . ($current_folder_upload > 0 ? "?dossier=" . $current_folder_upload : ""));
            exit;
        }
    }
    // CRÉER UN DOSSIER
    if ($action === 'create_folder' && isset($_POST['folder_name'])) {
        $folder_name = trim($_POST['folder_name']);
        $parent_id = isset($_POST['parent_id']) ? intval($_POST['parent_id']) : 0;
        
        // Construire idpage : "dos:X" si dans un sous-dossier
        $idpage = '';
        if ($parent_id > 0) {
            $idpage = 'dos:' . $parent_id;
        }
        
        $stmt = $conn->prepare("INSERT INTO sitecdos (doisernom, userid, idpage, addpageuserid) VALUES (?, ?, ?, ?)");
        $stmt->bind_param("siss", $folder_name, $user_id, $idpage, $user_id);
        $stmt->execute();
        $stmt->close();
        
        header("Location: " . $_SERVER['PHP_SELF'] . ($parent_id > 0 ? "?dossier=" . $parent_id : ""));
        exit;
    }
    
            // PARTAGER UN ÉLÉMENT
        if ($action === 'share' && isset($_POST['item_id']) && isset($_POST['share_data'])) {
            $item_id = intval($_POST['item_id']);
            $item_type = $_POST['item_type'];
            $share_data = $_POST['share_data']; // Format: userid:permissions,userid:permissions
            
            if ($item_type === 'folder') {
                $stmt = $conn->prepare("UPDATE sitecdos SET addpageuserid = ? WHERE iddosier = ? AND userid = ?");
                $stmt->bind_param("sii", $share_data, $item_id, $user_id);
                $stmt->execute();
                $stmt->close();
            } elseif ($item_type === 'file') {
                $stmt = $conn->prepare("UPDATE fichiers SET partage = ? WHERE id = ? AND id_utilisateur = ?");
                $stmt->bind_param("sii", $share_data, $item_id, $user_id);
                $stmt->execute();
                $stmt->close();
            } elseif ($item_type === 'page') {
                $stmt = $conn->prepare("UPDATE sitec SET addpageuserid = ? WHERE idpage = ? AND user_id = ?");
                $stmt->bind_param("sii", $share_data, $item_id, $user_id);
                $stmt->execute();
                $stmt->close();
            }
            
            header("Location: " . $_SERVER['PHP_SELF']);
            exit;
        }
    // MODIFIER LA VISIBILITÉ D'UNE PAGE
        if ($action === 'change_page_visibility' && isset($_POST['page_id']) && isset($_POST['new_visibility'])) {
            $page_id = intval($_POST['page_id']);
            $new_visibility = intval($_POST['new_visibility']);
            
            $stmt = $conn->prepare("UPDATE sitec SET prob = ? WHERE idpage = ? AND user_id = ?");
            $stmt->bind_param("iii", $new_visibility, $page_id, $user_id);
            $stmt->execute();
            $stmt->close();
            
            header("Location: " . $_SERVER['PHP_SELF']);
            exit;
        }
    // RENOMMER UN ÉLÉMENT
    if ($action === 'rename' && isset($_POST['item_id']) && isset($_POST['new_name'])) {
        $item_id = intval($_POST['item_id']);
        $item_type = $_POST['item_type'];
        $new_name = trim($_POST['new_name']);
        
        if ($item_type === 'folder') {
            $stmt = $conn->prepare("UPDATE sitecdos SET doisernom = ? WHERE iddosier = ? AND userid = ?");
            $stmt->bind_param("sii", $new_name, $item_id, $user_id);
        } elseif ($item_type === 'file') {
            $stmt = $conn->prepare("UPDATE fichiers SET nom = ? WHERE id = ? AND id_utilisateur = ?");
            $stmt->bind_param("sii", $new_name, $item_id, $user_id);
        } elseif ($item_type === 'page') {
            $stmt = $conn->prepare("UPDATE sitec SET nompage = ? WHERE idpage = ? AND user_id = ?");
            $stmt->bind_param("sii", $new_name, $item_id, $user_id);
        }
        
        if (isset($stmt)) {
            $stmt->execute();
            $stmt->close();
        }
        
        header("Location: " . $_SERVER['PHP_SELF']);
        exit;
    }
    // SUPPRIMER UN ÉLÉMENT
    if ($action === 'delete') {
        $item_id = intval($_POST['item_id']);
        $item_type = $_POST['item_type'];
        
        if ($item_type === 'file') {
            $stmt = $conn->prepare("DELETE FROM fichiers WHERE id = ? AND id_utilisateur = ?");
            $stmt->bind_param("ii", $item_id, $user_id);
        } elseif ($item_type === 'folder') {
            $stmt = $conn->prepare("DELETE FROM sitecdos WHERE iddosier = ? AND userid = ?");
            $stmt->bind_param("ii", $item_id, $user_id);
        } elseif ($item_type === 'page') {
            $stmt = $conn->prepare("DELETE FROM sitec WHERE idpage = ? AND user_id = ?");
            $stmt->bind_param("ii", $item_id, $user_id);
        }
        
        if (isset($stmt)) {
            $stmt->execute();
            $stmt->close();
        }
        
        $redirect = $_SERVER['PHP_SELF'];
        if (isset($_GET['dossier'])) {
            $redirect .= "?dossier=" . $_GET['dossier'];
        }
        header("Location: " . $redirect);
        exit;
    }
    
    // DÉPLACER UN ÉLÉMENT
    if ($action === 'move' && isset($_POST['item_id']) && isset($_POST['target_folder'])) {
        $item_id = intval($_POST['item_id']);
        $target_folder = intval($_POST['target_folder']);
        $item_type = $_POST['item_type'];
        
        if ($item_type === 'folder') {
            $idpage_val = $target_folder > 0 ? 'dos:' . $target_folder : '';
            $stmt = $conn->prepare("UPDATE sitecdos SET idpage = ? WHERE iddosier = ? AND userid = ?");
            $stmt->bind_param("sii", $idpage_val, $item_id, $user_id);
            $stmt->execute();
            $stmt->close();
        } elseif ($item_type === 'page') {
            $idpage_val = $target_folder > 0 ? strval($target_folder) : '';
            $stmt = $conn->prepare("UPDATE sitec SET idpage = ? WHERE idpage = ? AND user_id = ?");
            $stmt->bind_param("sii", $idpage_val, $item_id, $user_id);
            $stmt->execute();
            $stmt->close();
        }
        
        header("Location: " . $_SERVER['PHP_SELF']);
        exit;
    }
}
// ========================================
// MODE PARTAGÉ AVEC MOI
// ========================================
$shared_mode = isset($_GET['shared']) && $_GET['shared'] == 1;

if ($shared_mode) {
    $current_folder = 0;
    $folder_path = [];
}
// ========================================
// NAVIGATION
// ========================================
$current_folder = 0;
$folder_path = [];
$addpageauto = 1; // Par défaut, l'utilisateur a accès

// Fonction pour construire le chemin complet récursivement
function buildFolderPath($conn, $folder_id, $user_id) {
    $path = [];
    $current_id = $folder_id;
     
    while ($current_id > 0) {
        $stmt = $conn->prepare("SELECT iddosier, doisernom, idpage, userid, addpageuserid FROM sitecdos WHERE iddosier = ?");
        $stmt->bind_param("i", $current_id);
        $stmt->execute();
        $result = $stmt->get_result();
        
        if ($result->num_rows > 0) {
            $row = $result->fetch_assoc();
            
            // Vérifier les droits d'accès
            $folder_userid = intval($row['userid']);
            $addpageuserid_str = $row['addpageuserid'];
            $allowed_users = [];
            
            if (!empty($addpageuserid_str)) {
                $allowed_users = array_map('intval', explode(',', $addpageuserid_str));
            }
            
            if ($user_id != $folder_userid && !in_array($user_id, $allowed_users)) {
                $stmt->close();
                return null; // Pas d'accès
            }
            
            // Ajouter au chemin (en début car on remonte)
            array_unshift($path, [
                'id' => $row['iddosier'], 
                'nom' => $row['doisernom']
            ]);
            
            // Trouver le parent dans idpage (format: dos:X)
            $idpage_content = $row['idpage'];
            if (preg_match('/dos:(\d+)/', $idpage_content, $matches)) {
                $current_id = intval($matches[1]);
            } else {
                $current_id = 0; // Plus de parent
            }
        } else {
            $current_id = 0;
        }
        $stmt->close();
    }
    
    return $path;
}

if (isset($_GET['dossier'])) {
    $current_folder = intval($_GET['dossier']);
    
    if ($current_folder > 0) {
        $folder_path = buildFolderPath($conn, $current_folder, $user_id);
    }
}
// ========================================
// RÉCUPÉRATION DES DONNÉES
// ========================================
$mes_fichiers = [];
$mes_dossiers = [];
$pages_web = [];
$mes_fichiers = [];
// --- DOSSIERS ---
if ($current_folder == 0) {
    // Racine : afficher UNIQUEMENT les dossiers qui N'ONT PAS de parent (idpage vide ou ne contient pas "dos:")
    if ($shared_mode) {
        $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE userid != ? ORDER BY popluardose DESC");
        $stmt->bind_param("i", $user_id);
    } else {
        $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE userid = ? ORDER BY popluardose DESC");
        $stmt->bind_param("i", $user_id);
    }
    $stmt->execute();
    $result = $stmt->get_result();
    
    while ($folder = $result->fetch_assoc()) {
        if ($folder['iddosier'] == $current_folder) continue;
        
        // FILTRE PRINCIPAL : N'afficher que les dossiers qui ne sont PAS dans un autre dossier
        // Un dossier est à la racine si son idpage est vide OU ne contient pas "dos:"
        $idpage_value = trim($folder['idpage']);
        $is_root_folder = empty($idpage_value) || strpos($idpage_value, 'dos:') === false;
        
        if (!$is_root_folder) continue; // Sauter les sous-dossiers
        
        $folder_userid = intval($folder['userid']);
        $folder_addpageuserid = $folder['addpageuserid'];
        $allowed_users = [];
        
        if (!empty($folder_addpageuserid)) {
            $allowed_users = array_map('intval', explode(',', $folder_addpageuserid));
        }
        
        // En mode partagé, n'afficher que ce qui m'est partagé
        if ($shared_mode && !in_array($user_id, $allowed_users)) continue;
        
        if ($user_id == $folder_userid || in_array($user_id, $allowed_users)) {
            $mes_dossiers[] = $folder;
        }
    }
    $stmt->close();
    
} else {
    // Dans un dossier : afficher UNIQUEMENT les sous-dossiers DIRECTS
    if ($shared_mode) {
        $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE userid != ? ORDER BY popluardose DESC");
        $stmt->bind_param("i", $user_id);
    } else {
        $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE userid = ? ORDER BY popluardose DESC");
        $stmt->bind_param("i", $user_id);
    }
    
    $stmt->execute();
    $result = $stmt->get_result();
    
    while ($folder = $result->fetch_assoc()) {
        $idpage_value = trim($folder['idpage']);
        
        // Extraire le DERNIER "dos:X" dans idpage (c'est le parent direct)
        preg_match_all('/dos:(\d+)/', $idpage_value, $matches);
        
        if (empty($matches[1])) continue; // Pas de parent, on saute
        
        // Prendre le dernier ID extrait (parent direct)
        $direct_parent_id = intval(end($matches[1]));
        
        // Afficher uniquement si le parent direct est le dossier actuel
        if ($direct_parent_id !== $current_folder) continue;
        
        $folder_userid = intval($folder['userid']);
        $folder_addpageuserid = $folder['addpageuserid'];
        $allowed_users = [];
        
        if (!empty($folder_addpageuserid)) {
            $allowed_users = array_map('intval', explode(',', $folder_addpageuserid));
        }
        
        // En mode partagé, n'afficher que ce qui m'est partagé
        if ($shared_mode && !in_array($user_id, $allowed_users)) continue;
        
        if ($user_id == $folder_userid || in_array($user_id, $allowed_users)) {
            $mes_dossiers[] = $folder;
        }
    }
    $stmt->close();
}


// --- FICHIERS ---
if ($current_folder == 0) {
    // Racine : afficher tous les fichiers de l'utilisateur
    $stmt = $conn->prepare("SELECT * FROM fichiers WHERE id_utilisateur = ? ORDER BY date DESC LIMIT 100");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();
    while ($file = $result->fetch_assoc()) {
        $mes_fichiers[] = $file;
    }
    $stmt->close();
   
    
} else {
    // Dans un dossier : récupérer les fichiers liés via idpage
    $stmt = $conn->prepare("SELECT idpage FROM sitecdos WHERE iddosier = ?");
    $stmt->bind_param("i", $current_folder);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows > 0) {
        $row = $result->fetch_assoc();
        $idpage_content = $row['idpage'];
        
        // Extraire les IDs de fichiers (format: fich:34)
        preg_match_all('/fich:(\d+)/', $idpage_content, $matches);
        
        if (!empty($matches[1])) {
            $file_ids = array_map('intval', $matches[1]);
            $placeholders = implode(',', array_fill(0, count($file_ids), '?'));
            
            $stmt2 = $conn->prepare("SELECT * FROM fichiers WHERE id IN ($placeholders) AND id_utilisateur = ? ORDER BY date DESC");
            $types = str_repeat('i', count($file_ids)) . 'i';
            $params = array_merge($file_ids, [$user_id]);
            $stmt2->bind_param($types, ...$params);
            $stmt2->execute();
            $result2 = $stmt2->get_result();
            
            while ($file = $result2->fetch_assoc()) {
                $mes_fichiers[] = $file;
            }
            $stmt2->close();
        }
    }
    $stmt->close();
}

// --- PAGES WEB ---
if ($current_folder == 0) {
    // Racine : afficher toutes les pages de l'utilisateur
    $stmt = $conn->prepare("SELECT * FROM sitec WHERE user_id = ? ORDER BY nompage");
    $stmt->bind_param("i", $user_id);
} else {
    // Dans un dossier : afficher les pages référencées dans idpage du dossier
    $stmt_parent = $conn->prepare("SELECT idpage FROM sitecdos WHERE iddosier = ?");
    $stmt_parent->bind_param("i", $current_folder);
    $stmt_parent->execute();
    $result_parent = $stmt_parent->get_result();
    
    if ($result_parent->num_rows > 0) {
        $row_parent = $result_parent->fetch_assoc();
        $idpage_content = $row_parent['idpage'];
        
        // Extraire les IDs numériques simples (18,51,52) - ce sont les pages
        $page_ids = [];
        $parts = explode(',', $idpage_content);
        foreach ($parts as $part) {
            $part = trim($part);
            if (is_numeric($part)) {
                $page_ids[] = intval($part);
            }
        }
        
        if (!empty($page_ids)) {
            $placeholders = implode(',', array_fill(0, count($page_ids), '?'));
            $stmt = $conn->prepare("SELECT * FROM sitec WHERE idpage IN ($placeholders) AND user_id = ? ORDER BY nompage");
            $types = str_repeat('i', count($page_ids)) . 'i';
            $params = array_merge($page_ids, [$user_id]);
            $stmt->bind_param($types, ...$params);
        } else {
            $stmt = $conn->prepare("SELECT * FROM sitec WHERE idpage = -1");
        }
    } else {
        $stmt = $conn->prepare("SELECT * FROM sitec WHERE idpage = -1");
    }
    $stmt_parent->close();
}

$stmt->execute();
$result = $stmt->get_result();
while ($page = $result->fetch_assoc()) {
    $pages_web[] = $page;
}
$stmt->close();

// --- TOUS LES DOSSIERS (pour déplacement) ---
$all_folders = [];
$stmt = $conn->prepare("SELECT iddosier, doisernom, userid, addpageuserid FROM sitecdos ORDER BY doisernom");
$stmt->execute();
$result = $stmt->get_result();

while ($folder = $result->fetch_assoc()) {
    $folder_userid = intval($folder['userid']);
    $folder_addpageuserid = $folder['addpageuserid'];
    $allowed_users = [];
    
    if (!empty($folder_addpageuserid)) {
        $allowed_users = array_map('intval', explode(',', $folder_addpageuserid));
    }
    
    if ($user_id == $folder_userid || in_array($user_id, $allowed_users)) {
        $all_folders[] = $folder;
    }
}
$stmt->close();

// ========================================
// FONCTIONS HELPER
// ========================================
function getFileIcon($nomfichier) {
    $icon_map = [
        'jpg' => 'fa-file-image', 'jpeg' => 'fa-file-image', 'png' => 'fa-file-image',
        'gif' => 'fa-file-image', 'svg' => 'fa-file-image', 'webp' => 'fa-file-image',
        'tiff' => 'fa-file-image', 'tif' => 'fa-file-image', 'psd' => 'fa-file-image',
        'mp4' => 'fa-file-video', 'webm' => 'fa-file-video', 'avi' => 'fa-file-video',
        'mov' => 'fa-file-video', 'mkv' => 'fa-file-video',
        'pdf' => 'fa-file-pdf',
        'doc' => 'fa-file-word', 'docx' => 'fa-file-word',
        'xls' => 'fa-file-excel', 'xlsx' => 'fa-file-excel',
        'ppt' => 'fa-file-powerpoint', 'pptx' => 'fa-file-powerpoint',
        'txt' => 'fa-file-lines', 'csv' => 'fa-file-csv',
        'zip' => 'fa-file-zipper', 'gz' => 'fa-file-zipper', 'rar' => 'fa-file-zipper', '7z' => 'fa-file-zipper',
        'sql' => 'fa-database',
        'php' => 'fa-file-code', 'html' => 'fa-file-code', 'css' => 'fa-file-code',
        'js' => 'fa-file-code', 'json' => 'fa-file-code', 'xml' => 'fa-file-code',
        'exe' => 'fa-file', 'bat' => 'fa-file',
        'mtl' => 'fa-cube', 'obj' => 'fa-cube', 'fbx' => 'fa-cube', 'stl' => 'fa-cube', 'gcode' => 'fa-cube',
        'default' => 'fa-file'
    ];
    
    $ext = strtolower(pathinfo(basename($nomfichier), PATHINFO_EXTENSION));
    return $icon_map[$ext] ?? $icon_map['default'];
}

function getFileType($nomfichier) {
    $ext = strtolower(pathinfo(basename($nomfichier), PATHINFO_EXTENSION));
    
    $image_exts = ['jpg', 'jpeg', 'png', 'gif', 'svg', 'webp', 'tiff', 'tif', 'psd'];
    $video_exts = ['mp4', 'webm', 'avi', 'mov', 'mkv'];
    $doc_exts = ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'csv'];
    $code_exts = ['php', 'html', 'css', 'js', 'json', 'xml', 'sql'];
    
    if (in_array($ext, $image_exts)) return 'image';
    if (in_array($ext, $video_exts)) return 'video';
    if (in_array($ext, $doc_exts)) return 'document';
    if (in_array($ext, $code_exts)) return 'code';
    
    return 'file';
}

function formatDate($date) {
    $diff = time() - strtotime($date);
    if ($diff < 86400) return "Aujourd'hui";
    if ($diff < 172800) return "Hier";
    if ($diff < 604800) return ceil($diff / 86400) . " jours";
    return date('d/m/Y', strtotime($date));
}

displayNavigation($conn);

// Détection du thème utilisateur pour adapter la page
$isDarkTheme = false;
if ($user_id > 0 && function_exists('getUserPreferences')) {
    $prefs = getUserPreferences($user_id, $conn);
    $isDarkTheme = ($prefs && isset($prefs['teme']) && $prefs['teme'] === 1);
}
?>
<script>
<?php if ($isDarkTheme): ?>
document.body.classList.add('dark-theme');
<?php endif; ?>
</script>
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Gestionnaire - Exodrive</title>
    <link rel="icon" href="/vex.png" type="image/png">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>
/* Reset simple */
* { 
    margin: 0; 
    padding: 0; 
    box-sizing: border-box; 
}

/* Variables adaptatives au thème */
:root {
    --exo-primary: #4CAF50;
    --exo-primary-dark: #45a049;
    
    /* Thème clair par défaut */
    --nav-bg: #4CAF50;
    --nav-gradient: linear-gradient(135deg, #66BB6A 0%, #4CAF50 50%, #43A047 100%);
    --exo-bg: #f0f2f5;
    --exo-card-bg: #ffffff;
    --exo-text: #1c1e21;
    --exo-text-secondary: #65676b;
    --exo-border: #e4e6eb;
    --exo-hover: #f0f2f5;
    --exo-sidebar-bg: #ffffff;
    --exo-input-bg: #ffffff;
}

/* Mode sombre automatique */
body.dark-theme {
    --nav-bg: #1a1a1a;
    --nav-gradient: linear-gradient(135deg, #2d5f2e 0%, #1a3a1c 50%, #0a1f0b 100%);
    --exo-bg: #121212;
    --exo-card-bg: #1e1e1e;
    --exo-text: #e0e0e0;
    --exo-text-secondary: #b0b0b0;
    --exo-border: #2a2a2a;
    --exo-hover: #2a2a2a;
    --exo-sidebar-bg: #1a1a1a;
    --exo-input-bg: #2a2a2a;
}

/* Force la navigation verte avec dégradé adaptatif */
.top-nav-bar-haut7844 {
    background: var(--nav-gradient) !important;
}

body.dark-theme .top-nav-bar-haut7844 {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.8) !important;
}

body { 
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
    background: var(--exo-bg); 
    color: var(--exo-text); 
    -webkit-font-smoothing: antialiased;
    transition: background-color 0.3s ease, color 0.3s ease;
}

.existingShares {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 8px;
    font-size: 13px;
    color: var(--exo-bg);
}
.main-container { 
    display: flex; 
    height: 100vh; 
    padding-top: 60px; 
}

.sidebar { 
    width: 260px; 
    background: var(--exo-sidebar-bg); 
    border-right: 1px solid var(--exo-border); 
    padding: 20px 10px; 
    overflow-y: auto;
    transition: background-color 0.3s ease;
}

.create-btn { 
    width: 100%; 
    padding: 12px 20px; 
    background: var(--exo-primary); 
    color: #fff; 
    border: none; 
    border-radius: 5px; 
    font-size: 14px; 
    font-weight: 600; 
    cursor: pointer; 
    display: flex; 
    align-items: center; 
    justify-content: center; 
    gap: 10px; 
    margin-bottom: 24px; 
    transition: all .3s; 
}

.create-btn:hover { 
    background: var(--exo-primary-dark); 
}

.nav-item { 
    display: flex; 
    align-items: center; 
    gap: 12px; 
    padding: 12px 16px; 
    color: var(--exo-text); 
    text-decoration: none; 
    border-radius: 8px; 
    margin: 4px 0; 
    transition: all .2s; 
    cursor: pointer; 
    font-size: 14px; 
    font-weight: 500; 
}

.nav-item:hover { 
    background: var(--exo-hover); 
}

.nav-item.active { 
    background: var(--exo-primary); 
    color: #fff; 
    font-weight: 600; 
}

.nav-item i { 
    width: 20px; 
    font-size: 18px; 
    text-align: center; 
    color: var(--exo-text-secondary); 
}

.nav-item.active i { 
    color: #fff; 
}

.content { 
    flex: 1; 
    overflow-y: auto; 
    padding: 24px 32px; 
    background: var(--exo-bg);
    transition: background-color 0.3s ease;
}

.content-header { 
    display: flex; 
    justify-content: space-between; 
    align-items: center; 
    margin-bottom: 20px; 
}

.breadcrumb { 
    display: flex; 
    align-items: center; 
    gap: 8px; 
    margin-bottom: 10px; 
    font-size: 13px; 
    color: var(--exo-text-secondary); 
}

.breadcrumb a { 
    color: var(--exo-primary); 
    text-decoration: none; 
}

.breadcrumb a:hover { 
    text-decoration: underline; 
}

.content-title { 
    font-size: 28px; 
    font-weight: 600; 
    color: var(--exo-text); 
}

.view-toggle { 
    display: flex; 
    gap: 0; 
    background: var(--exo-card-bg); 
    border-radius: 5px; 
    border: 1px solid var(--exo-border); 
    overflow: hidden; 
}

.view-btn { 
    padding: 8px 14px; 
    background: transparent; 
    border: none; 
    color: var(--exo-text-secondary); 
    cursor: pointer; 
    transition: all .2s; 
    border-right: 1px solid var(--exo-border); 
}

.view-btn:last-child { 
    border-right: none; 
}

.view-btn:hover { 
    background: var(--exo-hover); 
}

.view-btn.active { 
    background: var(--exo-primary); 
    color: #fff; 
}

.files-grid { 
    display: grid; 
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); 
    gap: 16px; 
    margin-bottom: 40px; 
}

.files-grid.hidden { 
    display: none; 
}

.file-card, .folder-card, .page-card { 
    background: var(--exo-card-bg); 
    border: 1px solid var(--exo-border); 
    border-radius: 8px; 
    padding: 20px 16px; 
    text-align: center; 
    cursor: pointer; 
    transition: all .2s; 
    text-decoration: none; 
    color: inherit; 
    display: block; 
    position: relative; 
}

.file-card:hover, .folder-card:hover, .page-card:hover { 
    border-color: var(--exo-primary); 
    box-shadow: 0 2px 8px rgba(76, 175, 80, .2); 
    transform: translateY(-2px); 
}

body.dark-theme .file-card:hover, 
body.dark-theme .folder-card:hover, 
body.dark-theme .page-card:hover { 
    box-shadow: 0 2px 8px rgba(76, 175, 80, .4); 
}

.item-menu { 
    position: absolute; 
    top: 10px; 
    right: 10px; 
    cursor: pointer; 
    padding: 5px 8px; 
    border-radius: 3px; 
    transition: background .2s; 
    z-index: 10; 
    color: var(--exo-text-secondary); 
}

.item-menu:hover { 
    background: var(--exo-hover); 
}

.context-menu { 
    display: none; 
    position: fixed; 
    background: var(--exo-card-bg); 
    border: 1px solid var(--exo-border); 
    border-radius: 8px; 
    box-shadow: 0 4px 12px rgba(0, 0, 0, .15); 
    z-index: 1000; 
    min-width: 200px; 
    padding: 8px 0; 
}

body.dark-theme .context-menu { 
    box-shadow: 0 4px 12px rgba(0, 0, 0, .5); 
}

.context-menu.show { 
    display: block; 
}

.context-menu-item { 
    padding: 10px 16px; 
    cursor: pointer; 
    transition: background .2s; 
    display: flex; 
    align-items: center; 
    gap: 10px; 
    font-size: 14px; 
    color: var(--exo-text); 
}

.context-menu-item:hover { 
    background: var(--exo-hover); 
}

.context-menu-item.delete { 
    color: #f44336; 
}

.context-menu-item i { 
    width: 18px; 
    text-align: center; 
    font-size: 16px; 
    color: var(--exo-text-secondary); 
}

.context-menu-item.delete i { 
    color: #f44336; 
}

.file-icon, .folder-icon, .page-icon { 
    font-size: 48px; 
    margin-bottom: 12px; 
    display: flex; 
    align-items: center; 
    justify-content: center; 
    min-height: 48px; 
    color: var(--exo-text-secondary); 
}

.folder-icon i { 
    color: #FFA726; 
}

.page-icon i { 
    color: #42A5F5; 
}

.file-name, .folder-name, .page-name { 
    font-weight: 500; 
    margin-bottom: 6px; 
    overflow: hidden; 
    text-overflow: ellipsis; 
    white-space: nowrap; 
    color: var(--exo-text); 
    font-size: 13px; 
}

.file-date { 
    font-size: 11px; 
    color: var(--exo-text-secondary); 
}

.files-list { 
    background: var(--exo-card-bg); 
    border: 1px solid var(--exo-border); 
    border-radius: 8px; 
    overflow: hidden; 
    display: none; 
}

.files-list.active { 
    display: block; 
}

.list-header { 
    display: grid; 
    grid-template-columns: 2fr 1fr 1fr 60px; 
    gap: 20px; 
    padding: 12px 20px; 
    background: var(--exo-hover); 
    border-bottom: 1px solid var(--exo-border); 
    font-weight: 600; 
    font-size: 11px; 
    color: var(--exo-text-secondary); 
    text-transform: uppercase; 
}

.list-item { 
    display: grid; 
    grid-template-columns: 2fr 1fr 1fr 60px; 
    gap: 20px; 
    padding: 14px 20px; 
    border-bottom: 1px solid var(--exo-border); 
    transition: background .2s; 
    cursor: pointer; 
    text-decoration: none; 
    color: inherit; 
    position: relative; 
}

.list-item:hover { 
    background: var(--exo-hover); 
}

.list-item-name { 
    display: flex; 
    align-items: center; 
    gap: 12px; 
    font-weight: 500; 
    font-size: 14px; 
    color: var(--exo-text); 
}

.list-item-name i { 
    font-size: 18px; 
    min-width: 20px; 
    text-align: center; 
    color: var(--exo-text-secondary); 
}

.empty-state { 
    text-align: center; 
    padding: 60px 20px; 
    background: var(--exo-card-bg); 
    border-radius: 8px; 
    border: 1px solid var(--exo-border); 
}

.empty-state i { 
    font-size: 64px; 
    color: var(--exo-text-secondary); 
    opacity: 0.5;
    margin-bottom: 20px; 
}

.empty-state h3 {
    color: var(--exo-text);
}

.empty-state p {
    color: var(--exo-text-secondary);
}

.modal { 
    display: none; 
    position: fixed; 
    top: 0; 
    left: 0; 
    width: 100%; 
    height: 100%; 
    background: rgba(0, 0, 0, .5); 
    z-index: 2000; 
    align-items: center; 
    justify-content: center; 
}

body.dark-theme .modal { 
    background: rgba(0, 0, 0, .7); 
}

.modal.active { 
    display: flex; 
}

.modal-content { 
    background: var(--exo-card-bg); 
    border-radius: 12px; 
    padding: 28px; 
    width: 90%; 
    max-width: 460px; 
    box-shadow: 0 8px 32px rgba(0, 0, 0, .3); 
}

body.dark-theme .modal-content { 
    box-shadow: 0 8px 32px rgba(0, 0, 0, .6); 
}

.modal-header { 
    font-size: 20px; 
    font-weight: 600; 
    margin-bottom: 20px; 
    color: var(--exo-text); 
}

.form-group { 
    margin-bottom: 18px; 
}

.form-group label { 
    display: block; 
    margin-bottom: 6px; 
    font-weight: 500; 
    color: var(--exo-text); 
    font-size: 13px; 
}

.form-group input, .form-group select { 
    width: 100%; 
    padding: 10px 12px; 
    border: 1px solid var(--exo-border); 
    border-radius: 6px; 
    font-size: 14px; 
    background: var(--exo-input-bg); 
    color: var(--exo-text); 
    transition: border-color 0.2s;
}

.form-group input:focus, .form-group select:focus { 
    outline: none; 
    border-color: var(--exo-primary); 
}

.modal-footer { 
    display: flex; 
    justify-content: flex-end; 
    gap: 10px; 
    margin-top: 20px; 
}

.modal-body { 
    margin-bottom: 20px;
    color: var(--exo-text);
}

.btn { 
    padding: 10px 20px; 
    border: none; 
    border-radius: 5px; 
    font-size: 14px; 
    font-weight: 600; 
    cursor: pointer; 
    transition: all .3s; 
}

.btn-primary { 
    background: var(--exo-primary); 
    color: #fff; 
}

.btn-primary:hover { 
    background: var(--exo-primary-dark); 
}

.btn-secondary { 
    background: transparent; 
    color: var(--exo-text-secondary); 
    border: 1px solid var(--exo-border); 
}

.btn-secondary:hover { 
    background: var(--exo-hover); 
}

.btn-action { 
    width: 100%; 
    margin-bottom: 10px; 
    display: flex; 
    align-items: center; 
    justify-content: flex-start; 
    gap: 10px; 
    padding: 12px 16px; 
    text-align: left; 
}

.btn-action i { 
    font-size: 16px; 
    color: var(--exo-text-secondary); 
    width: 20px; 
}

.filter-select, .search-box { 
    padding: 8px 12px; 
    border: 1px solid var(--exo-border); 
    border-radius: 5px; 
    font-size: 14px; 
    background: var(--exo-card-bg); 
    color: var(--exo-text); 
}

.filter-select { 
    min-width: 150px; 
}

.search-box { 
    min-width: 200px; 
}

.filter-select:focus, .search-box:focus { 
    outline: none; 
    border-color: var(--exo-primary); 
}

.share-indicator {
    position: absolute;
    top: 10px;
    left: 10px;
    background: rgba(33, 150, 243, 0.9);
    color: white;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 4px;
    z-index: 5;
}

.file-input-wrapper input[type="file"] {
    display: none;
}

.file-input-label {
    border: 2px dashed var(--exo-primary);
    padding: 20px;
    text-align: center;
    cursor: pointer;
    border-radius: 8px;
    background: var(--exo-hover);
    transition: all 0.3s;
}

.file-input-label:hover {
    background: var(--exo-border);
}

.visibility-select {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--exo-border);
    border-radius: 6px;
    font-size: 14px;
    background: var(--exo-input-bg);
    color: var(--exo-text);
}

@media(max-width: 768px) { 
    .sidebar { 
        width: 70px; 
    } 
    .create-btn span, .nav-item span { 
        display: none; 
    } 
}
    </style>
</head>
<body>
    <div class="main-container">
        <aside class="sidebar">
            <button class="create-btn" onclick="openCreateModal()">
                <i class="fas fa-plus"></i><span>Nouveau</span>
            </button>
            <nav>
                <a href="?" class="nav-item <?php echo (!isset($_GET['shared']) && $current_folder == 0) ? 'active' : ''; ?>">
                    <i class="fas fa-folder"></i><span>Mes fichiers</span>
                </a>
                <a href="?shared=1" class="nav-item <?php echo (isset($_GET['shared'])) ? 'active' : ''; ?>">
                    <i class="fas fa-share-alt"></i><span>Partagé avec moi</span>
                </a>
            </nav>
        </aside>

        <main class="content">
            <div class="content-header">
                <div>
                   <div class="breadcrumb">
                        <a href="?"><i class="fas fa-home"></i> Accueil</a>
                        <?php if ($current_folder > 0 && !empty($folder_path)): ?>
                            <?php foreach ($folder_path as $index => $fp): ?>
                            <span>/</span>
                            <a href="?dossier=<?php echo $fp['id']; ?>"><?php echo htmlspecialchars($fp['nom']); ?></a>
                            <?php endforeach; ?>
                        <?php endif; ?>
                    </div>
                    <h1 class="content-title">
                        <?php echo $current_folder > 0 ? htmlspecialchars($folder_path[0]['nom']) : "Mes fichiers"; ?>
                    </h1>
                </div>
                <div style="display:flex;gap:10px;align-items:center">
                    <select id="filterType" class="filter-select" onchange="filterFiles()">
                        <option value="all">Tous les types</option>
                        <option value="folder">Dossiers</option>
                        <option value="page">Pages web</option>
                        <option value="file">Fichiers</option>
                        <option value="image">Images</option>
                        <option value="video">Vidéos</option>
                        <option value="document">Documents</option>
                        <option value="code">Code</option>
                    </select>
                    <input type="text" id="searchBox" class="search-box" placeholder="Rechercher..." oninput="searchFiles()">
                    <div class="view-toggle">
                        <button class="view-btn active" data-view="grid"><i class="fas fa-th"></i></button>
                        <button class="view-btn" data-view="list"><i class="fas fa-list"></i></button>
                    </div>
                </div>
            </div>

            <div class="files-grid">
                <?php
                $total = count($mes_dossiers) + count($mes_fichiers) + count($pages_web);
                if ($total > 0):
                    // Afficher les dossiers
                    foreach ($mes_dossiers as $f):
                ?>
                    <div class="folder-card" 
                        data-type="folder" 
                        data-name="<?php echo htmlspecialchars($f['doisernom']); ?>"
                         <?php 
                        $current_path = isset($_GET['path']) ? $_GET['path'] : '';
                        $new_path = $current_path ? $current_path . ',' . $f['iddosier'] : $f['iddosier'];
                        ?>
                        onclick="window.location.href='?dossier=<?php echo $f['iddosier']; ?>&path=<?php echo $new_path; ?>'">
                        <div class="item-menu" onclick="event.stopPropagation();showContextMenu(event,'folder',<?php echo $f['iddosier']; ?>)">
                            <i class="fas fa-ellipsis-v"></i>
                        </div>
                        <?php
                            $folder_addpageuserid = $f['addpageuserid'];
                            if (!empty($folder_addpageuserid) && $folder_addpageuserid != $user_id):
                            ?>
                                <div style="position:absolute;top:10px;left:10px;background:rgba(33,150,243,0.9);color:white;padding:4px 8px;border-radius:4px;font-size:11px;display:flex;align-items:center;gap:4px;">
                                    <i class="fas fa-share-alt"></i>
                                </div>
                            <?php endif; ?>
                        <div class="folder-icon"><i class="fas fa-folder"></i></div>
                        <div class="folder-name"><?php echo htmlspecialchars($f['doisernom']); ?></div>
                        <div class="file-date">Dossier</div>
                    </div>
                <?php
                    endforeach;
                    
                    // Afficher les pages web
                    foreach ($pages_web as $p):
                ?>
                    <a href="/sitec/pages/<?php echo $p['urlpage']; ?>.php" 
                       target="_blank" 
                       class="page-card"
                       data-type="page"
                       data-name="<?php echo htmlspecialchars($p['nompage']); ?>">
                        <div class="item-menu" onclick="event.stopPropagation();event.preventDefault();showContextMenu(event,'page',<?php echo $p['idpage']; ?>)">
                            <i class="fas fa-ellipsis-v"></i>
                        </div>
                         <?php
                            $pageporb = $p['porb'];
                            if (!empty($pageporb) && $pageporb == '1'):
                            ?>
                                <div style="position:absolute;top:10px;left:10px;background:rgba(33,150,243,0.9);color:white;padding:4px 8px;border-radius:4px;font-size:11px;display:flex;align-items:center;gap:4px;">
                                    <i class="fas fa-share-alt"></i>
                                </div>
                            <?php endif; ?>
                       
                        <div class="page-icon"><i class="fas fa-globe"></i></div>
                        <div class="page-name"><?php echo htmlspecialchars($p['nompage']); ?></div>
                        <div class="file-date">Page web</div>
                    </a>
                <?php
                    endforeach;
                    
                    // Afficher les fichiers
                    foreach ($mes_fichiers as $file):
                        $fileIcon = getFileIcon($file['nom']);
                        $fileType = getFileType($file['nom']);
                ?>
                    <a href="/tel/tel.php?id=<?php echo $file['id']; ?>" 
                       target="_blank" 
                       class="file-card"
                       data-type="<?php echo $fileType; ?>"
                       data-name="<?php echo htmlspecialchars($file['nom']); ?>">
                        <div class="item-menu" onclick="event.stopPropagation();event.preventDefault();showContextMenu(event,'file',<?php echo $file['id']; ?>)">
                            <i class="fas fa-ellipsis-v"></i>
                        </div>
                           <?php
                            $filepartage = $file['partage'];
                            if (!empty($filepartage) && $filepartage != null):
                            ?>
                                <div style="position:absolute;top:10px;left:10px;background:rgba(33,150,243,0.9);color:white;padding:4px 8px;border-radius:4px;font-size:11px;display:flex;align-items:center;gap:4px;">
                                    <i class="fas fa-share-alt"></i>
                                </div>
                            <?php endif; ?>
                        <div class="file-icon"><i class="fa-solid <?php echo $fileIcon; ?>"></i></div>
                        <div class="file-name"><?php echo htmlspecialchars($file['nom']); ?></div>
                        <div class="file-date"><?php echo formatDate($file['date']); ?></div>
                    </a>
                <?php
                    endforeach;
                else:
                ?>
                    <div class="empty-state" style="grid-column:1/-1">
                        <i class="fas fa-folder-open"></i>
                        <h3>Aucun fichier</h3>
                        <p>Créez votre premier document</p>
                    </div>
                <?php endif; ?>
            </div>

            <div class="files-list">
                <div class="list-header">
                    <span>Nom</span>
                    <span>Type</span>
                    <span>Date</span>
                    <span></span>
                </div>
                <?php foreach ($mes_dossiers as $f): ?>
                <div class="list-item" 
                     data-type="folder"
                     data-name="<?php echo htmlspecialchars($f['doisernom']); ?>"
                     onclick="window.location.href='?dossier=<?php echo $f['iddosier']; ?>'">
                    <div class="list-item-name">
                        <i class="fas fa-folder"></i>
                        <span><?php echo htmlspecialchars($f['doisernom']); ?></span>
                    </div>
                    <div>Dossier</div>
                    <div>-</div>
                    <div class="item-menu" onclick="event.stopPropagation();showContextMenu(event,'folder',<?php echo $f['iddosier']; ?>)">
                        <i class="fas fa-ellipsis-v"></i>
                    </div>
                </div>
                <?php endforeach; ?>
                
                <?php foreach ($pages_web as $p): ?>
                <a href="/sitec/pages/<?php echo $p['urlpage']; ?>.php" 
                   target="_blank" 
                   class="list-item"
                   data-type="page"
                   data-name="<?php echo htmlspecialchars($p['nompage']); ?>">
                    <div class="list-item-name">
                        <i class="fas fa-globe"></i>
                        <span><?php echo htmlspecialchars($p['nompage']); ?></span>
                    </div>
                    <div>Page web</div>
                    <div>-</div>
                    <div class="item-menu" onclick="event.stopPropagation();event.preventDefault();showContextMenu(event,'page',<?php echo $p['idpage']; ?>)">
                        <i class="fas fa-ellipsis-v"></i>
                    </div>
                </a>
                <?php endforeach; ?>
                
                <?php foreach ($mes_fichiers as $file): 
                    $fileIcon = getFileIcon($file['nom']);
                    $fileType = getFileType($file['nom']);
                ?>
                <a href="/tel/tel.php?id=<?php echo $file['id']; ?>" 
                   target="_blank" 
                   class="list-item"
                   data-type="<?php echo $fileType; ?>"
                   data-name="<?php echo htmlspecialchars($file['nom']); ?>">
                    <div class="list-item-name">
                        <i class="fa-solid <?php echo $fileIcon; ?>"></i>
                        <span><?php echo htmlspecialchars($file['nom']); ?></span>
                    </div>
                    <div><?php echo strtoupper(pathinfo($file['type_fichier'], PATHINFO_EXTENSION)); ?></div>
                    <div><?php echo formatDate($file['date']); ?></div>
                    <div class="item-menu" onclick="event.stopPropagation();event.preventDefault();showContextMenu(event,'file',<?php echo $file['id']; ?>)">
                        <i class="fas fa-ellipsis-v"></i>
                    </div>
                </a>
                <?php endforeach; ?>
            </div>
        </main>
    </div>
    <!-- Modal Upload Fichier -->
        <div class="modal" id="uploadModal">
            <div class="modal-content">
                <div class="modal-header">Importer un fichier</div>
                <form method="POST" enctype="multipart/form-data" id="uploadForm">
                    <input type="hidden" name="upload" value="1">
                    <input type="hidden" name="current_folder" value="<?php echo $current_folder; ?>">
                    
                    <div class="form-group">
                        <label>Sélectionner un fichier</label>
                        <div class="file-input-wrapper">
                            <input type="file" id="fileUploadInput" name="screenshot" required>
                            <label for="fileUploadInput" class="file-input-label" style="border:2px dashed #4CAF50;padding:20px;text-align:center;cursor:pointer;">
                                <i class="fas fa-cloud-upload-alt" style="font-size:48px;color:#4CAF50;"></i>
                                <p id="fileUploadText">Cliquez ou glissez un fichier ici</p>
                            </label>
                        </div>
                    </div>
                    
                    <div class="form-group">
                        <label>Visibilité</label>
                        <select name="visble" class="visibility-select">
                            <option value="1">Privé</option>
                            <option value="0">Public</option>
                            <option value="3">Lien unique</option>
                        </select>
                    </div>
                    
                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" onclick="closeModal('uploadModal')">Annuler</button>
                        <button type="submit" class="btn btn-primary">
                            <i class="fas fa-upload"></i> Téléverser
                        </button>
                    </div>
                </form>
            </div>
        </div>                
    <!-- Menu contextuel -->
    <div class="context-menu" id="contextMenu">
        <div class="context-menu-item" onclick="handleAction('open')" id="menu-open">
            <i class="fas fa-folder-open"></i>Ouvrir
        </div>
        <div class="context-menu-item" onclick="handleAction('edit')" id="menu-edit" style="display:none;">
            <i class="fas fa-edit"></i>Éditer
        </div>
        <div class="context-menu-item" onclick="handleAction('rename')">
            <i class="fas fa-i-cursor"></i>Renommer
        </div>
        <div class="context-menu-item" onclick="handleAction('visibility')" id="menu-visibility" style="display:none;">
        <i class="fas fa-eye"></i>Visibilité
       </div>
        <div class="context-menu-item" onclick="handleAction('move')">
            <i class="fas fa-arrows-alt"></i>Déplacer
        </div>
        <div class="context-menu-item" onclick="handleAction('share')">
            <i class="fas fa-share"></i>Partager
        </div>
        <div class="context-menu-item delete" onclick="handleAction('delete')">
            <i class="fas fa-trash"></i>Supprimer
        </div>
    </div>

    <!-- Modal Créer -->
    <div class="modal" id="createModal">
        <div class="modal-content">
            <div class="modal-header">Créer</div>
            <div class="modal-body">
                <button type="button" class="btn btn-primary btn-action" onclick="showFolderModal()">
                    <i class="fas fa-folder"></i>Nouveau dossier
                </button>
                <button type="button" class="btn btn-primary btn-action" onclick="window.location.href='/sitec/create_page.php<?php echo $current_folder > 0 ? '?dossier=' . $current_folder : ''; ?>'">
                    <i class="fas fa-globe"></i>Créer une page web
                </button>
                <button type="button" class="btn btn-primary btn-action" onclick="openUploadPopup()">
                    <i class="fas fa-upload"></i>Importer un fichier
                </button>
            </div>
            <div class="modal-footer">
                <button type="button" class="btn btn-secondary" onclick="closeModal('createModal')">Fermer</button>
            </div>
        </div>
    </div>

    <!-- Modal Nouveau dossier -->
    <div class="modal" id="folderModal">
        <div class="modal-content">
            <div class="modal-header">Nouveau dossier</div>
            <form method="POST">
                <input type="hidden" name="action" value="create_folder">
                <input type="hidden" name="parent_id" value="<?php echo $current_folder; ?>">
                <div class="form-group">
                    <label>Nom du dossier</label>
                    <input type="text" name="folder_name" placeholder="Mon dossier" required>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" onclick="closeModal('folderModal')">Annuler</button>
                    <button type="submit" class="btn btn-primary">Créer</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Modal Déplacer -->
    <div class="modal" id="moveModal">
        <div class="modal-content">
            <div class="modal-header">Déplacer</div>
            <form method="POST">
                <input type="hidden" name="action" value="move">
                <input type="hidden" name="item_id" id="moveItemId">
                <input type="hidden" name="item_type" id="moveItemType">
                <div class="form-group">
                    <label>Dossier de destination</label>
                    <select name="target_folder" required>
                        <option value="0">Racine</option>
                        <?php foreach ($all_folders as $f): ?>
                        <option value="<?php echo $f['iddosier']; ?>">
                            <?php echo htmlspecialchars($f['doisernom']); ?>
                        </option>
                        <?php endforeach; ?>
                    </select>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" onclick="closeModal('moveModal')">Annuler</button>
                    <button type="submit" class="btn btn-primary">Déplacer</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Modal Supprimer -->
    <div class="modal" id="deleteModal">
        <div class="modal-content">
            <div class="modal-header">Confirmer la suppression</div>
            <form method="POST">
                <input type="hidden" name="action" value="delete">
                <input type="hidden" name="item_id" id="deleteItemId">
                <input type="hidden" name="item_type" id="deleteItemType">
                <div class="modal-body">
                    <p>Êtes-vous sûr de vouloir supprimer cet élément ?</p>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" onclick="closeModal('deleteModal')">Annuler</button>
                    <button type="submit" class="btn btn-primary" style="background:#f44336">Supprimer</button>
                </div>
            </form>
               </div>
                </div>

                <!-- Modal Partager -->
                <div class="modal" id="shareModal">
                    <div class="modal-content" style="max-width:700px;">
                        <div class="modal-header">Partager <span id="shareItemName" style="color:#4CAF50;"></span></div>
                        <form method="POST" id="shareForm">
                            <input type="hidden" name="action" value="share">
                            <input type="hidden" name="item_id" id="shareItemId">
                            <input type="hidden" name="item_type" id="shareItemType">
                            <input type="hidden" name="share_data" id="shareDataHidden">
                            
                            <!-- Liste des partages existants -->
                            <div id="existingShares" style="margin-bottom:20px;border:1px solid #e4e6eb;border-radius:8px;padding:15px;">
                                <h4 style="margin-bottom:10px;color:#1c1e21;display:flex;align-items:center;gap:8px;">
                                    <i class="fas fa-users"></i> Partagé avec :
                                </h4>
                                <div id="sharesList" style="max-height:250px;overflow-y:auto;"></div>
                            </div>
                            
                            <div class="form-group">
                                <label style="display:flex;align-items:center;gap:8px;">
                                    <i class="fas fa-user-plus"></i> Ajouter un utilisateur
                                </label>
                                <input type="text" id="shareUsersInput" placeholder="Tapez un pseudo..." autocomplete="off" style="padding:12px;border:1px solid #e4e6eb;border-radius:6px;width:100%;">
                                <div id="userSuggestions" style="display:none;position:relative;background:white;border:1px solid #e4e6eb;border-radius:8px;max-height:200px;overflow-y:auto;width:100%;margin-top:5px;z-index:1001;box-shadow:0 4px 12px rgba(0,0,0,0.1);"></div>
                            </div>
                            
                           <!-- Sélecteur global pour appliquer à tous -->
                            
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" onclick="closeModal('shareModal')">Annuler</button>
                                <button type="submit" class="btn btn-primary">
                                    <i class="fas fa-check"></i> Enregistrer
                                </button>
                            </div>
                        </form>
                    </div>
                </div>

                <!-- Modal Renommer -->
                <div class="modal" id="renameModal">
                    <div class="modal-content">
                        <div class="modal-header">Renommer</div>
                        <form method="POST">
                            <input type="hidden" name="action" value="rename">
                            <input type="hidden" name="item_id" id="renameItemId">
                            <input type="hidden" name="item_type" id="renameItemType">
                            <div class="form-group">
                                <label>Nouveau nom</label>
                                <input type="text" name="new_name" id="renameInput" required>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" onclick="closeModal('renameModal')">Annuler</button>
                                <button type="submit" class="btn btn-primary">Renommer</button>
                            </div>
                        </form>
                    </div>
                </div>
                <!-- Modal Visibilité Page -->
                <div class="modal" id="visibilityModal">
                    <div class="modal-content">
                        <div class="modal-header">Modifier la visibilité</div>
                        <form method="POST">
                            <input type="hidden" name="action" value="change_page_visibility">
                            <input type="hidden" name="page_id" id="visibilityPageId">
                            <div class="form-group">
                                <label>Visibilité de la page</label>
                                <select name="new_visibility" required style="width:100%;padding:10px;border:1px solid #e4e6eb;border-radius:6px;">
                                    <option value="1">🔒 Privé (seulement moi)</option>
                                    <option value="0">🌐 Public (visible par tous)</option>
                                </select>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" onclick="closeModal('visibilityModal')">Annuler</button>
                                <button type="submit" class="btn btn-primary">Enregistrer</button>
                            </div>
                        </form>
                    </div>
                </div>            
               <script>
let currentItemId = null;
let currentItemType = null;
let currentItemName = '';
let selectedUsers = [];

// Toggle vue grille/liste
document.querySelectorAll('.view-btn').forEach(btn => {
    btn.addEventListener('click', function() {
        document.querySelectorAll('.view-btn').forEach(b => b.classList.remove('active'));
        this.classList.add('active');
        
        const grid = document.querySelector('.files-grid');
        const list = document.querySelector('.files-list');
        
        if (this.dataset.view === 'list') {
            grid.classList.add('hidden');
            list.classList.add('active');
        } else {
            grid.classList.remove('hidden');
            list.classList.remove('active');
        }
    });
});

// Modales
function openCreateModal() {
    document.getElementById('createModal').classList.add('active');
}

function closeModal(id) {
    document.getElementById(id).classList.remove('active');
}

function showFolderModal() {
    closeModal('createModal');
    document.getElementById('folderModal').classList.add('active');
}

// Menu contextuel
function showContextMenu(event, type, id) {
    event.stopPropagation();

    currentItemId = id;
    currentItemType = type;

    const item = event.target.closest('[data-name]');
    currentItemName = item ? item.getAttribute('data-name') : '';

    const menuEdit = document.getElementById('menu-edit');
    const menuOpen = document.getElementById('menu-open');
    const menuShare = document.querySelector('.context-menu-item[onclick*="share"]');

    menuEdit.style.display = (type === 'page' || type === 'file') ? 'flex' : 'none';
    menuOpen.style.display = (type === 'folder') ? 'flex' : 'none';

    const menuVisibility = document.getElementById('menu-visibility');
    if (menuVisibility) {
        menuVisibility.style.display = (type === 'page') ? 'flex' : 'none';
    }

    // Masquer le partage pour les pages
    if (menuShare) {
        menuShare.style.display = (type === 'page') ? 'none' : 'flex';
    }
    const menu = document.getElementById('contextMenu');
    menu.classList.add('show');

    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const menuRect = menu.getBoundingClientRect();

    let x = event.clientX;
    let y = event.clientY;

    if (x + menuRect.width > viewportWidth) {
        x = viewportWidth - menuRect.width - 10;
    }

    if (y + menuRect.height > viewportHeight) {
        y = viewportHeight - menuRect.height - 10;
    }

    if (x < 10) x = 10;
    if (y < 10) y = 10;

    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
}

function handleAction(action) {
    document.getElementById('contextMenu').classList.remove('show');
    
    if (action === 'delete') {
        document.getElementById('deleteItemId').value = currentItemId;
        document.getElementById('deleteItemType').value = currentItemType;
        document.getElementById('deleteModal').classList.add('active');
    } else if (action === 'move') {
        document.getElementById('moveItemId').value = currentItemId;
        document.getElementById('moveItemType').value = currentItemType;
        document.getElementById('moveModal').classList.add('active');
    } else if (action === 'rename') {
        document.getElementById('renameItemId').value = currentItemId;
        document.getElementById('renameItemType').value = currentItemType;
        document.getElementById('renameInput').value = currentItemName;
        document.getElementById('renameModal').classList.add('active');
    } else if (action === 'share') {
        const itemName = currentItemName || 'cet élément';
        openShareModalAction(currentItemId, currentItemType, itemName);
    } else if (action === 'visibility') {
    document.getElementById('visibilityPageId').value = currentItemId;
    document.getElementById('visibilityModal').classList.add('active');
    } else if (action === 'edit') {
        if (currentItemType === 'page') {
            const currentFolder = new URLSearchParams(window.location.search).get('dossier') || '';
            window.location.href = '/sitec/create_page.php?dossier=' + currentFolder + '&id=' + currentItemId + '&edit=1';
        } else if (currentItemType === 'file') {
            window.location.href = '/edite1/editor.php?id=' + currentItemId;
        }
    } else if (action === 'open' && currentItemType === 'folder') {
        window.location.href = '?dossier=' + currentItemId;
    }
}

document.addEventListener('click', e => {
    if (!e.target.closest('.item-menu')) {
        document.getElementById('contextMenu').classList.remove('show');
    }
});

document.querySelectorAll('.modal').forEach(modal => {
    modal.addEventListener('click', e => {
        if (e.target === modal) {
            modal.classList.remove('active');
        }
    });
});

function filterFiles() {
    const filterType = document.getElementById('filterType').value;
    const allItems = document.querySelectorAll('.folder-card, .page-card, .file-card, .list-item');
    
    allItems.forEach(item => {
        const itemType = item.getAttribute('data-type');
        item.style.display = (filterType === 'all' || filterType === itemType) ? '' : 'none';
    });
}

function searchFiles() {
    const searchTerm = document.getElementById('searchBox').value.toLowerCase();
    const allItems = document.querySelectorAll('.folder-card, .page-card, .file-card, .list-item');
    
    allItems.forEach(item => {
        const itemName = item.getAttribute('data-name').toLowerCase();
        item.style.display = itemName.includes(searchTerm) ? '' : 'none';
    });
}

function openShareModalAction(itemId, itemType, itemName) {
    currentItemId = itemId;
    currentItemType = itemType;
    
    document.getElementById('shareItemId').value = itemId;
    document.getElementById('shareItemType').value = itemType;
    document.getElementById('shareItemName').textContent = itemName;
    
    loadExistingShares(itemId, itemType);
    document.getElementById('shareModal').classList.add('active');
}

async function loadExistingShares(itemId, itemType) {
    try {
        const response = await fetch(`get_shares.php?id=${itemId}&type=${itemType}`);
        const data = await response.json();
        selectedUsers = data.users || [];
        updateSharesList();
    } catch(e) {
        console.error('Erreur chargement partages:', e);
        selectedUsers = [];
        updateSharesList();
    }
}

document.getElementById('shareUsersInput')?.addEventListener('input', async function(e) {
    const query = e.target.value.trim();
    const suggestions = document.getElementById('userSuggestions');
    
    if (query.length < 2) {
        suggestions.style.display = 'none';
        return;
    }
    
    const users = await searchUsers(query);
    
    if (users.length > 0) {
        suggestions.innerHTML = users.map(u => 
            `<div onclick="selectUser(${u.id}, '${escapeHtml(u.nom)}')" style="padding:12px;cursor:pointer;border-bottom:1px solid #f0f2f5;transition:background 0.2s;" onmouseover="this.style.background='#f0f2f5'" onmouseout="this.style.background='white'">
                <i class="fas fa-user" style="color:#4CAF50;margin-right:8px;"></i> <strong>${escapeHtml(u.nom)}</strong>
            </div>`
        ).join('');
        suggestions.style.display = 'block';
    } else {
        suggestions.innerHTML = '<div style="padding:12px;color:#65676b;text-align:center;"><i>Aucun utilisateur trouvé</i></div>';
        suggestions.style.display = 'block';
    }
});

async function searchUsers(query) {
    try {
        const response = await fetch(`search_users.php?q=${encodeURIComponent(query)}`);
        return await response.json();
    } catch(e) {
        console.error('Erreur recherche:', e);
        return [];
    }
}

function selectUser(userId, userName) {
    if (!selectedUsers.find(u => u.id === userId)) {
        selectedUsers.push({
            id: userId, 
            name: userName,
            permission: 'L'
        });
        updateSharesList();
    }
    document.getElementById('shareUsersInput').value = '';
    document.getElementById('userSuggestions').style.display = 'none';
}

function updateSharesList() {
    const container = document.getElementById('sharesList');
    
    if (selectedUsers.length === 0) {
        container.innerHTML = '<div style="text-align:center;color:#65676b;padding:20px;"><i class="fas fa-users-slash" style="font-size:32px;margin-bottom:8px;"></i><p>Aucun partage actif</p></div>';
        document.getElementById('shareDataHidden').value = '';
        return;
    }
    
    container.innerHTML = `
    <table style="width:100%;border-collapse:collapse;">
        <thead>
            <tr style="border-bottom:2px solid ">
                <th style="padding:12px;text-align:left;font-weight:600;">
                    <input type="checkbox" id="selectAllUsers" onchange="toggleSelectAll(this)" style="margin-right:8px;cursor:pointer;width:18px;height:18px;">
                    <label for="selectAllUsers" style="cursor:pointer;">Tout sélectionner</label>
                </th>
                <th style="padding:12px;text-align:center;font-weight:600;">
                    <input type="radio" name="perm_all" value="L" onchange="applyPermissionToAll('L')" style="cursor:pointer;width:18px;height:18px;">
                </th>
                <th style="padding:12px;text-align:center;font-weight:600;">
                    <input type="radio" name="perm_all" value="E" onchange="applyPermissionToAll('E')" style="cursor:pointer;width:18px;height:18px;">
                </th>
                <th style="padding:12px;text-align:center;font-weight:600;">
                    <input type="radio" name="perm_all" value="S" onchange="applyPermissionToAll('S')" style="cursor:pointer;width:18px;height:18px;">
                </th>
                <th style="padding:12px;text-align:center;font-weight:600;">Actions</th>
            </tr>
            <tr style="border-bottom:1px solid #e4e6eb;">
                <th style="padding:8px;text-align:left;font-weight:600;font-size:12px;">Utilisateur</th>
                <th style="padding:8px;text-align:center;font-weight:600;font-size:12px;">Lecture</th>
                <th style="padding:8px;text-align:center;font-weight:600;font-size:12px;">Édition</th>
                <th style="padding:8px;text-align:center;font-weight:600;font-size:12px;">Contrôle total</th>
                <th style="padding:8px;text-align:center;font-weight:600;font-size:12px;"></th>
            </tr>
        </thead>
        <tbody>
            ${selectedUsers.map(u => `
                <tr style="border-bottom:1px solid #e4e6eb;">
                    <td style="padding:12px;">
                        <i class="fas fa-user" style="color:#4CAF50;margin-right:8px;"></i>
                        <strong>${escapeHtml(u.name)}</strong>
                    </td>
                    <td style="padding:12px;text-align:center;">
                        <input type="radio" name="perm_${u.id}" value="L" ${u.permission === 'L' ? 'checked' : ''} 
                               onchange="changeUserPermission(${u.id}, 'L')" style="cursor:pointer;width:18px;height:18px;">
                    </td>
                    <td style="padding:12px;text-align:center;">
                        <input type="radio" name="perm_${u.id}" value="E" ${u.permission === 'E' ? 'checked' : ''} 
                               onchange="changeUserPermission(${u.id}, 'E')" style="cursor:pointer;width:18px;height:18px;">
                    </td>
                    <td style="padding:12px;text-align:center;">
                        <input type="radio" name="perm_${u.id}" value="S" ${u.permission === 'S' ? 'checked' : ''} 
                               onchange="changeUserPermission(${u.id}, 'S')" style="cursor:pointer;width:18px;height:18px;">
                    </td>
                    <td style="padding:12px;text-align:center;">
                        <button type="button" onclick="removeUser(${u.id})" 
                                style="background:#f44336;color:white;border:none;padding:6px 12px;border-radius:4px;cursor:pointer;">
                            <i class="fas fa-trash"></i>
                        </button>
                    </td>
                </tr>
            `).join('')}
        </tbody>
    </table>
    `;
    
    const shareData = selectedUsers.map(u => `${u.id}:${u.permission}`).join(',');
    document.getElementById('shareDataHidden').value = shareData;
}

function changeUserPermission(userId, newPermission) {
    const user = selectedUsers.find(u => u.id === userId);
    if (user) {
        user.permission = newPermission;
        updateSharesList();
    }
}

function removeUser(userId) {
    selectedUsers = selectedUsers.filter(u => u.id !== userId);
    updateSharesList();
}

function applyPermissionToAll(permission) {
    if (!permission) return;
    
    selectedUsers.forEach(u => u.permission = permission);
    updateSharesList();
    
    let permText = permission === 'L' ? 'Lecture' : (permission === 'E' ? 'Édition' : 'Contrôle total');
    showNotification(`Permission "${permText}" appliquée à tous`, 'success');
}

function toggleSelectAll(checkbox) {
    console.log('Tout sélectionner:', checkbox.checked);
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

document.addEventListener('click', function(e) {
    if (!e.target.closest('#shareUsersInput') && !e.target.closest('#userSuggestions')) {
        document.getElementById('userSuggestions').style.display = 'none';
    }
});

function openUploadPopup() {
    closeModal('createModal');
    document.getElementById('uploadModal').classList.add('active');
}

document.getElementById('fileUploadInput')?.addEventListener('change', function(e) {
    const file = e.target.files[0];
    const text = document.getElementById('fileUploadText');
    
    if (file) {
        const size = (file.size / 1024 / 1024).toFixed(2);
        text.innerHTML = `<strong>${file.name}</strong><br><small>${size} MB</small>`;
    }
});

function showNotification(message, type = 'info') {
    let notifContainer = document.getElementById('notificationContainer');
    
    if (!notifContainer) {
        notifContainer = document.createElement('div');
        notifContainer.id = 'notificationContainer';
        notifContainer.style.cssText = 'position:fixed;top:80px;right:20px;z-index:9999;max-width:400px;';
        document.body.appendChild(notifContainer);
    }
    
    const colors = { success: '#4CAF50', error: '#f44336', info: '#2196F3', warning: '#FF9800' };
    const icons = { success: 'fa-check-circle', error: 'fa-exclamation-circle', info: 'fa-info-circle', warning: 'fa-exclamation-triangle' };
    
    const notif = document.createElement('div');
    notif.style.cssText = `background:${colors[type]};color:white;padding:16px 20px;border-radius:8px;margin-bottom:10px;box-shadow:0 4px 12px rgba(0,0,0,0.15);display:flex;align-items:center;gap:12px;`;
    notif.innerHTML = `<i class="fas ${icons[type]}"></i><span>${message}</span>`;
    
    notifContainer.appendChild(notif);
    setTimeout(() => notif.remove(), 5000);
}
</script>
</body>
</html>
<?php $conn->close(); ?>