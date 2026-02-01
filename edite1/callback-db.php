
<?php
ini_set('display_errors', 1);
error_reporting(E_ALL);
$logFile = '/tmp/onlyoffice-callback.log';
file_put_contents($logFile,
    "\n=== " . date('Y-m-d H:i:s') . " ===\n" .
    "POST data: " . file_get_contents('php://input') . "\n" .
    "GET params: " . print_r($_GET, true) . "\n",
    FILE_APPEND
);
error_log("CALLBACK: Script démarré");
require_once __DIR__ . '/config.php';

// Fonction pour logger
function logCallback($message) {
    $date = date('Y-m-d H:M:S');
    error_log("[$date] $message");
}

// Récupérer le JSON
$input = file_get_contents('php://input');
logCallback("Callback reçu: " . $input);

if (empty($input)) {
    logCallback("ERREUR: Callback vide");
    http_response_code(400);
    exit('{"error":1}');
}

$data = json_decode($input, true);

if (json_last_error() !== JSON_ERROR_NONE) {
    logCallback("ERREUR JSON: " . json_last_error_msg());
    http_response_code(400);
    exit('{"error":1}');
}

// Vérifier le token
//if (isset($data['token']) && !empty(ONLYOFFICE_SECRET)) {
    // Vérification JWT ici si nécessaire
//}

$doc_id = isset($_GET['id']) ? (int)$_GET['id'] : 0;
$status = $data['status'] ?? 0;

logCallback("Document ID: $doc_id, Status: $status");

// Gérer les différents statuts
switch ($status) {
    case 1: // Document en cours d'édition
        logCallback("Document $doc_id en cours d'édition");
        break;

    case 2: // Document prêt à être sauvegardé
    case 3: // Erreur de sauvegarde
        if (isset($data['url'])) {
            try {
                $downloadUrl = $data['url'];
                $newContent = file_get_contents($downloadUrl);

                if ($newContent === false) {
                    throw new Exception("Impossible de télécharger le document");
                }

                // Sauvegarder dans la base de données
                $conn = new mysqli("localhost", "orsql", "FQOvVV6IEE4RNt3N29ia", "user");
                $conn->set_charset("utf8mb4");
                $stmt = $conn->prepare("UPDATE fichiers SET fichier=?, date=NOW() WHERE id=?");
                 $stmt->bind_param("si", $newContent, $doc_id);

                if ($stmt->execute()) {
                    logCallback("Document $doc_id sauvegardé avec succès");
                } else {
                    throw new Exception("Erreur SQL: " . $stmt->error);
                }

                $stmt->close();
                $conn->close();

            } catch (Exception $e) {
                logCallback("ERREUR lors de la sauvegarde: " . $e->getMessage());
                http_response_code(500);
                exit('{"error":1}');
            }
        }
        break;

    case 4: // Document fermé sans modification
        logCallback("Document $doc_id fermé sans modification");
        break;

    case 6: // Document en cours de conversion
    case 7: // Erreur de conversion
        logCallback("Status $status pour document $doc_id");
        break;
}

// Réponse de succès avec headers explicites
header('Content-Type: application/json');
http_response_code(200);
echo json_encode(['error' => 0]);
exit;
?>
