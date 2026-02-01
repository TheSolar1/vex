<?php
include '/var/www/html/function.php';
include '/var/www/html/access_control.php';
footer();
ini_set('display_errors', 1);
ini_set('error_log', '/var/log/apache2/error.log');
error_reporting(E_ALL);


// Connexion à la base de données
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("Erreur de connexion : " . $conn->connect_error);
}

if (!$conn->set_charset("utf8mb4")) {
    die("Erreur lors du chargement du jeu de caractères utf8mb4 : " . $conn->error);
}
$exodriveColorScheme = [
    'nav_bg' => '#4CAF50',
    'nav_gradient' => 'linear-gradient(135deg, #4CAF50 0%, #45a049 100%)',
    'shadow' => '0 1px 4px rgba(0,0,0,0.15)',
    'hover_bg' => 'rgba(255,255,255,0.1)',
    'active_color' => 'rgba(255, 255, 255, 0.9)',
    'text_primary' => '#ffffff',
    'muted' => 'rgba(255,255,255,0.8)',
    'admin_sep' => 'rgba(255,255,255,0.06)',
    'popup_bg' => '#45a049',
    'popup_border' => 'rgba(255,255,255,0.1)',
    'app_item_bg' => 'rgba(255,255,255,0.1)',
    'app_item_border' => 'rgba(255,255,255,0.08)',
    'app_item_hover' => 'rgba(255,255,255,0.2)',
    'btn_bg' => 'rgba(255,255,255,0.1)',
    'btn_border' => 'rgba(255,255,255,0.1)',
    'profile_icon_bg' => 'rgba(255,255,255,0.15)'
];

// Appel de la fonction avec le schéma de couleurs
displayNavigation($conn, [], [], $exodriveColorScheme);
// Variables globales
$user_id = 0;
$user_nom = "Invité";
$user_email = "";
$user_privilege = 0;
$user_vip = 0;
$is_connected = false;
$user_idcokier = null;
$message = "";

// Vérification de la connexion utilisateur
if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    
    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=?");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        $user_idcokier = $row['idcokier'];
        $pc = $row['pc'];
        $navi = $row['navi'];
        $user_email = $row['email'];
        $user_nom = $row['nom'];
        $datecra = $row['datecra'];
        
        if ($pc === $_SERVER['REMOTE_ADDR'] && $navi === $_SERVER['HTTP_USER_AGENT']) {
            $one_hour_ago = strtotime('-1 hour');
            $datecra_timestamp = strtotime($datecra);
            
            if ($datecra_timestamp > $one_hour_ago) {
                $stmt2 = $conn->prepare("SELECT id, privilege, vip FROM login WHERE email=?");
                $stmt2->bind_param("s", $user_email);
                $stmt2->execute();
                $result2 = $stmt2->get_result();
                
                if ($result2->num_rows == 1) {
                    $row2 = $result2->fetch_assoc();
                    $user_id = (int)$row2['id'];
                    $user_privilege = (int)$row2['privilege'];
                    $user_vip = $row2['vip'];
                    $is_connected = true;
                }
                $stmt2->close();
            }
        } else {
            $auteur = "Les informations de connexion ne correspondent pas. Il se peut que votre session ait été compromise.";
            $sql = "INSERT INTO `sus-hac` (`id-c`, `auteur`) VALUES (?, ?)";
            $stmt_sus = $conn->prepare($sql);
            $stmt_sus->bind_param("ss", $user_idcokier, $auteur);
            $stmt_sus->execute();
            $stmt_sus->close();
        }
    }
    $stmt->close();
}


// TRAITEMENT DU TÉLÉVERSEMENT
// TRAITEMENT DU TÉLÉVERSEMENT
// TRAITEMENT DU TÉLÉVERSEMENT
echo "<script>console.log('REQUEST_METHOD: " . $_SERVER['REQUEST_METHOD'] . "');</script>";
echo "<script>console.log('POST data: " . json_encode(array_keys($_POST)) . "');</script>";
echo "<script>console.log('FILES data: " . json_encode(array_keys($_FILES)) . "');</script>";

if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['upload'])or isset($_FILES['screenshot'])) {
    echo "<script>console.log('Upload request received');</script>";
    if (isset($_FILES['screenshot']) && $_FILES['screenshot']['error'] === UPLOAD_ERR_OK) {
        $file = $_FILES['screenshot'];
        $nom_fichier = $file['name'];
        $type_fichier = $file['type'];
        $visibilite = $is_connected ? strval($_POST['visble']) : '0';
        $user_id_upload = $is_connected ? strval($user_id) : '0';
        $date_actuelle = date('Y-m-d');
        $taille_fichier = $file['size'];

        // Préparation de la requête
        $stmt = $conn->prepare("INSERT INTO fichiers (nom, type_fichier, taille, fichier, visble, id_utilisateur, date) VALUES (?, ?, ?, ?, ?, ?, ?)");
        
        if (!$stmt) {
            echo "<script>showNotification('Erreur préparation: " . addslashes($conn->error) . "', 'error');</script>";
        } else {
            // IMPORTANT: Pour un BLOB, utilisez "b" et passez NULL
            $null = NULL;
            $stmt->bind_param("ssibsss", $nom_fichier, $type_fichier, $taille_fichier, $null, $visibilite, $user_id_upload, $date_actuelle);
            
            // Envoyer le fichier avec send_long_data (paramètre 3 = index du blob)
            $fp = fopen($file['tmp_name'], 'rb');
            if ($fp) {
                while (!feof($fp)) {
                    $stmt->send_long_data(3, fread($fp, 8192));
                }
                fclose($fp);
                
                if ($stmt->execute()) {
                    echo "<script>showNotification('Fichier téléversé avec succès!', 'success'); setTimeout(() => window.location.reload(), 1500);</script>";
                } else {
                    echo "<script>showNotification('Erreur exécution: " . addslashes($stmt->error) . "', 'error');</script>";
                }
            } else {
                echo "<script>showNotification('Erreur ouverture fichier', 'error');</script>";
            }
            
            $stmt->close();
        }
    } else {
        echo "<script>showNotification('Erreur de fichier', 'error');</script>";
    }
}



