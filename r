
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <title>Redirection</title>
    <script>
        setTimeout(function() {
            window.location.href = 'recupephp.php';
        }, 5); // 5000 millisecondes = 5 secondes
    </script>
</head>
<body>
    
</body>
</html>
<?php
header("Location: recupephp.php");
?>
