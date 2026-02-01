<?php
// ============================================
// Node.class.php - Classe principale
// ============================================

class Node {
    private $nodeId;
    private $knownNodes = [];
    private $bandwidth = 0;
    private $memory = 0;
    private $requestCount = 0;
    private $startTime;
    private $connectionType = 'fiber';
    private $lastPingCheck = 0;
    private $privateKey = 'crilstal_kibeur_2025_PRIVATE'; // Clé privée (seul le créateur l'a)
    private $publicKey = 'crilstal_kibeur_2025_PUBLIC'; // Clé publique (tous les nœuds l'ont)
    private $messageHistory = []; // Historique anti-boucle
    
    public function __construct() {
        $this->nodeId = 'node_' . gethostname() . '_' . getmypid();
        $this->startTime = microtime(true);
        $this->loadKnownNodes();
        $this->detectConnectionType();
        $this->autoPing(); // Ping automatique à chaque requête
    }
    
    // Ping automatique (appelé à chaque requête)
    private function autoPing() {
        $now = time();
        if ($now - $this->lastPingCheck >= 10) {
            $this->pingNodes();
            $this->lastPingCheck = $now;
        }
    }
    
    private function detectConnectionType() {
        $start = microtime(true);
        @fsockopen('8.8.8.8', 80, $errno, $errstr, 1);
        $latency = round((microtime(true) - $start) * 1000);
        $this->connectionType = ($latency < 50) ? 'fiber' : 'satellite';
    }
    
    private function loadKnownNodes() {
        if (file_exists(__DIR__ . '/nodes.dat')) {
            $data = @file_get_contents(__DIR__ . '/nodes.dat');
            if ($data) {
                $this->knownNodes = json_decode($this->decrypt($data), true) ?: [];
            }
        }
    }
    
    private function saveKnownNodes() {
        $encrypted = $this->encrypt(json_encode($this->knownNodes));
        @file_put_contents(__DIR__ . '/nodes.dat', $encrypted);
    }
    
    private function encrypt($data) {
        $key = 'k3yS3cr3tP2P2025';
        $encrypted = '';
        $keyLen = strlen($key);
        for ($i = 0; $i < strlen($data); $i++) {
            $encrypted .= chr(ord($data[$i]) ^ ord($key[$i % $keyLen]));
        }
        return base64_encode($encrypted);
    }
    
    private function decrypt($data) {
        $key = 'k3yS3cr3tP2P2025';
        $data = base64_decode($data);
        $decrypted = '';
        $keyLen = strlen($key);
        for ($i = 0; $i < strlen($data); $i++) {
            $decrypted .= chr(ord($data[$i]) ^ ord($key[$i % $keyLen]));
        }
        return $decrypted;
    }
    
    // Envoyer un message à un nœud
    public function sendMessage($targetNodeId, $data) {
        $this->requestCount++;
        $this->updateMetrics();
        
        if (isset($this->knownNodes[$targetNodeId])) {
            return $this->transmit($targetNodeId, $data);
        } else {
            return $this->distribute($targetNodeId, $data);
        }
    }
    
    private function transmit($nodeId, $data) {
        if (!isset($this->knownNodes[$nodeId]) || !$this->knownNodes[$nodeId]['active']) {
            return ['success' => false, 'error' => 'Node not available'];
        }
        
        // Ajouter les infos du nœud émetteur pour l'auto-découverte
        $message = [
            'from' => $this->nodeId,
            'from_host' => $_SERVER['SERVER_NAME'] ?? gethostname(),
            'from_port' => $_SERVER['SERVER_PORT'] ?? 80,
            'to' => $nodeId,
            'data' => $data,
            'timestamp' => time()
        ];
        
        $encrypted = $this->encrypt(json_encode($message));
        $node = $this->knownNodes[$nodeId];
        
        $ch = curl_init("http://{$node['host']}:{$node['port']}/node/receive.php");
        curl_setopt($ch, CURLOPT_POST, 1);
        curl_setopt($ch, CURLOPT_POSTFIELDS, $encrypted);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
        curl_setopt($ch, CURLOPT_TIMEOUT, 2);
        curl_setopt($ch, CURLOPT_HTTPHEADER, ['Content-Type: application/octet-stream']);
        
        $result = curl_exec($ch);
        $httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);
        