// TRAITEMENT DES MODIFICATIONS
if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['id_fichier'])) {
    if (!$is_connected) {
        $message = "<script>showNotification('Vous devez être connecté', 'error');</script>";
    } else {
        $id_fichier = intval($_POST['id_fichier']);
        
        $stmt_check = $conn->prepare("SELECT id_utilisateur FROM fichiers WHERE id = ?");
        $stmt_check->bind_param("i", $id_fichier);
        $stmt_check->execute();
        $result_check = $stmt_check->get_result();
        
        if ($result_check->num_rows == 1) {
            $row_check = $result_check->fetch_assoc();
            $file_owner = $row_check['id_utilisateur'];
            
            $can_modify = ($file_owner == $user_id) || ($user_privilege >= 1);
            
            if ($can_modify) {
                if (isset($_POST['nouvelle_visibilite'])) {
                    $nouvelle_visibilite = intval($_POST['nouvelle_visibilite']);
                    $date_actuelle = date('Y-m-d');
                    
                    $stmt = $conn->prepare("UPDATE fichiers SET visble = ?, date = ? WHERE id = ? AND id_utilisateur = ?");
                    $stmt->bind_param("isii", $nouvelle_visibilite, $date_actuelle, $id_fichier, $file_owner);
                    
                    if ($stmt->execute()) {
                        $message = "<script>showNotification('Visibilité modifiée avec succès', 'success'); setTimeout(() => window.location.reload(), 1500);</script>";
                    } else {
                        $message = "<script>showNotification('Erreur lors de la modification', 'error');</script>";
                    }
                    $stmt->close();
                    
                } elseif (isset($_POST['action']) && $_POST['action'] === 'delete') {
                    $stmt = $conn->prepare("DELETE FROM fichiers WHERE id = ? AND id_utilisateur = ?");
                    $stmt->bind_param("ii", $id_fichier, $file_owner);
                    
                    if ($stmt->execute()) {
                        $message = "<script>showNotification('Fichier supprimé avec succès', 'success'); setTimeout(() => window.location.reload(), 1500);</script>";
                    } else {
                        $message = "<script>showNotification('Erreur lors de la suppression', 'error');</script>";
                    }
                    $stmt->close();
                }
            } else {
                $message = "<script>showNotification('Vous n\\'avez pas les droits pour modifier ce fichier', 'error');</script>";
            }
        } else {
            $message = "<script>showNotification('Fichier introuvable', 'error');</script>";
        }
        $stmt_check->close();
    }
}

// Récupération des fichiers publics
$fichiers_publics = [];
$stmt = $conn->prepare("SELECT f.id, f.nom, f.type_fichier, f.id_utilisateur, f.visble, l.nom as proprietaire_nom FROM fichiers f LEFT JOIN login l ON f.id_utilisateur = l.id WHERE f.visble = 0 ORDER BY f.date DESC");
$stmt->execute();
$result = $stmt->get_result();
while ($row = $result->fetch_assoc()) {
    $fichiers_publics[] = $row;
}
$stmt->close();

// Récupération des fichiers de l'utilisateur connecté
$fichiers_utilisateur = [];
if ($is_connected) {
    $stmt = $conn->prepare("SELECT id, nom, type_fichier, visble, date FROM fichiers WHERE id_utilisateur = ? AND visble != 0 ORDER BY date DESC");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();
    while ($row = $result->fetch_assoc()) {
        $fichiers_utilisateur[] = $row;
    }
    $stmt->close();
}

function getFileIcon($nom_fichier) {
    $icon_map = fcichier();
    $extension = strtolower(pathinfo($nom_fichier, PATHINFO_EXTENSION));
    
    if (isset($icon_map[$extension])) {
        return '<i class="fa-solid ' . $icon_map[$extension] . '"></i>';
    }
    return '<i class="fa-solid ' . $icon_map['default'] . '"></i>';
}

function getVisibilityText($visibilite) {
    switch($visibilite) {
        case 0: return "Public";
        case 1: return "Privé";
        case 3: return "Lien unique";
        default: return "Inconnu";
    }
}
       
?>

