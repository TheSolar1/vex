<?php
include '/var/www/html/function.php';
session_start();
error_reporting(E_ALL);
ini_set('display_errors', 1);
// Connexion à la base de données
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("La connexion a échoué : " . $conn->connect_error);
}

if (!$conn->set_charset("utf8mb4")) {
    die("Erreur UTF-8");
}

// Variables utilisateur
$user_privilege = 0;
$user_id = 0;
$is_connected = false;
$user_email = "";
$user_nom = "";

// Verification de la connexion
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
                
                $stmt2 = $conn->prepare("SELECT * FROM login WHERE email=?");
                $stmt2->bind_param("s", $user_email);
                $stmt2->execute();
                $result2 = $stmt2->get_result();
                
                if ($result2->num_rows == 1) {
                    $row2 = $result2->fetch_assoc();
                    $user_id = (int)$row2['id'];
                    $user_privilege = (int)$row2['privilege'];
                    $is_connected = true;
                }
                $stmt2->close();
            }
        }
    }
    $stmt->close();
}

// Récupération de l'ID du fichier
if (!isset($_GET['id']) || empty($_GET['id'])) {
    die("Fichier non trouvé");
}

$file_id = intval($_GET['id']);

// Récupération du fichier
$stmt = $conn->prepare("SELECT id, nom, type_fichier, fichier, visble, id_utilisateur FROM fichiers WHERE id = ?");
$stmt->bind_param("i", $file_id);
$stmt->execute();
$result = $stmt->get_result();

if ($result->num_rows !== 1) {
    die("Fichier introuvable");
}

$fichier = $result->fetch_assoc();

// VÉRIFICATION DES DROITS D'ACCÈS - SÉCURISÉ
$can_access = false;
$is_owner = ($is_connected && $fichier['id_utilisateur'] == $user_id);
$is_admin = ($is_connected && $user_privilege >= 1);

if ($fichier['visble'] == 0) {
    // Public : tout le monde peut accéder
    $can_access = true;
} elseif ($fichier['visble'] == 1) {
    // Privé : seulement le propriétaire ou admin
    if ($is_owner || $is_admin) {
        $can_access = true;
    } else {
        die("Accès refusé : vous devez être connecté et propriétaire de ce fichier pour y accéder.");
    }
} elseif ($fichier['visble'] == 3) {
    // Lien unique : tout le monde avec le lien
    $can_access = true;
}

if (!$can_access) {
    die("Accès refusé à ce fichier");
}

// Si download est demandé, télécharger le fichier
if (isset($_GET['download'])) {
    header('Content-Type: application/octet-stream');
    header('Content-Disposition: attachment; filename="' . $fichier['nom'] . '"');
    header('Content-Length: ' . strlen($fichier['fichier']));
    echo $fichier['fichier'];
    exit;
}

// Récupérer l'icône appropriée
$icon_map = fcichier();
$extension = strtolower(pathinfo($fichier['nom'], PATHINFO_EXTENSION));
$icon_html = isset($icon_map[$extension]) ? $icon_map[$extension] : $icon_map['default'];

$is_custom_icon = strpos($icon_html, '<img') !== false;

// Déterminer le type de fichier
$type = $fichier['type_fichier'];
$nom = htmlspecialchars($fichier['nom']);
$contenu = $fichier['fichier'];

$all_image_extensions = ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tif', 'tiff', 'heic', 'heif', 'avif', 'jfif', 'pjpeg', 'pjp'];
$is_image = strpos($type, 'image/') === 0 || in_array($extension, $all_image_extensions);
$is_pdf = $type === 'application/pdf';
$is_text = strpos($type, 'text/') === 0 || in_array($extension, ['txt', 'log', 'md']);
$is_video = strpos($type, 'video/') === 0;
$is_audio = strpos($type, 'audio/') === 0;
$is_code = in_array($extension, ['php', 'html', 'css', 'js', 'json', 'xml', 'sql', 'py', 'java', 'c', 'cpp', 'h', 'hpp', 'cs', 'rb', 'go', 'rs', 'swift', 'kt']);
$is_3d = in_array($extension, ['obj', 'mtl', 'stl', 'fbx', 'gltf', 'glb', 'ply', '3ds', 'dae', 'blend']);
$is_gcode = in_array($extension, ['gcode', 'nc', 'cnc', 'g', 'ngc']);
$is_excel = in_array($extension, ['xls', 'xlsx', 'ods', 'csv']);
$is_word = in_array($extension, ['doc', 'docx', 'odt']);
$is_powerpoint = in_array($extension, ['ppt', 'pptx', 'odp']);

