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

    // ── conxiont ──────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `conxiont` (
            `id`       INT          NOT NULL AUTO_INCREMENT,
            `username` VARCHAR(255) NOT NULL,
            `password` VARCHAR(255) NOT NULL,
            PRIMARY KEY (`id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci",
    )?;

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

    // ── p2p_nodes ─────────────────────────────────────────────────
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS `p2p_nodes` (
            `user_id`  INT          NOT NULL,
            `node_id`  VARCHAR(100) DEFAULT NULL,
            `status`   ENUM('online','offline','away') DEFAULT 'offline',
            `last_seen` DATETIME    NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            `metadata` LONGTEXT     DEFAULT NULL,
            PRIMARY KEY (`user_id`),
            UNIQUE KEY `node_id` (`node_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci",
    )?;

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

    eprintln!("[db_init] Base '{}' initialisée avec succès.", cfg.dbname);
    Ok(())
}