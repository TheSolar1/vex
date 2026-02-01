<?php
// Désactiver complètement le buffering de sortie
while (ob_get_level()) {
    ob_end_clean();
}

error_reporting(0);
ini_set('display_errors', 0);

// Header AVANT toute sortie
header('Content-Type: application/json; charset=utf-8');

session_start();
include '/var/www/html/function.php';

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";

$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    echo json_encode([]);
    exit;
}

$query = isset($_GET['q']) ? trim($_GET['q']) : '';

if (strlen($query) < 2) {
    echo json_encode([]);
    exit;
}

$search = "%{$query}%";
$stmt = $conn->prepare("SELECT id, nom FROM login WHERE nom LIKE ? LIMIT 10");

if (!$stmt) {
    echo json_encode([]);
    exit;
}

$stmt->bind_param("s", $search);
$stmt->execute();
$result = $stmt->get_result();

$users = [];
while ($row = $result->fetch_assoc()) {
    $users[] = [
        'id' => (int)$row['id'],
        'nom' => htmlspecialchars($row['nom'], ENT_QUOTES, 'UTF-8')
    ];
}

$stmt->close();
$conn->close();

echo json_encode($users, JSON_UNESCAPED_UNICODE);
exit;
?>