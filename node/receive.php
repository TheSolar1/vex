<?php
require_once __DIR__ . '/Node.class.php';

header('Content-Type: application/json');

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    http_response_code(405);
    die(json_encode(['error' => 'Method not allowed']));
}

try {
    $node = new Node();
    $encrypted = file_get_contents('php://input');
    
    if (empty($encrypted)) {
        throw new Exception('No data received');
    }
    
    $result = $node->receiveMessage($encrypted);
    
    http_response_code($result['success'] ? 200 : 400);
    echo json_encode($result);
    
} catch (Exception $e) {
    http_response_code(500);
    echo json_encode(['success' => false, 'error' => $e->getMessage()]);
}
?>