// VÉRIFIER SI ON PEUT ÉDITER/SUPPRIMER (seulement propriétaire ou admin)
$can_edit = ($is_owner || $is_admin);
?>
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title><?php echo $nom; ?> - Exodrive</title>
    <link rel="icon" href="/vex.png" type="image/png">
    <script src="https://kit.fontawesome.com/c20cede3fa.js" crossorigin="anonymous"></script>
    
    <!-- Three.js pour visualisation 3D -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/loaders/OBJLoader.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/loaders/STLLoader.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/loaders/PLYLoader.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
    
    <!-- SheetJS pour Excel/ODS -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/xlsx/0.18.5/xlsx.full.min.js"></script>
    
    <!-- Mammoth.js pour DOCX -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/mammoth/1.6.0/mammoth.browser.min.js"></script>
    
    <!-- PDF.js -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.min.js"></script>
    
    <!-- Prism.js pour coloration syntaxique -->
    <link href="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/themes/prism-tomorrow.min.css" rel="stylesheet" />
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/prism.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/components/prism-php.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/components/prism-javascript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/components/prism-css.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/components/prism-sql.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/components/prism-python.min.js"></script>
    <?php
// Récupérer le thème de l'utilisateur
$isDark = false;
if ($is_connected && function_exists('getUserPreferences')) {
    $prefs = getUserPreferences($user_id, $conn);
    $isDark = ($prefs && isset($prefs['teme']) && $prefs['teme'] === 1);
    
$stmt->close();
$conn->close();

}
?>
<style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f0f2f5;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            color: #1c1e21;
        }

        .header {
            background: linear-gradient(135deg, #4caf50 0%, #45a049 100%);
            padding: 15px 30px;
            display: flex;
            justify-content: space-between;
            align-items: center;
            color: white;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }

        .header h1 {
            font-size: 1.5rem;
            display: flex;
            align-items: center;
            gap: 10px;
            font-weight: 600;
        }

        .actions {
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
        }

        .btn {
            padding: 10px 20px;
            border: none;
            border-radius: 6px;
            cursor: pointer;
            font-weight: 600;
            transition: all 0.3s;
            text-decoration: none;
            display: inline-flex;
            align-items: center;
            gap: 8px;
            font-size: 14px;
        }

        .btn-primary {
            background: white;
            color: #4caf50;
        }

        .btn-primary:hover {
            background: #f0f0f0;
            transform: translateY(-2px);
        }

        .btn-secondary {
            background: rgba(255, 255, 255, 0.2);
            color: white;
            border: 2px solid white;
        }

        .btn-secondary:hover {
            background: rgba(255, 255, 255, 0.3);
        }

        .btn-danger {
            background: #f44336;
            color: white;
        }

        .btn-danger:hover {
            background: #d32f2f;
            transform: translateY(-2px);
        }

        .btn-share {
            background: #2196F3;
            color: white;
        }

        .btn-share:hover {
            background: #1976D2;
            transform: translateY(-2px);
        }

        .viewer {
            flex: 1;
            display: flex;
            justify-content: center;
            align-items: center;
            padding: 20px;
        }

        .viewer-content {
            background: white;
            border-radius: 12px;
            padding: 20px;
            max-width: 95%;
            max-height: 85vh;
            overflow: auto;
            box-shadow: 0 1px 4px rgba(0,0,0,0.15);
        }

        .viewer-content img {
            max-width: 100%;
            height: auto;
            display: block;
            border-radius: 8px;
        }

        .viewer-content video,
        .viewer-content audio {
            max-width: 100%;
            border-radius: 8px;
        }

        .viewer-content iframe {
            width: 100%;
            min-height: 70vh;
            border: none;
            border-radius: 8px;
        }

        .viewer-content pre {
            background: #2d2d2d;
            padding: 20px;
            border-radius: 8px;
            overflow-x: auto;
            margin: 0;
        }

        .viewer-content pre code {
            font-family: 'Courier New', monospace;
            font-size: 14px;
            line-height: 1.5;
            color: #e4e6eb;
        }

        .download-only {
            text-align: center;
            padding: 40px;
        }

        .download-only i {
            font-size: 4rem;
            color: #4caf50;
            margin-bottom: 20px;
        }

        .download-only h2 {
            color: #333;
            margin-bottom: 10px;
        }

        .download-only p {
            color: #666;
            margin-bottom: 20px;
        }

        .file-info {
            background: #f7f7f7;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
            border-left: 4px solid #4caf50;
        }

        .file-info p {
            margin: 5px 0;
            color: #666;
            font-size: 14px;
        }

        .file-info strong {
            color: #333;
        }

        #viewer-3d {
            width: 100%;
            height: 600px;
            border-radius: 8px;
            background: #1a1a1a;
        }

        #excel-viewer {
            overflow-x: auto;
        }

        #excel-viewer table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 10px;
        }

        #excel-viewer th,
        #excel-viewer td {
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
        }

        #excel-viewer th {
            background-color: #4caf50;
            color: white;
            font-weight: 600;
        }

        #excel-viewer tr:nth-child(even) {
            background-color: #f9f9f9;
        }

        .gcode-info {
            background: #4caf50;
            color: white;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 15px;
            display: flex;
            align-items: center;
            gap: 10px;
        }

        .gcode-info h3 {
            margin: 0;
            font-size: 1.1rem;
        }

        .controls-info {
            background: #f7f7f7;
            padding: 10px 15px;
            border-radius: 8px;
            margin-bottom: 15px;
            font-size: 0.9em;
            color: #666;
        }

        #docx-viewer {
            padding: 20px;
            background: white;
            border-radius: 8px;
            line-height: 1.6;
        }

        #docx-viewer img {
            max-width: 100%;
            height: auto;
        }

        .loading {
            text-align: center;
            padding: 40px;
            color: #4caf50;
        }

        .loading i {
            font-size: 3rem;
            animation: spin 2s linear infinite;
        }

        .share-modal {
            display: none;
            position: fixed;
            z-index: 3000;
            left: 0;
            top: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0,0,0,0.5);
            animation: fadeIn 0.3s ease;
        }

        .share-modal-content {
            background-color: white;
            margin: 10% auto;
            padding: 30px;
            border-radius: 12px;
            max-width: 500px;
            box-shadow: 0 8px 32px rgba(0,0,0,0.3);
            animation: slideInDown 0.3s ease;
        }

        .share-modal-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 20px;
            border-bottom: 2px solid #4caf50;
            padding-bottom: 10px;
        }

        .share-modal-header h3 {
            color: #4caf50;
            font-size: 1.5rem;
        }

        .close-modal {
            color: #aaa;
            font-size: 28px;
            font-weight: bold;
            cursor: pointer;
            transition: color 0.3s;
        }

        .close-modal:hover {
            color: #000;
        }

        .share-link-container {
            background: #f9f9f9;
            padding: 15px;
            border-radius: 8px;
            margin: 15px 0;
            border: 1px solid #ddd;
        }

        .share-link-input {
            width: 100%;
            padding: 10px;
            border: 1px solid #ddd;
            border-radius: 5px;
            font-size: 14px;
            margin-bottom: 10px;
        }

        .copy-btn {
            width: 100%;
            padding: 12px;
            background: #4caf50;
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

        @keyframes spin {
            from { transform: rotate(0deg); }
            to { transform: rotate(360deg); }
        }

        @keyframes fadeIn {
            from { opacity: 0; }
            to { opacity: 1; }
        }

        @keyframes slideInDown {
            from { opacity: 0; transform: translateY(-50px); }
            to { opacity: 1; transform: translateY(0); }
        }

        .icone-fichier {
            width: 30px;
            height: 30px;
            vertical-align: middle;
            object-fit: contain;
        }

        @media (max-width: 768px) {
            .header {
                flex-direction: column;
                gap: 10px;
                text-align: center;
            }

            .actions {
                flex-direction: column;
                width: 100%;
            }

            .btn {
                width: 100%;
                justify-content: center;
            }

            #viewer-3d {
                height: 400px;
            }

            .share-modal-content {
                margin: 20% auto;
                padding: 20px;
                max-width: 90%;
            }
        }

        /* MODE SOMBRE */
        <?php if ($isDark): ?>
        body {
            background: #18191a;
            color: #e4e6eb;
        }

        .header {
            background: linear-gradient(135deg, #2d5f2e 0%, #234d23 100%);
        }

        .viewer-content {
            background: #242526;
        }

        .file-info {
            background: #3a3b3c;
            border-left-color: #2d5f2e;
        }

        .file-info p {
            color: #b0b3b8;
        }

        .file-info strong {
            color: #e4e6eb;
        }

        .download-only h2 {
            color: #e4e6eb;
        }

        .download-only p {
            color: #b0b3b8;
        }

        .download-only i {
            color: #2d5f2e;
        }

        #excel-viewer th,
        #excel-viewer td {
            border-color: #3a3b3c;
            color: #e4e6eb;
        }

        #excel-viewer th {
            background-color: #2d5f2e;
        }

        #excel-viewer tr:nth-child(even) {
            background-color: #3a3b3c;
        }

        .gcode-info {
            background: #2d5f2e;
        }

        .controls-info {
            background: #3a3b3c;
            color: #b0b3b8;
        }

        #docx-viewer {
            background: #242526;
            color: #e4e6eb;
        }

        .loading {
            color: #2d5f2e;
        }

        .share-modal-content {
            background-color: #242526;
        }

        .share-modal-header {
            border-bottom-color: #2d5f2e;
        }

        .share-modal-header h3 {
            color: #2d5f2e;
        }

        .close-modal {
            color: #b0b3b8;
        }

        .close-modal:hover {
            color: #e4e6eb;
        }

        .share-link-container {
            background: #3a3b3c;
            border-color: #3a3b3c;
        }

        .share-link-input {
            background: #242526;
            color: #e4e6eb;
            border-color: #3a3b3c;
        }

        .copy-btn {
            background: #2d5f2e;
        }

        .copy-btn:hover {
            background: #234d23;
        }

        .copy-btn.copied {
            background: #1976D2;
        }
        <?php endif; ?>
    </style>
