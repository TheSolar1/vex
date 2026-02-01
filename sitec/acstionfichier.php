<?php


include '/var/www/html/access_control.php';
//debegu ini_set('display_errors', 1);
//ini_set('display_startup_errors', 1);
//error_reporting(E_ALL);
$connecte = false;

// Connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

// Verification de la connexion
if ($conn->connect_error) {
    die("La connexion a echoue : " . $conn->connect_error);
}

// Verification si le cookie de session existe
if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];

    // Requete preparee pour recuperer les informations de connexion correspondant au cookie
    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=? ");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        $user_idcokier = $row['idcokier'];
        $datecra = $row['datecra'];
        $pc = $row['pc'];
        $navi = $row['navi'];
        $user_email = $row['email'];
        $user_nom = $row['nom'];
        

        // Verifie si l'adresse IP et le navigateur correspondent
        if ($pc === $_SERVER['REMOTE_ADDR'] && $navi === $_SERVER['HTTP_USER_AGENT']) {
            // Verifie si la date de creation est correcte
            $one_hour_ago = strtotime('-1 hour');
            $datecra_timestamp = strtotime($datecra);
            $is_recent = $datecra_timestamp > $one_hour_ago;

            if ($is_recent) {
                // Utilisateur connecte
                 
              
                 
                 $stmt = $conn->prepare("SELECT * FROM login WHERE email=?");
                 $stmt->bind_param("s", $user_email);
                 $stmt->execute();
                 $result = $stmt->get_result();
                     
                     if ($result->num_rows == 1) {
                     if ($row) {
                     $row = $result->fetch_assoc();
                     }
                     $user_vip = $row['vip'];
                     $user_id = (int)$row['id'];
                     $user_privilege = (int)$row['privilege'];
                   
                     }
                    }else {
                        $user_id = 0;
                    }
                }else {
                    $user_id = 0;
                }
            }else {
                $user_id = 0;
            }
        }else {
            $user_id = 0;
        }
       
      

if (isset($_GET['id']) && isset($_GET['action'])) {
    $fichier_id = intval($_GET['id']); // Récupérer l'ID du fichier
    $action = $_GET['action']; // Récupérer l'action demandée
}if ($action === 'view') {
     $stmt = $conn->prepare("SELECT * FROM fichiers WHERE id = ?");
    $stmt->bind_param("i", $fichier_id);
    $stmt->execute();
    $result = $stmt->get_result();

    if ($result->num_rows > 0) {
        $fichier = $result->fetch_assoc();
        $fichier_nom = $fichier['nom'];
        $visible = $fichier['visble'];
        $userc = $fichier['id_utilisateur'];
        $type_fichier = $fichier['type_fichier'];

        // Contrôle d'accès
        if ($visible === '1' && $userc != $user_id) {
            echo "Ce document est privé";
            exit;
        }

        // Si contenu stocké en base (BLOB)
        if (!empty($fichier['fichier'])) {
            header("Content-Type: $type_fichier");
            header("Content-Disposition: inline; filename=\"" . basename($fichier_nom) . "\"");
            echo $fichier['fichier'];
            exit;
        }

        // Sinon, si fichier sur disque (ex: $fichier['fichier'] = chemin)
        $chemin_fichier = __DIR__ . '/uploads/' . $fichier['fichier']; // adapte ce chemin
        if (file_exists($chemin_fichier)) {
            header("Content-Type: $type_fichier");
            header("Content-Disposition: inline; filename=\"" . basename($fichier_nom) . "\"");
            readfile($chemin_fichier);
            exit;
        }

        echo "Fichier introuvable";
        exit;

    } else {
        echo "<p>Fichier non trouvé.</p>";
        exit;
    }


}else {

if (isset($_GET['id'])) {
    $fichier_id = intval($_GET['id']); // Récupérer l'ID du fichier à supprimer
    if ($addpageauto === 1) {
    // Requête pour supprimer le fichier
    $stmt = $conn->prepare("DELETE FROM fichiers WHERE id = ?");
    $stmt->bind_param("i", $fichier_id);
    if ($stmt->execute()) {
        echo "<p>Fichier supprimé avec succès.</p>";
    } else {
        echo "<p>Erreur lors de la suppression du fichier.</p>";
    }
    // Rediriger après suppression
    header("Location: dossier.php"); // Ou redirigez où vous voulez
    exit();
}
}
}

?>
