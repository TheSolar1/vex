<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>.hopto.org</title>
    <link rel="icon" sizes="2x2" type="image/png" href="/vex.png">
    <style>
           body {
            font-family: Arial, sans-serif;
    background-color: #f4f4f4;
   
 
    
   
    font-family: Arial, sans-serif;

}
        .user-nom {
                display: inline-block;    
                vertical-align: middle;
            }
            #textBienvenue, 
            .user-nom {
                display: inline-block;    
                vertical-align: middle;
            }

            #textBienvenue {
                font-size: 24px; /* Ajuste la taille pour un meilleur rendu */
                font-weight: bold;
                color: #000;
                line-height: 1.5;
                margin: 0 10px 0 0; /* Espace entre "Bienvenue" et le texte suivant */
            }


            /* Style pour "fona" */
            #fona {
                /* Taille du texte pour l'effet visuel */
                font-weight: bold;
                color: transparent; /* Texte transparent pour voir l'image */
                -webkit-background-clip: text; /* Applique l'image seulement sur le texte */
                background-image: url('/admin/image2.png'), 
                                  url('/admin/image3.png'); /* Deux images superposees */
                background-size: 100% 200%; /* Taille des images pour couvrir le texte */
                background-position: 100% 100%, 100% 200%; /* Position des images */
                background-repeat: no-repeat, no-repeat; /* Empecher la repetition */
                animation: slideImages 6s linear infinite; /* Animation continue des images */
                line-height: 1.5;
                margin: 0; /* Supprime les marges */
            }
            /* Style pour "fona" */
          
        
        /* Animation pour le texte */
        @keyframes slideImages {
            0% {
                background-position: 100% 100%, 100% 200%; /* Image 1 visible, Image 2 hors champ */
            }
            50% {
                background-position: 100% 0%, 100% 100%; /* Image 1 glisse vers le haut, Image 2 arrive */
            }
            100% {
                background-position: 100% -100%, 100% 0%; /* Image 1 hors champ, Image 2 visible */
            }
        }
        .page-item {
    
    margin-bottom: 15px;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 5px;
    background-color: #f9f9f9;
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 80%; /* Reduction de la largeur de l'element */
    max-width: 500px; /* Largeur maximale */
    
    margin-right: 0px;
}

/* Style du nom de la page */
.page-item a {
    font-size: 18px;
    color: #333;
    text-decoration: none;
    font-weight: bold;
}

/* Bouton "..." pour la gestion des pages */
.page-item .manage-button {
    padding: 5px 10px;
    background-color: #ddd;
    color: #333;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 14px;
}

.page-item .manage-button:hover {
    background-color: #bbb;
}
.page-box {
    width: 200px; /* Largeur fixe de la boete */
   /* Hauteur fixe de la boete */
    display: flex;
     justify-content: flex-start; /* Aligne le contenu (le lien) a droite */
    align-items: center;
    overflow: hidden; /* Cache le texte qui depasse */
  
   
    border-radius: 5px;
    margin-right: 10px;
}

.page-box a {
    text-decoration: none;
    color: #000;
    font-size: 18px;
     white-space: nowrap; /* Empeche le texte de se couper en plusieurs lignes */
    overflow: hidden; /* Cache le texte qui depasse */
    text-overflow: ellipsis; /* Ajoute "..." si le texte depasse la largeur */
}
.modalll {
          /* background-color:#4CAF50;*/
          
		  padding: 4px;
    border: 1px solid #ddd;
    border-radius: 5px;
    /* Aligner a gauche */
    display: block; /* Le bouton occupe toute la largeur de son parent */
    margin-left: 0; /* Aligne a gauche */
			
	}
    .dossier-links {
    margin-top: 10px;
}
.dossier-link a {
    text-decoration: none;
    color: #000000;  /* Couleur bleue pour les liens */
    font-size: 14px;
    display: block;  /* Chaque lien occupe une ligne separee */
    margin-bottom: 5px;
    font-weight: bold;
    font-size: 18px;
}
.dossier-link {
	 font-weight: bold;
     margin-bottom: 15px;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 5px;
    background-color: #f9f9f9;
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 80%; /* Reduction de la largeur de l'element */
    max-width: 500px; /* Largeur maximale */
    
    margin-right: 0px;
    }

