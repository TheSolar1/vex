<?php
// upload-document.php - Upload de fichiers existants vers ONLYOFFICE
require_once __DIR__ . '/config.php';

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("Erreur de connexion à la base de données");
}

session_start();

// Vérification utilisateur
$user_id = null;
if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    $stmt = $conn->prepare("SELECT email FROM loginc WHERE idcokier=? AND pc=? AND navi=?");
    $stmt->bind_param("sss", $cookie_value, $_SERVER['REMOTE_ADDR'], $_SERVER['HTTP_USER_AGENT']);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        $stmt2 = $conn->prepare("SELECT id FROM login WHERE email=?");
        $stmt2->bind_param("s", $row['email']);
        $stmt2->execute();
        $result2 = $stmt2->get_result();
        
        if ($result2->num_rows == 1) {
            $user_row = $result2->fetch_assoc();
            $user_id = (int)$user_row['id'];
        }
        $stmt2->close();
    }
    $stmt->close();
}

if (!$user_id) {
    header('Location: /login/login.php');
    exit;
}

// Traitement de l'upload
$success = false;
$error = null;
$new_doc_id = null;

if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_FILES['document'])) {
    $file = $_FILES['document'];
    
    // Vérification des erreurs d'upload
    if ($file['error'] !== UPLOAD_ERR_OK) {
        $error = "Erreur lors de l'upload du fichier";
    } else {
        $filename = basename($file['name']);
        $extension = strtolower(pathinfo($filename, PATHINFO_EXTENSION));
        
        // Extensions autorisées
        $allowed = ['docx', 'doc', 'xlsx', 'xls', 'pptx', 'ppt', 'odt', 'ods', 'odp', 'txt', 'rtf', 'csv', 'pdf'];
        
        if (!in_array($extension, $allowed)) {
            $error = "Type de fichier non autorisé. Extensions acceptées : " . implode(', ', $allowed);
        } else {
            // Lire le contenu du fichier
            $file_content = file_get_contents($file['tmp_name']);
            $file_size = $file['size'];
            
            // Type MIME
            $mime_types = [
                'docx' => 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
                'doc' => 'application/msword',
                'xlsx' => 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
                'xls' => 'application/vnd.ms-excel',
                'pptx' => 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
                'ppt' => 'application/vnd.ms-powerpoint',
                'odt' => 'application/vnd.oasis.opendocument.text',
                'ods' => 'application/vnd.oasis.opendocument.spreadsheet',
                'odp' => 'application/vnd.oasis.opendocument.presentation',
                'txt' => 'text/plain',
                'rtf' => 'application/rtf',
                'csv' => 'text/csv',
                'pdf' => 'application/pdf'
            ];
            
            $mime = $mime_types[$extension] ?? 'application/octet-stream';
            
            // Insérer dans la DB
            $stmt = $conn->prepare("INSERT INTO fichiers (nom, fichier, type_fichier, taille, visble, id_utilisateur, date) VALUES (?, ?, ?, ?, 'public', ?, NOW())");
            
            $null = NULL;
            $stmt->bind_param("sssii", $filename, $null, $mime, $file_size, $user_id);
            $stmt->send_long_data(1, $file_content);
            
            if ($stmt->execute()) {
                $new_doc_id = $conn->insert_id;
                $success = true;
            } else {
                $error = "Erreur lors de la sauvegarde : " . $stmt->error;
            }
            
            $stmt->close();
        }
    }
}

$conn->close();

