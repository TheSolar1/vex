<?php

session_start();  // Assurez-vous que la session est demarree

include '/var/www/html/access_control.php';
//debegu ini_set('display_errors', 1);
//ini_set('display_startup_errors', 1);
//error_reporting(E_ALL);
$connecte = false;

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
                     
$connecte ="1";


		if ($_SERVER["REQUEST_METHOD"] == "POST" && $connecte == "1") {
		   // Exemple d'utilisation : genere une chaene hexadecimale de 10 caracteres
		 $page_name = preg_replace("/[^a-zA-Z0-9_-]/", "", $_POST['page_name']);
		 $text = $_POST['content'];
		 $pageurl  = md5(uniqid(rand(), true));
		  
		 // Ouvrir le fichier "bonjour" en mode ecriture
		 $handle = fopen($pageurl .".php", "w");
		
					//debegu echo $text;
			
			$result = [];
			 

			//debegu echo "Texte rezu pour analyse : $text<br>";

			// Expressions regulieres pour les conditions IF et NOT
			$patterns = [
				 'ipif' => '/<strong>Condition:<\/strong>\s*Si l\'adresse IP est\s*([0-9\.]+)(.*?)<hr>/is',
				'ipnot' => '/<strong>Condition:<\/strong>\s*Si l\'adresse IP n\'est pas\s*([0-9\.]+)(.*?)<hr>/is',
				'dateif' => '/<strong>Condition:<\/strong>\s*Si la date est\s*([0-9\/]+)(.*?)<hr>/is',
				'datenot' => '/<strong>Condition:<\/strong>\s*Si la date n\'est pas\s*([0-9\/]+)(.*?)<hr>/is',
				'userif' => '/<strong>Condition:<\/strong>\s*Si l\'utilisateur est\s*([\w\s]+)(.*?)<hr>/is',
				'usernot' => '/<strong>Condition:<\/strong>\s*Si l\'utilisateur n\'est pas\s*([\w\s]+)(.*?)<hr>/is',
				'browserif' => '/<strong>Condition:<\/strong>\s*Si le navigateur est\s*([\w\s]+)(.*?)<hr>/is',
				'browsernot' => '/<strong>Condition:<\/strong>\s*Si le navigateur n\'est pas\s*([\w\s]+)(.*?)<hr>/is',
				'osif' => '/<strong>Condition:<\/strong>\s*Si le os est\s*([\w\s]+)(.*?)<hr>/is',
				'osnot' => '/<strong>Condition:<\/strong>\s*Si os n\'est pas\s*([\w\s]+)(.*?)<hr>/is'
			];
			 $script ="";
			 foreach ($patterns as $type => $pattern) {
        if (preg_match_all($pattern, $text, $matches, PREG_SET_ORDER)) {
            foreach ($matches as $match) {
                $value = trim($match[1]);  // Valeur de la condition (ex : adresse IP, date)
                $visibleText = isset($match[2]) ? trim(strip_tags($match[2])) : 'Texte non specifie';

                // Creer le code PHP conditionnel en fonction du type
                if ($type == "ipif") {
                    $script .= "<?php if ('$value' === \$_SERVER['REMOTE_ADDR']) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "dateif") {
                    $script .= "<?php if ('$value' === date('d/m/Y')) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "userif") {
                    $script .= "<?php if ('$value' === \$_SESSION['username']) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "ipnot") {
                    $script .= "<?php if ('$value' !== \$_SERVER['REMOTE_ADDR']) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "datenot") {
                    $script .= "<?php if ('$value' !== date('d/m/Y')) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "usernot") {
                    $script .= "<?php if ('$value' !== \$_SESSION['username']) { echo \"$visibleText\"; } ?>\n";
                
                } elseif ($type == "browserif") {
                    $script .= "<?php if ('$value' !== \$br ) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "browsernot") {
                    $script .= "<?php if ('$value' !== \$br ) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "osif") {
                    $script .= "<?php if ('$value' !== \$os ) { echo \"$visibleText\"; } ?>\n";
                } elseif ($type == "osnot") {
                    $script .= "<?php if ('$value' !== \$os ) { echo \"$visibleText\"; } ?>\n";
                }
                 $text = preg_replace($pattern, '', $text);
                echo $text;
				 if (!is_null($text)) {
					$text = preg_replace("/<hr>/", '', $text);
					$text = preg_replace("/<p><!-- Condition IF --><\/p>/", '', $text);
					$text = preg_replace("/<p>/&nbsp;<\/p>/", '', $text);
					$text = preg_replace("/<p>\<!-- Condition NOT -->\<\/p>/", '', $text);
				}
            }
		}
	}	 
	 $text = preg_replace($pattern, '', $text);
	 $textb = "<?php include '/var/www/html/access_control.php'; include '/var/www/html/sitec/pages/polular.php'; ?>
	 <html lang='fr'>
		<head>
			<meta charset='UTF-8'>
			<meta name='viewport' content='width=device-width, initial-scale=1.0'>
			<link rel='icon' type='image/png' href='/vex.png'>
			<title>page ".$page_name."</title>
		</head>
		<body>
    ".$script."".$text."
    </body>
	</html>";
	  $text = $textb;
				// Boucle pour extraire chaque condition et son texte associe
						
					//debeguecho "Texte reeu pour analyse : $text<br>";
		
						// Si aucune condition trouvee
					
	
	
	
		 // Verifier si le fichier s'est ouvert avec succes
		// Definir le chemin vers le fichier dans le dossier "pages"
		   $chemin_fichier = '/var/www/html/sitec/pages/' . $pageurl. ".php"; // Utilisez './pages/mon_fichier.txt' si vous etes dans un dossier different

		// Ouvrir le fichier en mode ecriture

		$handle = fopen($chemin_fichier, "w");
		// Verifier si le fichier s'est ouvert avec succes
		if ($handle) {
			// Texte e ecrire dans le fichier


			// ecrire le texte dans le fichier
			fwrite($handle, $text);

			//debeguecho "Texte ecrit avec succes dans le fichier !";
			echo "<p>Page cree avec succes : <a href='/sitec/pages/".$pageurl. ".php'>".$page_name."</a></p>";
			// Fermer le fichier apres l'ecriture
			fclose($handle);
		} else {
			echo "echec de l'ouverture du fichier.";
			 
		}
		$scalese = "1";
		   if (!isset($_POST['scales'])) {
			   $scalese = "0";
			   }
			 $stmt_insert = $conn->prepare("INSERT INTO sitec (urlpage,nompage,`user_id` ,porb) VALUES (?, ?,?,?)");

			 $stmt_insert->bind_param("ssii",$pageurl,$page_name ,$user_id, $scalese);

			 $stmt_insert->execute();

}
}
}
}
}


