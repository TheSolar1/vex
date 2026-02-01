<?php
 include '/var/www/html/access_control.php';
// /login/autologin.php?idutilsteur=1&nombre=tsCIttQKS7MEpCbHSRr2lmwFDrHL1vOqsd4wMbUuG2U7HKXgCFj6PSatz1AGtzXfbTrelZCdTQ6TiQZJy6uV0ucjqeNtRxrhwJf1POEctmE1XHcHkCAOqYffpyBB5T8YgGnWvInGiDOTsqh3qBhtKLYNKgEHrIBYUCpEvxxbQu6TGFY601UsnRJceNukNeG1v5aFR6ca
ini_set('display_errors', 1);
ini_set('display_startup_errors', 1);
error_reporting(E_ALL);
// Configuration de la connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";

// Creation de la connexion
$conn = new mysqli($servername, $username, $password, $dbname);

// Verification de la connexion
if ($conn->connect_error) {
    die("Connexion echouee : " . $conn->connect_error);
}

// Fonction pour securiser les entrees
function secure_input($data, $conn) {
    return htmlspecialchars(mysqli_real_escape_string($conn, $data));
}


// Verification des parametres d'URL pour l'auto-login
if (isset($_GET['idutilsteur']) && isset($_GET['nombre'])) {
    $id_utilisateur = secure_input($_GET['idutilsteur'], $conn);
    $nombre = secure_input($_GET['nombre'], $conn);

    // Requete SQL pour verifier l'existence de l'ID utilisateur et du nombre dans la table autologin
    $sql = "SELECT nombre FROM autologin WHERE compteid = ? AND nombre = ?";
    $stmt = $conn->prepare($sql);
    $stmt->bind_param("ss", $id_utilisateur, $nombre);
    $stmt->execute();
    $result = $stmt->get_result();
    
       
    if ($result->num_rows > 0) {
        // Si l'utilisateur et le nombre correspondent, recuperer les informations de l'utilisateur
        $stmt_user = $conn->prepare("SELECT nom, email FROM login WHERE id = ?");
        $stmt_user->bind_param("s", $id_utilisateur);
        $stmt_user->execute();
        $result_user = $stmt_user->get_result();
        echo ("2");
        if ($result_user->num_rows == 1) {

            $tagUserColumn =  'vAutologin'; 
            $tagUserValue =verifierrr(iduserav: $userId, tagUserColumn: $tagUserColumn, conn: $conn);

            if ( $tagUserValue = "non") {
                echo "Connexions  bloque par un admin <br> Concise demande un admin pour débouque ";
            }else {
            $user_row = $result_user->fetch_assoc();
            $_SESSION['id_utilisateur'] = $id_utilisateur;
            $_SESSION['user_nom'] = $user_row['nom'];
            $_SESSION['user_email'] = $user_row['email'];

            // Informations PC, IP et Navigateur
            $pc_ip = $_SERVER['REMOTE_ADDR'];
            $browser = $_SERVER['HTTP_USER_AGENT'];
            
			
			$dfdsfe754 = "oui";
            
            // Insertion dans la table loginc
            $datecra = date('Y-m-d H:i:s');
            $cookie_value = md5(uniqid(rand(), true));
            $cookie_expiry = time() + (60 * 60 * 24 * 30);
            setcookie('connexion_cookie', $cookie_value, $cookie_expiry, '/', '', false, true);
            $stmt_insert = $conn->prepare("INSERT INTO loginc (idcokier, datecra, pc, navi, email, nom, autologin) VALUES (?, ?, ?, ?, ?, ?, ?)");
            $stmt_insert->bind_param("sssssss", $cookie_value, $datecra, $pc_ip, $browser, $user_row['email'], $user_row['nom'], $dfdsfe754);
            $stmt_insert->execute();

            if ($stmt_insert->errno) {
                echo "Erreur d'insertion : " . $stmt_insert->error;
            } else {
                // Redirection vers la page de dashboard
                echo "<script>window.location.href = 'dashboard.php';</script>";
                exit();
            }
        }
            $stmt_insert->close();
        }
        $stmt_user->close();
    } else {
        echo "ID utilisateur ou nombre incorrect.";
    }
    $stmt->close();
} else {
    echo "Parametres manquants dans l'URL.";
}

// Fermeture de la connexion
$conn->close();
?>
