<!DOCTYPE html>
<html lang="fr">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>.hopto.org</title>
    <link rel="icon" sizes="2x2" type="image/png" href="/vex.png">
    <style>
        
		body {
    font-family: Arial, sans-serif;
    margin: 20px;
    background-color: #f5f5f5;
}
.selection-form {
    display: flex;
    flex-direction: column;
   
    max-width: 500px;
    margin: 0 auto;
    padding: 20px;
 
    border-radius: 8px;
  
}

.selection-form h3 {
    font-size: 18px;
    color: #333;
    margin-bottom: 10px;
}

.selection-form label {
    font-size: 16px;
    margin-bottom: 8px;
    color: #555;
}

.selection-form input[type="radio"] {
    margin-right: 10px;
}

.selection-form button {
    width: 100%;
    padding: 10px;
    background-color: #4CAF50;
    border: none;
    color: white;
    font-size: 16px;
    border-radius: 5px;
    cursor: pointer;
    box-sizing: border-box;
    margin-top: 20px;
}

.selection-form button:hover {
    background-color: #45a049;
}

/* Pour le fond de la page lorsque le pop-up est visible */
body.modal-open {
    overflow: hidden; /* Empêche le défilement */
}
/* search-form------------------------------------------------------------------------------------------------------*/
.search-form {
    width: 100%;
    max-width: 500px; /* Largeur maximale du formulaire */
    margin: 0 auto; /* Centrer le formulaire */
    padding: 20px;
  
    border-radius: 8px;
  
}

.form-group {
    margin-bottom: 20px; /* Espacement entre les éléments du formulaire */
}

.form-input, .form-select {
    width: 100%; /* Largeur de 100% pour les champs */
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 5px;
    font-size: 16px;
    box-sizing: border-box;
}

.form-input::placeholder, .form-select option {
    color: #888; /* Couleur pour les textes de placeholder et options */
}

.form-input:focus, .form-select:focus {
    border-color: #4CAF50; /* Bordure verte au focus */
    outline: none; /* Supprime l'outline */
}

/* Style du bouton */
.form-button {
    width: 100%; /* Largeur du bouton */
    padding: 10px;
    background-color: #4CAF50; /* Couleur de fond verte */
    border: none;
    color: white;
    font-size: 18px;
    border-radius: 5px;
    cursor: pointer;
    box-sizing: border-box;
}

.form-button:hover {
    background-color: #45a049; /* Couleur du bouton au survol */
}

/* Pour les petits écrans (Mobile) */
@media screen and (max-width: 600px) {
    .search-form {
        width: 90%; /* Formulaire plus large sur mobile */
    }
    .form-button {
        font-size: 16px; /* Taille du texte réduite sur mobile */
    }
}
h1, h2 {
    color: #333;
}

.dossier-item {
    background-color: #fff;
    border: 1px solid #ddd;
    border-radius: 5px;
    padding: 15px;
    margin-bottom: 15px;
 
}

.dossier-item ul {
    list-style-type: none;
    padding: 0;
}

.dossier-item ul li {
    background-color: #f9f9f9;
    border: 1px solid #ddd;
    border-radius: 3px;
    padding: 5px 10px;
    margin-bottom: 5px;
}

.dossier-item ul li:last-child {
    margin-bottom: 0;
}
a,.ichiernom {
	 text-decoration: none;
    color: #000;
    font-size: 18px;
     white-space: nowrap; /* Empeche le texte de se couper en plusieurs lignes */
    overflow: hidden; /* Cache le texte qui depasse */
    text-overflow: ellipsis; /* Ajoute "..." si le texte depasse la largeur */
	}
	.ichiernom {
    width: 200px; /* Largeur fixe de la boete */
   /* Hauteur fixe de la boete */
    display: flex;
     justify-content: flex-start; /* Aligne le contenu (le lien) a droite */
    font-weight: bold;
    overflow: hidden; /* Cache le texte qui depasse */

   
    border-radius: 5px;
    margin-right: 10px;
}
.page-box , li {
	  display: inline-block;
	
	}
	li {
		width:400px;
		 font-weight: bold;
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
.user-name {
	 font-weight: bold;
	}
	.addpage {
  font-weight: bold;
    background-color: #4CAF50; /* Fond similaire a un <li> */
    border: 1px solid #ddd; /* Bordure legere */
    border-radius: 5px; /* Coins arrondis */
    padding: 1; /* Pas de padding interne (on ajuste avec Flexbox) */
    margin-bottom: 5px; /* Espacement sous la box */
    width: 422px; /* Largeur fixe */
    height: 32px; /* Hauteur fixe pour eviter le debordement */
    display: flex; /* Utilisation de Flexbox */
    align-items: center; /* Centre verticalement le texte */
    justify-content: center; /* Centre horizontalement le texte */
    box-sizing: border-box; /* Inclut la bordure dans les dimensions */
    text-align: center; /* Centre le texte */
    color :#ffffff;
}
.addpage:hover {
            background-color: #45a049;
        }
.addpage p {
    margin: 0; /* Retire les marges par defaut du <p> */
}
.popup {
        display: none; /* Cache par défaut */
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background-color: rgba(0, 0, 0, 0.5);
        justify-content: center;
        align-items: center;
    }

    .popup-content {
      
        background-color: white;
        padding: 20px;
        border-radius: 5px;
        max-width: 570px;
        text-align: center;
        position: relative;
    }

        .popup .close {
            position: absolute;
            top: 10px;
            right: 10px;
            color: #aaa;
            font-size: 20px;
            cursor: pointer;
        }
.lipage {
	width:300px;
		 font-weight: bold;
	
	}
    .dossier-item ul li .icon {
  width: 24px; /* Taille d'icône ajustée */
  height: 24px;
  margin-right: 10px;
}

.icon {
  width: 12 px; /* Taille d'icône ajustée */
  height: 12px;
  margin-right: 10px;
}
#icon {
    margin-right: 21px;
  width: 34 px; /* Taille d'icône ajustée */
  height: 12px;
  margin-left: 5px;
  font-size: 1.0rem;
}
.ichiernom {


}
.vst a {
    margin-left: 10px;
}