<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Exodrive - .hopto.org</title>
  <link rel="icon" href="/vex.png" type="image/png">
  <script src="/fa-local.js" defer></script>
  <style>
      * {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

/* Override pour la navigation verte */
:root {
  --nav-bg: #4CAF50 !important;
  --nav-gradient: linear-gradient(135deg, #4CAF50 0%, #45a049 100%) !important;
  --shadow: 0 1px 4px rgba(0,0,0,0.15) !important;
  --hover-bg: rgba(255,255,255,0.1) !important;
  --active-color: rgba(255, 255, 255, 0.9) !important;
  --text-primary: #ffffff !important;
  --muted: rgba(255,255,255,0.8) !important;
  --admin-sep: rgba(255,255,255,0.06) !important;
}

html, body {
  height: 100vh;

  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  color: #1c1e21;
  overflow-x: hidden;
}

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 80px 20px 20px 20px;
  min-height: 100vh;
  position: relative;
}

.header {
  text-align: center;
  margin-bottom: 40px;
  animation: slideInDown 1s ease-out;
}

.header h1 {
  font-size: 2.5rem;
  color: white;
  margin-bottom: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
  font-weight: 600;
  text-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

.header h1 i {
  font-size: 2.2rem;
  color: white;
}

.header p {
  color: rgba(255,255,255,0.9);
  font-size: 1.1rem;
}

.connection-status {
  background: rgba(255, 255, 255, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.3);
  padding: 10px 20px;
  border-radius: 20px;
  margin: 10px auto;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: white;
}

.upload-section {
  background: white;
  border-radius: 12px;
  padding: 30px;
  margin-bottom: 30px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.5);
  animation: slideInUp 1s ease-out 0.2s both;
}

.upload {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group label {
  font-weight: 600;
  color: #1c1e21;
  font-size: 15px;
}

.file-input-wrapper {
  position: relative;
  display: inline-block;
  width: 100%;
}

.file-input-wrapper input[type="file"] {
  opacity: 0;
  position: absolute;
  z-index: -1;
}

.file-input-label {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  border: 2px dashed #4CAF50;
  border-radius: 10px;
  background: rgba(76, 175, 80, 0.05);
  cursor: pointer;
  transition: all 0.3s ease;
  font-weight: 500;
  color: #4CAF50;
  min-height: 60px;
}

.file-input-label:hover {
  border-color: #45a049;
  background: rgba(76, 175, 80, 0.1);
  transform: scale(1.01);
}

.file-input-label.has-file {
  border-color: #4CAF50;
  background: rgba(76, 175, 80, 0.15);
  color: #2e7d32;
}

.file-input-icon {
  margin-right: 10px;
  font-size: 1.2em;
}

.visibility-select {
  padding: 12px 15px;
  border: 1px solid #e4e6eb;
  border-radius: 8px;
  font-size: 15px;
  transition: all 0.3s ease;
  background: white;
  color: #1c1e21;
}

.visibility-select:focus {
  outline: none;
  border-color: #4CAF50;
  box-shadow: 0 0 0 3px rgba(76, 175, 80, 0.1);
}

.upload-btn {
  padding: 12px 24px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}

.upload-btn:hover:not(:disabled) {
  background: #45a049;
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
}

.upload-btn:disabled {
  background: #ccc;
  cursor: not-allowed;
  transform: none;
}

.files-section {
  background: white;
  border-radius: 12px;
  padding: 30px;
  margin-bottom: 30px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.5);
  animation: slideInUp 1s ease-out 0.4s both;
}

.section-title {
  font-size: 1.5rem;
  margin-bottom: 20px;
  color: #1c1e21;
  border-bottom: 2px solid #4CAF50;
  padding-bottom: 10px;
  display: flex;
  align-items: center;
  gap: 12px;
  font-weight: 600;
}

.file-list {
  list-style: none;
}

.file-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 15px;
  margin-bottom: 10px;
  background: #f9f9f9;
  border: 1px solid #e4e6eb;
  border-radius: 8px;
  transition: all 0.3s ease;
  animation: fadeInLeft 0.5s ease-out;
}

.file-item:hover {
  background: #f0f2f5;
  transform: translateX(5px);
  border-color: #4CAF50;
  box-shadow: 0 2px 8px rgba(76, 175, 80, 0.1);
}

.file-info {
  display: flex;
  align-items: center;
  gap: 15px;
  flex-grow: 1;
  min-width: 0;
}

.file-meta {
  display: flex;
  align-items: center;
  gap: 18px;
  min-width: 0;
  width: 100%;
}

.file-meta > .file-name-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
}

.file-name-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.file-owner-row {
  background: #e8f5e9;
  color: #2e7d32;
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  flex-shrink: 0;
  white-space: nowrap;
}

.owner-badge {
  background: #e8f5e9;
  color: #2e7d32;
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: 0;
  flex-shrink: 0;
  white-space: nowrap;
}

.file-visibility {
  background: rgba(76, 175, 80, 0.1);
  padding: 6px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  color: #4CAF50;
  white-space: nowrap;
  flex-shrink: 0;
  border: 1px solid rgba(76, 175, 80, 0.2);
}

.file-links {
  display: flex;
  gap: 5px;
  align-items: center;
  margin-left: 12px;
}

.file-link {
  text-decoration: none;
  color: #4CAF50;
  font-weight: 500;
  padding: 8px 12px;
  border-radius: 6px;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  background: transparent;
}

.file-link:hover {
  background: rgba(76, 175, 80, 0.1);
  color: #45a049;
  transform: scale(1.1);
}

.file-name {
  color: #1c1e21;
  font-weight: 500;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 15px;
}

.file-actions {
  display: flex;
  gap: 5px;
  align-items: center;
  margin-left: -10px;
}

.file-actions .action-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: #4CAF50;
  font-size: 16px;
  padding: 8px 12px;
  border-radius: 6px;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.file-actions .visibility-btn:hover,
.file-actions .share-btn:hover {
  background: rgba(76, 175, 80, 0.1);
  color: #45a049;
  transform: scale(1.1);
}

.file-actions .delete-btn {
  color: #65676b;
}

.file-actions .delete-btn:hover {
  background: rgba(244, 67, 54, 0.1);
  color: #f44336;

}

.dropdown {
  position: relative;
  display: inline-block;
}

.dropdown-content {
  display: none;
  position: absolute;
  background-color: white;
  min-width: 200px;
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.15);
  border-radius: 8px;
  z-index: 1000;
  left: 50%;
  transform: translateX(-50%);
  top: 100%;
  margin-top: 5px;
  border: 1px solid #e4e6eb;
}

.dropdown-content form {
  margin: 0;
}

.dropdown-content button {
  width: 100%;
  padding: 12px 16px;
  border: none;
  background: none;
  text-align: left;
  cursor: pointer;
  transition: background 0.3s ease;
  font-size: 14px;
  color: #1c1e21;
  display: flex;
  align-items: center;
  gap: 10px;
  font-weight: 500;
}

.dropdown-content button:hover {
  background-color: #f0f2f5;
}

.dropdown-content button:first-child {
  border-radius: 8px 8px 0 0;
}

.dropdown-content button:last-child {
  border-radius: 0 0 8px 8px;
}

.dropdown.show .dropdown-content {
  display: block;
}

.user-files {
  border-left: 3px solid #4CAF50;
  padding-left: 20px;
  margin-top: 30px;
}

.notification {
  position: fixed;
  top: 80px;
  right: 20px;
  padding: 15px 20px;
  border-radius: 8px;
  color: white;
  font-weight: 500;
  transform: translateX(400px);
  transition: transform 0.3s ease;
  z-index: 2000;
  max-width: 300px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.notification.success {
  background: #4CAF50;
}

.notification.error {
  background: #f44336;
}

.notification.show {
  transform: translateX(0);
}

.empty-state {
  text-align: center;
  padding: 40px 20px;
  color: #65676b;
}

.icone-fichier {
  width: 18px;
  height: 18px;
  vertical-align: middle;
  object-fit: contain;
}

.share-modal {
  display: none;
  position: fixed;
  z-index: 3000;
  left: 0;
  top: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.5);
  animation: fadeIn 0.3s ease;
}