</head>
<body>
    <div class="header">
        <h1>
            <?php 
            if ($is_custom_icon) {
                $clean_icon = str_replace('">', '', $icon_html);
                echo $clean_icon . '" />';
            } else {
                echo '<i class="fa-solid ' . $icon_html . '"></i>';
            }
            ?>
            <?php echo $nom; ?>
        </h1>
        <div class="actions">
            <a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary">
                <i class="fa-solid fa-download"></i> Télécharger
            </a>
            
            <?php if ($can_edit && $is_owner): ?>
            <button class="btn btn-share" onclick="openShareModal()">
                <i class="fa-solid fa-share-nodes"></i> Partager
            </button>
            <?php endif; ?>
            
            <?php if ($can_edit): ?>
            <form method="POST" action="supp_fichier.php" style="display: inline;" onsubmit="return confirm('Êtes-vous sûr de vouloir supprimer ce fichier ?');">
                <input type="hidden" name="id_fichier" value="<?php echo $file_id; ?>">
                <input type="hidden" name="action" value="delete">
                <button type="submit" class="btn btn-danger">
                    <i class="fa-solid fa-trash"></i> Supprimer
                </button>
            </form>
            <?php endif; ?>
            
            <a href="index.php" class="btn btn-secondary">
                <i class="fa-solid fa-arrow-left"></i> Retour
            </a>
        </div>
    </div>

    <div class="viewer">
        <div class="viewer-content">
            <div class="file-info">
                <p><strong>Nom :</strong> <?php echo $nom; ?></p>
                <p><strong>Type :</strong> <?php echo htmlspecialchars($type); ?></p>
                <p><strong>Taille :</strong> <?php echo number_format(strlen($contenu) / 1024, 2); ?> KB</p>
                <?php if ($is_owner): ?>
                <p><strong>Statut :</strong> <span style="color: #4CAF50; font-weight: bold;">Vous êtes le propriétaire</span></p>
                <?php endif; ?>
            </div>

            <?php if ($is_image): ?>
                <?php
                if (in_array($extension, ['tif', 'tiff'])) {
                    echo '<div class="download-only">';
                    echo '<i class="fa-solid fa-image"></i>';
                    echo '<h2>Image TIFF</h2>';
                    echo '<p>Les fichiers TIFF ne peuvent pas être affichés directement dans le navigateur.</p>';
                    echo '<p>Veuillez télécharger le fichier pour le visualiser avec une application compatible.</p>';
                    echo '<a href="?id=' . $file_id . '&download=1" class="btn btn-primary" style="display: inline-flex;">';
                    echo '<i class="fa-solid fa-download"></i> Télécharger';
                    echo '</a>';
                    echo '</div>';
                } else {
                    echo '<img src="data:' . $type . ';base64,' . base64_encode($contenu) . '" alt="' . $nom . '" style="max-width: 100%; height: auto;">';
                }
                ?>
            
            <?php elseif ($is_pdf): ?>
                <iframe src="data:application/pdf;base64,<?php echo base64_encode($contenu); ?>"></iframe>
            
            <?php elseif ($is_video): ?>
                <video controls>
                    <source src="data:<?php echo $type; ?>;base64,<?php echo base64_encode($contenu); ?>" type="<?php echo $type; ?>">
                    Votre navigateur ne supporte pas la lecture vidéo.
                </video>
            
            <?php elseif ($is_audio): ?>
                <audio controls style="width: 100%;">
                    <source src="data:<?php echo $type; ?>;base64,<?php echo base64_encode($contenu); ?>" type="<?php echo $type; ?>">
                    Votre navigateur ne supporte pas la lecture audio.
                </audio>
            
            <?php elseif ($is_code): ?>
                <pre><code class="language-<?php echo $extension; ?>"><?php echo htmlspecialchars($contenu); ?></code></pre>
            
            <?php elseif ($is_word && $extension === 'docx'): ?>
                <div class="loading" id="docx-loading">
                    <i class="fa-solid fa-spinner fa-spin"></i>
                    <p>Chargement du document...</p>
                </div>
                <div id="docx-viewer" style="display: none;"></div>
                <script>
                (async function() {
                    try {
                        const arrayBuffer = Uint8Array.from(atob('<?php echo base64_encode($contenu); ?>'), c => c.charCodeAt(0)).buffer;
                        const result = await mammoth.convertToHtml({arrayBuffer: arrayBuffer});
                        document.getElementById('docx-loading').style.display = 'none';
                        document.getElementById('docx-viewer').style.display = 'block';
                        document.getElementById('docx-viewer').innerHTML = result.value;
                    } catch(e) {
                        document.getElementById('docx-loading').innerHTML = '<div class="download-only"><i class="fa-solid fa-file-word"></i><h2>Erreur de chargement</h2><p>Impossible de prévisualiser ce document Word</p><a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary"><i class="fa-solid fa-download"></i> Télécharger</a></div>';
                    }
                })();
                </script>
            
            <?php elseif ($is_word): ?>
                <div class="download-only">
                    <i class="fa-solid fa-file-word"></i>
                    <h2>Document Word</h2>
                    <p>La prévisualisation n'est disponible que pour les fichiers .docx</p>
                    <a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary">
                        <i class="fa-solid fa-download"></i> Télécharger
                    </a>
                </div>
            
            <?php elseif ($is_powerpoint): ?>
                <div class="download-only">
                    <i class="fa-solid fa-file-powerpoint"></i>
                    <h2>Présentation PowerPoint</h2>
                    <p>Téléchargez le fichier pour l'ouvrir avec PowerPoint ou un logiciel compatible</p>
                    <a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary">
                        <i class="fa-solid fa-download"></i> Télécharger
                    </a>
                </div>
            
            <?php elseif ($is_gcode): ?>
                <div class="gcode-info">
                    <i class="fa-solid fa-cube"></i>
                    <div>
                        <h3>Visualisation 3D du G-Code</h3>
                        <p>Parcours d'impression / usinage</p>
                    </div>
                </div>
                <div class="controls-info">
                    <i class="fa-solid fa-info-circle"></i> <strong>Contrôles :</strong> Clic gauche pour tourner | Molette pour zoomer | Clic droit pour déplacer
                </div>
                <div id="viewer-3d"></div>
                <script>
                (function() {
                    try {
                        const gcodeContent = atob('<?php echo base64_encode($contenu); ?>');
                        const lines = gcodeContent.split('\n');
                        
                        let points = [];
                        let currentPos = {x: 0, y: 0, z: 0};
                        
                        lines.forEach(line => {
                            const trimmed = line.trim().split(';')[0];
                            if (!trimmed) return;
                            
                            if (trimmed.startsWith('G0') || trimmed.startsWith('G1')) {
                                let newPos = {...currentPos};
                                let hasE = false;
                                
                                const xMatch = trimmed.match(/X([-\d.]+)/);
                                const yMatch = trimmed.match(/Y([-\d.]+)/);
                                const zMatch = trimmed.match(/Z([-\d.]+)/);
                                const eMatch = trimmed.match(/E([-\d.]+)/);
                                
                                if (xMatch) newPos.x = parseFloat(xMatch[1]);
                                if (yMatch) newPos.y = parseFloat(yMatch[1]);
                                if (zMatch) newPos.z = parseFloat(zMatch[1]);
                                if (eMatch) hasE = true;
                                
                                if (hasE && trimmed.startsWith('G1')) {
                                    points.push({
                                        start: {...currentPos},
                                        end: {...newPos},
                                        type: 'extrude'
                                    });
                                } else {
                                    points.push({
                                        start: {...currentPos},
                                        end: {...newPos},
                                        type: 'travel'
                                    });
                                }
                                
                                currentPos = newPos;
                            }
                        });
                        
                        if (points.length === 0) {
                            throw new Error("Aucun mouvement détecté");
                        }
                        
                        const container = document.getElementById('viewer-3d');
                        const scene = new THREE.Scene();
                        scene.background = new THREE.Color(0x1a1a1a);
                        
                        const camera = new THREE.PerspectiveCamera(75, container.offsetWidth / 600, 0.1, 10000);
                        const renderer = new THREE.WebGLRenderer({ antialias: true });
                        renderer.setSize(container.offsetWidth, 600);
                        container.appendChild(renderer.domElement);
                        
                        const controls = new THREE.OrbitControls(camera, renderer.domElement);
                        controls.enableDamping = true;
                        controls.dampingFactor = 0.05;
                        
                        const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
                        scene.add(ambientLight);
                        const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
                        directionalLight.position.set(100, 100, 100);
                        scene.add(directionalLight);
                        
                        const extrudeMaterial = new THREE.LineBasicMaterial({ color: 0x4CAF50, linewidth: 2 });
                        const travelMaterial = new THREE.LineBasicMaterial({ color: 0xff5722, linewidth: 1 });
                        
                        points.forEach(segment => {
                            const geometry = new THREE.BufferGeometry();
                            const positions = new Float32Array([
                                segment.start.x, segment.start.z, segment.start.y,
                                segment.end.x, segment.end.z, segment.end.y
                            ]);
                            geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
                            
                            const material = segment.type === 'extrude' ? extrudeMaterial : travelMaterial;
                            const line = new THREE.Line(geometry, material);
                            scene.add(line);
                        });
                        
                        const box = new THREE.Box3().setFromObject(scene);
                        const center = box.getCenter(new THREE.Vector3());
                        const size = box.getSize(new THREE.Vector3());
                        
                        const maxDim = Math.max(size.x, size.y, size.z);
                        const fov = camera.fov * (Math.PI / 180);
                        let cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2));
                        cameraZ *= 1.5;
                        
                        camera.position.set(center.x + cameraZ, center.y + cameraZ, center.z + cameraZ);
                        camera.lookAt(center);
                        controls.target.copy(center);
                        
                        const gridSize = Math.ceil(maxDim / 10) * 10;
                        const grid = new THREE.GridHelper(gridSize, 20, 0x444444, 0x222222);
                        grid.position.y = 0;
                        scene.add(grid);
                        
                        function animate() {
                            requestAnimationFrame(animate);
                            controls.update();
                            renderer.render(scene, camera);
                        }
                        animate();
                        
                        window.addEventListener('resize', () => {
                            const width = container.offsetWidth;
                            camera.aspect = width / 600;
                            camera.updateProjectionMatrix();
                            renderer.setSize(width, 600);
                        });
                        
                    } catch(e) {
                        document.getElementById('viewer-3d').innerHTML = '<div class="download-only"><i class="fa-solid fa-file-code"></i><h2>Erreur de chargement</h2><p>Impossible de visualiser le G-Code: ' + e.message + '</p></div>';
                    }
                })();
                </script>
            
            <?php elseif ($is_3d): ?>
                <?php if (in_array($extension, ['obj', 'stl', 'ply'])): ?>
                    <div class="gcode-info">
                        <i class="fa-solid fa-cube"></i>
                        <div>
                            <h3>Modèle 3D <?php echo strtoupper($extension); ?></h3>
                        </div>
                    </div>
                    <div class="controls-info">
                        <i class="fa-solid fa-info-circle"></i> <strong>Contrôles :</strong> Clic gauche pour tourner | Molette pour zoomer | Clic droit pour déplacer
                    </div>
                    <div id="viewer-3d"></div>
                    <script>
                    (function() {
                        try {
                            const container = document.getElementById('viewer-3d');
                            const scene = new THREE.Scene();
                            scene.background = new THREE.Color(0x1a1a1a);
                            
                            const camera = new THREE.PerspectiveCamera(75, container.offsetWidth / 600, 0.1, 1000);
                            const renderer = new THREE.WebGLRenderer({ antialias: true });
                            renderer.setSize(container.offsetWidth, 600);
                            renderer.shadowMap.enabled = true;
                            renderer.shadowMap.type = THREE.PCFSoftShadowMap;
                            container.appendChild(renderer.domElement);
                            
                            const controls = new THREE.OrbitControls(camera, renderer.domElement);
                            controls.enableDamping = true;
                            controls.dampingFactor = 0.05;
                            
                            const ambientLight = new THREE.AmbientLight(0xffffff, 0.5);
                            scene.add(ambientLight);
                            
                            const directionalLight1 = new THREE.DirectionalLight(0xffffff, 0.7);
                            directionalLight1.position.set(5, 10, 5);
                            scene.add(directionalLight1);
                            
                            <?php if ($extension === 'obj'): ?>
                            const loader = new THREE.OBJLoader();
                            const objData = atob('<?php echo base64_encode($contenu); ?>');
                            const object = loader.parse(objData);
                            
                            object.traverse(function(child) {
                                if (child instanceof THREE.Mesh) {
                                    child.material = new THREE.MeshPhongMaterial({ 
                                        color: 0x4CAF50,
                                        shininess: 30
                                    });
                                }
                            });
                            
                            const box = new THREE.Box3().setFromObject(object);
                            const center = box.getCenter(new THREE.Vector3());
                            const size = box.getSize(new THREE.Vector3());
                            
                            object.position.sub(center);
                            const maxDim = Math.max(size.x, size.y, size.z);
                            const scale = 2 / maxDim;
                            object.scale.multiplyScalar(scale);
                            
                            scene.add(object);
                            
                            <?php elseif ($extension === 'stl'): ?>
                            const loader = new THREE.STLLoader();
                            const stlData = atob('<?php echo base64_encode($contenu); ?>');
                            const geometry = loader.parse(stlData);
                            geometry.computeVertexNormals();
                            
                            const material = new THREE.MeshPhongMaterial({ 
                                color: 0x4CAF50,
                                shininess: 30
                            });
                            const mesh = new THREE.Mesh(geometry, material);
                            
                            geometry.computeBoundingBox();
                            const box = geometry.boundingBox;
                            const center = box.getCenter(new THREE.Vector3());
                            const size = box.getSize(new THREE.Vector3());
                            
                            mesh.position.sub(center);
                            const maxDim = Math.max(size.x, size.y, size.z);
                            const scale = 2 / maxDim;
                            mesh.scale.multiplyScalar(scale);
                            
                            scene.add(mesh);
                            
                            <?php elseif ($extension === 'ply'): ?>
                            const loader = new THREE.PLYLoader();
                            const plyData = atob('<?php echo base64_encode($contenu); ?>');
                            const geometry = loader.parse(plyData);
                            geometry.computeVertexNormals();
                            
                            const material = new THREE.MeshPhongMaterial({ 
                                color: 0x4CAF50,
                                shininess: 30,
                                vertexColors: geometry.attributes.color !== undefined
                            });
                            const mesh = new THREE.Mesh(geometry, material);
                            
                            geometry.computeBoundingBox();
                            const box = geometry.boundingBox;
                            const center = box.getCenter(new THREE.Vector3());
                            const size = box.getSize(new THREE.Vector3());
                            
                            mesh.position.sub(center);
                            const maxDim = Math.max(size.x, size.y, size.z);
                            const scale = 2 / maxDim;
                            mesh.scale.multiplyScalar(scale);
                            
                            scene.add(mesh);
                            <?php endif; ?>
                            
                            camera.position.set(3, 3, 3);
                            camera.lookAt(0, 0, 0);
                            
                            const gridHelper = new THREE.GridHelper(5, 20, 0x444444, 0x222222);
                            scene.add(gridHelper);
                            
                            function animate() {
                                requestAnimationFrame(animate);
                                controls.update();
                                renderer.render(scene, camera);
                            }
                            animate();
                            
                            window.addEventListener('resize', () => {
                                const width = container.offsetWidth;
                                camera.aspect = width / 600;
                                camera.updateProjectionMatrix();
                                renderer.setSize(width, 600);
                            });
                            
                        } catch(e) {
                            document.getElementById('viewer-3d').innerHTML = '<div class="download-only"><i class="fa-solid fa-cube"></i><h2>Erreur de chargement</h2><p>Impossible de charger le modèle 3D</p></div>';
                        }
                    })();
                    </script>
                
                <?php else: ?>
                    <div class="download-only">
                        <i class="fa-solid fa-cube"></i>
                        <h2>Modèle 3D</h2>
                        <p>Le format <?php echo strtoupper($extension); ?> n'est pas encore supporté.</p>
                        <a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary" style="display: inline-flex;">
                            <i class="fa-solid fa-download"></i> Télécharger
                        </a>
                    </div>
                <?php endif; ?>
            
            <?php elseif ($is_excel): ?>
                <div id="excel-viewer"></div>
                <script>
                try {
                    const excelData = atob('<?php echo base64_encode($contenu); ?>');
                    const workbook = XLSX.read(excelData, { type: 'binary' });
                    const firstSheet = workbook.Sheets[workbook.SheetNames[0]];
                    const html = XLSX.utils.sheet_to_html(firstSheet);
                    document.getElementById('excel-viewer').innerHTML = html;
                } catch(e) {
                    document.getElementById('excel-viewer').innerHTML = '<div class="download-only"><i class="fa-solid fa-file-excel"></i><h2>Erreur de chargement</h2><p>Impossible de charger le fichier Excel</p></div>';
                }
                </script>
            
            <?php elseif ($is_text): ?>
                <pre><code><?php echo htmlspecialchars($contenu); ?></code></pre>
            
            <?php else: ?>
                <div class="download-only">
                    <i class="fa-solid fa-file-archive"></i>
                    <h2>Aperçu non disponible</h2>
                    <p>Ce type de fichier ne peut pas être visualisé directement.</p>
                    <a href="?id=<?php echo $file_id; ?>&download=1" class="btn btn-primary" style="display: inline-flex;">
                        <i class="fa-solid fa-download"></i> Télécharger maintenant
                    </a>
                </div>
            <?php endif; ?>
        </div>
    </div>

    <!-- Modal de partage (seulement si propriétaire) -->
    <?php if ($can_edit && $is_owner): ?>
    <div id="shareModal" class="share-modal">
        <div class="share-modal-content">
            <div class="share-modal-header">
                <h3><i class="fa-solid fa-share-nodes"></i> Partager le fichier</h3>
                <span class="close-modal" onclick="closeShareModal()">&times;</span>
            </div>
            <p style="margin-bottom: 15px; color: #666;"><strong><?php echo $nom; ?></strong></p>
            <div class="share-link-container">
                <label style="font-weight: 600; color: #555; margin-bottom: 8px; display: block;">Lien de partage :</label>
                <input type="text" id="shareLink" class="share-link-input" value="/tel/tel.php?id=<?php echo $file_id; ?>" readonly>
                <button class="copy-btn" onclick="copyShareLink()">
                    <i class="fa-solid fa-copy"></i>
                    <span id="copyBtnText">Copier le lien</span>
                </button>
            </div>
        </div>
    </div>
    <?php endif; ?>

    <?php footer(); ?>

    <script>
        // Modal de partage
        function openShareModal() {
            document.getElementById('shareModal').style.display = 'block';
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
    </script>
</body>
</html>