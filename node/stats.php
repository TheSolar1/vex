<?php
require_once __DIR__ . '/Node.class.php';

header('Content-Type: application/json');

try {
    $node = new Node();
    echo json_encode($node->getStats());
    
} catch (Exception $e) {
    http_response_code(500);
    echo json_encode(['error' => $e->getMessage()]);
}
?>
