<?php
// callback.php - Reçoit les sauvegardes d'ONLYOFFICE
error_reporting(E_ALL);
ini_set('display_errors', 0); // Pas d'affichage pour ne pas casser le JSON

require_once __DIR__ . '/config.php';
require_once __DIR__ . '/vendor/autoload.php';

use \Firebase\JWT\JWT;
use \Firebase\JWT\Key;

// Log des requêtes
$logFile = __DIR__ . '/callback.log';
$input = file_get_contents('php://input');
file_put_contents($logFile, date('Y-m-d H:i:s') . " - Reçu: " . $input . "\n", FILE_APPEND);

// Décoder les données
$data = json_decode($input, true);

if (!$data) {
    file_put_contents($logFile, "Erreur: données invalides\n", FILE_APPEND);
    die(json_encode(['error' => 1]));
}

// Vérifier le JWT si présent
$token = $_GET['token'] ?? ($data['token'] ?? null);
if ($token) {
    try {
        JWT::decode($token, new Key(ONLYOFFICE_SECRET, 'HS256'));
    } catch (Exception $e) {
        file_put_contents($logFile, "Erreur JWT: " . $e->getMessage() . "\n", FILE_APPEND);
    }
}

$status = $data['status'] ?? 0;
$filename = $_GET['file'] ?? 'document.docx';
$filepath = DOCUMENTS_DIR . $filename;

file_put_contents($logFile, "Status: $status, Fichier: $filename\n", FILE_APPEND);

switch ($status) {
    case 1: // Édition en cours
        echo json_encode(['error' => 0]);
        break;
        
    case 2: // Document prêt à être sauvegardé
    case 3: // Erreur lors de la sauvegarde (on réessaye)
        if (isset($data['url'])) {
            $downloadUrl = $data['url'];
            file_put_contents($logFile, "Téléchargement depuis: $downloadUrl\n", FILE_APPEND);
            
            $newContent = file_get_contents($downloadUrl);
            
            if ($newContent !== false) {
                file_put_contents($filepath, $newContent);
                file_put_contents($logFile, "Document sauvegardé: $filepath\n", FILE_APPEND);
                echo json_encode(['error' => 0]);
            } else {
                file_put_contents($logFile, "Erreur de téléchargement\n", FILE_APPEND);
                echo json_encode(['error' => 1]);
            }
        } else {
            echo json_encode(['error' => 0]);
        }
        break;
        
    case 4: // Document fermé sans modifications
    case 6: // En cours de sauvegarde
    case 7: // Erreur lors de la conversion
    default:
        echo json_encode(['error' => 0]);
        break;
}
?>
