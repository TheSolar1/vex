<?php
require_once __DIR__ . '/config.php';
require_once __DIR__ . '/vendor/autoload.php';

use \Firebase\JWT\JWT;
use \Firebase\JWT\Key;

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

$input = file_get_contents('php://input');
$data = json_decode($input, true);

$doc_id = isset($_GET['id']) ? (int)$_GET['id'] : 0;
$status = $data['status'] ?? 0;

file_put_contents(__DIR__ . '/callback.log', date('Y-m-d H:i:s') . " - Doc ID: $doc_id, Status: $status\n", FILE_APPEND);

if ($status == 2 || $status == 3) {
    if (isset($data['url'])) {
        $newContent = file_get_contents($data['url']);
        
        $stmt = $conn->prepare("UPDATE fichiers SET fichier=?, date=NOW() WHERE id=?");
        $stmt->bind_param("bi", $null, $doc_id);
        $stmt->send_long_data(0, $newContent);
        $stmt->execute();
        $stmt->close();
        
        file_put_contents(__DIR__ . '/callback.log', "Document $doc_id sauvegardé\n", FILE_APPEND);
    }
}

$conn->close();
echo json_encode(['error' => 0]);
?>
