<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
       <title>.hopto.org</title>
    <link rel="icon" type="image/png" href="/vex.png">
    <style>
        body {
            font-family: Arial, sans-serif;
            display: flex;
            justify-content: center;
            
            height: 100vh;
            background-color: #f0f0f0;
        }
        #countdown {
            font-size: 17x;
            color: #333;
        }
    </style>
</head>
<body>


<?php
 include '/var/www/html/function.php';
footer();
session_start();
// Connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

// Verification de la connexion
if ($conn->connect_error) {
    die("Erreur de connexion : " . $conn->connect_error);
}
// Definir le jeu de caracteres a utf8mb4
if (!$conn->set_charset("utf8mb4")) {
    die("Erreur lors du chargement de la page de caracteres utf8mb4 : " . $conn->error);
}

// Verification de la connexion
if ($conn->connect_error) {
    die("La connexion a echoue : " . $conn->connect_error);
}

// Recuperation des adresses IP et MAC
$ipAddress = $_SERVER['REMOTE_ADDR'];
$macAddress = $_SERVER['HTTP_USER_AGENT'];
$page = $_SERVER['PHP_SELF'];

// Executer la commande ifconfig pour obtenir l'adresse MAC
$mac = exec('ifconfig | grep -o -E "([0-9a-f]{2}:){5}([0-9a-f]{2})" | head -n 1');

// Generer un identifiant aleatoire de 10 caracteres
$id = bin2hex(random_bytes(5));

// Variables pour les informations de session
$user_idcokier = null;
$user_nom = null;
$etat_connexion = "La session a expire. Veuillez vous reconnecter."; // Message par defaut
$etat_connexion_utf8 = mb_convert_encoding($etat_connexion, 'UTF-8');

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
                    }
        } else {
            $auteur = "Les informations de connexion ne correspondent pas. Il se peut que votre session ait  compromise.";
            $sql = "INSERT INTO `sus-hac` (`id-c`, `auteur`) VALUES (?, ?)";
            $stmt = $conn->prepare($sql);
            $stmt->bind_param("ss", $user_idcokier, $auteur); // Utilisation de "ss" pour deux c
            $stmt->execute();
            $stmt->close();
            $conn->close();
        }
    } 




} 


    if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['id_fichier']) ) {
        $id_fichier = intval($_POST['id_fichier']);
        $stmt = $conn->prepare("SELECT id_utilisateur FROM fichiers WHERE id_fichier=? ");	 
            $stmt->bind_param("s", $id_fichier);
            $stmt->execute();
            $result = $stmt->get_result();
            
            if ($result->num_rows == 1) {
                $ffuser_id = $row['id_utilisateur'];
                if ($ffuser_id !== $user_id) {
                    echo "Vous n'avez pas la permission de modifier ou supprimer ce fichier.";
                    exit;
             
        // Verification si l'ID du fichier et l'action ont ete envoyes via POST
    if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['id_fichier']) && isset($_POST['nouvelle_visibilite'])) {
    

    // Commencer la session pour recuperer l'ID de l'utilisateur connecte
    session_start();
        
        
        // Verifier si l'utilisateur souhaite modifier la visibilite ou supprimer le fichier
        if (isset($_POST['nouvelle_visibilite'])) {
           
            $datepophpmy = date('j'); 
            $datepophpmy = str_pad($datepophpmy, 2, '0', STR_PAD_LEFT); // pour avoir toujours 2 chiffres
            
            $dateyear = date('Y'); 
            $datemonth = date('m'); 
            
            $datepophpmyfin = $dateyear . '-' . $datemonth . '-' . $datepophpmy; 
            // Mise a jour de la visibilite
            $nouvelle_visibilite = intval($_POST['nouvelle_visibilite']);
             
            $sql = "UPDATE fichiers SET visble = ? , `date`= ? WHERE id = ? AND `id_utilisateur` = ?";
            $stmt = $conn->prepare($sql);
       
            $stmt->bind_param("isii", $nouvelle_visibilite, $datepophpmyfin,$id_fichier, $user_id);
            
            if ($stmt->execute()) {
                echo "Visibilite du fichier mise a jour avec succes.";
                echo " <script>
						setTimeout(function() {
								window.location.href = 'index.php';
								}, 3000); // 3000 millisecondes = 5 secondes
						</script>
					
						redirection dans : 
						
							";
                // Redirection ou message de confirmation
                //header('Location: index.php'); // Redirection optionnelle
            } else {
                echo "Erreur lors de la mise a jour de la visibilite : " . $stmt->error;
            }
            $stmt->close();

        } elseif (isset($_POST['action']) && $_POST['action'] === 'delete') {
            // Suppression du fichier
            $sql = "DELETE FROM fichiers WHERE id = ? AND id_utilisateur = ?";
            $stmt = $conn->prepare($sql);
            $stmt->bind_param("ii", $id_fichier, $user_id);

            if ($stmt->execute()) {
                echo "Fichier supprime avec succes.";
                 echo " <script>
						setTimeout(function() {
								window.location.href = 'index.php';
								}, 3000); // 3000 millisecondes = 5 secondes
						</script>
						
						redirection dans : 
						
							";
                // Redirection ou message de confirmation
                // header('Location: index.php'); // Redirection optionnelle
            } else {
                echo "Erreur lors de la suppression : " . $stmt->error;
            }
            $stmt->close();
        }

    

            } else {
                echo "ID de fichier ou action non valide.";
            }
   }
            }
        }
// Insertion dans la base de donnees avec la date actuelle et la page
$sql = "INSERT INTO user1 (mac1, ip_address, mac_address, page, date_added, id, `id-c`, `c-nom`, `etat-connexion`) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?, ?)";
$stmt = $conn->prepare($sql);
$stmt->bind_param("ssssssss", $mac, $ipAddress, $macAddress, $page, $id, $user_idcokier, $user_nom, $etat_connexion_utf8);

// Executer la requete
if ($stmt->execute() === TRUE) {
    ?>
    <script>
        console.log("Enregistrement reussi");
        console.log("Donnees : mac_address : ip");
    </script>
    <?php
} else {
    ?>
    <script>
        console.log("Erreur d'enregistrement");
        console.log("Donnees : rien");
    </script>
    <?php
}

// Fermeture de la connexion
$stmt->close();
$conn->close();

?>

<div id="countdown"></div>
<script>
    // Duree du decompte en secondes
    let countdownDuration = 3; // 10 secondes
    const countdownElement = document.getElementById('countdown');

    // Met a jour l'affichage du decompte
    function updateCountdown() {
        countdownElement.textContent = countdownDuration;

        if (countdownDuration <= 0) {
            clearInterval(countdownInterval);
            countdownElement.textContent = "";
        } else {
            countdownDuration--;
        }
    }

    // Demarre le decompte
    const countdownInterval = setInterval(updateCountdown, 1000);
    updateCountdown(); // Appel initial pour afficher le temps
</script>

</body>
</html>
</body>
</html>
