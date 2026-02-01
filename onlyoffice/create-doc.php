<?php
// editor-db.php - Éditeur ONLYOFFICE avec support documents publics (id_utilisateur=0)
error_reporting(E_ALL);
ini_set('display_errors', 1);

require_once __DIR__ . '/config.php';
require_once __DIR__ . '/vendor/autoload.php';

use \Firebase\JWT\JWT;

// Connexion DB
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("Erreur de connexion à la base de données");
}

session_start();

// Vérification de connexion utilisateur
$user_id = null;
$user_name = "Utilisateur";
$user_privilege = 10;

if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    $stmt = $conn->prepare("SELECT email FROM loginc WHERE idcokier=? AND pc=? AND navi=?");
    $stmt->bind_param("sss", $cookie_value, $_SERVER['REMOTE_ADDR'], $_SERVER['HTTP_USER_AGENT']);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        $stmt2 = $conn->prepare("SELECT id, nom, privilege FROM login WHERE email=?");
        $stmt2->bind_param("s", $row['email']);
        $stmt2->execute();
        $result2 = $stmt2->get_result();
        
        if ($result2->num_rows == 1) {
            $user_row = $result2->fetch_assoc();
            $user_id = (int)$user_row['id'];
            $user_name = $user_row['nom'];
            $user_privilege = (int)$user_row['privilege'];
        }
        $stmt2->close();
    }
    $stmt->close();
}

// Récupérer l'ID du document
$doc_id = isset($_GET['id']) ? (int)$_GET['id'] : 0;

