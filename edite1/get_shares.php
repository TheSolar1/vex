<?php
// Nettoyer tous les buffers
while (ob_get_level()) {
    ob_end_clean();
}

error_reporting(0);
ini_set('display_errors', 0);

header('Content-Type: application/json; charset=utf-8');
header('Access-Control-Allow-Origin: *');

$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";

try {
    $conn = new mysqli($servername, $username, $password, $dbname);
    
    if ($conn->connect_error) {
        echo json_encode(['users' => [], 'error' => 'DB connection failed']);
        exit;
    }
    
    $item_id = isset($_GET['id']) ? intval($_GET['id']) : 0;
    $item_type = isset($_GET['type']) ? trim($_GET['type']) : '';
    
    if ($item_id <= 0 || empty($item_type)) {
        echo json_encode(['users' => [], 'error' => 'Invalid parameters']);
        exit;
    }
    
    $users = [];
    
    // Fonction pour récupérer le nom d'utilisateur
    function getUserName($conn, $user_id) {
        try {
            $stmt = $conn->prepare("SELECT nom FROM login WHERE id = ?");
            if (!$stmt) {
                return "Utilisateur #" . $user_id;
            }
            
            $stmt->bind_param("i", $user_id);
            $stmt->execute();
            $result = $stmt->get_result();
            
            if ($row = $result->fetch_assoc()) {
                $name = $row['nom'];
                $stmt->close();
                return $name;
            }
            $stmt->close();
            return "Utilisateur #" . $user_id;
        } catch (Exception $e) {
            return "Utilisateur #" . $user_id;
        }
    }
    
    // Fonction pour parser les partages
    // Format: "userid:permission,userid:permission"
    // Permissions: L = Lecture, E = Edition, S = Tout
    function parseShares($conn, $share_string) {
        $users = [];
        
        if (empty($share_string)) {
            return $users;
        }
        
        $share_string = trim($share_string);
        $shares = explode(',', $share_string);
        
        foreach ($shares as $share) {
            $share = trim($share);
            if (empty($share)) continue;
            
            // Format attendu: "userid:permission"
            if (strpos($share, ':') !== false) {
                $parts = explode(':', $share, 2);
                $uid = intval(trim($parts[0]));
                $perm = isset($parts[1]) ? trim($parts[1]) : 'L';
            } else {
                // Si pas de ":", c'est juste un ID (permission par défaut: Lecture)
                $uid = intval(trim($share));
                $perm = 'L';
            }
            
            if ($uid > 0) {
                $user_name = getUserName($conn, $uid);
                $users[] = [
                    'id' => $uid,
                    'name' => $user_name,
                    'permission' => $perm
                ];
            }
        }
        
        return $users;
    }
    
    // Récupérer les données selon le type
    if ($item_type === 'file') {
        // Pour les fichiers: utiliser la colonne 'partage'
        $stmt = $conn->prepare("SELECT partage FROM fichiers WHERE id = ?");
        if (!$stmt) {
            throw new Exception("Prepare failed for fichiers");
        }
        
        $stmt->bind_param("i", $item_id);
        $stmt->execute();
        $result = $stmt->get_result();
        
        if ($row = $result->fetch_assoc()) {
            $users = parseShares($conn, $row['partage']);
        }
        $stmt->close();
        
    } elseif ($item_type === 'folder') {
        // Pour les dossiers: utiliser addpageuserid
        $stmt = $conn->prepare("SELECT addpageuserid FROM sitecdos WHERE iddosier = ?");
        if (!$stmt) {
            throw new Exception("Prepare failed for sitecdos");
        }
        
        $stmt->bind_param("i", $item_id);
        $stmt->execute();
        $result = $stmt->get_result();
        
        if ($row = $result->fetch_assoc()) {
            $users = parseShares($conn, $row['addpageuserid']);
        }
        $stmt->close();
        
    } elseif ($item_type === 'page') {
        // Les pages n'ont PAS de partage utilisateur, juste public/privé (prob)
        $stmt = $conn->prepare("SELECT prob FROM sitec WHERE idpage = ?");
        if (!$stmt) {
            throw new Exception("Prepare failed for sitec");
        }
        
        $stmt->bind_param("i", $item_id);
        $stmt->execute();
        $result = $stmt->get_result();
        
        if ($row = $result->fetch_assoc()) {
            // prob = 0 = public, prob = 1 = privé
            // Retourner un tableau vide car pas de partage individuel
            $users = [];
        }
        $stmt->close();
        
    } else {
        echo json_encode(['users' => [], 'error' => 'Invalid type: ' . $item_type]);
        exit;
    }
    
    $conn->close();
    echo json_encode(['users' => $users], JSON_UNESCAPED_UNICODE);
    
} catch (Exception $e) {
    if (isset($conn) && $conn) {
        $conn->close();
    }
    echo json_encode(['users' => [], 'error' => 'Server error', 'debug' => $e->getMessage()]);
}

exit;
?>