// Redirection si succès
if ($success && $new_doc_id) {
    header('Location: editor-db.php?id=' . $new_doc_id);
    exit;
}
?>
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Importer un document</title>
    <script src="/js/fa-local.js" defer></script>
    <style>
        * { box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
        }
        .container {
            max-width: 600px;
            margin: 50px auto;
        }
        .card {
            background: white;
            border-radius: 12px;
            padding: 30px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.2);
        }
        h1 {
            margin-top: 0;
            color: #333;
            font-size: 24px;
            margin-bottom: 24px;
        }
        .upload-area {
            border: 2px dashed #667eea;
            border-radius: 8px;
            padding: 40px;
            text-align: center;
            cursor: pointer;
            transition: all 0.3s;
            margin-bottom: 20px;
        }
        .upload-area:hover {
            background: #f8f9ff;
            border-color: #5568d3;
        }
        .upload-area.dragover {
            background: #f0f2ff;
            border-color: #5568d3;
        }
        .upload-icon {
            font-size: 48px;
            color: #667eea;
            margin-bottom: 15px;
        }
        .upload-text {
            color: #666;
            margin-bottom: 10px;
        }
        .upload-hint {
            font-size: 12px;
            color: #999;
        }
        input[type="file"] {
            display: none;
        }
        .file-info {
            background: #f5f5f5;
            padding: 15px;
            border-radius: 6px;
            margin-bottom: 20px;
            display: none;
        }
        .file-info.active {
            display: block;
        }
        .file-name {
            font-weight: 600;
            color: #333;
            margin-bottom: 5px;
        }
        .file-size {
            font-size: 12px;
            color: #999;
        }
        .btn-group {
            display: flex;
            gap: 10px;
        }
        .btn {
            flex: 1;
            padding: 12px 24px;
            border: none;
            border-radius: 6px;
            font-size: 16px;
            cursor: pointer;
            transition: background 0.3s;
            text-decoration: none;
            text-align: center;
        }
        .btn-primary {
            background: #667eea;
            color: white;
        }
        .btn-primary:hover {
            background: #5568d3;
        }
        .btn-primary:disabled {
            background: #ccc;
            cursor: not-allowed;
        }
        .btn-secondary {
            background: #e0e0e0;
            color: #333;
        }
        .btn-secondary:hover {
            background: #d0d0d0;
        }
        .error {
            background: #fee;
            color: #c00;
            padding: 12px;
            border-radius: 6px;
            margin-bottom: 20px;
        }
        .allowed-formats {
            font-size: 12px;
            color: #999;
            margin-top: 15px;
            text-align: center;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>📤 Importer un document</h1>
            
            <?php if ($error): ?>
                <div class="error"><?php echo htmlspecialchars($error); ?></div>
            <?php endif; ?>
            
            <form method="POST" enctype="multipart/form-data" id="uploadForm">
                <div class="upload-area" id="uploadArea">
                    <div class="upload-icon">
                        <i class="fas fa-cloud-upload-alt"></i>
                    </div>
                    <div class="upload-text">
                        Cliquez ou glissez-déposez votre fichier ici
                    </div>
                    <div class="upload-hint">
                        Taille maximale : 50 MB
                    </div>
                </div>
                
                <input type="file" name="document" id="fileInput" accept=".docx,.doc,.xlsx,.xls,.pptx,.ppt,.odt,.ods,.odp,.txt,.rtf,.csv,.pdf">
                
                <div class="file-info" id="fileInfo">
                    <div class="file-name" id="fileName"></div>
                    <div class="file-size" id="fileSize"></div>
                </div>
                
                <div class="btn-group">
                    <a href="index-db.php" class="btn btn-secondary">Annuler</a>
                    <button type="submit" class="btn btn-primary" id="submitBtn" disabled>Importer</button>
                </div>
                
                <div class="allowed-formats">
                    Formats acceptés : Word (.docx, .doc), Excel (.xlsx, .xls), PowerPoint (.pptx, .ppt), 
                    OpenDocument (.odt, .ods, .odp), Texte (.txt, .rtf), CSV, PDF
                </div>
            </form>
        </div>
    </div>
    
    <script>
        const uploadArea = document.getElementById('uploadArea');
        const fileInput = document.getElementById('fileInput');
        const fileInfo = document.getElementById('fileInfo');
        const fileName = document.getElementById('fileName');
        const fileSize = document.getElementById('fileSize');
        const submitBtn = document.getElementById('submitBtn');
        
        // Clic sur la zone d'upload
        uploadArea.addEventListener('click', () => {
            fileInput.click();
        });
        
        // Sélection de fichier
        fileInput.addEventListener('change', (e) => {
            handleFile(e.target.files[0]);
        });
        
        // Drag & drop
        uploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            uploadArea.classList.add('dragover');
        });
        
        uploadArea.addEventListener('dragleave', () => {
            uploadArea.classList.remove('dragover');
        });
        
        uploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            uploadArea.classList.remove('dragover');
            
            const file = e.dataTransfer.files[0];
            if (file) {
                fileInput.files = e.dataTransfer.files;
                handleFile(file);
            }
        });
        
        function handleFile(file) {
            if (!file) return;
            
            fileName.textContent = '📄 ' + file.name;
            fileSize.textContent = formatFileSize(file.size);
            fileInfo.classList.add('active');
            submitBtn.disabled = false;
        }
        
        function formatFileSize(bytes) {
            if (bytes === 0) return '0 Bytes';
            const k = 1024;
            const sizes = ['Bytes', 'KB', 'MB', 'GB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
        }
    </script>
</body>
</html>