if ($doc_id === 0) {
    die('
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="UTF-8">
        <title>Erreur</title>
        <style>
            body { font-family: Arial, sans-serif; padding: 40px; text-align: center; background: #f5f5f5; }
            .error-box { background: white; padding: 30px; border-radius: 8px; max-width: 500px; margin: 0 auto; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
            .error-icon { font-size: 48px; color: #ff6f3d; margin-bottom: 20px; }
            h1 { color: #333; }
            .btn { background: #ff6f3d; color: white; padding: 12px 24px; border: none; border-radius: 6px; text-decoration: none; display: inline-block; margin-top: 20px; }
            .btn:hover { background: #e55a2b; }
        </style>
    </head>
    <body>
        <div class="error-box">
            <div class="error-icon">⚠️</div>
            <h1>Document non spécifié</h1>
            <p>Aucun document n\'a été sélectionné. Veuillez retourner à la liste des documents.</p>
            <a href="index-db.php" class="btn">← Retour à mes documents</a>
        </div>
    </body>
    </html>
    ');
}

// Récupérer le document depuis la DB
// Autoriser l'accès si :
// - Le document appartient à l'utilisateur connecté (id_utilisateur = $user_id)
// - OU le document est public (id_utilisateur = 0)
$stmt = $conn->prepare("SELECT nom, fichier, type_fichier, id_utilisateur FROM fichiers WHERE id=?");
$stmt->bind_param("i", $doc_id);
$stmt->execute();
$result = $stmt->get_result();

if ($result->num_rows !== 1) {
    $stmt->close();
    $conn->close();
    die('
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="UTF-8">
        <title>Erreur</title>
        <style>
            body { font-family: Arial, sans-serif; padding: 40px; text-align: center; background: #f5f5f5; }
            .error-box { background: white; padding: 30px; border-radius: 8px; max-width: 500px; margin: 0 auto; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
            .error-icon { font-size: 48px; color: #ff6f3d; margin-bottom: 20px; }
            h1 { color: #333; }
            .btn { background: #ff6f3d; color: white; padding: 12px 24px; border: none; border-radius: 6px; text-decoration: none; display: inline-block; margin-top: 20px; }
            .btn:hover { background: #e55a2b; }
        </style>
    </head>
    <body>
        <div class="error-box">
            <div class="error-icon">🔍</div>
            <h1>Document introuvable</h1>
            <p>Ce document n\'existe pas dans la base de données.</p>
            <a href="index-db.php" class="btn">← Retour à mes documents</a>
        </div>
    </body>
    </html>
    ');
}

$doc = $result->fetch_assoc();
$doc_owner_id = (int)$doc['id_utilisateur'];
$filename = $doc['nom'];
$fileExtension = pathinfo($filename, PATHINFO_EXTENSION);
$documentKey = 'doc_' . $doc_id . '_' . md5($filename . time());

$stmt->close();

// Vérifier les permissions
$is_public = ($doc_owner_id === 0);
$is_owner = ($user_id && $user_id === $doc_owner_id);
$can_view = $is_public || $is_owner;

if (!$can_view) {
    $conn->close();
    die('
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="UTF-8">
        <title>Erreur</title>
        <style>
            body { font-family: Arial, sans-serif; padding: 40px; text-align: center; background: #f5f5f5; }
            .error-box { background: white; padding: 30px; border-radius: 8px; max-width: 500px; margin: 0 auto; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
            .error-icon { font-size: 48px; color: #ff6f3d; margin-bottom: 20px; }
            h1 { color: #333; }
            .btn { background: #ff6f3d; color: white; padding: 12px 24px; border: none; border-radius: 6px; text-decoration: none; display: inline-block; margin-top: 20px; }
            .btn:hover { background: #e55a2b; }
        </style>
    </head>
    <body>
        <div class="error-box">
            <div class="error-icon">🔒</div>
            <h1>Accès refusé</h1>
            <p>Vous n\'avez pas les droits pour accéder à ce document.</p>
            <a href="index-db.php" class="btn">← Retour à mes documents</a>
        </div>
    </body>
    </html>
    ');
}

// Pour les documents publics, lecture seule sauf pour les admins
$can_edit = $is_owner || ($is_public && $user_privilege <= 6);

// Créer un fichier temporaire pour ONLYOFFICE
$temp_file = DOCUMENTS_DIR . 'temp_' . $doc_id . '_' . $filename;
file_put_contents($temp_file, $doc['fichier']);

$documentUrl = BASE_URL . '/documents/temp_' . $doc_id . '_' . $filename;

// Type de document
$documentTypes = [
    'docx' => 'word', 'doc' => 'word', 'odt' => 'word', 'txt' => 'word', 'rtf' => 'word',
    'xlsx' => 'cell', 'xls' => 'cell', 'ods' => 'cell', 'csv' => 'cell',
    'pptx' => 'slide', 'ppt' => 'slide', 'odp' => 'slide'
];
$documentType = $documentTypes[$fileExtension] ?? 'word';

// Configuration
$config = [
    "document" => [
        "fileType" => $fileExtension,
        "key" => $documentKey,
        "title" => $filename,
        "url" => $documentUrl,
        "permissions" => [
            "edit" => $can_edit,
            "download" => true,
            "print" => true
        ]
    ],
    "documentType" => $documentType,
    "editorConfig" => [
        "mode" => $can_edit ? "edit" : "view",
        "lang" => "fr",
        "callbackUrl" => $can_edit ? BASE_URL . '/callback-db.php?id=' . $doc_id . '&key=' . $documentKey : null,
        "user" => [
            "id" => (string)($user_id ?? 'guest'),
            "name" => $user_name
        ],
        "customization" => [
            "forcesave" => $can_edit,
            "autosave" => $can_edit,
            "goback" => [
                "url" => BASE_URL . '/index-db.php',
                "text" => "Retour à la liste"
            ]
        ]
    ]
];

$token = JWT::encode($config, ONLYOFFICE_SECRET, 'HS256');
$conn->close();
?>
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title><?php echo htmlspecialchars($filename); ?></title>
    <script src="<?php echo ONLYOFFICE_URL_PUBLIC; ?>/web-apps/apps/api/documents/api.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        html, body { height: 100%; width: 100%; overflow: hidden; font-family: Arial, sans-serif; }
        #placeholder { position: absolute; top: 0; left: 0; right: 0; bottom: 0; width: 100%; height: 100%; }
        #loading { position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; z-index: 9999; background: white; padding: 30px; border-radius: 10px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
        .spinner { border: 4px solid #f3f3f3; border-top: 4px solid #ff6f3d; border-radius: 50%; width: 50px; height: 50px; animation: spin 1s linear infinite; margin: 0 auto 20px; }
        @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
        <?php if ($is_public && !$can_edit): ?>
        .public-banner { position: fixed; top: 0; left: 0; right: 0; background: #2196f3; color: white; padding: 10px; text-align: center; z-index: 10000; font-size: 14px; }
        #placeholder { top: 40px; height: calc(100% - 40px); }
        <?php endif; ?>
    </style>
</head>
<body>
    <?php if ($is_public && !$can_edit): ?>
    <div class="public-banner">
        📄 Document public en lecture seule
    </div>
    <?php endif; ?>
    
    <div id="loading">
        <div class="spinner"></div>
        <p>Chargement de <?php echo htmlspecialchars($filename); ?>...</p>
        <?php if (!$can_edit): ?>
        <p style="color: #666; font-size: 12px; margin-top: 10px;">Mode lecture seule</p>
        <?php endif; ?>
    </div>
    <div id="placeholder"></div>
    
    <script>
        var config = <?php echo json_encode($config); ?>;
        config.token = "<?php echo $token; ?>";
        config.height = "100%";
        config.width = "100%";
        
        config.events = {
            "onDocumentReady": function() {
                document.getElementById('loading').style.display = 'none';
            },
            "onError": function(event) {
                console.error("Erreur:", event);
                alert("Erreur lors du chargement du document");
            }
        };
        
        var docEditor = new DocsAPI.DocEditor("placeholder", config);
    </script>
</body>
</html>