$stmt->close();

$conn->close();
?>

<!DOCTYPE html>
<html>
<head>
	<script>
        function migrant() {
            window.location.href = "/login/login.php";
        }
    </script>
     <link rel="icon" type="image/png" href="/vex.png">
    <title>CreerPage</title>
    <?php
    if ($connecte === "1") {
        // Charger TinyMCE 8.3.2 en local depuis le dossier js/tinymce
        echo "<script src='js/tinymce/tinymce.min.js'></script>";
    } else {
        echo '<script>migrant() </script>';
        header("Location: /login/login.php");
    }
    ?>
<style>
	 body {
            font-family: Arial, sans-serif;
            background-color: #f4f7fc;
            margin: 0;
            padding: 0;
        }

        .container {
            width: 100%;
            margin: 0 auto;
            padding: 20px;
        }

        h1 {
            text-align: center;
            color: #333;
        }

        form {
            background-color: #fff;
            padding: 20px;
         
          
     
         
        }

        label {
            font-size: 16px;
            font-weight: bold;
            color: #333;
            display: block;
            margin-bottom: 8px;
        }

        /* Ajout du style pour limiter la largeur de page_name */
        #page_name {
            width: calc(100% - 200px); /* 100% de la largeur moins 200px */
            padding: 12px;
            margin-right: 200px;  /* Marge a droite */
           
            border-radius: 5px;
            background-color: #f9f9f9;
            font-size: 16px;
        }

        input[type="text"], textarea {
            width: 100%;
            padding: 12px;
            margin: 10px 0 20px;
            border: 1px solid #ccc;
            border-radius: 5px;
            font-size: 16px;
            background-color: #f9f9f9;
        }

        /* Alignement de la case a cocher avec son label */
        .checkbox-container {
            display: flex;
            align-items: center;
            margin-bottom: 15px;
        }

        input[type="checkbox"] {
            margin-right: 8px;
        }

        .checkbox-container label {
            font-size: 14px;
            color: #666;
            margin: 0;
        }

        input[type="submit"] {
            background-color: #4CAF50;
            color: white;
            padding: 12px 24px;
            border: none;
            border-radius: 5px;
            font-size: 16px;
            cursor: pointer;
            transition: background-color 0.3s ease;
        }

        input[type="submit"]:hover {
            background-color: #45a049;
        }

        textarea {
            height: 150px;
            resize: vertical;
        }

    </style>
