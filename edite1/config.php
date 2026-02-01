<?php
// config.php - Configuration ONLYOFFICE pour VM Epire

// IMPORTANT : Pour Docker, utiliser l'IP de la machine hôte
$scheme = (!empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off') ? 'https' : 'http';

// Détecter si on est en local (VM) ou sur le serveur vex
if ($_SERVER['HTTP_HOST'] === 'localhost' || strpos($_SERVER['HTTP_HOST'], '127.0.0.1') !== false) {
    // Sur VM locale : utiliser l'IP de la machine hôte pour le callback Docker
    $BASE_URL = $scheme . 'http://10.0.2.15';
} else {
    // Sur serveur vex : utiliser le domaine
    $host = $_SERVER['HTTP_HOST'];
    $BASE_URL = $scheme . '://' . $host;
}

// URL publique ONLYOFFICE (même site)
define('ONLYOFFICE_URL_PUBLIC', $BASE_URL . '/onlyoffice');

// URL interne (identique car même domaine)
define('ONLYOFFICE_URL_INTERNAL', $BASE_URL . '/onlyoffice');

// URL de base de ton application
define('BASE_URL', $BASE_URL . '/edite1');

// Jeton secret
define('ONLYOFFICE_SECRET', 'aB3dF9kL2mN5pQ8rT1vW4xY7zA0cE6gH9jK2lM5nP8q');
// Dossier documents
define('DOCUMENTS_DIR', __DIR__ . '/documents/');

// Création auto du dossier documents
if (!is_dir(DOCUMENTS_DIR)) {
    mkdir(DOCUMENTS_DIR, 0755, true);
}

function getOnlyOfficeSecret() {
    return ONLYOFFICE_SECRET;
}