.ichiernom {
    flex-grow: 1; /* Permet au nom de prendre l'espace restant */
}

.btn-supprimer {
    margin-left: auto;
}
.btn-supprimer:hover {
    color: #7f0000;
}
.Télécharger, .Visualiser {
    margin-left: 10px;
}

.header-container {
    display: flex;
    align-items: center; /* Alignement vertical */
    justify-content: space-between; /* Espacement horizontal */
    margin-bottom: 20px; /* Facultatif : espace sous le conteneur */
}

.h2etheader {
    margin: 0; /* Retirer les marges par défaut du titre */
    font-size: 24px; /* Ajustez la taille si nécessaire */
}

.btn-gerer-acces {
    background-color: #4CAF50;
    color: white;
    border: none;
    padding: 10px 15px;
    border-radius: 5px;
    cursor: pointer;
    font-weight: bold;
}
#modal-acces {
    display: none;
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 500px;
    max-width: 90%; /* Pour les écrans plus petits */
    background: #ffffff;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
    z-index: 1000;
    padding: 20px;
    border-radius: 8px;
    font-family: Arial, sans-serif;
}

/* Titre de la modale */
#modal-acces h3 {
    margin-top: 0;
    font-size: 20px;
    color: #333;
    border-bottom: 1px solid #eaeaea;
    padding-bottom: 10px;
}

/* Texte dans la modale */
#modal-acces p {
    margin: 10px 0;
    font-size: 16px;
    color: #555;
}

/* Formulaire dans la modale */
#modal-acces form {
    margin: 15px 0;
}

#modal-acces form input[type="text"] {
    width: calc(100% - 22px);
    padding: 10px;
    margin-bottom: 10px;
    border: 1px solid #ccc;
    border-radius: 5px;
    font-size: 14px;
    outline: none;
    transition: border-color 0.3s;
}

#modal-acces form input[type="text"]:focus {
    border-color: #4caf50;
}

#modal-acces form button {
    background-color: #4caf50;
    color: white;
    border: none;
    padding: 10px 15px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 14px;
    transition: background-color 0.3s;
}

#modal-acces form button:hover {
    background-color: #367a39;
}

#modal-acces .submit:hover {
    background-color: #630c0c; /* Couleur au survol */
}
/* Bouton de fermeture */
#modal-acces button {
    background-color: #e0e0e0;
    color: #333;
    border: none;
    padding: 10px 15px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 14px;
    margin-top: 10px;
    transition: background-color 0.3s;
}

#modal-acces button:hover {
    background-color: #c0c0c0;
}

/* Overlay */
.popup-overlay {
    display: none;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.5);
    z-index: 999;
}
</style>
</head>
<script src="/js/fa-local.js" defer></script>
<body>
  
          

               
         
         

<?php
 include '/var/www/html/access_control.php';
 include '/var/www/html/function.php';

