<?php
// Verifie si le cookie de connexion existe
if (isset($_COOKIE['connexion_cookie'])) {
    // Supprime le cookie en le definissant avec une date d'expiration dans le passe
    setcookie("connexion_cookie", "", time() - 3600, '/');
    
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

    // Suppression des informations de connexion de la table loginc
    $cookie_value = $_COOKIE['connexion_cookie'];
    $delete_stmt = $conn->prepare("DELETE FROM loginc WHERE idcokier = ?");
    $delete_stmt->bind_param("s", $cookie_value);
    $delete_stmt->execute();
    $delete_stmt->close();

    // Fermeture de la connexion
    $conn->close();

    echo "Vous avez ete deconnecte avec succes.";
} else {
    echo "Vous n'etes pas connecte.";
}

?>
<script>
        
            window.location.href = "/login/login.php";
        
    </script>