        return ['success' => ($httpCode === 200), 'response' => $result];
    }
    
    private function distribute($targetNodeId, $data) {
        foreach ($this->knownNodes as $nodeId => $node) {
            if ($node['active']) {
                $result = $this->transmit($nodeId, [
                    'type' => 'search',
                    'target' => $targetNodeId,
                    'original_data' => $data
                ]);
                if ($result['success']) {
                    return $result;
                }
            }
        }
        return ['success' => false, 'error' => 'Target node not found in network'];
    }
    
    // Recevoir un message
    public function receiveMessage($encryptedData) {
        $data = json_decode($this->decrypt($encryptedData), true);
        
        if (!$data) {
            return ['success' => false, 'error' => 'Invalid data'];
        }
        
        // Auto-découverte : ajouter le nœud émetteur s'il est inconnu
        if (isset($data['from']) && !isset($this->knownNodes[$data['from']])) {
            if (isset($data['from_host']) && isset($data['from_port'])) {
                $this->addNode($data['from'], $data['from_host'], $data['from_port']);
                $this->logMessage("Nouveau nœud découvert: {$data['from']}");
            }
        }
        
        // Message pour ce nœud
        if ($data['to'] === $this->nodeId) {
            return $this->processMessage($data);
        }
        
        // Recherche d'un nœud
        if (isset($data['type']) && $data['type'] === 'search') {
            if (isset($this->knownNodes[$data['target']])) {
                return $this->transmit($data['target'], $data['original_data']);
            }
        }
        
        // Annonce de présence d'un nouveau nœud
        if (isset($data['type']) && $data['type'] === 'announce') {
            $this->addNode($data['node_id'], $data['host'], $data['port']);
            $this->logMessage("Nœud annoncé: {$data['node_id']}");
            return ['success' => true, 'message' => 'Node registered'];
        }
        
        // Message relayé : propager si pas déjà traité ET signature valide
        if (isset($data['type']) && $data['type'] === 'relay') {
            // VÉRIFIER LA SIGNATURE AVEC LA CLÉ PUBLIQUE
            if (!$this->verifyMessageIntegrity($data)) {
                $this->logMessage("ALERTE: Message avec signature invalide rejeté!");
                return ['success' => false, 'error' => 'Invalid signature - message may be altered'];
            }
            
            $messageId = $data['message_id'];
            
            // Anti-boucle : vérifier si déjà traité
            if (in_array($messageId, $this->messageHistory)) {
                return ['success' => true, 'message' => 'Already processed'];
            }
            
            // Ajouter à l'historique
            $this->messageHistory[] = $messageId;
            if (count($this->messageHistory) > 1000) {
                array_shift($this->messageHistory);
            }
            
            // Propager aux autres nœuds (message vérifié et intact)
            foreach ($this->knownNodes as $nodeId => $node) {
                if ($nodeId !== $data['signed_content']['sender'] && $node['active']) {
                    $this->transmit($nodeId, $data);
                }
            }
            
            $this->logMessage("Message VÉRIFIÉ et propagé: $messageId");
            return [
                'success' => true,
                'message' => 'Verified and relayed',
                'content' => $data['signed_content']['message'],
                'signature_valid' => true
            ];
        }
        
        return ['success' => false, 'error' => 'Message not for this node'];
    }
    
    private function processMessage($data) {
        // Répondre automatiquement
        $response = [
            'status' => 'received',
            'node' => $this->nodeId,
            'timestamp' => time(),
            'original_message' => $data['data']
        ];
        
        $this->logMessage("Message reçu de {$data['from']}");
        
        return ['success' => true, 'response' => $response];
    }
    
    private function updateMetrics() {
        $this->bandwidth = strlen(json_encode($this->knownNodes));
        $this->memory = memory_get_usage(true);
        
        $uptime = microtime(true) - $this->startTime;
        $load = $uptime > 0 ? ($this->requestCount / $uptime) : 0;
        
        if ($load > 0.9) {
            $this->redistributeLoad();
        }
    }
    
    private function redistributeLoad() {
        $minLoad = PHP_INT_MAX;
        $targetNode = null;
        
        foreach ($this->knownNodes as $id => $node) {
            if ($node['active'] && isset($node['load']) && $node['load'] < $minLoad) {
                $minLoad = $node['load'];
                $targetNode = $id;
            }
        }
        
        if ($targetNode) {
            $info = [
                'type' => 'connection_info',
                'connection_type' => $this->connectionType,
                'bandwidth' => $this->bandwidth,
                'requests' => $this->requestCount,
                'load' => $minLoad
            ];
            $this->transmit($targetNode, $info);
        }
    }
    
    public function addNode($nodeId, $host, $port) {
        $this->knownNodes[$nodeId] = [
            'host' => $host,
            'port' => $port,
            'lastPing' => time(),
            'load' => 0,
            'active' => true
        ];
        $this->saveKnownNodes();
        
        // Annoncer ce nœud à tous les autres nœuds connus
        $this->announceToNetwork($nodeId, $host, $port);
    }
    
    // Annoncer un nouveau nœud à tout le réseau
    private function announceToNetwork($newNodeId, $newHost, $newPort) {
        foreach ($this->knownNodes as $id => $node) {
            if ($id !== $newNodeId && $node['active']) {
                $announcement = [
                    'type' => 'announce',
                    'node_id' => $newNodeId,
                    'host' => $newHost,
                    'port' => $newPort
                ];
                
                $encrypted = $this->encrypt(json_encode([
                    'from' => $this->nodeId,
                    'from_host' => $_SERVER['SERVER_NAME'] ?? gethostname(),
                    'from_port' => $_SERVER['SERVER_PORT'] ?? 80,
                    'to' => $id,
                    'data' => $announcement,
                    'timestamp' => time()
                ]));
                
                $ch = curl_init("http://{$node['host']}:{$node['port']}/node/receive.php");
                curl_setopt($ch, CURLOPT_POST, 1);
                curl_setopt($ch, CURLOPT_POSTFIELDS, $encrypted);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_TIMEOUT, 1);
                curl_setopt($ch, CURLOPT_HTTPHEADER, ['Content-Type: application/octet-stream']);
                curl_exec($ch);
                curl_close($ch);
            }
        }
    }
    
    private function pingNodes() {
        foreach ($this->knownNodes as $id => &$node) {
            $alive = @fsockopen($node['host'], $node['port'], $errno, $errstr, 1);
            $node['active'] = ($alive !== false);
            $node['lastPing'] = time();
            if ($alive) fclose($alive);
        }
        $this->saveKnownNodes();
    }
    
    public function getStats() {
        return [
            'node_id' => $this->nodeId,
            'known_nodes' => count($this->knownNodes),
            'active_nodes' => count(array_filter($this->knownNodes, fn($n) => $n['active'])),
            'bandwidth' => $this->bandwidth . ' bytes',
            'memory' => round($this->memory / 1024 / 1024, 2) . ' MB',
            'requests' => $this->requestCount,
            'uptime' => round(microtime(true) - $this->startTime, 2) . 's',
            'connection_type' => $this->connectionType,
            'nodes' => $this->knownNodes
        ];
    }
    
    private function logMessage($message) {
        @file_put_contents(__DIR__ . '/node.log', date('[Y-m-d H:i:s] ') . $message . "\n", FILE_APPEND);
    }
    
    // Vérifier la clé privée
    private function verifyPrivateKey($key) {
        return hash_equals($this->privateKey, $key);
    }
    
    // Signer un message avec la clé privée (seul le créateur peut faire ça)
    private function signMessage($message) {
        $messageString = json_encode($message);
        return hash_hmac('sha256', $messageString, $this->privateKey);
    }
    
    // Vérifier la signature avec la clé publique (tous les nœuds peuvent faire ça)
    private function verifySignature($message, $signature) {
        $messageString = json_encode($message);
        $expectedSignature = hash_hmac('sha256', $messageString, $this->publicKey);
        return hash_equals($expectedSignature, $signature);
    }
    
    // Vérifier l'intégrité du message (pas altéré)
    private function verifyMessageIntegrity($data) {
        if (!isset($data['signature']) || !isset($data['signed_content'])) {
            return false;
        }
        
        return $this->verifySignature($data['signed_content'], $data['signature']);
    }
    
    // Mettre à jour le site avec clé privée
    public function updateSite($key, $files) {
        if (!$this->verifyPrivateKey($key)) {
            return ['success' => false, 'error' => 'Invalid private key'];
        }
        
        $content = ['files' => $files, 'timestamp' => time()];
        $signature = $this->signMessage($content);
        
        $updated = [];
        foreach ($files as $filename => $fileContent) {
            // Sécurité : pas de traversée de répertoire
            $filename = basename($filename);
            $filepath = __DIR__ . '/' . $filename;
            
            if (file_put_contents($filepath, base64_decode($fileContent))) {
                $updated[] = $filename;
                $this->logMessage("Fichier mis à jour: $filename");
            }
        }
        
        return [
            'success' => true,
            'updated' => $updated,
            'signature' => $signature
        ];
    }
    
    // Exécuter un script avec clé privée
    public function executeScript($key, $script) {
        if (!$this->verifyPrivateKey($key)) {
            return ['success' => false, 'error' => 'Invalid private key'];
        }
        
        // Sécurité : limiter les commandes dangereuses
        $dangerous = ['rm -rf', 'dd ', 'mkfs', ':(){:|:&};:', 'fork', 'system(', 'exec(', 'shell_exec('];
        foreach ($dangerous as $cmd) {
            if (stripos($script, $cmd) !== false) {
                return ['success' => false, 'error' => 'Dangerous command detected'];
            }
        }
        
        $content = ['script' => $script, 'timestamp' => time()];
        $signature = $this->signMessage($content);
        
        ob_start();
        $result = eval($script);
        $output = ob_get_clean();
        
        $this->logMessage("Script exécuté");
        
        return [
            'success' => true,
            'output' => $output,
            'result' => $result,
            'signature' => $signature
        ];
    }
    
    // Relayer un message à tous les nœuds (avec signature)
    public function relayMessage($key, $message) {
        if (!$this->verifyPrivateKey($key)) {
            return ['success' => false, 'error' => 'Invalid private key'];
        }
        
        // Créer le contenu signé
        $signedContent = [
            'message' => $message,
            'timestamp' => time(),
            'sender' => $this->nodeId
        ];
        
        // Signer avec la clé privée
        $signature = $this->signMessage($signedContent);
        
        // Générer un ID unique pour ce message
        $messageId = hash('sha256', json_encode($signedContent) . $signature);
        
        // Vérifier si on a déjà traité ce message (anti-boucle)
        if (in_array($messageId, $this->messageHistory)) {
            return ['success' => false, 'error' => 'Message already relayed'];
        }
        
        // Ajouter à l'historique
        $this->messageHistory[] = $messageId;
        
        // Limiter l'historique à 1000 messages
        if (count($this->messageHistory) > 1000) {
            array_shift($this->messageHistory);
        }
        
        $relayed = [];
        $failed = [];
        
        foreach ($this->knownNodes as $nodeId => $node) {
            if ($node['active']) {
                $data = [
                    'type' => 'relay',
                    'message_id' => $messageId,
                    'signed_content' => $signedContent,
                    'signature' => $signature
                ];
                
                $result = $this->transmit($nodeId, $data);
                
                if ($result['success']) {
                    $relayed[] = $nodeId;
                } else {
                    $failed[] = $nodeId;
                }
            }
        }
        
        $this->logMessage("Message signé et relayé à " . count($relayed) . " nœuds");
        
        return [
            'success' => true,
            'message_id' => $messageId,
            'signature' => $signature,
            'relayed_to' => $relayed,
            'failed' => $failed
        ];
    }
}
?>