.share-modal-content {
  background-color: white;
  margin: 10% auto;
  padding: 30px;
  border-radius: 12px;
  max-width: 500px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  animation: slideInDown 0.3s ease;
}

.share-modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  border-bottom: 2px solid #4CAF50;
  padding-bottom: 10px;
}

.share-modal-header h3 {
  color: #4CAF50;
  font-size: 1.5rem;
  font-weight: 600;
}

.close-modal {
  color: #aaa;
  font-size: 28px;
  font-weight: bold;
  cursor: pointer;
  transition: color 0.3s;
  line-height: 1;
}

.close-modal:hover {
  color: #1c1e21;
}

.share-link-container {
  background: #f9f9f9;
  padding: 15px;
  border-radius: 8px;
  margin: 15px 0;
  border: 1px solid #e4e6eb;
}

.share-link-input {
  width: 100%;
  padding: 12px;
  border: 1px solid #e4e6eb;
  border-radius: 6px;
  font-size: 14px;
  margin-bottom: 10px;
  color: #1c1e21;
  background: white;
}

.copy-btn {
  width: 100%;
  padding: 12px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 8px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.copy-btn:hover {
  background: #45a049;
  transform: translateY(-2px);
}

.copy-btn.copied {
  background: #2196F3;
}

.share-info {
  background: #e3f2fd;
  padding: 12px;
  border-radius: 8px;
  margin-bottom: 15px;
  color: #1565c0;
  font-size: 0.9em;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  border: 1px solid #bbdefb;
}

.share-info i {
  margin-top: 2px;
}

@keyframes slideInDown {
  from {
    opacity: 0;
    transform: translateY(-50px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes slideInUp {
  from {
    opacity: 0;
    transform: translateY(50px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes fadeInLeft {
  from {
    opacity: 0;
    transform: translateX(-30px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@media (max-width: 768px) {
  .container {
    padding: 80px 10px 20px 10px;
  }
  
  .header h1 {
    font-size: 2rem;
  }
  
  .file-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  
  .file-actions {
    width: 100%;
    justify-content: flex-start;
  }
  
  .file-info {
    flex-direction: column;
    align-items: flex-start;
  }
  
  .file-meta {
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .file-links {
    width: 100%;
  }
  
  .share-modal-content {
    margin: 20% auto;
    padding: 20px;
    max-width: 90%;
  }
}

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 80px 20px 20px 20px;
  min-height: 100vh;
  position: relative;
}

.header {
  text-align: center;
  margin-bottom: 40px;
  animation: slideInDown 1s ease-out;
}

.header h1 {
  font-size: 2.5rem;
  color: #1c1e21;
  margin-bottom: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
  font-weight: 600;
}

.header h1 i {
  font-size: 2.2rem;
  color: #4CAF50;
}

.header p {
  color: #65676b;
  font-size: 1.1rem;
}

.connection-status {
  background: rgba(76, 175, 80, 0.1);
  border: 1px solid #4CAF50;
  padding: 10px 20px;
  border-radius: 20px;
  margin: 10px auto;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: #4CAF50;
}

.upload-section {
  background: white;
  border-radius: 12px;
  padding: 30px;
  margin-bottom: 30px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  border: 1px solid #e4e6eb;
  animation: slideInUp 1s ease-out 0.2s both;
}

.upload {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group label {
  font-weight: 600;
  color: #1c1e21;
  font-size: 15px;
}

.file-input-wrapper {
  position: relative;
  display: inline-block;
  width: 100%;
}

.file-input-wrapper input[type="file"] {
  opacity: 0;
  position: absolute;
  z-index: -1;
}

.file-input-label {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  border: 2px dashed #4CAF50;
  border-radius: 10px;
  background: rgba(76, 175, 80, 0.05);
  cursor: pointer;
  transition: all 0.3s ease;
  font-weight: 500;
  color: #4CAF50;
  min-height: 60px;
}

.file-input-label:hover {
  border-color: #45a049;
  background: rgba(76, 175, 80, 0.1);
  transform: scale(1.01);
}

.file-input-label.has-file {
  border-color: #4CAF50;
  background: rgba(76, 175, 80, 0.15);
  color: #2e7d32;
}

.file-input-icon {
  margin-right: 10px;
  font-size: 1.2em;
}

.visibility-select {
  padding: 12px 15px;
  border: 1px solid #e4e6eb;
  border-radius: 8px;
  font-size: 15px;
  transition: all 0.3s ease;
  background: white;
  color: #1c1e21;
}

.visibility-select:focus {
  outline: none;
  border-color: #4CAF50;
  box-shadow: 0 0 0 3px rgba(76, 175, 80, 0.1);
}

.upload-btn {
  padding: 12px 24px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}

.upload-btn:hover:not(:disabled) {
  background: #45a049;
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
}

.upload-btn:disabled {
  background: #ccc;
  cursor: not-allowed;
  transform: none;
}

.files-section {
  background: white;
  border-radius: 12px;
  padding: 30px;
  margin-bottom: 30px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  border: 1px solid #e4e6eb;
  animation: slideInUp 1s ease-out 0.4s both;
}

.section-title {
  font-size: 1.5rem;
  margin-bottom: 20px;
  color: #1c1e21;
  border-bottom: 2px solid #4CAF50;
  padding-bottom: 10px;
  display: flex;
  align-items: center;
  gap: 12px;
  font-weight: 600;
}

.file-list {
  list-style: none;
}

.file-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 15px;
  margin-bottom: 10px;
  background: #f9f9f9;
  border: 1px solid #e4e6eb;
  border-radius: 8px;
  transition: all 0.3s ease;
  animation: fadeInLeft 0.5s ease-out;
}

.file-item:hover {
  background: #f0f2f5;
  transform: translateX(5px);
  border-color: #4CAF50;
  box-shadow: 0 2px 8px rgba(76, 175, 80, 0.1);
}

.file-info {
  display: flex;
  align-items: center;
  gap: 15px;
  flex-grow: 1;
  min-width: 0;
}

.file-meta {
  display: flex;
  align-items: center;
  gap: 18px;
  min-width: 0;
  width: 100%;
}

.file-meta > .file-name-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
}

.file-name-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.file-owner-row {
  background: #e8f5e9;
  color: #2e7d32;
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  flex-shrink: 0;
  white-space: nowrap;
}

.owner-badge {
  background: #e8f5e9;
  color: #2e7d32;
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: 0;
  flex-shrink: 0;
  white-space: nowrap;
}

.file-visibility {
  background: rgba(76, 175, 80, 0.1);
  padding: 6px 12px;
  border-radius: 12px;
  font-size: 0.85em;
  font-weight: 600;
  color: #4CAF50;
  white-space: nowrap;
  flex-shrink: 0;
  border: 1px solid rgba(76, 175, 80, 0.2);
}

.file-links {
  display: flex;
  gap: 5px;
  align-items: center;
  margin-left: 12px;
}

.file-link {
  text-decoration: none;
  color: #4CAF50;
  font-weight: 500;
  padding: 8px 12px;
  border-radius: 6px;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  background: transparent;
}

.file-link:hover {
  background: rgba(76, 175, 80, 0.1);
  color: #45a049;
  transform: scale(1.1);
}

.file-name {
  color: #1c1e21;
  font-weight: 500;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 15px;
}

.file-actions {
  display: flex;
  gap: 5px;
  align-items: center;
  margin-left: -10px;
}

.file-actions .action-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: #4CAF50;
  font-size: 16px;
  padding: 8px 12px;
  border-radius: 6px;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.file-actions .visibility-btn:hover,
.file-actions .share-btn:hover {
  background: rgba(76, 175, 80, 0.1);
  color: #45a049;
  transform: scale(1.1);
}

.file-actions .delete-btn {
  color: #65676b;
}

.file-actions .delete-btn:hover {
  background: rgba(244, 67, 54, 0.1);
  color: #f44336;

}

.dropdown {
  position: relative;
  display: inline-block;
}

.dropdown-content {
  display: none;
  position: absolute;
  background-color: white;
  min-width: 200px;
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.15);
  border-radius: 8px;
  z-index: 1000;
  left: 50%;
  transform: translateX(-50%);
  top: 100%;
  margin-top: 5px;
  border: 1px solid #e4e6eb;
  pointer-events: auto;
  
}

.dropdown-content form {
  margin: 0;
}

.dropdown-content button {
  width: 100%;
  padding: 12px 16px;
  border: none;
  background: none;
  text-align: left;
  cursor: pointer;
  transition: background 0.3s ease;
  font-size: 14px;
  color: #1c1e21;
  display: flex;
  align-items: center;
  gap: 10px;
  font-weight: 500;
}

.dropdown-content form button[type="submit"] {
  width: 100%;
  padding: 12px 16px;
  border: none;
  background: none;
  text-align: left;
  cursor: pointer;
  transition: background 0.3s ease;
  font-size: 14px;
  color: #1c1e21;
  display: flex;
  justify-content: start;
  gap: 10px;
  font-weight: 500;
}
.dropdown-content form button[type="submit"]:hover {
  background-color: #f0f2f5;
}
.dropdown-content button:hover {
  background-color: #f0f2f5;
}

.dropdown-content button:first-child {
  border-radius: 8px 8px 0 0;
}

.dropdown-content button:last-child {
  border-radius: 0 0 8px 8px;
}

.dropdown.show .dropdown-content {
  display: block;
}

.user-files {
  border-left: 3px solid #4CAF50;
  padding-left: 20px;
  margin-top: 30px;
}

.notification {
  position: fixed;
  top: 80px;
  right: 20px;
  padding: 15px 20px;
  border-radius: 8px;
  color: white;
  font-weight: 500;
  transform: translateX(400px);
  transition: transform 0.3s ease;
  z-index: 2000;
  max-width: 300px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.notification.success {
  background: #4CAF50;
}

.notification.error {
  background: #f44336;
}

.notification.show {
  transform: translateX(0);
}

.empty-state {
  text-align: center;
  padding: 40px 20px;
  color: #65676b;
}

.icone-fichier {
  width: 18px;
  height: 18px;
  vertical-align: middle;
  object-fit: contain;
}

.share-modal {
  display: none;
  position: fixed;
  z-index: 3000;
  left: 0;
  top: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.5);
  animation: fadeIn 0.3s ease;
}

.share-modal-content {
  background-color: white;
  margin: 10% auto;
  padding: 30px;
  border-radius: 12px;
  max-width: 500px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  animation: slideInDown 0.3s ease;
}

.share-modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  border-bottom: 2px solid #4CAF50;
  padding-bottom: 10px;
}

.share-modal-header h3 {
  color: #4CAF50;
  font-size: 1.5rem;
  font-weight: 600;
}

.close-modal {
  color: #aaa;
  font-size: 28px;
  font-weight: bold;
  cursor: pointer;
  transition: color 0.3s;
  line-height: 1;
}

.close-modal:hover {
  color: #1c1e21;
}

.share-link-container {
  background: #f9f9f9;
  padding: 15px;
  border-radius: 8px;
  margin: 15px 0;
  border: 1px solid #e4e6eb;
}

.share-link-input {
  width: 100%;
  padding: 12px;
  border: 1px solid #e4e6eb;
  border-radius: 6px;
  font-size: 14px;
  margin-bottom: 10px;
  color: #1c1e21;
  background: white;
}

.copy-btn {
  width: 100%;
  padding: 12px;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 8px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.copy-btn:hover {
  background: #45a049;
  transform: translateY(-2px);
}

.copy-btn.copied {
  background: #2196F3;
}

.share-info {
  background: #e3f2fd;
  padding: 12px;
  border-radius: 8px;
  margin-bottom: 15px;
  color: #1565c0;
  font-size: 0.9em;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  border: 1px solid #bbdefb;
}

.share-info i {
  margin-top: 2px;
}

@keyframes slideInDown {
  from {
    opacity: 0;
    transform: translateY(-50px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes slideInUp {
  from {
    opacity: 0;
    transform: translateY(50px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes fadeInLeft {
  from {
    opacity: 0;
    transform: translateX(-30px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@media (max-width: 768px) {
  .container {
    padding: 80px 10px 20px 10px;
  }
  
  .header h1 {
    font-size: 2rem;
  }
  
  .file-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  
  .file-actions {
    width: 100%;
    justify-content: flex-start;
  }
  
  .file-info {
    flex-direction: column;
    align-items: flex-start;
  }
  
  .file-meta {
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .file-links {
    width: 100%;
  }
  
  .share-modal-content {
    margin: 20% auto;
    padding: 20px;
    max-width: 90%;
  }
}
</style>

</head>
<body>
  <br>
   <br>
    <br>
     <br>
  <div class="container">
    <div class="header">
      <h1><i class="fa-solid fa-folder-open"></i> Exodrive</h1>
      <p style="color: rgba(0, 0, 0, 0.9);">Votre plateforme de partage de fichiers</p>
     
    </div>

    <div class="upload-section">
      <h2 class="section-title"><i class="fa-solid fa-cloud-arrow-up"></i> Téléverser un fichier</h2>
      <form class="upload" action="" method="POST" enctype="multipart/form-data" id="upload">
        <div class="form-group">
          <label for="screenshot">Sélectionner votre fichier :</label>
          <div class="file-input-wrapper">
            <input type="file" id="screenshot" name="screenshot" required>
            <label for="screenshot" class="file-input-label" id="fileInputLabel">
              <span class="file-input-icon"><i class="fa-solid fa-folder-open"></i></span>
              <span id="fileInputText">Cliquez pour parcourir ou glissez un fichier ici</span>
            </label>
          </div>
        </div>
        
        <?php if ($is_connected): ?>
        <div class="form-group">
          <label for="visble">Visibilité du fichier :</label>
          <select class="visibility-select" id="visble" name="visble" required>
            <option value="0">Public</option>
            <option value="1">Privé</option>
            <option value="3">Lien unique</option>
          </select>
        </div>
        <?php else: ?>
        <input type="hidden" name="visble" value="0">
        <p style="color: #666; font-size: 0.9em;">Les fichiers non connectés sont automatiquement publics.</p>
        <?php endif; ?>
        
        <button type="submit" name="upload" class="upload-btn">
          <i class="fa-solid fa-cloud-arrow-up"></i>
          <span>Téléverser</span>
        </button>
      </form>
    </div>

    <div class="files-section">
      <h2 class="section-title"><i class="fa-solid fa-folder"></i> Fichiers publics (<?php echo count($fichiers_publics); ?>)</h2>
      <ul class="file-list" id="publicFiles">
        <?php if (empty($fichiers_publics)): ?>
        <li class="file-item" style="justify-content: center;">
          <div class="empty-state">
            <span>Aucun fichier public disponible pour le moment</span>
          </div>
        </li>
        <?php else: ?>
        <?php foreach ($fichiers_publics as $fichier): ?>
        <?php 
          $is_owner = ($is_connected && $fichier['id_utilisateur'] == $user_id);
          $is_admin = ($is_connected && $user_privilege >= 1);
        ?>
        <li class="file-item">
          <div class="file-info">
            <span class="file-visibility">Public</span>
            <div class="file-meta">
              <div class="file-name-row">
                <?php echo getFileIcon($fichier['nom']); ?>
                <span class="file-name"><?php echo htmlspecialchars($fichier['nom']); ?></span>
                   <div class="file-owner-row">
                        <?php if ($is_owner): ?>
                            <span class="owner-badge">À vous</span>
                        <?php else: ?>
                            <span>par <?php
                                echo !empty($fichier['proprietaire_nom']) 
                                    ? htmlspecialchars($fichier['proprietaire_nom']) 
                                    : "anonyme";
                            ?></span>
                        <?php endif; ?>
                    </div>
                  
            </div>
          </div>

          <div style="display:flex; align-items:center; gap:8px;">
            <div class="file-links">
              <a href="/tel/tel.php?id=<?php echo $fichier['id']; ?>" target="_blank" class="file-link" title="Visualiser le fichier">
                <i class="fa-solid fa-eye"></i>
              </a>
              <a class="file-link" href="/tel/tel.php?id=<?php echo $fichier['id']; ?>&download=1" title="Télécharger le fichier">
                <i class="fa-solid fa-download"></i>
              </a>
            </div>

            <div class="file-actions">
              <button class="action-btn share-btn" type="button" onclick="openShareModal(<?php echo $fichier['id']; ?>, '<?php echo htmlspecialchars($fichier['nom'], ENT_QUOTES); ?>', '<?php echo getVisibilityText($fichier['visble']); ?>')" title="Partager le fichier">
                <i class="fa-solid fa-share-nodes"></i>
              </button>

              <?php if ($is_owner): ?>
                <div class="dropdown" id="dropdown<?php echo $fichier['id']; ?>">
                  <button class="action-btn visibility-btn" type="button" onclick="toggleDropdown(<?php echo $fichier['id']; ?>)" title="Gérer la visibilité">
                    <i class="fa-solid fa-gear"></i>
                  </button>
                  <div class="dropdown-content">
                    <form method="POST">
                      <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                      <input type="hidden" name="nouvelle_visibilite" value="1">
                      <button type="submit"><i class="fa-solid fa-lock"></i> Rendre privé</button>
                    </form>
                    <form method="POST">
                      <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                      <input type="hidden" name="nouvelle_visibilite" value="3">
                      <button type="submit"><i class="fa-solid fa-link"></i> Lien unique</button>
                    </form>
                    <form method="POST" style="display: inline;" onsubmit="return confirm('Êtes-vous sûr de vouloir supprimer ce fichier ?');">
                  <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                  <input type="hidden" name="action" value="delete">
                  <button type="submit" class="action-btn delete-btn" title="Supprimer le fichier">
                    <i class="fa-solid fa-trash"></i> supprimer
                  </button>
                </form>
                  </div>
                  
                </div>

                
              <?php endif; ?>
            </div>
          </div>
        </li>
        <?php endforeach; ?>
        <?php endif; ?>
      </ul>

      <?php if ($is_connected && !empty($fichiers_utilisateur)): ?>
      <div class="user-files">
        <h3 class="section-title"><i class="fa-solid fa-user"></i> Mes fichiers (<?php echo count($fichiers_utilisateur); ?>)</h3>
        <ul class="file-list" id="userFiles">
          <?php foreach ($fichiers_utilisateur as $fichier): ?>
          <?php $is_owner = true; ?>
          <li class="file-item">
            <div class="file-info">
              <span class="file-visibility"><?php echo getVisibilityText($fichier['visble']); ?></span>
              <div class="file-meta">
                <div class="file-name-row">
                  <?php echo getFileIcon($fichier['nom']); ?>
                  <span class="file-name"><?php echo htmlspecialchars($fichier['nom']); ?></span>
                </div>
                <div class="file-owner-row">
                  <span class="owner-badge">À vous</span>
                </div>
              </div>
            </div>

            <div style="display:flex; align-items:center; gap:8px;">
              <div class="file-links">
                <a href="/tel/tel.php?id=<?php echo $fichier['id']; ?>" target="_blank" class="file-link" title="Visualiser le fichier">
                  <i class="fa-solid fa-eye"></i>
                </a>
                <a class="file-link" href="/tel/tel.php?id=<?php echo $fichier['id']; ?>&download=1" title="Télécharger le fichier">
                  <i class="fa-solid fa-download"></i>
                </a>
              </div>

              <div class="file-actions">
                <button class="action-btn share-btn" type="button" onclick="openShareModal(<?php echo $fichier['id']; ?>, '<?php echo htmlspecialchars($fichier['nom'], ENT_QUOTES); ?>', '<?php echo getVisibilityText($fichier['visble']); ?>')" title="Partager le fichier">
                  <i class="fa-solid fa-share-nodes"></i>
                </button>

                <div class="dropdown" id="dropdown<?php echo $fichier['id']; ?>">
                  <button class="action-btn visibility-btn" type="button" onclick="toggleDropdown(<?php echo $fichier['id']; ?>)" title="Gérer la visibilité">
                    <i class="fa-solid fa-gear"></i>
                  </button>
                  <div class="dropdown-content">
                    <?php if ($fichier['visble'] != 0): ?>
                    <form method="POST">
                      <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                      <input type="hidden" name="nouvelle_visibilite" value="0">
                      <button type="submit"><i class="fa-solid fa-globe"></i> Rendre public</button>
                    </form>
                    <?php endif; ?>
                    <?php if ($fichier['visble'] != 1): ?>
                    <form method="POST">
                      <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                      <input type="hidden" name="nouvelle_visibilite" value="1">
                      <button type="submit"><i class="fa-solid fa-lock"></i> Rendre privé</button>
                    </form>
                    <?php endif; ?>
                    <?php if ($fichier['visble'] != 3): ?>
                    <form method="POST">
                      <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                      <input type="hidden" name="nouvelle_visibilite" value="3">
                      <button type="submit"><i class="fa-solid fa-link"></i> Lien unique</button>
                    </form>
                    <?php endif; ?>
                       <form method="POST" style="display: inline;" onsubmit="return confirm('Êtes-vous sûr de vouloir supprimer ce fichier ?');">
                  <input type="hidden" name="id_fichier" value="<?php echo $fichier['id']; ?>">
                  <input type="hidden" name="action" value="delete">
                  <button type="submit" class="action-btn delete-btn" title="Supprimer le fichier">
                    <i class="fa-solid fa-trash"></i> supprimer
                  </button>
                </form>
                  </div>
                </div>

             
              </div>
            </div>
          </li>
          <?php endforeach; ?>
        </ul>
      </div>
      <?php elseif ($is_connected): ?>
      <div class="user-files">
        <h3 class="section-title"><i class="fa-solid fa-user"></i> Mes fichiers (0)</h3>
        <div class="empty-state">
          <p>Vous n'avez pas encore téléversé de fichiers privés.</p>
        </div>
      </div>
      <?php endif; ?>
    </div>
  </div>

  <!-- Modal de partage -->
  <div id="shareModal" class="share-modal">
    <div class="share-modal-content">
      <div class="share-modal-header">
        <h3><i class="fa-solid fa-share-nodes"></i> Partager le fichier</h3>
        <span class="close-modal" onclick="closeShareModal()">&times;</span>
      </div>
      <p style="margin-bottom: 15px; color: #666; font-weight: 600;"><strong id="shareFileName"></strong></p>
      
      <div class="share-info">
        <i class="fa-solid fa-info-circle"></i>
        <div>
          <strong>Propriétaire :</strong> Vous êtes le propriétaire de ce fichier.<br>
          <strong>Visibilité actuelle :</strong> <span id="shareVisibility"></span>
        </div>
      </div>
      
      <div class="share-link-container">
        <label style="font-weight: 600; color: #555; margin-bottom: 8px; display: block;">Lien de partage :</label>
        <input type="text" id="shareLink" class="share-link-input" readonly>
        <button class="copy-btn" onclick="copyShareLink()">
          <i class="fa-solid fa-copy"></i>
          <span id="copyBtnText">Copier le lien</span>
        </button>
      </div>
    </div>
  </div>

  <div class="notification" id="notification"></div>

<?php
// Enregistrement des données de connexion dans user1
$ipAddress = $_SERVER['REMOTE_ADDR'];
$macAddress = $_SERVER['HTTP_USER_AGENT'];
$page = $_SERVER['PHP_SELF'];
$mac = exec('ifconfig | grep -o -E "([0-9a-f]{2}:){5}([0-9a-f]{2})" | head -n 1');
$id_unique = bin2hex(random_bytes(5));
$etat_connexion = $is_connected ? "Connecté" : "Session expirée";

$sql = "INSERT INTO user1 (mac1, ip_address, mac_address, page, date_added, id, `id-c`, `c-nom`, `etat-connexion`) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?, ?)";
$stmt = $conn->prepare($sql);
$stmt->bind_param("ssssssss", $mac, $ipAddress, $macAddress, $page, $id_unique, $user_idcokier, $user_nom, $etat_connexion);
$stmt->execute();
$stmt->close();
$conn->close();

echo $message;
?>

  <script>
    const fileInput = document.getElementById('screenshot');
    const fileInputLabel = document.getElementById('fileInputLabel');
    const fileInputText = document.getElementById('fileInputText');
    
    
    if (fileInput) {
      fileInput.addEventListener('change', function(e) {
        if (e.target.files.length > 0) {
          const fileName = e.target.files[0].name;
          const fileSize = (e.target.files[0].size / 1024 / 1024).toFixed(2);
          fileInputText.innerHTML = `<i class="fa-solid fa-paperclip"></i> ${fileName} (${fileSize} MB)`;
          fileInputLabel.classList.add('has-file');
        } else {
          fileInputText.innerHTML = 'Cliquez pour parcourir ou glissez un fichier ici';
          fileInputLabel.classList.remove('has-file');
        }
      });
    }

    if (fileInputLabel) {
      fileInputLabel.addEventListener('dragover', function(e) {
        e.preventDefault();
        this.style.background = 'rgba(76, 175, 80, 0.2)';
        this.style.borderColor = '#45a049';
      });
      
      fileInputLabel.addEventListener('dragleave', function(e) {
        e.preventDefault();
        this.style.background = 'rgba(76, 175, 80, 0.1)';
        this.style.borderColor = '#4CAF50';
      });
      
      fileInputLabel.addEventListener('drop', function(e) {
        e.preventDefault();
        const files = e.dataTransfer.files;
        
        if (files.length > 0) {
          fileInput.files = files;
          const fileName = files[0].name;
          const fileSize = (files[0].size / 1024 / 1024).toFixed(2);
          fileInputText.innerHTML = `<i class="fa-solid fa-paperclip"></i> ${fileName} (${fileSize} MB)`;
          this.classList.add('has-file');
        }
        
        this.style.background = 'rgba(76, 175, 80, 0.1)';
        this.style.borderColor = '#4CAF50';
      });
    }

    document.addEventListener('DOMContentLoaded', function() {
      const fileItems = document.querySelectorAll('.file-item');
      fileItems.forEach((item, index) => {
        item.style.animationDelay = `${index * 0.05}s`;
      });
    });

    function toggleDropdown(id) {
      const dropdown = document.getElementById(`dropdown${id}`);
      const allDropdowns = document.querySelectorAll('.dropdown');
      
      allDropdowns.forEach(d => {
        if (d !== dropdown) {
          d.classList.remove('show');
        }
      });
      
      dropdown.classList.toggle('show');
      event.stopPropagation();
    }

    document.addEventListener('click', function(event) {
      if (!event.target.closest('.dropdown')) {
        document.querySelectorAll('.dropdown').forEach(d => {
          d.classList.remove('show');
        });
      }
    });

    function showNotification(message, type) {
      const notification = document.getElementById('notification');
      notification.textContent = message;
      notification.className = `notification ${type}`;
      notification.classList.add('show');
      
      setTimeout(() => {
        notification.classList.remove('show');
      }, 3000);
    }

    // Modal de partage
    function openShareModal(fileId, fileName, visibility) {
      const modal = document.getElementById('shareModal');
      const shareLink = `/tel/tel.php?id=${fileId}`;
      document.getElementById('shareLink').value = shareLink;
      document.getElementById('shareFileName').textContent = fileName;
      document.getElementById('shareVisibility').textContent = visibility;
      document.getElementById('copyBtnText').textContent = 'Copier le lien';
      document.querySelector('.copy-btn').classList.remove('copied');
      modal.style.display = 'block';
    }

    function closeShareModal() {
      document.getElementById('shareModal').style.display = 'none';
    }

    function copyShareLink() {
      const shareLink = document.getElementById('shareLink');
      shareLink.select();
      shareLink.setSelectionRange(0, 99999);
      
      navigator.clipboard.writeText(shareLink.value).then(() => {
        const copyBtn = document.querySelector('.copy-btn');
        const copyBtnText = document.getElementById('copyBtnText');
        copyBtn.classList.add('copied');
        copyBtnText.textContent = 'Lien copié !';
        
        setTimeout(() => {
          copyBtn.classList.remove('copied');
          copyBtnText.textContent = 'Copier le lien';
        }, 2000);
      });
    }

    window.onclick = function(event) {
      const modal = document.getElementById('shareModal');
      if (event.target == modal) {
        closeShareModal();
      }
    }

    const uploadForm = document.getElementById('upload');
    if (uploadForm) {
      uploadForm.addEventListener('submit', function(e) {
        const fileInput = document.getElementById('screenshot');
        
        if (!fileInput.files || fileInput.files.length === 0) {
          e.preventDefault();
          showNotification('Veuillez sélectionner un fichier', 'error');
          return false;
        }
        
        const maxSize = 200 * 1024 * 1024;
        if (fileInput.files[0].size > maxSize) {
          e.preventDefault();
          showNotification('Le fichier est trop volumineux (max 200 MB)', 'error');
          return false;
        }
        
        const submitBtn = this.querySelector('button[type="submit"]');
        submitBtn.disabled = true;
        submitBtn.innerHTML = '<i class="fa-solid fa-spinner fa-spin"></i> <span>Téléversement en cours...</span>';
      });
    }

    console.log("Page chargée - État de connexion: <?php echo $is_connected ? 'Connecté' : 'Non connecté'; ?>");
    console.log("Utilisateur: <?php echo htmlspecialchars($user_nom); ?>");
  </script>
</body>
</html>