.dossier-link a:hover {
    text-decoration: underline;
}
</style>
</head>
<body>
    <?php 
    include '/var/www/html/access_control.php';
    include '/var/www/html/function.php';

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
                     $fraiuser_vip = $row['vip'];
                     $fraiuser_id = (int)$row['id'];
                     $fraiuser_privilege = (int)$row['privilege'];
                   
                     }
                   
                    }
                }

            }
        }
        if (isset($_GET['user'])) {
            $urliduser = (int)$_GET['user'];
            
            $stmt = $conn->prepare("SELECT * FROM login WHERE id=?");
            $stmt->bind_param("i",      $urliduser );
            $stmt->execute();
            $result = $stmt->get_result();
                
                if ($result->num_rows === 1) {
              
                $row = $result->fetch_assoc();
               
                $urluser_nom = $row['nom'];
                $urluser_vip = (int)$row['vip'];
                $user_privilege = (int)$row['privilege'];
              

                 $couleur_privilege = "#000000"; // Couleur par defaut au cas oe aucune condition ne correspond
                $nom_privilege = "sans-papiers"; // Nom par defaut au cas oe aucune condition ne correspond

                 $details = getPrivilegeDetails($user_privilege);

                  $nom_privilege = $details['nom_privilege'];
                 $couleur_privilege = $details['couleur_privilege'];
              
              echo "<h1 id='textBienvenue'>profile de :</h1>";
                 if (  $user_privilege == "1") {
					 
                   echo "<h1 id='fona' class='user-nom'>".$urluser_nom."</h1>";
                 }else {
                 echo "<h1 id='textBienvenue' style='color:".$couler_privilege. ";'>".$urluser_nom."</h1>";
                 }
               
                echo "<br>";
                echo "<h3>privilege :".   $nom_privilege ."</h3>";
                if ($fraiuser_privilege > 8) {
                    echo "<h3>nombre privilège =".  $user_privilege.".</h3>"
                    ;}
                    echo "<br>";
                    echo "<br>";
                    echo "<h3>il a cree :</h3>";
                  //  -------------------------------page------------------------------------------------------------------
                 
                 
                  $check_stmt = $conn->prepare("SELECT urlpage, nompage, `user_id`, porb FROM sitec  WHERE `user_id`=? ORDER BY popular DESC");
                  $check_stmt->bind_param("i",   $urliduser );
                  $check_stmt->execute();
                  $result = $check_stmt->get_result();
                  
                  if ($result->num_rows >= 1) {
                   
                      echo "<h2> Pages :</h2>";
                    
                      echo "<br><br>";
                  
                      while ($row = $result->fetch_assoc()) {
                      $pageurl = "/sitec/pages/".$row['urlpage'].".php";
                      $page_name = $row['nompage'];
                      $scalese = $row['porb'];
                      $user1111_id = $row['user_id'];
                      
                      // Determiner la visibilite
                      if ($scalese === 1 ) {
                        if ($fraiuser_id ===    $urliduser) {}else {continue; }

                          $visibility = "Privee";
                      } elseif ($scalese === 0) {
                          $visibility = "Publique";
                      }
                  
                      // Verification de l'ID utilisateur
                     
                          echo "<div class='page-item'>";
                  
                         
                  
                          
                  
                          // Afficher le lien avec la page modifiee
                          echo "<div class='page-box'><a href='$pageurl'>$page_name</a></div>";
                         
                          echo "<span>$visibility</span>";
                  
                          // Bouton d'acces reduit
                          echo " <button class='modalll' onclick='window.location.href=\"$pageurl\";'>Acceder</button>";
                  
                          // Bouton "..." pour gerer la page
                          echo " <button class='manage-button' onclick='window.location.href=\"/sitec/\";'>...</button>";
                  
                          echo "</div>";
                      
                    }
                }
                //---------------------------dos--------------------------------------------
                $stmt = $conn->prepare("SELECT * FROM sitecdos ORDER BY popluardose  DESC");
   
                $stmt->execute();
                $result = $stmt->get_result();
                if ($result->num_rows > 0) {
                     echo "<br>";
                    echo "<h2>dossier :</h2>";
                 while ($row = $result->fetch_assoc()) {
                     $idpage = $row['idpage'];
                    $dois_name = $row['doisernom'];
                    $cdois_user11_id = $row['userid'];
                     $iddois = $row['iddosier'];
                    $popluardose = $row['popluardose'];
                    $addpageuserid = $row['addpageuserid'];
                    
            
            
                   $iddoisst =[];
                      if ($iddois !== "aucune" && !empty($iddois)) {
                        // Si 'doiser' a une ou plusieurs valeurs, les separer et afficher un lien pour chaque
                        $iddoisst = explode(',', $iddois);  // Separer les dossiers par virgule
                        echo "<div class='dossier-links'>";
                        foreach ($iddoisst as $iddois) {
                            
                            $iddois = "/sitec/dossier.php?dossier=" . urlencode(trim($iddois));
                            echo "<div class='dossier-link'>";
                            echo "<a href='$iddois' target='_self'> $dois_name</a>";
                          //  echo "<a href='$iddois' target='_self'class='user-name'>de : $cdois_user11_id</a>";
                            echo "</div>";
                        }
                    }
                }
            }
            //------------------------------------------------------------fihier---------------------------------------------------

                }else {
                    echo "prolement seveur";
                }
        }else {
          
        }

?>
</body>
</html>