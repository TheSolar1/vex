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
    
    if (!isset($input['target']) || !isset($input['message'])) {
        throw new Exception('Missing target or message');
    }
    
    $result = $node->sendMessage($input['target'], $input['message']);
    
    echo json_encode($result);
    
} catch (Exception $e) {
    http_response_code(500);
    echo json_encode(['success' => false, 'error' => $e->getMessage()]);
}
?>