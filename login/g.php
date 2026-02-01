<?php
 include '/var/www/html/access_control.php';
///login/autologin.php?idutilsteur=---&nombre=
// Configuration de la connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";

// Creation de la connexion
$conn = new mysqli($servername, $username, $password, $dbname);

// Verification de la connexion
if ($conn->connect_error) {
    die("Connexion echouee: " . $conn->connect_error);
}

// Fonction pour securiser les entrees
function secure_input($data, $conn) {
    $data = trim($data);
    $data = stripslashes($data);
    $data = htmlspecialchars($data);
    return $conn->real_escape_string($data);
}

// Fonction pour generer une chaine de 200 caractares aleatoires
function generate_random_string($length = 200) {
    $characters = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
    $characters_length = strlen($characters);
    $random_string = '';
    for ($i = 0; $i < $length; $i++) {
        $random_string .= $characters[rand(0, $characters_length - 1)];
    }
    return $random_string;
}

// Selectionner un ID utilisateur dans la table login (par exemple, le premier utilisateur)
$query = "SELECT id FROM login LIMIT 1";
$result = $conn->query($query);

if ($result->num_rows > 0) {
    // Recuperer l'ID de l'utilisateur
    $row = $result->fetch_assoc();
    $id_utilisateur = $row['id'];

    // Verifier si un autologin existe deja pour cet utilisateur
    $stmt_check = $conn->prepare("SELECT compteid FROM autologin WHERE compteid = ?");
    $stmt_check->bind_param("s", $id_utilisateur);
    $stmt_check->execute();
    $stmt_check->store_result();

    if ($stmt_check->num_rows > 0) {
        echo "Un autologin existe deja pour cet utilisateur.";
    } else {
        // Generer une chaine aleatoire de 200 caractares
        $random_string = generate_random_string();

        // Insertion des donnees dans la table autologin
        $stmt_insert = $conn->prepare("INSERT INTO autologin (compteid, nombre) VALUES (?, ?)");
        $stmt_insert->bind_param("ss", $id_utilisateur, $random_string);
        $stmt_insert->execute();

        if ($stmt_insert->errno) {
            echo "Erreur d'insertion : " . $stmt_insert->error;
        } else {
            // Afficher l'URL avec les paramatres idutilsteur et nombre
            $url = "/login/autologin.php?idutilsteur=" . $id_utilisateur . "&nombre=" . $random_string;
            echo "URL generee : <a href='$url'>$url</a>";
        }

        $stmt_insert->close();
    }
    
    $stmt_check->close();
} else {
    echo "Aucun utilisateur trouve dans la table login.";
}

// Fermeture de la connexion
$conn->close();
?>
