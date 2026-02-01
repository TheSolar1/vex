<?php
require_once __DIR__ . '/config.php';

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die(json_encode(['error' => 'Connexion échouée']));
}

// Récupérer l'ID utilisateur depuis la session/cookie
session_start();
$user_id = isset($_SESSION['user_id']) ? $_SESSION['user_id'] : null;

if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_FILES['file'])) {
    $file = $_FILES['file'];
    $nom = $file['name'];
    $type = $file['type'];
    $taille = $file['size'];
    $contenu = file_get_contents($file['tmp_name']);
    
    $stmt = $conn->prepare("INSERT INTO fichiers (nom, fichier, type_fichier, taille, visble, id_utilisateur, date) VALUES (?, ?, ?, ?, 'public', ?, NOW())");
    $stmt->bind_param("ssbis", $nom, $contenu, $type, $taille, $user_id);
    $stmt->send_long_data(1, $contenu);
    
    if ($stmt->execute()) {
        echo json_encode(['success' => true, 'id' => $conn->insert_id]);
    } else {
        echo json_encode(['error' => 'Erreur sauvegarde']);
    }
    
    $stmt->close();
}

$conn->close();
?>
