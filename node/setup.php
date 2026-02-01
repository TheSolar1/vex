
<?php
require_once __DIR__ . '/Node.class.php';

header('Content-Type: application/json');

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    http_response_code(405);
    die(json_encode(['error' => 'Method not allowed']));
}

try {
    $node = new Node();
    $input = json_decode(file_get_contents('php://input'), true);
    
    if (!isset($input['node_id']) || !isset($input['host']) || !isset($input['port'])) {
        throw new Exception('Missing node_id, host or port');
    }
    
    $node->addNode($input['node_id'], $input['host'], $input['port']);
    
    echo json_encode(['success' => true, 'message' => 'Node added successfully']);
    
} catch (Exception $e) {
    http_response_code(500);
    echo json_encode(['success' => false, 'error' => $e->getMessage()]);
}
?>