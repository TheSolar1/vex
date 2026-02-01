<?php
error_reporting(E_ERROR | E_PARSE);


session_start(); // Demarre la session

// Connexion a la base de donnees
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

// Definir le jeu de caracteres a utf8mb4
if (!$conn->set_charset("utf8mb4")) {
    die("e");
}

// Verification de la connexion
if ($conn->connect_error) {
    die("e " );
}
   $enpalcementpages = $_SERVER['REQUEST_URI'];

   $enpalcementpages = str_replace('.php', '', $enpalcementpages);
    $enpalcementpages = str_replace('/sitec/pages/', '', $enpalcementpages);
      // Mettre a jour la visibilite de la page (publique/privee)
        $stmt = $conn->prepare("UPDATE sitec SET popular  = popular +1 WHERE urlpage = ?");
        $stmt->bind_param("s",  $enpalcementpages);
         $stmt->execute();
   
  
// Fermeture de la connexion
$stmt->close();
$conn->close();
$agent = $_SERVER['HTTP_USER_AGENT']; 
if(preg_match('/Firefox/i',$agent)) $br = 'Firefox'; 
elseif(preg_match('/Mac/i',$agent)) $br = 'Mac';
elseif(preg_match('/Chrome/i',$agent)) $br = 'Chrome'; 
elseif(preg_match('/Opera/i',$agent)) $br = 'Opera'; 
elseif(preg_match('/MSIE/i',$agent)) $br = 'IE'; 

else $bs = 'Unknown'; 
if(preg_match('/Linux/i',$agent)) $os = 'Linux';
elseif(preg_match('/Mac/i',$agent)) $os = 'Mac'; 
elseif(preg_match('/iPhone/i',$agent)) $os = 'iPhone'; 
elseif(preg_match('/iPad/i',$agent)) $os = 'iPad'; 
elseif(preg_match('/Droid/i',$agent)) $os = 'Droid'; 
elseif(preg_match('/Unix/i',$agent)) $os = 'Unix'; 
elseif(preg_match('/Windows/i',$agent)) $os = 'Windows';
else $os = 'Unknown';
?>
