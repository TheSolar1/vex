<!DOCTYPE html>
<link rel="icon" type="image/png" href="/vex.png">
<?php
 include '/var/www/html/access_control.php';

// Connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";

// Creation de la connexion
$conn = new mysqli($servername, $username, $password, $dbname);

// Verification de la connexion
if ($conn->connect_error) {
    die("La connexion a echoue : " . $conn->connect_error);
}

// Fonction pour securiser les donnees
function secure_input($data, $conn) {
    $data = trim($data);
    $data = stripslashes($data);
    $data = htmlspecialchars($data);
    return $conn->real_escape_string($data);
}

$eureuco = "";
$eureui = "";
// Processus d'inscription
$reuisteorno = "1";

if ($_SERVER["REQUEST_METHOD"] == "POST" && isset($_POST['signup'])) {
    $reuisteorno = "2";
  
    if (!isset($_POST['scales'])) {
        $eureui = "Veuillez accepter la politique de confidentialite pour creer un compte.";
     
    } else {
        $nom = secure_input($_POST['nom'], $conn);
        $email = secure_input($_POST['email'], $conn);
        $motdepass = secure_input($_POST['motdepass'], $conn);
        $motdepass = password_hash($motdepass, PASSWORD_DEFAULT);
        
        $check_stmt = $conn->prepare("SELECT nom, email FROM login WHERE nom = ? OR email = ?");
        if (!$check_stmt) {
            die("Erreur de preparation de la requete : " . $conn->error);
        }
        $check_stmt->bind_param("ss", $nom, $email);
        $check_stmt->execute();
        $check_stmt->store_result();

        if ($check_stmt->num_rows > 0) {
            $eureui =  "Ce nom d'utilisateur ou cet email existe deje.";
        } else {
           
            $insert_stmt = $conn->prepare("INSERT INTO login (nom, email, motdepass, vip) VALUES (?, ?, ?, 0)");
            $insert_stmt->bind_param("sss", $nom, $email, $motdepass);

            if ($insert_stmt->execute()) {
                 $eureuco = "Inscription reussie.";
                 $reuisteorno = "3";
            } else {
                 $eureuco = "Erreur d'inscription : " . $conn->error;
            }

            $insert_stmt->close();
        }
        $check_stmt->close();
    }
}

if ($conn->connect_error) {
    die("Erreur de connexion a la base de donnees : " . $conn->connect_error);
}

//co

session_start(); // Demarre la session

if ($_SERVER["REQUEST_METHOD"] == "POST" && isset($_POST['login'])) {
    $reuisteorno = "4";
       // Verifiez le jeton CSRF
    if (!isset($_POST['csrf_token']) || $_POST['csrf_token'] !== $_SESSION['csrf_token']) {
        // Generer le jeton CSRF si necessaire
    }
    
    if (isset($_POST['email'])) {
        $sui_email = secure_input($_POST["email"], $conn); // Securisation de l'email
        
        if (isset($_POST['motdepass'])) {
            $sui_motdepass = secure_input($_POST["motdepass"], $conn); // Securisation du mot de passe

            // Requete pour recuperer l'utilisateur
            $query = "SELECT * FROM login WHERE email = ?";
            $stmt = $conn->prepare($query);

            if ($stmt) {
                $stmt->bind_param("s", $sui_email);
                $stmt->execute();
                $result = $stmt->get_result();

                if ($result->num_rows > 0) {
                   // Recuperation du mot de passe stocke dans la base de donnees
                    $row = $result->fetch_assoc();
                    $motdepasse_stocke = $row['motdepass']; // Mot de passe stocke

                    // Verification du mot de passe
                    if (password_verify($sui_motdepass, $motdepasse_stocke)) {
                        $eureuco ="Connexion reussie.";

                        // Creation d'un cookie securise pour la session
                        $cookie_value = md5(uniqid(rand(), true)); // Valeur aleatoire securisee
                        $cookie_expiry = time() + (60 * 60 * 24 * 30); // Expire dans 30 jours
                        setcookie('connexion_cookie', $cookie_value, $cookie_expiry, '/', '', false, true);

                        // Informations PC, IP et Navigateur
                        $pc_ip = $_SERVER['REMOTE_ADDR']; // IP
                        $browser = $_SERVER['HTTP_USER_AGENT']; // Navigateur
                        $datecra = date('Y-m-d H:i:s'); // Date actuelle
                        $user_email = $row['email']; // Email de l'utilisateur
                        $user_nom = $row['nom']; // Nom de l'utilisateur (si present dans la table)

                        // Insertion dans la table loginc
                        $stmt_insert = $conn->prepare("INSERT INTO loginc (idcokier, datecra, pc, navi, email, nom) VALUES (?, ?, ?, ?, ?, ?)");
                        $stmt_insert->bind_param("ssssss", $cookie_value, $datecra, $pc_ip, $browser, $user_email, $user_nom);
                        $stmt_insert->execute();

                        if ($stmt_insert->error) {
                            echo "Erreur d'insertion : " . $stmt_insert->error;
                        } else {
                            $eureuco = "Connexion reussie et informations enregistrees.";
                            $reuisteorno = "5";
                            // Redirection vers le dashboard ou une autre page
                           header("Location: dashboard.php");
                            exit(); // Arrete l'execution apres la redirection
                        }
                        
                    } else {
                        $eureuco = "Mot de passe incorrect.";
                    }
                } else {
                    $eureuco = "Aucun utilisateur trouve avec cet email.";
                }
            } else {
                $eureuco = "Erreur lors de la preparation de la requete.";
            }
        } else {
            $eureuco = "Erreur : Le champ mot de passe est manquant.<br>";
        }
    } else {
       $eureuco = "Erreur : Le champ email est manquant.<br>";
    }
}
    // Generer le jeton CSRF si necessaire
