<html lang="fr">
<head>
<?php
ob_start();
?>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sitec - Mes Pages</title>
    <link rel="icon" type="image/png" href="/vex.png">
    <script src="/fa-local.js" defer></script>
    <style>
    /* Reset simple */
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

/* Variables adaptatives au thème */
:root {
    --sitec-primary: #4CAF50;
    --sitec-primary-dark: #45a049;
    
    /* Thème clair par défaut */
    --nav-bg: #4CAF50;
    --nav-gradient: linear-gradient(135deg, #66BB6A 0%, #4CAF50 50%, #43A047 100%);
    --sitec-bg: #ffffff;
    --sitec-card-bg: #f9f9f9;
    --sitec-text: #1c1e21;
    --sitec-text-secondary: #65676b;
    --sitec-border: #e4e6eb;
    --sitec-hover: #f0f2f5;
    --sitec-section-title: #1c1e21;
}

/* Mode sombre automatique */
body.dark-theme {
    --nav-bg: #2d5f2e;
    --nav-gradient: linear-gradient(135deg, #2d5f2e 0%, #1e4620 50%, #0d2e0f 100%);
    --sitec-bg: #1a1a1a;
    --sitec-card-bg: #2a2a2a;
    --sitec-text: #e0e0e0;
    --sitec-text-secondary: #b0b0b0;
    --sitec-border: #3a3a3a;
    --sitec-hover: #333333;
    --sitec-section-title: #e0e0e0;
}

/* Force la navigation verte avec dégradé adaptatif */
.top-nav-bar-haut7844 {
    background: linear-gradient(135deg, #4caf50 0%, #45a049 100%) !important;

}
.apps-btn-nav-7844 >  i{
    color: white !important;
}
.top-nav-left-7844 button {
    color: #f8f8f8 !important;
    background: transparent !important;
}
body.dark-theme .top-nav-bar-haut7844 {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5) !important;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: var(--sitec-bg);
    color: var(--sitec-text);
    padding-top: 80px;
    min-height: 100vh;
    transition: background-color 0.3s ease, color 0.3s ease;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
}

h2 {
    font-size: 2rem;
    margin-bottom: 20px;
    font-weight: 600;
    text-shadow: 0 2px 4px rgba(0,0,0,0.1);
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--sitec-text);
}

.create-page {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    background: var(--sitec-primary);
    color: white;
    text-decoration: none;
    border-radius: 8px;
    font-weight: 600;
    transition: all 0.3s ease;
    margin-bottom: 30px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
}

.create-page:hover {
    transform: translateY(-2px);
    background: var(--sitec-primary-dark);
    box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
}

.section {
    background: var(--sitec-bg);
    border: 1px solid var(--sitec-border);
    border-radius: 12px;
    padding: 30px;
    margin-bottom: 30px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

body.dark-theme .section {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.section-title {
    font-size: 1.5rem;
    margin-bottom: 20px;
    color: var(--sitec-section-title);
    border-bottom: 2px solid var(--sitec-primary);
    padding-bottom: 10px;
    display: flex;
    align-items: center;
    gap: 12px;
    font-weight: 600;
}

.page-item, .dossier-link {
    background: var(--sitec-card-bg);
    border: 1px solid var(--sitec-border);
    border-radius: 8px;
    padding: 15px;
    margin-bottom: 10px;
    transition: all 0.3s ease;
}

.page-item:hover, .dossier-link:hover {
    border-color: var(--sitec-primary);
    box-shadow: 0 2px 8px rgba(76, 175, 80, 0.2);
}

body.dark-theme .page-item:hover,
body.dark-theme .dossier-link:hover {
    background: #333333;
    box-shadow: 0 2px 8px rgba(76, 175, 80, 0.3);
}

.page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--sitec-border);
}

.page-main-info {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
}

.page-icon {
    font-size: 24px;
    color: var(--sitec-primary);
    flex-shrink: 0;
}

.page-details {
    flex: 1;
    min-width: 0;
}

.page-name {
    font-size: 18px;
    font-weight: 600;
    color: var(--sitec-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-decoration: none;
}

.page-name:hover {
    color: var(--sitec-primary);
}

.page-meta {
    font-size: 13px;
    color: var(--sitec-text-secondary);
    margin-top: 4px;
}

.page-badges {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
}

.badge {
    padding: 4px 12px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
}

.badge-public {
    background: rgba(76, 175, 80, 0.1);
    color: #4CAF50;
    border: 1px solid rgba(76, 175, 80, 0.2);
}

.badge-private {
    background: rgba(255, 152, 0, 0.1);
    color: #ff9800;
    border: 1px solid rgba(255, 152, 0, 0.2);
}

.badge-owner {
    background: #e8f5e9;
    color: #2e7d32;
    cursor: pointer;
    transition: all 0.2s;
}

body.dark-theme .badge-owner {
    background: rgba(76, 175, 80, 0.2);
    color: #81c784;
}

.badge-owner:hover {
    background: #c8e6c9;
}

body.dark-theme .badge-owner:hover {
    background: rgba(76, 175, 80, 0.3);
}

.page-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

.btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: all 0.3s ease;
    text-decoration: none;
}

.btn-primary {
    background: var(--sitec-primary);
    color: white;
}

.btn-primary:hover {
    background: var(--sitec-primary-dark);
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(76, 175, 80, 0.3);
}

.btn-secondary {
    background: var(--sitec-hover);
    color: var(--sitec-text);
    border: 1px solid var(--sitec-border);
}

.btn-secondary:hover {
    background: var(--sitec-border);
}

body.dark-theme .btn-secondary:hover {
    background: #3a3a3a;
}

.btn-icon {
    padding: 8px 12px;
}

/* Popup/Modal */
.popup {
    display: none;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background-color: rgba(0, 0, 0, 0.5);
    justify-content: center;
    align-items: center;
    z-index: 3000;
}

.popup-content {
    background-color: var(--sitec-bg);
    padding: 30px;
    border-radius: 12px;
    max-width: 500px;
    width: 90%;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    border: 1px solid var(--sitec-border);
    position: relative;
}

.popup-content h3 {
    margin-top: 0;
    margin-bottom: 20px;
    color: var(--sitec-primary);
    font-size: 1.5rem;
    display: flex;
    align-items: center;
    gap: 10px;
}

.close {
    position: absolute;
    top: 10px;
    right: 10px;
    color: var(--sitec-text-secondary);
    font-size: 28px;
    font-weight: bold;
    cursor: pointer;
    line-height: 1;
    transition: color 0.2s;
}

.close:hover {
    color: var(--sitec-text);
}

.popup-content form {
    margin-bottom: 15px;
}

.popup-content label {
    display: block;
    margin-bottom: 8px;
    font-weight: 600;
    color: var(--sitec-text);
}

.popup-content input[type="text"] {
    width: 100%;
    padding: 12px;
    border: 1px solid var(--sitec-border);
    border-radius: 6px;
    font-size: 15px;
    margin-bottom: 10px;
    background: var(--sitec-bg);
    color: var(--sitec-text);
    transition: border-color 0.2s;
}

.popup-content input[type="text"]:focus {
    outline: none;
    border-color: var(--sitec-primary);
    box-shadow: 0 0 0 3px rgba(76, 175, 80, 0.1);
}

.popup-content button {
    width: 100%;
    margin-top: 10px;
}

.btn-danger {
    background: rgba(244, 67, 54, 0.1);
    color: #f44336;
    border: 1px solid rgba(244, 67, 54, 0.2);
}

.btn-danger:hover {
    background: #f44336;
    color: white;
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(244, 67, 54, 0.3);
}

.dossier-link a {
    display: flex;
    align-items: center;
    gap: 10px;
    text-decoration: none;
    color: var(--sitec-text);
    font-weight: 600;
    font-size: 16px;
    transition: color 0.2s;
}

.dossier-link a:hover {
    color: var(--sitec-primary);
}

@media (max-width: 768px) {
    .page-header {
        flex-direction: column;
        align-items: flex-start;
        gap: 10px;
    }

    .page-badges {
        width: 100%;
        justify-content: flex-start;
    }

    .page-actions {
        width: 100%;
    }

    .btn {
        flex: 1;
        justify-content: center;
    }
}
    </style>
</head>
<body>
<?php
 include '/var/www/html/access_control.php';

include '/var/www/html/function.php';

// Connexion à la base de données
$servername = "localhost";
$username = "orsql";
$password = "iDq]25F0u8v*z[1d";
$dbname = "user";
$conn = new mysqli($servername, $username, $password, $dbname);

if ($conn->connect_error) {
    die("La connexion a échoué : " . $conn->connect_error);
}


$user_id = 0;
$user_email = "";
$user_nom = "Invité";

// Vérification du cookie de session
if (isset($_COOKIE['connexion_cookie']) && !empty($_COOKIE['connexion_cookie'])) {
    $cookie_value = $_COOKIE['connexion_cookie'];
    $stmt = $conn->prepare("SELECT idcokier, datecra, pc, navi, email, nom FROM loginc WHERE idcokier=?");
    $stmt->bind_param("s", $cookie_value);
    $stmt->execute();
    $result = $stmt->get_result();
    
    if ($result->num_rows == 1) {
        $row = $result->fetch_assoc();
        
        if ($row['pc'] === $_SERVER['REMOTE_ADDR'] && $row['navi'] === $_SERVER['HTTP_USER_AGENT']) {
            $one_hour_ago = strtotime('-1 hour');
            $datecra_timestamp = strtotime($row['datecra']);
            
            if ($datecra_timestamp > $one_hour_ago) {
                $user_email = $row['email'];
                $user_nom = $row['nom'];
                
                $stmt = $conn->prepare("SELECT * FROM login WHERE email=?");
                $stmt->bind_param("s", $user_email);
                $stmt->execute();
                $result = $stmt->get_result();
                
                if ($result->num_rows == 1) {
                    $row = $result->fetch_assoc();
                    $user_id = (int)$row['id'];
                }
            }
        }
    }
    $stmt->close();
}
// La navigation s'adapte automatiquement au thème de l'utilisateur
displayNavigation($conn);

// Détection du thème utilisateur
$isDarkTheme = false;
if ($user_id > 0) {
    $prefs = getUserPreferences($user_id, $conn);
    $isDarkTheme = ($prefs && isset($prefs['teme']) && $prefs['teme'] === 1);
}
?>
<script>
<?php if ($isDarkTheme): ?>
document.body.classList.add('dark-theme');
<?php endif; ?>
</script>
<?php
// Traitement des formulaires
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    if (isset($_POST['change-name'], $_POST['page-url'])) {
        $new_name = $_POST['change-name'];
        $page_url = str_replace(['pages/', '.php'], '', $_POST['page-url']);
        
        $stmt = $conn->prepare("UPDATE sitec SET nompage = ? WHERE urlpage = ?");
        $stmt->bind_param("ss", $new_name, $page_url);
        $stmt->execute();
        header("Location: ".$_SERVER['PHP_SELF']);
        exit;
    }

    if (isset($_POST['delete-page'], $_POST['page-url'])) {
        $page_url = str_replace(['pages/', '.php'], '', $_POST['page-url']);
        
        $stmt = $conn->prepare("DELETE FROM sitec WHERE urlpage = ?");
        $stmt->bind_param("s", $page_url);
        $stmt->execute();
        header("Location: ".$_SERVER['PHP_SELF']);
        exit;
    }

    if (isset($_POST['visibility'], $_POST['page-url'])) {
        $new_visibility = intval($_POST['visibility']);
        $page_url = str_replace(['pages/', '.php'], '', $_POST['page-url']);
        
        $stmt = $conn->prepare("UPDATE sitec SET porb = ? WHERE urlpage = ?");
        $stmt->bind_param("is", $new_visibility, $page_url);
        $stmt->execute();
        header("Location: ".$_SERVER['PHP_SELF']);
        exit;
    }
}
?>