</head>
<body>
<form method="post" action="">
    <label for="page_name">Nom de la page :</label>
    <input type="text" id="page_name" name="page_name" maxlength="20" required>
    
    <label for="content">Contenu :</label>
    <textarea id="editor" name="content"></textarea>
    </br>
     </br>
      </br>
       <div class="checkbox-container">
                <input type="checkbox" id="scales" name="scales">
                <label for="scales">Piver</label>
            </div>
    </br>
     </br>
      </br>
    <input type="submit" value="Enregistrer la Page">
</form>
<?php
$br = "";
$os = "";
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
   
<script>
	const userIp = "<?php echo $_SERVER['REMOTE_ADDR']; ?>";
const currentDate = new Date().toLocaleDateString();
const username = "<?php echo $user_nom; ?>";
const navigateur = "<?php echo $br ;?>";
const os = "<?php echo $os ;?>";

tinymce.init({
    selector: '#editor',
    license_key: 'gpl',  // Nécessaire pour TinyMCE 8.x
    plugins: [
        'code', 'anchor', 'autolink', 'charmap', 'codesample', 'emoticons', 
        'image', 'link', 'lists', 'media', 'searchreplace', 'table', 
        'visualblocks', 'wordcount'
    ],
    toolbar: 'conditional_if conditional_not | undo redo | blocks fontfamily fontsize | bold italic underline strikethrough | link image media table | align lineheight | checklist numlist bullist indent outdent | emoticons charmap | removeformat',
    content_style: 'body { font-family:Helvetica,Arial,sans-serif; font-size:14px }',
    promotion: false,  // Désactive les promotions TinyMCE
	
    setup: function (editor) {
        // Bouton pour ajouter une condition "if"
        
        editor.ui.registry.addButton('conditional_if', {
            text: 'Ajouter Condition',
            onAction: function () {
                editor.windowManager.open({
                    title: 'Condition',
                    body: {
                        type: 'panel',
                        items: [
                            {
                                type: 'selectbox',
                                name: 'conditionType',
                                label: 'Type de Condition',
                                items: [
                                    { value: 'ip', text: 'Adresse IP' },
                                    { value: 'date', text: 'Date actuelle' },
                                    { value: 'username', text: 'Nom utilisateur' },
                                    { value: 'navigateur', text: 'navigateur' },
                                    { value: 'os', text: 'os' }
                                ]
                            },
                            {
                                type: 'input',
                                name: 'conditionValue',
                                label: 'Valeur a comparer'
                            }
                        ]
                    },
                    buttons: [
                        { type: 'submit', text: 'Inserer' },
                        { type: 'cancel', text: 'Annuler' }
                    ],
                    onSubmit: function (api) {
                        const data = api.getData();
                        let content = `<hr><!-- Condition IF -->`;
		
                        if (data.conditionType === 'ip') {
                            content += `<p ><strong>Condition:</strong> Si l'adresse IP est ${data.conditionValue}</p>`;
                            if (userIp === data.conditionValue) {
                                content += `<p><em>Contenu visible pour l'IP ${data.conditionValue}</em></p>`;
                            }
                        } else if (data.conditionType === 'date') {
                            content += `<p ><strong>Condition:</strong> Si la date est ${data.conditionValue}</p>`;
                            if (currentDate === data.conditionValue) {
                                content += `<p><em>Contenu visible pour la date ${data.conditionValue}</em></p>`;
                            }
                        } else if (data.conditionType === 'username') {
                            content += `<p"><strong>Condition:</strong> Si l'utilisateur est ${data.conditionValue}</p>`;
                            if (username === data.conditionValue) {
                                content += `<p><em>Contenu visible pour l'utilisateur ${data.conditionValue}</em></p>`;
                            }
                        } else if (data.conditionType === 'navigateur') {
                            content += `<p"><strong>Condition:</strong> Si le navigateur est ${data.conditionValue}</p>`;
                            if (navigateur === data.conditionValue) {
                                content += `<p><em>Contenu visible pour ${data.conditionValue}</em></p>`;
                            }
                        } else if (data.conditionType === 'os') {
                            content += `<p"><strong>Condition:</strong> Si le os est ${data.conditionValue}</p>`;
                            if (os === data.conditionValue) {
                                content += `<p><em>Contenu visible pour ${data.conditionValue}</em></p>`;
                            }
                        }
                        
                        content += `<hr>`;
                        
                   
                        editor.insertContent(content);
                        api.close();
                    }
                });
            }
        });
        
        // Bouton pour ajouter une condition "not"
        editor.ui.registry.addButton('conditional_not', {
            text: 'Ajouter Negation',
            onAction: function () {
                editor.windowManager.open({
                    title: 'Condition de Negation',
                    body: {
                        type: 'panel',
                        items: [
                            {
                                type: 'selectbox',
                                name: 'conditionType',
                                label: 'Type de Condition',
                                items: [
                                    { value: 'ip', text: 'Adresse IP' },
                                    { value: 'date', text: 'Date actuelle' },
                                    { value: 'username', text: 'Nom utilisateur' },
                                    { value: 'navigateur', text: 'navigateur' },
                                    { value: 'os', text: 'os' }
                                ]
                            },
                            {
                                type: 'input',
                                name: 'conditionValue',
                                label: 'Valeur e comparer'
                            }
                        ]
                    },
                    buttons: [
                        { type: 'submit', text: 'Inserer' },
                        { type: 'cancel', text: 'Annuler' }
                    ],
                    onSubmit: function (api) {
                        const data = api.getData();
                        let content = `<hr><!-- Condition NOT -->`;

                        if (data.conditionType === 'ip') {
                            content += `<p><strong>Condition:</strong> Si l'adresse IP n'est pas ${data.conditionValue}</p>`;
                            if (userIp !== data.conditionValue) {
                              
                            }
                        } else if (data.conditionType === 'date') {
                            content += `<p><strong>Condition:</strong> Si la date n'est pas ${data.conditionValue}</p>`;
                            if (currentDate !== data.conditionValue) {
                             
                            }
                        } else if (data.conditionType === 'navigateur') {
                            content += `<p><strong>Condition:</strong> Si le navigateur n'est pas ${data.conditionValue}</p>`;
                            if (navigateur !== data.conditionValue) {
						   }
                         } else if (data.conditionType === 'username') {
                            content += `<p><strong>Condition:</strong> Si l'utilisateur n'est pas ${data.conditionValue}</p>`;
                            if (username !== data.conditionValue) {
						   }
					   } else if (data.conditionType === 'os') {
                            content += `<p><strong>Condition:</strong> Si os n'est pas ${data.conditionValue}</p>`;
                            if (os !== data.conditionValue) {
						   }
					   }
					   
                        content += `<hr>`;
                        content += `<p><!-- fin Condition NOT --></p>`;
                        
                        editor.insertContent(content);
                        api.close();
                    }
                });
            }
        });
    }
});

</script>
</body>
</html>