if (empty($_SESSION['csrf_token'])) {
    $_SESSION['csrf_token'] = bin2hex(random_bytes(32)); // Genere un jeton alaatoire
}

?>

<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Connexion</title>
    <link href="https://fonts.googleapis.com/css2?family=Roboto:wght@400;500;600;700&display=swap" rel="stylesheet">
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #4caf50 0%, #45a049 100%);
            min-height: 100vh;
            display: flex;
            justify-content: center;
            align-items: center;
            padding: 20px;
        }

        .login-wrapper {
            background: white;
            border-radius: 12px;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            width: 100%;
            max-width: 420px;
            padding: 40px;
            animation: slideIn 0.4s ease-out;
        }

        @keyframes slideIn {
            from {
                opacity: 0;
                transform: translateY(-20px);
            }
            to {
                opacity: 1;
                transform: translateY(0);
            }
        }

        .logo {
            text-align: center;
            margin-bottom: 32px;
        }

        .logo h1 {
            font-size: 28px;
            color: #1c1e21;
            font-weight: 700;
            margin-bottom: 8px;
        }

        .logo p {
            font-size: 14px;
            color: #65676b;
        }

        .form-group {
            margin-bottom: 20px;
        }

        .form-label {
            display: block;
            margin-bottom: 8px;
            font-weight: 600;
            font-size: 14px;
            color: #1c1e21;
        }

        .form-input {
            width: 100%;
            padding: 12px 16px;
            border: 1px solid #e4e6eb;
            border-radius: 8px;
            font-size: 15px;
            transition: all 0.3s;
            font-family: inherit;
        }

        .form-input:focus {
            outline: none;
            border-color: #4caf50;
            box-shadow: 0 0 0 3px rgba(76, 175, 80, 0.1);
        }

        .btn-primary {
            width: 100%;
            padding: 12px 16px;
            background: linear-gradient(135deg, #4caf50 0%, #45a049 100%);
            color: white;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s;
            margin-top: 8px;
        }

        .btn-primary:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
        }

        .btn-primary:active {
            transform: translateY(0);
        }

        .divider {
            text-align: center;
            margin: 24px 0;
            position: relative;
        }

        .divider::before {
            content: '';
            position: absolute;
            left: 0;
            top: 50%;
            width: 100%;
            height: 1px;
            background: #e4e6eb;
        }

        .divider span {
            background: white;
            padding: 0 16px;
            color: #65676b;
            font-size: 14px;
            position: relative;
            z-index: 1;
        }

        .btn-secondary {
            width: 100%;
            padding: 12px 16px;
            background: #e4e6eb;
            color: #1c1e21;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s;
        }

        .btn-secondary:hover {
            background: #d0d2d6;
        }

        .message {
            padding: 12px 16px;
            border-radius: 8px;
            margin-bottom: 20px;
            font-size: 14px;
            text-align: center;
        }

        .message.success {
            background: #e8f5e9;
            color: #2e7d32;
            border: 1px solid #4caf50;
        }

        .message.error {
            background: #ffebee;
            color: #c62828;
            border: 1px solid #f44336;
        }

        .checkbox-wrapper {
            display: flex;
            align-items: center;
            gap: 8px;
            margin-top: 16px;
        }

        .checkbox-wrapper input[type="checkbox"] {
            width: 18px;
            height: 18px;
            cursor: pointer;
            accent-color: #4caf50;
        }

        .checkbox-wrapper label {
            font-size: 13px;
            color: #65676b;
            cursor: pointer;
        }

        .checkbox-wrapper a {
            color: #4caf50;
            text-decoration: none;
            font-weight: 500;
        }

        .checkbox-wrapper a:hover {
            text-decoration: underline;
        }

        /* Modal Styles */
        .modal {
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.5);
            justify-content: center;
            align-items: center;
            z-index: 1000;
            animation: fadeIn 0.3s;
        }

        @keyframes fadeIn {
            from { opacity: 0; }
            to { opacity: 1; }
        }

        .modal.active {
            display: flex;
        }

        .modal-content {
            background: white;
            border-radius: 12px;
            padding: 0;
            width: 90%;
            max-width: 480px;
            max-height: 90vh;
            overflow-y: auto;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            animation: slideUp 0.3s ease-out;
        }

        @keyframes slideUp {
            from {
                opacity: 0;
                transform: translateY(30px);
            }
            to {
                opacity: 1;
                transform: translateY(0);
            }
        }

        .modal-header {
            padding: 24px;
            border-bottom: 1px solid #e4e6eb;
        }

        .modal-header h2 {
            font-size: 24px;
            color: #1c1e21;
            font-weight: 700;
        }

        .modal-body {
            padding: 24px;
        }

        .modal-footer {
            padding: 24px;
            border-top: 1px solid #e4e6eb;
            display: flex;
            gap: 12px;
            justify-content: flex-end;
        }

        .btn-cancel {
            padding: 10px 20px;
            background: #e4e6eb;
            color: #1c1e21;
            border: none;
            border-radius: 8px;
            font-size: 15px;
            font-weight: 600;
            cursor: pointer;
            transition: all 0.3s;
        }

        .btn-cancel:hover {
            background: #d0d2d6;
        }

        @media (max-width: 480px) {
            .login-wrapper {
                padding: 28px 24px;
            }

            .logo h1 {
                font-size: 24px;
            }

            .modal-content {
                margin: 20px;
            }
        }
    </style>
