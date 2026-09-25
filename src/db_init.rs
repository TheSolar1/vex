// ══════════════════════════════════════════════════════════════════
// db_init.rs — VEX auto-initialisation de la base de données
// Crée la base et toutes les tables si elles n'existent pas.
// Compatible MySQL 8. Gère aussi les migrations (ADD COLUMN).
// ══════════════════════════════════════════════════════════════════

use mysql::prelude::*;
use mysql::*;

use crate::config_loader::DbConfig;

pub fn init_db(cfg: &DbConfig) -> Result<()> {
    let url_no_db = format!(
        "mysql://{}:{}@{}:{}",
        cfg.user, cfg.password, cfg.host, cfg.port
    );
    let opts = Opts::from_url(&url_no_db)?;
    let pool = Pool::new(opts)?;
    let mut conn = pool.get_conn()?;

    conn.query_drop(format!(
        "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        cfg.dbname
    ))?;
    conn.query_drop(format!("USE `{}`", cfg.dbname))?;
    conn.query_drop("SET time_zone = 'SYSTEM'")?;

    // ── autologin ─────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `autologin` (
            `nombre`   TEXT         NOT NULL,
            `compteid` VARCHAR(191) NOT NULL,
            UNIQUE KEY `compteid` (`compteid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    // Migration : compteur d'utilisation du lien autologin
    let _ = conn.query_drop(
        "ALTER TABLE `autologin` ADD COLUMN `utilisations` INT NOT NULL DEFAULT 0",
    );

    // ── appareil_jetons (flux d'autorisation d'appareil, type
    // "device flow" -- voir src/login/appareil.rs) ──────────────────
    // `jeton_brut` n'est rempli que brievement entre l'approbation et la
    // premiere recuperation par l'appareil (poll), puis efface -- seul
    // `jeton_hash` (SHA-256(secret:jeton)) subsiste ensuite, meme logique
    // que la table `autologin` existante.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `appareil_jetons` (
            `code`         VARCHAR(64)  NOT NULL,
            `jeton_brut`   VARCHAR(128) DEFAULT NULL,
            `jeton_hash`   VARCHAR(128) DEFAULT NULL,
            `user_id`      INT          DEFAULT NULL,
            `statut`       VARCHAR(20)  NOT NULL DEFAULT 'en_attente',
            `nom_appareil` VARCHAR(191) DEFAULT NULL,
            `created_at`   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`code`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── bloqpage ──────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `bloqpage` (
            `id`          INT  NOT NULL AUTO_INCREMENT,
            `iduserb`     TEXT DEFAULT NULL,
            `priviautro`  INT  NOT NULL,
            `iduserquiab` INT  NOT NULL,
            `pageb`       TEXT NOT NULL,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // (table `conxiont` supprimée — orpheline, aucune référence dans le code,
    // vraisemblablement un reste d'avant la migration SRP-6a. Renommée en
    // `conxiont_DEPRECATED` en production plutôt que droppée directement.)

    // ── fichiers ──────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `fichiers` (
            `id`             INT          NOT NULL AUTO_INCREMENT,
            `nom`            VARCHAR(255) NOT NULL,
            `fichier`        LONGTEXT     NOT NULL,
            `type_fichier`   VARCHAR(255) NOT NULL,
            `taille`         BIGINT       NOT NULL,
            `visble`         VARCHAR(20)  NOT NULL,
            `id_utilisateur` VARCHAR(99)  DEFAULT NULL,
            `partage`        TEXT         DEFAULT NULL,
            `date`           DATE         NOT NULL DEFAULT (CURRENT_DATE),
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── fchier_version ────────────────────────────────────────────
    // Compteur "y'a-t-il du nouveau ?" par utilisateur, incremente a
    // chaque mutation de ses fichiers/dossiers (upload, suppression,
    // renommage, deplacement, edition de contenu). Permet aux clients de
    // synchro (vex-cloudsync) de detecter un changement avec un poll TRES
    // frequent et TRES bon marche (un SELECT indexe par cle primaire, pas
    // un parcours recursif de l'arborescence) au lieu de faire tout le
    // travail de reconciliation a chaque fois -- voir bump_version_fichiers
    // dans appeldb.rs.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `fchier_version` (
            `id_utilisateur` INT    NOT NULL,
            `version`        BIGINT NOT NULL DEFAULT 0,
            PRIMARY KEY (`id_utilisateur`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── fchier_corbeille ──────────────────────────────────────────
    // Fichiers/dossiers supprimes depuis ExoDrive : la ligne d'origine
    // (fichiers ou sitecdos) est copiee ici en JSON puis retiree de sa
    // table, et peut etre restauree telle quelle (meme id) pendant
    // JOURS_CORBEILLE jours -- voir fchier/corbeille.rs.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `fchier_corbeille` (
            `id`             INT          NOT NULL AUTO_INCREMENT,
            `id_utilisateur` INT          NOT NULL,
            `item_type`      VARCHAR(10)  NOT NULL,
            `item_id`        BIGINT       NOT NULL,
            `nom`            VARCHAR(255) NOT NULL,
            `taille`         BIGINT       NOT NULL DEFAULT 0,
            `donnees`        LONGTEXT     NOT NULL,
            `supprime_le`    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            KEY `idx_corbeille_user` (`id_utilisateur`, `supprime_le`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── fchier_liens ──────────────────────────────────────────────
    // Liens de partage publics (voir fchier/liens.rs). `contenu` est une
    // copie RECHIFFREE du fichier avec une cle qui n'est que dans le lien
    // (fragment #...), jamais envoyee au serveur.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `fchier_liens` (
            `jeton`               CHAR(48)     NOT NULL,
            `id_utilisateur`      INT          NOT NULL,
            `id_fichier`          INT          NOT NULL,
            `nom`                 VARCHAR(255) NOT NULL,
            `mime`                VARCHAR(255) NOT NULL DEFAULT '',
            `taille`              BIGINT       NOT NULL DEFAULT 0,
            `contenu`             LONGTEXT     NOT NULL,
            `mdp_hash`            VARCHAR(100) DEFAULT NULL,
            `expire_le`           DATETIME     DEFAULT NULL,
            `telechargements`     INT          NOT NULL DEFAULT 0,
            `max_telechargements` INT          DEFAULT NULL,
            `cree_le`             DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`jeton`),
            KEY `idx_liens_user` (`id_utilisateur`, `id_fichier`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── login ─────────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `login` (
            `nom`       VARCHAR(250) NOT NULL,
            `email`     VARCHAR(250) NOT NULL,
            `motdepass` VARCHAR(250) NOT NULL,
            `vip`       VARCHAR(9)   NOT NULL DEFAULT '0',
            `id`        INT          NOT NULL AUTO_INCREMENT,
            `privilege` INT          DEFAULT 10,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;
    // Migration AUTO_INCREMENT si table existait sans
    let _ = conn.query_drop(
        "ALTER TABLE `login` MODIFY `id` INT NOT NULL AUTO_INCREMENT"
    );
    // Migration LONGBLOB → LONGTEXT pour stocker base64
    let _ = conn.query_drop(
        "ALTER TABLE `fichiers` MODIFY `fichier` LONGTEXT NOT NULL"
    );
    // Migration DATE → DATETIME : `date` doit garder l'heure/minute/seconde
    // (voir fchier.rs::api_upload / api_edit_content) pour qu'un client de
    // synchronisation puisse detecter une modification survenue le meme
    // jour que la precedente. Un DATE tronquerait silencieusement l'heure
    // (ou rejetterait l'insertion en mode SQL strict).
    let _ = conn.query_drop(
        "ALTER TABLE `fichiers` MODIFY `date` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP"
    );

    // ── FIX (migration SRP-6a) ──────────────────────────────────────
    // Ajoute les colonnes nécessaires à l'authentification SRP-6a
    // (voir crate::srp / login.rs). `motdepass` est rendu nullable
    // car il n'est plus utilisé par le nouveau flux d'inscription —
    // sans ça, tout INSERT signup échouait silencieusement (colonne
    // NOT NULL sans valeur fournie), ce qui donnait "Erreur lors de
    // l'inscription." sans plus de détail côté client.
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `srp_salt` VARCHAR(64) DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `srp_verifier` VARCHAR(512) DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` MODIFY `motdepass` VARCHAR(250) DEFAULT NULL");

    // ── FIX (connexion par pseudo) ──────────────────────────────────
    // Identifiant de connexion alternatif à l'email (facultatif, unique).
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `pseudo` VARCHAR(64) DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` ADD UNIQUE INDEX `idx_pseudo` (`pseudo`)");

    // ── FIX (récupération de compte sans email) ──────────────────────
    // La clé de chiffrement des fichiers (masterKey, voir crypto.js) est
    // enveloppée deux fois : une fois sous le mot de passe, une fois sous
    // un code de récupération à 20 caractères affiché une seule fois à
    // l'utilisateur. Le serveur ne stocke que des blobs chiffrés + un hash
    // de preuve, jamais la masterKey ni le code en clair.
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `file_key_wrapped_pwd` TEXT DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `file_key_wrapped_recovery` TEXT DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `recovery_salt` VARCHAR(64) DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `recovery_proof_hash` VARCHAR(64) DEFAULT NULL");

    // ── FIX (protection des roles payes) ─────────────────────────────
    // Marque un plan VIP obtenu via un vrai paiement (point 5, pas encore
    // branche : aucun code ne met encore cette colonne a 1 aujourd'hui --
    // elle est ajoutee en avance pour que le webhook de paiement n'ait
    // qu'a l'ecrire plus tard). Sert de garde dans /users/vip : un
    // admin/superadmin ne doit pas pouvoir retirer/changer silencieusement
    // le plan d'un client qui a paye pour l'obtenir -- seul le fondateur
    // le peut, avec un avertissement.
    let _ = conn.query_drop("ALTER TABLE `login` ADD COLUMN `vip_paye` TINYINT(1) NOT NULL DEFAULT 0");

    // ── srp_sessions (éphémère, corrèle srp_step1 → srp_step2) ──────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `srp_sessions` (
            `token`      VARCHAR(64)  NOT NULL,
            `email`      VARCHAR(255) NOT NULL,
            `b_hex`      VARCHAR(64)  NOT NULL,
            `created_at` DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`token`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── loginc ────────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `loginc` (
            `id`        INT          NOT NULL AUTO_INCREMENT,
            `idcokier`  VARCHAR(255) NOT NULL,
            `datecra`   DATETIME     NOT NULL,
            `pc`        VARCHAR(255) NOT NULL,
            `navi`      VARCHAR(255) NOT NULL,
            `email`     VARCHAR(191) NOT NULL,
            `nom`       VARCHAR(191) NOT NULL,
            `autologin` VARCHAR(4)   DEFAULT NULL,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;
    // Migration AUTO_INCREMENT si table existait sans
    let _ = conn.query_drop(
        "ALTER TABLE `loginc` MODIFY `id` INT NOT NULL AUTO_INCREMENT"
    );

    // ── Index de performance ──────────────────────────────────────
    // PERF : la session est relue a CHAQUE requete par `loginc.idcokier`
    // puis `login.email` -- sans index, MySQL parcourait toute la table a
    // chaque fois. Idem pour la liste/quota des fichiers d'un utilisateur.
    // "ADD INDEX" echoue simplement si l'index existe deja (erreur
    // ignoree), ce qui rend ces migrations idempotentes sur MySQL et
    // MariaDB.
    for sql in [
        "ALTER TABLE `loginc` ADD INDEX `idx_loginc_cookie` (`idcokier`)",
        "ALTER TABLE `loginc` ADD INDEX `idx_loginc_email` (`email`)",
        "ALTER TABLE `login` ADD INDEX `idx_login_email` (`email`)",
        "ALTER TABLE `fichiers` ADD INDEX `idx_fichiers_user` (`id_utilisateur`)",
        "ALTER TABLE `sitecdos` ADD INDEX `idx_sitecdos_user` (`userid`)",
    ] {
        let _ = conn.query_drop(sql);
    }

    // ── p2p_messages ──────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `p2p_messages` (
            `id`           INT         NOT NULL AUTO_INCREMENT,
            `from_user_id` INT         NOT NULL,
            `to_user_id`   INT         NOT NULL,
            `message_type` VARCHAR(50) NOT NULL,
            `content`      TEXT        NOT NULL,
            `metadata`     LONGTEXT    DEFAULT NULL,
            `status`       ENUM('sent','delivered','read') DEFAULT 'sent',
            `created_at`   DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            KEY `from_user_id` (`from_user_id`),
            KEY `to_user_id`   (`to_user_id`),
            KEY `status`       (`status`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // (table `p2p_nodes` supprimée — orpheline, aucune référence dans le code,
    // superseded par `p2p_peers` + `p2p_users`. Renommée en `p2p_nodes_DEPRECATED`
    // en production plutôt que droppée directement.)

    // ── p2p_peers ─────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `p2p_peers` (
            `id`       INT          NOT NULL AUTO_INCREMENT,
            `node_id`  VARCHAR(64)  NOT NULL,
            `vex_url`  VARCHAR(255) NOT NULL,
            `ip`       VARCHAR(128) NOT NULL,
            `port`     INT          NOT NULL DEFAULT 7700,
            `tor_addr` VARCHAR(255) DEFAULT NULL,
            `pub_key`  TEXT         NOT NULL,
            `status`   VARCHAR(16)  NOT NULL DEFAULT 'offline',
            `last_seen` DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
            `version`  VARCHAR(32)  DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `node_id` (`node_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    )?;

    // ── p2p_users ─────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `p2p_users` (
            `id`         INT          NOT NULL AUTO_INCREMENT,
            `user_id`    INT          NOT NULL,
            `node_id`    VARCHAR(64)  NOT NULL,
            `nom`        VARCHAR(128) NOT NULL,
            `pub_key`    TEXT         NOT NULL,
            `updated_at` DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            UNIQUE KEY `user_node` (`user_id`, `node_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    )?;

    // ── p2p_transfers ─────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `p2p_transfers` (
            `id`           INT          NOT NULL AUTO_INCREMENT,
            `transfer_id`  VARCHAR(64)  NOT NULL,
            `from_node`    VARCHAR(64)  NOT NULL,
            `to_node`      VARCHAR(64)  NOT NULL,
            `from_user`    INT          NOT NULL,
            `to_user`      INT          NOT NULL,
            `fichier_nom`  VARCHAR(255) NOT NULL,
            `fichier_size` BIGINT       NOT NULL DEFAULT 0,
            `chunk_size`   INT          NOT NULL DEFAULT 1048576,
            `chunks_total` INT          NOT NULL DEFAULT 1,
            `chunks_ok`    INT          NOT NULL DEFAULT 0,
            `status`       VARCHAR(32)  NOT NULL DEFAULT 'pending',
            `created_at`   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            `updated_at`   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            UNIQUE KEY `transfer_id` (`transfer_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    )?;
    // FIX (securite, consentement P2P) : colonnes pour le flux de
    // validation manuelle d'un fichier recu -- voir reconstituer_fichier
    // dans p2p.rs. `from_user_nom` est auto-declare par le noeud emetteur
    // (jamais garanti authentique, affiche a titre informatif seulement --
    // l'identite fiable est `from_node`, verifiee par signature).
    let _ = conn.query_drop("ALTER TABLE `p2p_transfers` ADD COLUMN `from_user_nom` VARCHAR(255) DEFAULT NULL");
    let _ = conn.query_drop("ALTER TABLE `p2p_transfers` ADD COLUMN `fichier_chemin_temp` VARCHAR(500) DEFAULT NULL");

    // ── pref ──────────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `pref` (
            `id-user`            INT          NOT NULL,
            `teme`               INT          DEFAULT NULL,
            `lan`                VARCHAR(20)  NOT NULL DEFAULT 'fr',
            `langue`             VARCHAR(5)   DEFAULT 'fr',
            `notifications_meet` TINYINT(1)   DEFAULT 1,
            `auto_record`        TINYINT(1)   DEFAULT 0,
            `mic_default`        TINYINT(1)   DEFAULT 0,
            `camera_default`     TINYINT(1)   DEFAULT 0,
            `quality_video`      VARCHAR(10)  DEFAULT 'auto',
            `profile_icon_type`  VARCHAR(20)  DEFAULT 'initials',
            `profile_icon_url`   VARCHAR(500) DEFAULT NULL,
            `nav_button_style`   VARCHAR(50)  DEFAULT 'default',
            `logo_pages`         TEXT         DEFAULT NULL,
            UNIQUE KEY `id-user` (`id-user`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    // Migrations colonnes pref
    for col in &[
        "ALTER TABLE `pref` ADD COLUMN `nav_button_style` VARCHAR(50) DEFAULT 'default'",
        "ALTER TABLE `pref` ADD COLUMN `logo_pages` TEXT DEFAULT NULL",
        "ALTER TABLE `pref` ADD COLUMN `profile_icon_type` VARCHAR(20) DEFAULT 'initials'",
        "ALTER TABLE `pref` ADD COLUMN `profile_icon_url` VARCHAR(500) DEFAULT NULL",
        // Choix utilisateur : tuiles du dashboard et apps du menu
        "ALTER TABLE `pref` ADD COLUMN `dashboard_tiles` TEXT DEFAULT NULL",
        "ALTER TABLE `pref` ADD COLUMN `nav_apps` TEXT DEFAULT NULL",
        "ALTER TABLE `pref` ADD COLUMN `dashboard_events` TEXT DEFAULT NULL",
        // VexIA : execution automatique des outils "scoped" sans confirmation
        "ALTER TABLE `pref` ADD COLUMN `vexia_auto_confirm` TINYINT(1) NOT NULL DEFAULT 0",
        // VexIA : cle API Anthropic personnelle (facturee sur le compte de
        // l'utilisateur), utilisee a la place de la cle partagee admin.
        "ALTER TABLE `pref` ADD COLUMN `vexia_api_key` VARCHAR(255) DEFAULT NULL",
        "ALTER TABLE `pref` ADD COLUMN `vexia_provider` VARCHAR(20) DEFAULT NULL",
        // Bulle de chat flottante VexIA (visible sur toutes les pages) :
        // certains utilisateurs ne veulent pas de VexIA du tout.
        "ALTER TABLE `pref` ADD COLUMN `vexia_widget_on` TINYINT(1) NOT NULL DEFAULT 1",
    ] {
        let _ = conn.query_drop(*col);
    }

    // ── sitec ─────────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `sitec` (
            `urlpage` TEXT         NOT NULL,
            `nompage` VARCHAR(191) NOT NULL,
            `user_id` INT          NOT NULL,
            `porb`    INT          NOT NULL,
            `popular` VARCHAR(800) NOT NULL DEFAULT '0',
            `idpage`  INT          NOT NULL AUTO_INCREMENT,
            PRIMARY KEY (`idpage`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── sitecdos ──────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `sitecdos` (
            `doisernom`     VARCHAR(191) NOT NULL,
            `userid`        VARCHAR(191) NOT NULL,
            `popluardose`   INT          NOT NULL DEFAULT 0,
            `idpage`        TEXT         NOT NULL,
            `addpageuserid` VARCHAR(99)  NOT NULL,
            `iddosier`      INT          NOT NULL AUTO_INCREMENT,
            PRIMARY KEY (`iddosier`),
            UNIQUE KEY `iddb` (`iddosier`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── sus-hac ───────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `sus-hac` (
            `id-c`   VARCHAR(191) NOT NULL,
            `auteur` TEXT         NOT NULL,
            `id`     INT          NOT NULL AUTO_INCREMENT,
            UNIQUE KEY `id` (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── vexia_audit ───────────────────────────────────────────────
    // Journal des actions declenchees par VexIA (outils Anthropic).
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `vexia_audit` (
            `id`          INT          NOT NULL AUTO_INCREMENT,
            `user_id`     INT          NOT NULL,
            `tool_name`   VARCHAR(100) NOT NULL,
            `tier`        VARCHAR(20)  NOT NULL,
            `args_json`   TEXT         NOT NULL,
            `success`     TINYINT(1)   NOT NULL,
            `result_json` TEXT         DEFAULT NULL,
            `error`       TEXT         DEFAULT NULL,
            `created_at`  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── tag-user ──────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `tag-user` (
            `user-id`       INT          NOT NULL,
            `tout`          LONGTEXT     DEFAULT NULL,
            `VMotdePasse`   VARCHAR(191) DEFAULT NULL,
            `VEmail`        VARCHAR(191) DEFAULT NULL,
            `VPrivilege`    VARCHAR(191) DEFAULT NULL,
            `VVIP`          VARCHAR(191) DEFAULT NULL,
            `vcreAutologin` VARCHAR(191) DEFAULT NULL,
            `vAutologin`    VARCHAR(191) DEFAULT NULL,
            `statut_compte` VARCHAR(99)  DEFAULT NULL,
            UNIQUE KEY `user-id` (`user-id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── revenus_snapshots (demande utilisateur : graphique dans le temps
    // au clic sur une tuile de Admin > Revenus) : une ligne par jour,
    // ecrasee/mise a jour a chaque chargement de la page ce jour-la --
    // pas de tache planifiee (l'architecture mono-thread de ce serveur
    // rend risque tout job periodique bloquant, voir le bug d'interblocage
    // P2P deja rencontre) donc l'historique s'accumule simplement a chaque
    // visite de l'admin sur cette page.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `revenus_snapshots` (
            `jour`         DATE  NOT NULL,
            `mrr`          FLOAT NOT NULL DEFAULT 0,
            `payants`      INT   NOT NULL DEFAULT 0,
            `total_users`  INT   NOT NULL DEFAULT 0,
            PRIMARY KEY (`jour`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

    // ── wiki_pages (app "Wiki", sous-app de Recherche -- voir
    // src/recherche/recherche.rs) : encyclopédie collaborative interne,
    // n'existe QUE dans la page /recherche, jamais dans la sidebar
    // principale. Ecriture ouverte a tout compte connecte (comme un vrai
    // wiki), suppression/edition d'un article d'autrui reservee aux
    // comptes de confiance (privilege <= 6, meme seuil que le mode "brut"
    // de Sitec) pour limiter le vandalisme sans bloquer la contribution.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `wiki_pages` (
            `id`         INT      NOT NULL AUTO_INCREMENT,
            `titre`      VARCHAR(255) NOT NULL,
            `contenu`    LONGTEXT NOT NULL,
            `auteur_id`  INT      NOT NULL,
            `auteur_nom` VARCHAR(250) NOT NULL DEFAULT '',
            `created_at` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            `maj`        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            `vues`       INT      NOT NULL DEFAULT 0,
            PRIMARY KEY (`id`),
            KEY `titre` (`titre`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    // Migration : index plein texte (classement par pertinence via
    // MATCH...AGAINST au lieu d'un simple LIKE '%...%' qui ne classe rien
    // et ne trouve que des sous-chaines litterales) -- demande utilisateur
    // "un vrai moteur de recherche". `let _` : echoue silencieusement si
    // deja cree par un demarrage precedent (pas d'equivalent portable a
    // "ADD FULLTEXT INDEX IF NOT EXISTS" sur toutes les versions MySQL 8).
    let _ = conn.query_drop(
        "ALTER TABLE `wiki_pages` ADD FULLTEXT INDEX `ft_wiki` (`titre`, `contenu`)",
    );

    // ── wiki_faq (app "FAQ", sous-app de Recherche) : questions/reponses
    // curatees -- ecriture reservee aux comptes de confiance (privilege
    // <= 6) contrairement au wiki, lecture/recherche ouverte a tous.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `wiki_faq` (
            `id`        INT      NOT NULL AUTO_INCREMENT,
            `question`  VARCHAR(500) NOT NULL,
            `reponse`   LONGTEXT NOT NULL,
            `auteur_id` INT      NOT NULL,
            `maj`       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    // ── wikipedia_cache (app Recherche) : miroir LOCAL des articles
    // Wikipedia deja consultes -- la recherche elle-meme reste un appel
    // reseau (impossible d'indexer localement toute Wikipedia), mais des
    // qu'un article est ouvert une fois, son contenu complet est stocke ici
    // et les lectures suivantes sont servies depuis cette base, sans
    // ressortir vers internet. Demande explicite : "je veux que tout soit
    // en local" -- ceci est le compromis realiste (miroir a la demande).
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `wikipedia_cache` (
            `titre`       VARCHAR(255) NOT NULL,
            `extrait`     LONGTEXT     NOT NULL,
            `recupere_le` DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`titre`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    // Migration : index plein texte sur le miroir local -- les articles
    // deja mis en cache deviennent ainsi cherchables INSTANTANEMENT (sans
    // appel reseau, classes par pertinence) en plus de la recherche live
    // sur l'API Wikipedia. Le miroir grandit a l'usage et sert de plus en
    // plus de resultats directement depuis cette base au fil du temps.
    let _ = conn.query_drop(
        "ALTER TABLE `wikipedia_cache` ADD FULLTEXT INDEX `ft_wikipedia_cache` (`titre`, `extrait`)",
    );
    // Migration : miniature de l'article (demande utilisateur : "tu prends
    // la page et tu la restylise" -- une vraie page Wikipedia a une image,
    // pas seulement du texte). NULL pour les articles deja en cache avant
    // cette migration -- ils resteront sans image tant qu'ils ne sont pas
    // rouverts (wikipedia_article la recupere alors et complete la ligne).
    let _ = conn.query_drop(
        "ALTER TABLE `wikipedia_cache` ADD COLUMN `image_url` VARCHAR(600) DEFAULT NULL",
    );

    // ── actualites (app "Actualités", sous-app de Recherche) : notes de
    // mise a jour VEX (quoi de neuf), curatees -- ecriture reservee aux
    // comptes de confiance (privilege <= 6, meme regle que la FAQ
    // desactivee), lecture/recherche ouverte a tous. Rien a voir avec un
    // flux externe (RSS...) : contenu 100% local, ecrit par l'equipe VEX.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `actualites` (
            `id`      INT      NOT NULL AUTO_INCREMENT,
            `titre`   VARCHAR(255) NOT NULL,
            `contenu` TEXT     NOT NULL,
            `date`    DATE     NOT NULL DEFAULT (CURRENT_DATE),
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    let _ = conn.query_drop(
        "ALTER TABLE `actualites` ADD FULLTEXT INDEX `ft_actualites` (`titre`, `contenu`)",
    );

    // ── meet_rooms / meet_participants / meet_signaling (app Viso,
    // visioconference -- src/viso/viso.rs) : BUG DECOUVERT EN PRATIQUE le
    // 22/09 -- ces 3 tables sont utilisees partout dans viso.rs (creation
    // de salle, participants, signalisation WebRTC chiffree) mais n'ont
    // JAMAIS ete ajoutees a cette fonction d'auto-init. Consequence
    // reelle : `creer_salle` echouait TOUJOURS avec "Table
    // 'user.meet_rooms' doesn't exist" (confirme dans les logs), donc Viso
    // n'a jamais pu fonctionner sur cette base tant que quelqu'un n'avait
    // pas cree les tables a la main.
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `meet_rooms` (
            `id`                INT          NOT NULL AUTO_INCREMENT,
            `room_code`         VARCHAR(20)  NOT NULL,
            `creator_id`        INT          NOT NULL,
            `title`             VARCHAR(255) NOT NULL,
            `is_public`         TINYINT      NOT NULL DEFAULT 0,
            `require_password`  TINYINT      NOT NULL DEFAULT 0,
            `password_hash`     VARCHAR(255) DEFAULT NULL,
            `max_participants`  INT          NOT NULL DEFAULT 8,
            `is_active`         TINYINT      NOT NULL DEFAULT 1,
            `created_at`        DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            UNIQUE KEY `room_code` (`room_code`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `meet_participants` (
            `id`         INT          NOT NULL AUTO_INCREMENT,
            `room_id`    INT          NOT NULL,
            `user_id`    INT          NOT NULL,
            `session_id` VARCHAR(64)  NOT NULL,
            `nom`        VARCHAR(250) NOT NULL DEFAULT '',
            `x25519_pub` VARCHAR(128) NOT NULL DEFAULT '',
            `status`     VARCHAR(20)  NOT NULL DEFAULT 'connected',
            `joined_at`  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            `last_seen`  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            UNIQUE KEY `session_id` (`session_id`),
            KEY `room_id` (`room_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `meet_signaling` (
            `id`           INT      NOT NULL AUTO_INCREMENT,
            `room_id`      INT      NOT NULL,
            `from_session` VARCHAR(64)  NOT NULL,
            `to_session`   VARCHAR(64)  NOT NULL,
            `payload_type` VARCHAR(50)  NOT NULL,
            `ciphertext`   LONGTEXT NOT NULL,
            `nonce`        VARCHAR(64)  NOT NULL,
            `created_at`   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (`id`),
            KEY `to_session` (`to_session`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

    eprintln!("[db_init] Base '{}' initialisée avec succès.", cfg.dbname);
    Ok(())
}