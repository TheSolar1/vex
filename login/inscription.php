<?php
 include '/var/www/html/access_control.php';
session_start();

if ($_SERVER["REQUEST_METHOD"] == "POST") {
    // Connexion � la base de donn�es
    $servername = "localhost";
    $username = "orsql";
    $password = "iDq]25F0u8v*z[1d";
    $dbname = "user";

    $conn = new mysqli($servername, $username, $password, $dbname);

    if ($conn->connect_error) {
        die("La connexion a �chou� : " . $conn->connect_error);
    }

    // R�cup�ration des donn�es du formulaire
    $nom = $_POST['nom'];
    $email = $_POST['email'];
    $motdepass = $_POST['motdepass'];

    // V�rification si l'email existe d�j�
    $checkEmailQuery = "SELECT id FROM login WHERE email = '$email'";
    $result = $conn->query($checkEmailQuery);

    if ($result->num_rows > 0) {
        $error = "Cet email est d�j� utilis� par un autre utilisateur.";
    } else {
        // Insertion des donn�es dans la base de donn�es
        $sql = "INSERT INTO login (nom, email, motdepass) VALUES ('$nom', '$email', '$motdepass')";

        if ($conn->query($sql) === TRUE) {
            $_SESSION['success'] = "Votre compte a �t� cr�� avec succ�s. Vous pouvez maintenant vous connecter.";
            header("Location: login.php");
            exit();
        } else {
            $error = "Une erreur s'est produite lors de la cr�ation de votre compte : " . $conn->error;
        }
    }

    $conn->close();
}

?>

<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Inscription</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            background-color: #f4f4f4;
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
        }

        .container {
            background-color: #fff;
            padding: 20px;
            border-radius: 5px;
            box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
            width: 300px;
        }

        h2 {
            text-align: center;
        }

        label {
            display: block;
            margin-bottom: 5px;
        }

        input[type="text"],
        input[type="password"] {
            width: 100%;
            padding: 8px;
            margin-bottom: 15px;
            border: 1px solid #ccc;
            border-radius: 3px;
        }

        input[type="submit"] {
            width: 100%;
            padding: 10px;
            border: none;
            background-color: #007bff;
            color: #fff;
            border-radius: 3px;
            cursor: pointer;
        }

        input[type="submit"]:hover {
            background-color: #0056b3;
        }

        .error {
            color: red;
            margin-bottom: 10px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h2>Inscription</h2>
        <?php if(isset($error)) echo "<p class='error'>$error</p>"; ?>
        <form method="post" action="<?php echo htmlspecialchars($_SERVER["PHP_SELF"]);?>">
            <label for="nom">Nom:</label>
            <input type="text" id="nom" name="nom" required><br>
            <label for="email">Email:</label>
            <input type="text" id="email" name="email" required><br>
            <label for="motdepass">Mot de passe:</label>
            <input type="password" id="motdepass" name="motdepass" required><br>
            <input type="submit" value="Inscription">
        </form>
    </div>
</body>
</html>
