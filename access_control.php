<?php
error_reporting(E_ERROR | E_PARSE);


session_start();
  $user_idcokier = null;
        $user_nom = null;
        $etat_connexion = "La session a expire. Veuillez vous reconnecter."; // Message par defaut
        $etat_connexion_utf8 = mb_convert_encoding($etat_connexion, 'UTF-8');
        $ipAddress = $_SERVER['REMOTE_ADDR'];
        $macAddress = $_SERVER['HTTP_USER_AGENT'];
        $page = $_SERVER['PHP_SELF'];
        $id = bin2hex(random_bytes(5));   
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
   $user_privilege = "10";
   $user_id="3";
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
                 $current_path = $_SERVER['REQUEST_URI']; // Ex : /admin/subfolder/page.php
				
              
                 
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
			 }
		 }
	 }
   
                        $current_path = $_SERVER['REQUEST_URI'];

                        // Extraire juste le chemin sans query string
                        $current_path = parse_url($current_path, PHP_URL_PATH);

                        // Chemin absolu du fichier actuel
                        $current_absolute_path = $_SERVER['SCRIPT_FILENAME'];

                        $stmt_bloque = $conn->prepare("SELECT iduserb, pageb, priviautro FROM bloqpage WHERE (iduserb=? OR iduserb='all')");
                        $stmt_bloque->bind_param("i", $user_id);
                        $stmt_bloque->execute();
                        $result_bloque = $stmt_bloque->get_result();

                        if ($result_bloque->num_rows > 0) {
                            while ($row_bloque = $result_bloque->fetch_assoc()) {
                                $blocked_pages = explode(',', $row_bloque['pageb']);
                                $priviautro = $row_bloque['priviautro'];
                                
                                if ($user_privilege > $priviautro) {
                                    foreach ($blocked_pages as $blocked_page) {
                                        $blocked_page = trim($blocked_page);
                                        
                                        if (empty($blocked_page)) continue;
                                        
                                        $is_blocked = false;
                                        
                                        // Cas 1 : Le blocked_page est un chemin absolu (commence par /)
                                        if (strpos($blocked_page, '/var/www/html') === 0 || strpos($blocked_page, '/home/') === 0) {
                                            // Comparer avec le chemin absolu du fichier actuel
                                            if (strpos($current_absolute_path, $blocked_page) !== false) {
                                                $is_blocked = true;
                                            }
                                            
                                            // Ou convertir en chemin URI et comparer
                                            $blocked_uri = str_replace('/var/www/html', '', $blocked_page);
                                            if (strpos($current_path, $blocked_uri) !== false) {
                                                $is_blocked = true;
                                            }
                                        }
                                        // Cas 2 : Le blocked_page est un chemin URI relatif (commence par /)
                                        else if (strpos($blocked_page, '/') === 0) {
                                            // Comparer directement avec l'URI
                                            if (strpos($current_path, $blocked_page) !== false) {
                                                $is_blocked = true;
                                            }
                                        }
                                        // Cas 3 : Le blocked_page est juste un nom de dossier/fichier
                                        else {
                                            // Vérifier dans l'URI et le chemin absolu
                                            if (strpos($current_path, $blocked_page) !== false || 
                                                strpos($current_absolute_path, $blocked_page) !== false) {
                                                $is_blocked = true;
                                            }
                                        }
                                        
                                        if ($is_blocked) {
                                            die("Accès refusé.");
                                        }
						// Verifie si "all" est specifie pour bloquer toutes les pages
                        if ($blocked_page === "all"  ) {
                            die("Acces refuse pour toutes les pages.");
						}
						if (strpos($current_path, $blocked_page)){
							  die("Acces refuse.pour se compte ");
							}
					}
					}
				}
			}
            function aquete($resaison) {
                $servername = "localhost";
                $username = "orsql";
                $password = "iDq]25F0u8v*z[1d";
                $dbname = "user";
                $conn = new mysqli($servername, $username, $password, $dbname);

                // Verification de la connexion
                if ($conn->connect_error) {
                    die("La connexion a echoue : " . $conn->connect_error);
                }
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
                 $current_path = $_SERVER['REQUEST_URI']; // Ex : /admin/subfolder/page.php
				
              
                 
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
			 }
		 }
	 }
                 $current_path = $_SERVER['REQUEST_URI'];
            // Definir la variable $resaisonpa correctement
            $resaisonpa = $current_path . $resaison;

            // Verifier si la variable $user_idcokier est definie et non vide
            if (!empty($user_idcokier)) {
                // Si $user_idcokier est deja defini, on le garde tel quel
            } else {
                // Si $user_idcokier n'est pas defini, on lui donne la valeur "pas"
                $user_idcokier = "pas";
            }

            // Preparer la requete SQL sans les backticks
            $sql = "INSERT INTO `sus-hac` (`id-c`, auteur) VALUES (?, ?)";

            // Preparer et executer la requete avec les parametres
            $stmt = $conn->prepare($sql);
              if ($stmt === false) {
        die('Erreur de preparation de la requete: ' . $conn->error);
    }
            // Lier les parametres (ici deux chaines de caracteres)
            $stmt->bind_param("ss", $user_idcokier, $resaisonpa);

            // Executer la requete
            $stmt->execute();
			// Fermeture de la connexion
             $stmt->close();
        }
       /*
        // Executer la commande ifconfig pour obtenir l'adresse MAC
        $mac = exec('ifconfig | grep -o -E "([0-9a-f]{2}:){5}([0-9a-f]{2})" | head -n 1');
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
*/
        
$conn->close();
                     ?>