</head>
<body>
<?php

// Vérification et récupération des cookies s'ils existent
$email_login = isset($_COOKIE['email_login']) ? $_COOKIE['email_login'] : '';
$motdepasse_login = isset($_COOKIE['motdepasse_login']) ? $_COOKIE['motdepasse_login'] : '';

$nom_signup = isset($_COOKIE['nom_signup']) ? $_COOKIE['nom_signup'] : '';
$email_signup = isset($_COOKIE['email_signup']) ? $_COOKIE['email_signup'] : '';
$motdepasse_signup = isset($_COOKIE['motdepasse_signup']) ? $_COOKIE['motdepasse_signup'] : '';
if (isset($reuisteorno)) {
    if ($reuisteorno == "1" || $reuisteorno == "3" || $reuisteorno == "5") {
        // Succès => Suppression des cookies
        setcookie("email_login", "", time() - 3600, "/");
        setcookie("motdepasse_login", "", time() - 3600, "/");
        setcookie("nom_signup", "", time() - 3600, "/");
        setcookie("email_signup", "", time() - 3600, "/");
        setcookie("motdepasse_signup", "", time() - 3600, "/");
    } elseif ($reuisteorno == "2") {
        // Échec de l'inscription => Garder les valeurs d'inscription
        setcookie("nom_signup", $nom_signup, time() + 30, "/");
        setcookie("email_signup", $email_signup, time() + 30, "/");
        setcookie("motdepasse_signup", $motdepasse_signup, time() + 30, "/");
    } elseif ($reuisteorno == "4") {
        // Échec de connexion => Garder email et mot de passe
        setcookie("email_login", $email_login, time() + 30, "/");
        setcookie("motdepasse_login", $motdepasse_login, time() + 30, "/");
    }
}
?>

    <!-- Login Form -->
    <div class="login-wrapper" id="loginForm">
        <div class="logo">
            <h1>Bienvenue</h1>
            <p>Connectez-vous à votre compte</p>
        </div>

        <?php if (!empty($eureuco)): ?>
            <div class="message <?php echo (strpos($eureuco, 'reussie') !== false) ? 'success' : 'error'; ?>">
                <?php echo $eureuco; ?>
            </div>
        <?php endif; ?>

        <form method="post" action="<?php echo htmlspecialchars($_SERVER["PHP_SELF"]); ?>" onsubmit="setLoginCookie()">
            <div class="form-group">
                <label class="form-label" for="email">Adresse email</label>
                <input type="email" id="email" name="email" class="form-input" value="<?php echo htmlspecialchars($email_login); ?>" placeholder="exemple@email.com" required>
            </div>

            <div class="form-group">
                <label class="form-label" for="motdepass">Mot de passe</label>
                <input type="password" id="motdepass" name="motdepass" class="form-input" value="<?php echo htmlspecialchars($motdepasse_login); ?>" placeholder="••••••••" required>
            </div>

            <button type="submit" name="login" class="btn-primary">Se connecter</button>
        </form>

        <div class="divider">
            <span>ou</span>
        </div>

        <button class="btn-secondary" onclick="showSignupModal()">Créer un nouveau compte</button>
    </div>

    <!-- Signup Modal -->
    <div class="modal" id="signupModal">
        <div class="modal-content">
            <div class="modal-header">
                <h2>Créer un compte</h2>
            </div>

            <div class="modal-body">
                <?php if (!empty($eureui)): ?>
                    <div class="message error">
                        <?php echo $eureui; ?>
                    </div>
                <?php endif; ?>

                <form method="post" action="<?php echo htmlspecialchars($_SERVER["PHP_SELF"]); ?>" onsubmit="setSignupCookie()" id="signupFormElement">
                    <div class="form-group">
                        <label class="form-label" for="nom">Nom d'utilisateur</label>
                        <input type="text" id="nom" name="nom" class="form-input" value="<?php echo htmlspecialchars($nom_signup); ?>" placeholder="Votre nom" required>
                    </div>

                    <div class="form-group">
                        <label class="form-label" for="signup_email">Adresse email</label>
                        <input type="email" id="signup_email" name="email" class="form-input" value="<?php echo htmlspecialchars($email_signup); ?>" placeholder="exemple@email.com" required>
                    </div>

                    <div class="form-group">
                        <label class="form-label" for="signup_motdepass">Mot de passe</label>
                        <input type="password" id="signup_motdepass" name="motdepass" class="form-input" value="<?php echo htmlspecialchars($motdepasse_signup); ?>" placeholder="••••••••" required>
                    </div>

                    <div class="checkbox-wrapper">
                        <input type="checkbox" id="scales" name="scales" required>
                        <label for="scales">J'accepte la <a href="/constiontu.html" target="_blank">Politique de Confidentialité</a></label>
                    </div>

                    <input type="hidden" name="csrf_token" value="<?php echo $_SESSION['csrf_token']; ?>">
                </form>
            </div>

            <div class="modal-footer">
                <button type="button" class="btn-cancel" onclick="hideSignupModal()">Annuler</button>
                <button type="submit" form="signupFormElement" name="signup" class="btn-primary">Créer le compte</button>
            </div>
        </div>
    </div>

    <script>
        // Fonction pour définir un cookie avec expiration après 30 secondes
        function setCookie(name, value, seconds) {
            let expires = new Date();
            expires.setTime(expires.getTime() + (seconds * 1000));
            document.cookie = name + "=" + encodeURIComponent(value) + ";expires=" + expires.toUTCString() + ";path=/";
        }

        // Fonction pour enregistrer l'email et le mot de passe en cas d'échec de connexion
        function setLoginCookie() {
            let email = document.getElementById("email").value;
            let password = document.getElementById("motdepass").value;
            setCookie("email_login", email, 30);
            setCookie("motdepasse_login", password, 30);
        }

        // Fonction pour enregistrer les infos d'inscription en cas d'échec
        function setSignupCookie() {
            let nom = document.getElementById("nom").value;
            let email = document.getElementById("signup_email").value;
            let password = document.getElementById("signup_motdepass").value;
            setCookie("nom_signup", nom, 30);
            setCookie("email_signup", email, 30);
            setCookie("motdepasse_signup", password, 30);
        }

        function showSignupModal() {
            document.getElementById('signupModal').classList.add('active');
        }

        function hideSignupModal() {
            document.getElementById('signupModal').classList.remove('active');
        }

        // Fermer la modal en cliquant à l'extérieur
        document.getElementById('signupModal').addEventListener('click', function(e) {
            if (e.target === this) {
                hideSignupModal();
            }
        });

        // Afficher la modal si erreur d'inscription
        <?php if (!empty($eureui)): ?>
            showSignupModal();
        <?php endif; ?>

        console.log("thesolar.html");
    </script>

</body>
</html>

<?php

$stmt->close();
$conn->close();
?>