ini_set('display_errors', 1);
ini_set('display_startup_errors', 1);
error_reporting(E_ALL);
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
					error_reporting(E_ALL);

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
				 }else {$user_id = 0;}
			 }else {$user_id = 0;}
		 }else {$user_id = 0;}
	 }else {$user_id = 0;}
 			 
		 
	
    
     // Vérifier si le paramètre 'dossier' est présent dans l'URL
     if (isset($_GET['dossier'])) {
        // Récupérer la chaîne des dossiers à partir de l'URL
        $dossier_ids = explode(',', $_GET['dossier']); // Transforme "1,5" en tableau [1, 5]
        $current_dossier_id = $dossier_ids[0]; // Le premier dossier dans l'URL est le dossier actuel
    
        // Préparer la requête pour récupérer les informations du dossier actuel
        $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE iddosier = ?");
        $stmt->bind_param("i", $current_dossier_id);
        $stmt->execute();
        $result = $stmt->get_result();
    
        if ($result->num_rows > 0) {
            $row = $result->fetch_assoc();
            $dois_name = $row['doisernom']; // Nom du dossier actuel
            $cdois_user11_id = $row['userid'];
            $addpageuserid  = $row['addpageuserid'];
       
            $addpageauto = 0;
            if (is_string($addpageuserid)) {
                // Convertir la chaîne en tableau en séparant par les virgules
                $addpageuserid = explode(',', $addpageuserid);
            }
            if ($user_id == $cdois_user11_id || in_array($user_id, $addpageuserid)) { 
                // Afficher le lien de suppression pour les utilisateurs autorisés
                $addpageauto = 1;
            } else {
                // Si l'utilisateur n'est pas autorisé, pas d'action
                $addpageauto = 0;
            }
            // Afficher un bouton pour revenir au dossier précédent (si ce n'est pas le dossier racine)
            if (count($dossier_ids) > 1) {
                // Supprimer le premier dossier de l'URL pour revenir au dossier précédent
                array_shift($dossier_ids); // Retirer le premier élément (dossier actuel)
                $prev_dossier_url = "dossier.php?dossier=" . implode(',', $dossier_ids); // Recomposer l'URL sans le premier dossier
                echo "<div><img src='/sitec/arriere.png' alt='Fichier' class='icon'><a href='" . $prev_dossier_url . "'> Dossier précédent</a></div><br>";
            }
    
            // Récupérer et afficher les sous-dossiers du dossier actuel
            $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE iddosier != ? AND FIND_IN_SET(?, idpage) > 0");
            $stmt->bind_param("ii", $current_dossier_id, $current_dossier_id);
            $stmt->execute();
            $sub_result = $stmt->get_result();
            
            if ($sub_result->num_rows > 0) {
                echo "<ul>";
                while ($sub_row = $sub_result->fetch_assoc()) {
                    $sub_dossier_id = $sub_row['iddosier'];
                    $sub_dossier_name = $sub_row['doisernom'];
    
                    // Lien vers le sous-dossier en ajoutant son ID au début de l'URL
                    $new_dossier_url = "dossier.php?dossier=" . $sub_dossier_id . "," . implode(',', $dossier_ids);
                    echo "<li><a href='" . $new_dossier_url . "'>
                            <img src='/sitec/dossier.png' alt='Sous-dossier' class='icon'> $sub_dossier_name
                          </a></li>";
                }
                echo "</ul>";
            } else {
               
            }
        } else {
         
        }
   
     
    }