<div class="container">
    <h2><i class="fas fa-globe"></i> Sitec - Gestion de Pages</h2>
    <a href="create_page.php" class="create-page">
        <i class="fas fa-plus"></i>
        Créer une Nouvelle Page
    </a>

    <?php
    // Mes Pages
    $stmt = $conn->prepare("SELECT urlpage, nompage, porb FROM sitec WHERE user_id = ? ORDER BY popular DESC");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();

    if ($result->num_rows > 0) {
        echo '<div class="section">';
        echo '<h3 class="section-title"><i class="fas fa-user"></i> Mes Pages</h3>';
        
        while ($row = $result->fetch_assoc()) {
            $pageurl = "pages/" . $row['urlpage'] . ".php";
            $page_name = $row['nompage'];
            $scalese = intval($row['porb']);
            $visibility = ($scalese === 1) ? "Privée" : "Publique";
            $badge_class = ($scalese === 1) ? "badge-private" : "badge-public";
            ?>
            
            <div class="page-item">
                <div class="page-header">
                    <div class="page-main-info">
                        <i class="fas fa-file-alt page-icon"></i>
                        <div class="page-details">
                            <a href="<?php echo $pageurl; ?>" class="page-name"><?php echo htmlspecialchars($page_name); ?></a>
                            <div class="page-meta">Page créée par vous</div>
                        </div>
                    </div>
                    <div class="page-badges">
                        <span class="badge <?php echo $badge_class; ?>"><?php echo $visibility; ?></span>
                        <span class="badge badge-owner">À vous</span>
                    </div>
                </div>
                <div class="page-actions">
                    <a href="<?php echo $pageurl; ?>" class="btn btn-primary">
                        <i class="fas fa-eye"></i>
                        Voir
                    </a>
                    <button class="btn btn-secondary" onclick="openPopup('<?php echo $pageurl; ?>', '<?php echo htmlspecialchars($page_name); ?>', <?php echo $scalese; ?>)">
                        <i class="fas fa-gear"></i>
                        Gérer
                    </button>
                </div>
            </div>
            <?php
        }
        echo '</div>';
    }
    $stmt->close();

    // Pages Publiques
    $stmt = $conn->prepare("SELECT urlpage, nompage, user_id FROM sitec WHERE porb = 0 AND user_id != ? ORDER BY popular DESC");
    $stmt->bind_param("i", $user_id);
    $stmt->execute();
    $result = $stmt->get_result();

    if ($result->num_rows > 0) {
        echo '<div class="section">';
        echo '<h3 class="section-title"><i class="fas fa-globe"></i> Pages Publiques</h3>';
        
        while ($row = $result->fetch_assoc()) {
            $pageurl = "pages/" . $row['urlpage'] . ".php";
            $page_name = $row['nompage'];
            $page_user_id = $row['user_id'];
            
            $stmt2 = $conn->prepare("SELECT nom FROM login WHERE id = ?");
            $stmt2->bind_param("i", $page_user_id);
            $stmt2->execute();
            $user_result = $stmt2->get_result();
            
            if ($user_result->num_rows === 1) {
                $user_row = $user_result->fetch_assoc();
                $creator_name = $user_row['nom'];
                $hrefidusername = "/login/user.php?user=" . $page_user_id;
                ?>
                
                <div class="page-item">
                    <div class="page-header">
                        <div class="page-main-info">
                            <i class="fas fa-file-alt page-icon"></i>
                            <div class="page-details">
                                <a href="<?php echo $pageurl; ?>" class="page-name"><?php echo htmlspecialchars($page_name); ?></a>
                                <div class="page-meta">Créée par <?php echo htmlspecialchars($creator_name); ?></div>
                            </div>
                        </div>
                        <div class="page-badges">
                            <span class="badge badge-public">Publique</span>
                            <span class="badge badge-owner" onclick="window.location.href='<?php echo $hrefidusername; ?>'">
                                par <?php echo htmlspecialchars($creator_name); ?>
                            </span>
                        </div>
                    </div>
                    <div class="page-actions">
                        <a href="<?php echo $pageurl; ?>" class="btn btn-primary">
                            <i class="fas fa-eye"></i>
                            Voir
                        </a>
                    </div>
                </div>
                <?php
            }
            $stmt2->close();
        }
        echo '</div>';
    }
    $stmt->close();

    // Dossiers
    $stmt = $conn->prepare("SELECT * FROM sitecdos ORDER BY popluardose DESC");
    $stmt->execute();
    $result = $stmt->get_result();

    $has_dossiers = false;
    $dossiers_html = '';

    while ($row = $result->fetch_assoc()) {
        $dois_name = $row['doisernom'];
        $iddois = $row['iddosier'];
        
        if ($iddois !== "aucune" && !empty($iddois)) {
            if (!$has_dossiers) {
                $has_dossiers = true;
                $dossiers_html .= '<div class="section">';
                $dossiers_html .= '<h3 class="section-title"><i class="fas fa-folder"></i> Dossiers</h3>';
            }
            
            $dossier_url = "/sitec/dossier.php?dossier=" . urlencode(trim($iddois));
            $dossiers_html .= '<div class="dossier-link">';
            $dossiers_html .= '<a href="' . $dossier_url . '" target="_blank">';
            $dossiers_html .= '<i class="fas fa-folder-open"></i>';
            $dossiers_html .= htmlspecialchars($dois_name);
            $dossiers_html .= '</a>';
            $dossiers_html .= '</div>';
        }
    }

    if ($has_dossiers) {
        $dossiers_html .= '</div>';
        echo $dossiers_html;
    }

    $stmt->close();
    $conn->close();
    ?>
