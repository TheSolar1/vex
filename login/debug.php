<?php
session_start(); // Demarre la session

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
    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=?");
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
                echo "Vous etes connecte en tant que " . $user_nom . ".";
            } else {
                echo "La session a expire. Veuillez vous reconnecter.";
            }
        } else {
            echo "Les informations de connexion ne correspondent pas. Il se peut que votre session ait ete compromise.";
        }
    } else {
        echo "Vous n'etes pas connecte.";
    }

    $stmt->close();
} else {
    echo "Vous n'etes pas connecte.";
}

// Fermeture de la connexion
$conn->close();
?>
<!DOCTYPE html>
<html lang="fr">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    
    <style>
        p {
            font-size: xx-small;
            font-family: 'Roboto', sans-serif;
        }

        #footer {
            text-align: center;
            padding: -10px;
            position: fixed;
            bottom: 0;
            display: flex;
            justify-content: space-around;
            align-items: center;
            flex-wrap: wrap;
        }

        h1 {
            font-family: 'Roboto', sans-serif;
        }
        #a1 {
            font-size: xx-small;
            font-family: 'Roboto', sans-serif;
        }
        a {
            font-family: 'Roboto', sans-serif;
        }
        #d1 {
      margin-left: 30px;
      margin-right: 30px;
        }
        #d2 {
      margin-left: 20px;
      margin-right: 30px;
        }
        .footer-column {
            margin-bottom: 20px;
        }

        .footer-bottom {
           
            width: 100%;
            text-align: center;
        }
        /* Styliser les liens */
    #a1 {
    color: #0077cc; /* Couleur du texte du lien */
    text-decoration: none; /* Supprimer le soulignement par défaut */
    transition: color 0.3s ease; /* Effet de transition pour la couleur du texte */
    cursor: pointer; /* Utiliser une main comme curseur pour les liens */
    }


    #a1:hover {
    text-decoration: underline;
    color: #004466; /* Nouvelle couleur du texte au survol */
    background-color: #f0f0f0; /* Ajouter une couleur de fond au survol */
    }


    </style>
   


</head>

<body>
    



    
</body>

</html>