if (isset($_GET['dossier'])) {
    $dossier = trim($_GET['dossier']); // Nettoyer la valeur

    // Preparer la requete pour recuperer les informations du dossier
    $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE iddosier = ?");
    $stmt->bind_param("i", $dossier);
    $stmt->execute();
    $result = $stmt->get_result();
	if ($result->num_rows > 0) {
    
} else {
   
}

    // Verifier si des resultats existent
    if ($result->num_rows > 0) {
        //echo "<h1>Dossier : " . htmlspecialchars($dossier) . "</h1>";

        // Parcourir les resultats
        while ($row = $result->fetch_assoc()) {
            $idpage = $row['idpage'];
            $dois_name = $row['doisernom'];
            $cdois_user11_id = $row['userid'];
            $iddois = $row['iddosier'];
            $popluardose = $row['popluardose'];
            $addpageuserid = $row['addpageuserid'];

            $fuser_id = $cdois_user11_id;
            $details = getnameDetails($fuser_id);

            $cidusername = getnameDetails($fuser_id);
        

            echo "<div class='dossier-item'>";

            echo "<div class='header-container'>";
            echo "<h2 class='h2etheader'>Dossier : " . htmlspecialchars($dois_name) . "</h2>";
            if ($user_id == $cdois_user11_id) {
                // Affiche le bouton "Gérer l'accès" si l'utilisateur est le propriétaire
                echo "<button class='btn-gerer-acces' onclick='ouvrirModale($current_dossier_id)'>Gérer l'accès</button>";
            }
            echo "</div>";
            $hrefidusername = "/login/user.php?user=". $cdois_user11_id;
            
            echo "<span class='user-name'onclick='window.location.href=\"$hrefidusername\";'>de $cidusername</span>";
            echo "<p>Popularite : " . htmlspecialchars($popluardose) . "</p>";
            echo "<br>";
          
			  if (!empty($idpage)) {
            $entries = explode(',', $idpage); // Saparer les valeurs par virgule

        
          
            
            if (!empty($idpage)) {
                $entries = explode(',', $idpage); // Séparer les valeurs par virgule
                
                echo "<ul>"; // Ouverture de la liste non ordonnée
                foreach ($entries as $entry) {
                    $entry = trim($entry);
        
                    if (str_starts_with($entry, 'fich:')) { // Fichier
                        $fichier_id = intval(substr($entry, 5)); // Extraire l'ID du fichier
                        $stmt_fichier = $conn->prepare("SELECT nom, visble, `id_utilisateur` FROM fichiers WHERE id = ?");
                        $stmt_fichier->bind_param("i", $fichier_id);
                        $stmt_fichier->execute();
                        $result_fichier = $stmt_fichier->get_result();
                    
                        if ($result_fichier->num_rows > 0) {
                            $fichier = $result_fichier->fetch_assoc();
                            $fichier_nom = $fichier['nom'];
                            $fichier_visble = $fichier['visble'];
                            $fichier_userc = $fichier['id_utilisateur'];
                    
                            // Vérifier si le fichier est visible
                            if ($fichier_visble === 0 && $fichier_userc !== $user_id) {
                                // Si non visible et non associé à l'utilisateur, ne pas afficher
                                return;
                            }

                            $icon_map = [
                                'jpg' => 'fa-file-image',
                                'jpeg' => 'fa-file-image',
                                'png' => 'fa-file-image',
                                'gif' => 'fa-file-image',
                                'mp4' => 'fa-file-video',
                                'webm' => 'fa-file-video',
                                'pdf' => 'fa-file-pdf',
                                'doc' => 'fa-file-word',
                                'docx' => 'fa-file-word',
                                'xls' => 'fa-file-excel',
                                'xlsx' => 'fa-file-excel',
                                'ppt' => 'fa-file-powerpoint',
                                'pptx' => 'fa-file-powerpoint',
                                'txt' => 'fa-file-lines',
                                'csv' => 'fa-file-csv',
                                'zip' => 'fa-file-zipper',
                                'rar' => 'fa-file-zipper',
                                '7z' => 'fa-file-zipper',
                                'sql' => 'fa-database', // Icône pour fichiers SQL
                                'php' => 'fa-file-code',
                                'html' => 'fa-file-code',
                                'css' => 'fa-file-code',
                                'js' => 'fa-file-code',
                                'json' => 'fa-file-code',
                                'xml' => 'fa-file-code',
                                // Icône générique par défaut
                                'default' => 'fa-file',
                            ];
                            $file_extension = strtolower(pathinfo($fichier_nom, PATHINFO_EXTENSION));
                            $file_extension = strtolower(trim(pathinfo($fichier_nom, PATHINFO_EXTENSION)));
                          
                                // Déterminez l'icône à utiliser
                                $file_icon_class = $icon_map[$file_extension] ?? $icon_map['default']; // Utilise "default" si l'extension n'est pas définie

                            // Lien pour supprimer le fichiereee
                            echo "<li class='dossier-item'>";
                            echo "<div class='page-box' style='display: flex; justify-content: space-between; align-items: center;'>";
                           
                            echo "<i class='fa-solid $file_icon_class' aria-hidden='true' id='icon'></i> <div class='ichiernom'> $fichier_nom </div>";
                            
                         
                            // Lien pour Voir ou Télécharger le fichier
                            $file_url = "/tel/tel.php?id=" . urlencode($fichier_id); // URL pour ouvrir le fichier
                            $file_extension = strtolower(pathinfo($fichier_nom, PATHINFO_EXTENSION));
                    
                            // Condition pour l'extension de fichier et afficher "Voir" ou "Télécharger"
                            
                                // Si c'est une image, vidéo ou PDF, offrir un lien pour l'ouvrir (Visualiser)
                                echo "<div class='vst'>";
                               
                               
                               $file_extension = strtolower(trim(pathinfo($fichier_nom, PATHINFO_EXTENSION)));

                               if (in_array($file_extension, ['jpg', 'jpeg', 'png', 'gif', 'mp4', 'webm', 'pdf'])) {
                                   // Si c'est une image, vidéo ou PDF, offrir un lien pour l'ouvrir
                                   echo "<a href='$file_url' target='_blank' title='Visualiser le fichier'><i class='fa-solid fa-eye'></i></a>";
                               } else {
                                   // Pour les autres types de fichiers, proposer un lien pour les afficher en texte brut
                                   echo "<a href='acstionfichier.php?id=" . urlencode($fichier_id) . "&action=view' target='_blank' title='Visualiser le fichier'><i class='fa-solid fa-eye'></i></a>";
                               }
                                // Pour d'autres types de fichiers, offrir un lien pour télécharger
                                echo "<a class='Télécharger' href='$file_url' download title='Télécharger le fichier'><i class='fa-solid fa-download'></i> </a>";
                             
                                // Suppression : Icône à droite
                           //     $addpageuserid = explode(',', $addpageuserid);

                                if ($addpageauto === 1) { 
                                    //oror
                                    echo "<a href='acstionfichier.php?id=" . urlencode($fichier_id) . "' class='btn-supprimer' title='Supprimer le fichier' style='margin-left: auto;'><i class='fa-solid fa-trash'></i></a>";

                                
                                 

                                }
                            
                                 echo "</div>";
                            echo "</div>";
                            echo "</li>";
                            echo "<br>";
                        
                        }
                    
                    

                    }elseif (str_starts_with($entry, 'dos:')) { // Sous-dossier
                        // Extraire l'ID du sous-dossier (ID après "dos:")
                        $sous_dossier_id = intval(substr($entry, 4));
                        
                        // Préparer la requête pour récupérer les informations du sous-dossier
                        $stmt_sous_dossier = $conn->prepare("SELECT doisernom FROM sitecdos WHERE iddosier = ?");
                        $stmt_sous_dossier->bind_param("i", $sous_dossier_id);
                        $stmt_sous_dossier->execute();
                        $result_sous_dossier = $stmt_sous_dossier->get_result();
                        
                        if ($result_sous_dossier->num_rows > 0) {
                            $sous_dossier = $result_sous_dossier->fetch_assoc();
                            $sous_dossier_nom = $sous_dossier['doisernom'];
                            
                            // Affichage du sous-dossier avec le lien pour y accéder
                            echo "<li class='dossier-item'>";
                            echo "<div class='page-box'>";
                            
                            // Ajouter l'ID du sous-dossier au début de l'URL
                            // Ici, on ajoute le sous-dossier ID au début de la chaîne de dossiers
                            $current_dossier_ids = $sous_dossier_id . ',' . implode(',', $dossier_ids); // Sous-dossier ajouté au début
                            $url_sous_dossier = "dossier.php?dossier=" . urlencode($current_dossier_ids);
                            
                            echo "<a href='" . $url_sous_dossier . "' target='_self'>";
                            echo "<img src='/sitec/dossier.png' alt='Sous-dossier' class='icon'> $sous_dossier_nom";
                            echo "</a>";
                            
                            // Bouton pour revenir au dossier précédent (si ce n'est pas le dossier racine)
                            if (count($dossier_ids) > 0) {
                                // Supprimer le premier dossier de l'URL pour revenir au dossier parent
                                array_shift($dossier_ids); // Retirer le premier élément (le sous-dossier actuel)
                                $prev_dossier_url = "dossier.php?dossier=" . implode(',', $dossier_ids); // Recomposer l'URL sans le premier dossier
                                echo "<br><a href='" . $prev_dossier_url . "' class='btn-back'></a>";
                            }
                            
                            echo "</div>";
                            echo "</li>";
                            echo "<br>";
                        }
                    
                    
                    } else { // Page
                        $page_id = intval($entry); // ID de la page
                        $stmt_page = $conn->prepare("SELECT urlpage, nompage, `user_id`,porb  FROM sitec WHERE idpage = ?");
                        $stmt_page->bind_param("i", $page_id);
                        $stmt_page->execute();
                        $result_page = $stmt_page->get_result();
                        if ($result_page->num_rows > 0) {
                            $page = $result_page->fetch_assoc();
                            $page_url = "/sitec/pages/" . $page['urlpage'] . ".php";
                            $page_nom = $page['nompage'];
                            $userapger_id = $page['user_id'];
                            $page_porb = $page['porb'];
                            if ($page_porb === 1) { 
                            if ( $userapger_id ===  $user_id) {
                                echo "<li class='dossier-item'>";
                                echo "<div class='page-box'><a href='$page_url' target='_blank'><img src='/sitec/page.png' alt='Page' class='icon'> $page_nom</a></div>";
                             
                                echo "</li>";
                                echo "<br>";
                            }else {
                                echo "<li class='dossier-item'>";
                                echo "<div class='page-box'><a href='$page_url' target='_blank'><img src='/sitec/page.png' alt='Page' class='icon'> $page_nom</a></div>";
                             
                                echo "</li>";
                                echo "<br>";
                            }
                            // Vérifier la visibilité
                           
                            
                        }
                    
                          
                        }
                    }
                }
                echo "</ul>"; // Fermeture de la liste non ordonnée
            }
            
            // Affichage du bouton "Ajouter un élément"
      
        

            if ($addpageauto === 1) { 
                //oror
                   
                    echo "<div class='addpage' id='open-popup-btn' onclick='onpenn()'>";
                    echo "<p>Ajouter un élément</p>";
                    echo "</div>";
              
                
            } 
        }
        }
        }
    } else {
	
     $stmt = $conn->prepare("SELECT * FROM sitecdos ORDER BY popluardose  DESC");
   
    $stmt->execute();
    $result = $stmt->get_result();
	if ($result->num_rows > 0) {
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
}





?>
<div id="popup" class="popup">
    <div class="popup-content">
        <h2>Rechercher une page, un dossier, ou un fichier</h2>
        <span class="close" id="close-popup-btn">&times;</span>
        
        <!-- Formulaire de recherche -->
       <?php $onpennnnnn = 0;  // Initialisation par défaut
         if (!empty($_POST['recherche'])) {$poupouv = 0;}else {$poupouv = 1 ;}
        if (empty($searchResults )&& $poupouv === 1) {
          
            echo '
           <form method="post" action="" class="search-form">
                <div class="form-group">
                    <input type="text" name="recherche" id="recherche" placeholder="Entrez un nom" class="form-input" />
                </div>

                <div class="form-group">
                    <!-- Dropdown pour choisir le type de recherche -->
                    <select name="search_type" id="search_type" class="form-select">
                        <option value="dossier">Recherche dans les dossiers</option>
                        <option value="page">Recherche dans les pages</option>
                        <option value="document">Recherche dans les documents</option>
                    </select>
                </div>

                <div class="form-group">
                    <!-- Un seul bouton Submit -->
                    <button type="submit" name="submit-selection" class="form-button">Valider la recherche</button>
                </div>
            </form>
            ';
            
        } else {
         
           // Autrement, vous pouvez définir $onpennnnnn = 1
          
        }

?>

        <!-- Affichage des résultats -->
        <div id="search-results">
            <?php
          
          
          if ($_SERVER['REQUEST_METHOD'] === 'POST') {
            if (!empty($_POST['recherche'])) {
                $onpennnnnn = 1;  
                $recherche = $_POST['recherche'];
                $searchType = $_POST['search_type'];
    
                // Afficher le formulaire
                echo '<form method="POST" action="" class="selection-form">';
                
                // Recherche selon le type sélectionné
                if ($searchType == 'dossier') {
                    // Recherche dans les dossiers
                    $stmt = $conn->prepare("SELECT * FROM sitecdos WHERE doisernom LIKE ?");
                    $searchTerm = "%" . $recherche . "%";
                    $stmt->bind_param("s", $searchTerm);
                    $stmt->execute();
                    $searchResults = $stmt->get_result();
                    
                    if ($searchResults->num_rows > 0) {
                        echo '<h3>Sélectionnez un Dossier</h3>';
                        while ($row = $searchResults->fetch_assoc()) {
                            echo '<label><input type="radio" name="dossier_select" value="' . $row['iddosier'] . '"> ' . htmlspecialchars($row['doisernom']) . '</label><br>';
                        }
                    } else {
                        echo "Aucun dossier trouvé.";
                    }
                } elseif ($searchType == 'page') {
                    // Recherche dans les pages
                    $stmt = $conn->prepare("SELECT * FROM sitec WHERE nompage LIKE ?");
                    $searchTerm = "%" . $recherche . "%";
                    $stmt->bind_param("s", $searchTerm);
                    $stmt->execute();
                    $searchResults = $stmt->get_result();
                    
                    if ($searchResults->num_rows > 0) {
                        echo '<h3>Sélectionnez une Page</h3>';
                        while ($row = $searchResults->fetch_assoc()) {
                            echo '<label><input type="radio" name="page_select" value="' . $row['idpage'] . '"> ' . htmlspecialchars($row['nompage']) . '</label><br>';
                        }
                    } else {
                        echo "Aucune page trouvée.";
                    }
                } elseif ($searchType == 'document') {
                    // Recherche dans les documents
                    $stmt = $conn->prepare("SELECT * FROM fichiers WHERE nom LIKE ?");
                    $searchTerm = "%" . $recherche . "%";
                    $stmt->bind_param("s", $searchTerm);
                    $stmt->execute();
                    $searchResults = $stmt->get_result();
                    
                    if ($searchResults->num_rows > 0) {
                        echo '<h3>Sélectionnez un Document</h3>';
                        while ($row = $searchResults->fetch_assoc()) {
                            // Vérifier la visibilité et l'utilisateur
                            if ($row['visble'] == 0 && $user_id == $row['id_utilisateur']) {
                                echo '<label><input type="radio" name="document_select" value="' . $row['id'] . '"> ' . htmlspecialchars($row['nom']) . '</label><br>';
                            }
                        }
                    } else {
                        echo "Aucun document trouvé.";
                    }
                }
                echo '<br>';
                echo '<button type="submit" name="submit-selection" class="form-button">Valider la sélection</button>';
                echo '</form>';
            }
        }
        
            ?>
      </div>
      </div>
    <?php // Gestion de la soumission de la sélection
    if (isset($_POST['submit-selection'])) {
        // Récupérer l'ID sélectionné
        $selectedDossierId =  $dossier;
        $addpageuserid = explode(',', $addpageuserid);
        echo "++++".$addpageauto ;
        if ($addpageauto === 1) { 
           //oror
      
        
            
        if (isset($_POST['dossier_select'])) {
            
          
          
    
            // Vérifier si `idpage` contient déjà des valeurs
            $stmt = $conn->prepare(query: "SELECT idpage FROM sitecdos WHERE iddosier = ?");
            $stmt->bind_param("i", $selectedDossierId);
            $stmt->execute();
            $result = $stmt->get_result();
            $row = $result->fetch_assoc();
            $idpage = $row['idpage'];

            // Ajouter le dossier à `idpage`
            if (!empty($idpage)) {
                $newIdPage = $idpage . ',dos:' . $selectedDossierId;
            } else {
                $newIdPage = 'dos:' . $selectedDossierId;
            }

            // Mettre à jour la table `sitecdos` avec le nouvel ID de page
            $stmt = $conn->prepare("UPDATE sitecdos SET idpage = ? WHERE iddosier = ?");
            $stmt->bind_param("si", $newIdPage, $selectedDossierId);
            $stmt->execute();
            echo "Le dossier a été mis à jour.";
        } elseif (isset($_POST['page_select'])) {

            $selectedPageId = $_POST['page_select'];
            // Vérifier si `idpage` contient déjà des valeurs
            $stmt = $conn->prepare("SELECT idpage FROM sitecdos WHERE idpage LIKE ?");
            $stmt->bind_param("s", "%" . $selectedPageId . "%");
            $stmt->execute();
            $result = $stmt->get_result();
            $row = $result->fetch_assoc();
            $idpage = $row['idpage'];

            // Ajouter la page à `idpage`
            if (!empty($idpage)) {
                $newIdPage = $idpage . ',' . $selectedPageId;
            } else {
                $newIdPage = $selectedPageId;
            }

            // Mettre à jour la table `sitecdos` avec le nouvel ID de page
            $stmt = $conn->prepare("UPDATE sitecdos SET idpage = ? WHERE iddosier = ?");
            $stmt->bind_param("si", $newIdPage, $selectedDossierId);
            $stmt->execute();
            echo "La page a été ajoutée.";
        } elseif (isset($_POST['document_select'])) {
            $selectedDocumentId = $_POST['document_select'];
            // Vérifier si `idpage` contient déjà des valeurs
            $searchPattern = "%fich:" . $selectedDocumentId . "%";

            // Préparer la requête avec la nouvelle chaîne
            $stmt = $conn->prepare("SELECT idpage FROM sitecdos  ");
           
            $stmt->execute();
            $result = $stmt->get_result();
            $row = $result->fetch_assoc();
            if ($row) {
                // Si $row contient une ligne, alors accéder à idpage
                $idpage = $row['idpage'];
                echo "Idpage trouvé : " . $idpage;
            } else {
                // Si aucune ligne n'a été trouvée
                echo "Aucun résultat trouvé pour ce document.";
            }

            // Ajouter le document à `idpage`
            echo "oooeo|=". $selectedDocumentId."|<br>";
            echo "pqskdqksk|=". $idpage."|<br>";
                $newIdPage = $idpage . ',fich:' . $selectedDocumentId;
           
              
            echo $newIdPage;

            // Mettre à jour la table `sitecdos` avec le nouvel ID de page
            $stmt = $conn->prepare("UPDATE sitecdos SET idpage = ? WHERE iddosier = ?");
            $stmt->bind_param("si", $newIdPage, $selectedDossierId);
            $stmt->execute();
           
            if ($stmt->affected_rows > 0) {
                echo "Le fichier a été ajouté et la table a été mise à jour.";
            } else {
                echo "Aucune mise à jour effectuée. Vérifiez si l'id dosier existe.";
            }
        }
    }
}
    
echo "</div>";

//------------------------------------------------------------------------------------------------
if ($_SERVER['REQUEST_METHOD'] === 'POST') {

   if (isset($_POST['action'])) {

    $action = $_POST['action'];
    $dossier_id = intval($_POST['dossier_id']);
    if ($cdois_user11_id  == $user_id) {
    if ($action === 'add_user') {
        $nouvel_utilisateur_nom = trim($_POST['nouvel_utilisateur_nom']);
    
        // Rechercher l'ID de l'utilisateur par son nom
        $stmt = $conn->prepare("SELECT id FROM login WHERE nom = ?");
        $stmt->bind_param("s", $nouvel_utilisateur_nom);
        $stmt->execute();
        $result = $stmt->get_result();
    
        if ($result->num_rows > 0  || $nouvel_utilisateur_nom === "all" ) {
            $row = $result->fetch_assoc();
            if ($nouvel_utilisateur_nom !== "all") {
            $nouvel_utilisateur_id = $row['id'];
            }else {
                $nouvel_utilisateur_id = 0;
            }
            // Ajouter l'ID de l'utilisateur
            $stmt = $conn->prepare("SELECT addpageuserid FROM sitecdos WHERE iddosier = ?");
            $stmt->bind_param("i", $dossier_id);
            $stmt->execute();
            $result = $stmt->get_result();
    
            if ($result->num_rows > 0) {
                $row = $result->fetch_assoc();
                $addpage_users = explode(',', $row['addpageuserid']);
    
                if (!in_array($nouvel_utilisateur_id, $addpage_users)) {
                    $addpage_users[] = $nouvel_utilisateur_id;
                    $nouvelle_liste = implode(',', $addpage_users);
    
                    // Mettre à jour la liste
                    $update_stmt = $conn->prepare("UPDATE sitecdos SET addpageuserid = ? WHERE iddosier = ?");
                    $update_stmt->bind_param("si", $nouvelle_liste, $dossier_id);
                    $update_stmt->execute();
                }
            }
        } else {
            echo "Utilisateur introuvable : $nouvel_utilisateur_nom";
        }
    }
    if ($action === 'change_owner') {
        $nouveau_proprietaire_nom = trim($_POST['nouveau_proprietaire_nom']);
    
        // Rechercher l'ID de l'utilisateur par son nom
        $stmt = $conn->prepare("SELECT id FROM login WHERE nom = ?");
        $stmt->bind_param("s", $nouveau_proprietaire_nom);
        $stmt->execute();
        $result = $stmt->get_result();
    
        if ($result->num_rows > 0) {
            $row = $result->fetch_assoc();
            $nouveau_proprietaire_id = $row['id'];
    
            // Mettre à jour le propriétaire
            $stmt = $conn->prepare("UPDATE sitecdos SET userid = ? WHERE iddosier = ?");
            $stmt->bind_param("ii", $nouveau_proprietaire_id, $dossier_id);
            $stmt->execute();
        } else {
            echo "Propriétaire introuvable : $nouveau_proprietaire_nom";
        }
    }
}else {
    $resaison = " a tente de post inlagement code=1 linge=1156";
    aquete($resaison);
}
   }
}


?>

<!-- Fenêtre modale pour gérer l'accès -->
<div id="modal-acces" >
    <h3>Gérer l'accès</h3>

    <?php
    // Charger dynamiquement les informations pour la modale
    $stmt = $conn->prepare("SELECT userid, addpageuserid FROM sitecdos WHERE iddosier = ?");
    $stmt->bind_param("i",  $current_dossier_id);
    $stmt->execute();
    $result = $stmt->get_result();
    if ($result->num_rows > 0) {
        $row = $result->fetch_assoc();
        $proprietaire_id = $row['userid'];
        $addpage_users = explode(',', $row['addpageuserid']);
        $accessible_par_tout = in_array(0, $addpage_users);
    
        // Récupérer le nom du propriétaire
        $stmt = $conn->prepare("SELECT nom FROM login WHERE id = ?");
        $stmt->bind_param("i", $proprietaire_id);
        $stmt->execute();
        $result = $stmt->get_result();
        $row = $result->fetch_assoc();
        $nom_proprietaire = $row['nom'];
    
        echo "<p>Propriétaire actuel : $nom_proprietaire</p>";
        echo "<p>Accès actuel : ";
        if ($accessible_par_tout) {
            echo "Tous les utilisateurs";
        } else {
            $nom_utilisateurs = [];
    
            // Récupérer les noms des utilisateurs ayant accès
            foreach ($addpage_users as $user_id) {
                if (!empty($user_id) && is_numeric($user_id)) {
                    $stmt->bind_param("i", $user_id);
                    $stmt->execute();
                    $result = $stmt->get_result();
                    if ($result->num_rows > 0) {
                        $row = $result->fetch_assoc();
                        $nom_utilisateurs[] = $row['nom'];
                    } else {
                        $nom_utilisateurs[] = "Utilisateur inconnu (ID $user_id)";
                    }
                }
            }
    
            echo "Utilisateurs spécifiques : " . implode(', ', $nom_utilisateurs);
        }
        echo "</p>";
    }
   
    ?>
     <!-- Formulaire pour ajouter un utilisateur -->
     <form method="POST">
    <input type="hidden" name="action" value="add_user">
    <input type="hidden" name="dossier_id" value="<?php echo $current_dossier_id; ?>">
    <input type="text" name="nouvel_utilisateur_nom" placeholder="Nom d'utilisateur" required>
    <button type="submit">Ajouter</button>
</form>
    <!-- Formulaire pour changer le propriétaire -->
    <form method="POST">
    <input type="hidden" name="action" value="change_owner">
    <input type="hidden" name="dossier_id" value="<?php echo $current_dossier_id; ?>">
    <input type="text" name="nouveau_proprietaire_nom" placeholder="Nom du propriétaire" required>
    <button class="submit" type="submit">Changer</button>
</form>

    <!-- Bouton pour fermer la modale -->
    <button onclick="fermerModale()">Fermer</button>
</div>
<div class="popup-overlay" onclick="fermerModale()"></div>
<script>
    function ouvrirModale(dossierId) {
        document.getElementById('modal-acces').style.display = 'block';
        document.querySelector('.popup-overlay').style.display = 'block';
  
}

function fermerModale() {

    document.getElementById('modal-acces').style.display = 'none';
    document.querySelector('.popup-overlay').style.display = 'none';
  
    
}

   // Attendre que le DOM soit completement charge avant d'ajouter les evenements
        document.addEventListener("DOMContentLoaded", function () {
            
            const closePopupBtn = document.getElementById("close-popup-btn");
            const popup = document.getElementById("popup");
              const openresult = document.getElementById("openresult");

            // Verifier si les elements existent avant d'ajouter les evenements
           

                // Ajouter l'evenement pour fermer le pop-up
                closePopupBtn.addEventListener("click", function () {
                    popup.style.display = "none";
                });

                // Fermer le pop-up si l'utilisateur clique en dehors du contenu
                popup.addEventListener("click", function (event) {
                    if (event.target === popup) {
                        popup.style.display = "none";
                    }
                });
           });
           
           function openresult() {
			  var searchresults = document.querySelectorAll('.searchresults');
			  
			  // Verifiez si des elements ont ete trouves
			  console.log(searchresults);

			  if (searchresults.length > 0) {
				searchresults.forEach(function(element) {
				  element.style.display = "flex"; // Affiche chaque resultat en flex
				});
			  } else {
				console.log("Aucun element trouve.");
			  }
			}
              
            function onpenn () {
         popup.style.display = "flex ";
           }
          
   </script>
   <?php if ($onpennnnnn === 1) {
		 echo '<script>console.log(onpenn());</script>';
		}
		?>


</body>

</html>
