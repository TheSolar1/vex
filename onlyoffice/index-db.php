<?php
require_once __DIR__ . '/../login/functions.php'; // Votre fichier de fonctions

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

// Applications ONLYOFFICE
$apps = [
    ['icon' => 'fa-file-word', 'label' => 'Documents', 'url' => '/onlyoffice/index-db.php'],
    ['icon' => 'fa-hard-drive', 'label' => 'Exodrive', 'url' => '/tel/'],
    ['icon' => 'fa-envelope', 'label' => 'Mail', 'url' => '#']
];

// Afficher la navigation (avec vérification admin automatique)
displayNavigation($conn, $apps);
?>
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Mes Documents</title>
    <script src="/js/fa-local.js" defer></script>
    <style>
        body { margin: 70px 20px 20px 20px; font-family: Arial, sans-serif; background: #f5f5f5; }
        .documents-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 20px; margin-top: 20px; }
        .doc-card { background: white; padding: 20px; border-radius: 8px; text-align: center; cursor: pointer; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .doc-card:hover { box-shadow: 0 4px 8px rgba(0,0,0,0.2); }
        .doc-icon { font-size: 48px; color: #ff6f3d; margin-bottom: 10px; }
        .btn { background: #ff6f3d; color: white; padding: 12px 24px; border: none; border-radius: 6px; cursor: pointer; }
    </style>
</head>
<body>
    <h1>📄 Mes Documents</h1>
    
    <button class="btn" onclick="window.location.href='create-doc.php'">+ Créer un document</button>
    
    <div class="documents-grid">
        <?php
        // Récupérer l'ID utilisateur (même logique que votre code)
        $user_id = null;
        if (isset($_COOKIE['connexion_cookie'])) {
            // ... votre code de vérification ...
        }
        
        if ($user_id) {
            $stmt = $conn->prepare("SELECT id, nom, type_fichier, date FROM fichiers WHERE id_utilisateur=? ORDER BY date DESC");
            $stmt->bind_param("i", $user_id);
            $stmt->execute();
            $result = $stmt->get_result();
            
            while ($row = $result->fetch_assoc()) {
                $icon_map = fcichier(); // Votre fonction d'icônes
                $ext = pathinfo($row['nom'], PATHINFO_EXTENSION);
                $icon = $icon_map[$ext] ?? $icon_map['default'];
                
                echo '<div class="doc-card" onclick="window.location.href=\'editor-db.php?id=' . $row['id'] . '\'">';
                echo '<div class="doc-icon"><i class="fas ' . $icon . '"></i></div>';
                echo '<p>' . htmlspecialchars($row['nom']) . '</p>';
                echo '<small>' . date('d/m/Y', strtotime($row['date'])) . '</small>';
                echo '</div>';
            }
            $stmt->close();
        }
        $conn->close();
        ?>
    </div>
</body>
</html>