</div>

<!-- Popup Modal -->
<div id="popup" class="popup">
    <div class="popup-content">
        <span class="close" onclick="closePopup()">&times;</span>
        <h3><i class="fas fa-gear"></i> Gérer la page</h3>

        <form method="POST" action="">
            <input type="hidden" name="page-url" id="popup-page-url" />
            <label for="change-name">Nom de la page :</label>
            <input type="text" name="change-name" id="popup-page-name" required />
            <button type="submit" class="btn btn-primary">Changer le Nom</button>
        </form>

        <form method="POST" action="">
            <input type="hidden" name="page-url" id="visibility" />
            <button type="submit" name="visibility" id="visibility-btn" class="btn btn-primary">
                Changer Visibilité
            </button>
        </form>

        <form method="POST" action="">
            <input type="hidden" name="page-url" id="supee" />
            <button type="submit" name="delete-page" class="btn btn-danger">
                <i class="fas fa-trash"></i>
                Supprimer la Page
            </button>
        </form>
    </div>
</div>

<script>
function openPopup(url, pageName, visibility) {
    document.getElementById("popup").style.display = "flex";
    document.getElementById("popup-page-name").value = pageName;
    document.getElementById("popup-page-url").value = url;
    document.getElementById("supee").value = url;
    document.getElementById("visibility").value = url;
    
    const visBtn = document.getElementById("visibility-btn");
    if (visibility === 1) {
        visBtn.value = "0";
        visBtn.innerHTML = '<i class="fas fa-globe"></i> Rendre Publique';
    } else {
        visBtn.value = "1";
        visBtn.innerHTML = '<i class="fas fa-lock"></i> Rendre Privée';
    }
}

function closePopup() {
    document.getElementById("popup").style.display = "none";
}

window.onclick = function(event) {
    const modal = document.getElementById("popup");
    if (event.target == modal) {
        closePopup();
    }
}
</script>

</body>
<?php
ob_end_flush();
?>
</html>