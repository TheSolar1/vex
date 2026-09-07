// ══════════════════════════════════════════════════════════════════
// admin/Admin.rs — VEX Admin Panel
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{
    compter_lignes, compter_sessions_actives, decrire_table, executer_sql_admin, get_taille_db,
    get_tailles_tables, inserer_ou_modifier, lire_lignes_table, lister_tables, selectionner,
    supprimer_ligne, verifier_connexion_avec_expiration, DbPool,
};
use crate::config_loader::{load_config, VexConfig};
use crate::function::{build_nav_html, get_user_language, get_user_preferences, NavContext};
use crate::i18n::{self, Cle};
use crate::p2p::p2p::{admin_handle_api as p2p_admin_handle_api, NodeState, P2pConfig};
use crate::utils::{parse_query, strip_port, url_decode};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock};
use tiny_http::{Request, Response};

use crate::admin::actions::{PRIVILEGE_MAX, PRIVILEGE_MIN_SET, PRIVILEGE_SUPER};

const HTML_PATH: &str = "static/admin/admin.html";

/// Cles injectees dans `const I18N = {...}` cote JS (voir {{I18N_JS}} dans admin.html).
const ADMIN_I18N_JS_KEYS: [(&str, Cle); 313] = [
    ("ACTIF", Cle::AdmActif),
    ("BACKUP_COL_TAILLE", Cle::AdmBackupColTaille),
    ("ACTION", Cle::AdmAction),
    ("ACTIONS", Cle::AdmActions),
    ("ADMIN_HASH_FALLBACK", Cle::AdmAdminHashFallback),
    ("AJOUTER", Cle::AdmAjouter),
    ("ANNULER", Cle::AdmAnnuler),
    ("BACKUP_AUCUNE_SAUVEGARDE", Cle::AdmBackupAucuneSauvegarde),
    ("BACKUP_CONFIRM_LANCER", Cle::AdmBackupConfirmLancer),
    ("BACKUP_CONFIRM_SUPPRIMER", Cle::AdmBackupConfirmSupprimer),
    ("BACKUP_DEMARREE_A", Cle::AdmBackupDemarreeA),
    ("BACKUP_DERNIERE_ECHOUEE", Cle::AdmBackupDerniereEchouee),
    ("BACKUP_DERNIERE_REUSSIE", Cle::AdmBackupDerniereReussie),
    ("BACKUP_EN_COURS", Cle::AdmBackupEnCours),
    ("BACKUP_LANCER_SAUVEGARDE", Cle::AdmBackupLancerSauvegarde),
    ("BACKUP_TELECHARGER", Cle::AdmBackupTelecharger),
    ("BACKUP_TITRE_SUPPRIMER", Cle::AdmBackupTitreSupprimer),
    ("BLOCKS_AUCUN_BLOCAGE", Cle::AdmBlocksAucunBlocage),
    ("BLOCKS_CIBLE", Cle::AdmBlocksCible),
    ("BLOCKS_COL_PAGES", Cle::AdmBlocksColPages),
    ("BLOCKS_COL_PRIVILEGE", Cle::AdmBlocksColPrivilege),
    ("BLOCKS_POSE_PAR", Cle::AdmBlocksPosePar),
    ("BLOCKS_TOUS_BADGE", Cle::AdmBlocksTousBadge),
    ("BLOCKS_TOUS_OPTION", Cle::AdmBlocksTousOption),
    ("CFG_ADRESSE_ONION", Cle::AdmCfgAdresseOnion),
    ("CFG_APPLICATION", Cle::AdmCfgApplication),
    ("CFG_BOOTSTRAP_URL", Cle::AdmCfgBootstrapUrl),
    ("CFG_CHIFFRE", Cle::AdmCfgChiffre),
    ("CFG_CHUNK_BYTES", Cle::AdmCfgChunkBytes),
    ("CFG_CLE_ACTIVATION", Cle::AdmCfgCleActivation),
    ("CFG_CLE_REQUISE", Cle::AdmCfgCleRequise),
    ("CFG_DEBUG", Cle::AdmCfgDebug),
    ("CFG_DOSSIER_CHUNKS", Cle::AdmCfgDossierChunks),
    ("CFG_DOSSIER_SORTIE", Cle::AdmCfgDossierSortie),
    ("CFG_DUREE_SESSION", Cle::AdmCfgDureeSession),
    ("CFG_EDITEUR_DOCUMENTS", Cle::AdmCfgEditeurDocuments),
    ("CFG_EDITION_COLLABORATIVE", Cle::AdmCfgEditionCollaborative),
    ("CFG_EDITION_LIGNE_ACTIVEE", Cle::AdmCfgEditionLigneActivee),
    ("CFG_FORMATS_SUPPORTES", Cle::AdmCfgFormatsSupportes),
    ("CFG_INSCRIPTION", Cle::AdmCfgInscription),
    ("CFG_LANGUE_DEFAUT", Cle::AdmCfgLangueDefaut),
    ("CFG_LONGUEUR_MIN_MDP", Cle::AdmCfgLongueurMinMdp),
    ("CFG_MAINTENANCE", Cle::AdmCfgMaintenance),
    ("CFG_MAJUSCULE", Cle::AdmCfgMajuscule),
    ("CFG_NB_MAX", Cle::AdmCfgNbMax),
    ("CFG_NOM", Cle::AdmCfgNom),
    ("CFG_OPTION_ACTIVE", Cle::AdmCfgOptionActive),
    ("CFG_OPTION_DESACTIVE", Cle::AdmCfgOptionDesactive),
    ("CFG_PORT_P2P", Cle::AdmCfgPortP2p),
    ("CFG_RETENTION_J", Cle::AdmCfgRetentionJ),
    ("CFG_SECURITE", Cle::AdmCfgSecurite),
    ("CFG_SERVICE_EDITION", Cle::AdmCfgServiceEdition),
    ("CFG_STOCKAGE", Cle::AdmCfgStockage),
    ("CFG_SYNC_INTERVAL", Cle::AdmCfgSyncInterval),
    ("CFG_TAILLE_MAX_MB", Cle::AdmCfgTailleMaxMb),
    ("CFG_TENTATIVES_MAX", Cle::AdmCfgTentativesMax),
    ("CFG_UTILISER_TOR", Cle::AdmCfgUtiliserTor),
    ("CFG_VERROUILLAGE", Cle::AdmCfgVerrouillage),
    ("CFG_VERSION", Cle::AdmCfgVersion),
    ("CONFIRMER", Cle::AdmConfirmer),
    ("CONFIRMER_SUPPRIMER_BLOCAGE", Cle::AdmConfirmerSupprimerBlocage),
    ("CONFIRMER_SUPPRIMER_UTILISATEUR", Cle::AdmConfirmerSupprimerUtilisateur),
    ("DASH_COL_ROLE", Cle::AdmDashColRole),
    ("DASH_COL_VIP", Cle::AdmDashColVip),
    ("DATE", Cle::AdmDate),
    ("DB_AJOUTER_DANS", Cle::AdmDbAjouterDans),
    ("DB_AJOUTER_LIGNE", Cle::AdmDbAjouterLigne),
    ("DB_CLIQUEZ_POUR_MODIFIER", Cle::AdmDbCliquezPourModifier),
    ("DB_COL_LIGNES", Cle::AdmDbColLignes),
    ("DB_COL_TABLE", Cle::AdmDbColTable),
    ("DB_COL_TAILLE_KB", Cle::AdmDbColTailleKb),
    ("DB_CONFIRMER_SUPPRIMER_LIGNE", Cle::AdmDbConfirmerSupprimerLigne),
    ("DB_ERR_COLONNES_VALEURS_INVALIDES_JS", Cle::AdmDbErrColonnesValeursInvalidesJs),
    ("DB_IMPOSSIBLE_RECUPERER_STRUCTURE", Cle::AdmDbImpossibleRecupererStructure),
    ("DB_MODIFIER_CELL_PROMPT", Cle::AdmDbModifierCellPrompt),
    ("DB_N_TABLES", Cle::AdmDbNTables),
    ("DB_TABLES_COEUR", Cle::AdmDbTablesCoeur),
    ("DB_TABLES_EXTENSIONS", Cle::AdmDbTablesExtensions),
    ("DB_TABLE_VIDE", Cle::AdmDbTableVide),
    ("DB_TAILLE_TABLES_COEUR", Cle::AdmDbTailleTablesCoeur),
    ("DB_TAILLE_TABLES_EXT", Cle::AdmDbTailleTablesExt),
    ("DB_TOUTES_LES_TABLES", Cle::AdmDbToutesLesTables),
    ("DB_TRIER_PAR_COL", Cle::AdmDbTrierParCol),
    ("DESACTIF", Cle::AdmDesactif),
    ("EDITEUR_DEFAULT", Cle::AdmEditeurDefault),
    ("ED_AJOUTER_SYSTEME", Cle::AdmEdAjouterSysteme),
    ("ED_AJOUTER_SYSTEME_TITRE", Cle::AdmEdAjouterSystemeTitre),
    ("ED_ARRETER", Cle::AdmEdArreter),
    ("ED_ATTENTE_DEMARRAGE", Cle::AdmEdAttenteDemarrage),
    ("ED_CHEMIN_CALLBACK", Cle::AdmEdCheminCallback),
    ("ED_CHEMIN_HEALTHCHECK", Cle::AdmEdCheminHealthcheck),
    ("ED_CHEMIN_SCRIPT_API", Cle::AdmEdCheminScriptApi),
    ("ED_CHEMIN_WOPI", Cle::AdmEdCheminWopi),
    ("ED_COMMANDE_ARRET", Cle::AdmEdCommandeArret),
    ("ED_COMMANDE_DEMARRAGE", Cle::AdmEdCommandeDemarrage),
    ("ED_CONFIRM_SUPPRIMER", Cle::AdmEdConfirmSupprimer),
    ("ED_CONNEXION_OK", Cle::AdmEdConnexionOk),
    ("ED_DEMARRER", Cle::AdmEdDemarrer),
    ("ED_ECHEC_LABEL", Cle::AdmEdEchecLabel),
    ("ED_ECHEC_PT", Cle::AdmEdEchecPt),
    ("ED_EDITION_LIGNE_ACTIVEE", Cle::AdmEdEditionLigneActivee),
    ("ED_FORMAT_DEFAUT", Cle::AdmEdFormatDefaut),
    ("ED_IDENTIFIANT_INTERNE", Cle::AdmEdIdentifiantInterne),
    ("ED_JWT_ACTIVE", Cle::AdmEdJwtActive),
    ("ED_PARAMETRES_DE", Cle::AdmEdParametresDe),
    ("ED_RETESTER", Cle::AdmEdRetester),
    ("ED_SAUVEGARDE_AUTO", Cle::AdmEdSauvegardeAuto),
    ("ED_SECRET_JWT", Cle::AdmEdSecretJwt),
    ("ED_SERVEUR_ARRETE", Cle::AdmEdServeurArrete),
    ("ED_SERVEUR_DEMARRE", Cle::AdmEdServeurDemarre),
    ("ED_SUITE_BUREAUTIQUE", Cle::AdmEdSuiteBureautique),
    ("ED_SYSTEME_ACTIF", Cle::AdmEdSystemeActif),
    ("ED_TAILLE_MAX_MO", Cle::AdmEdTailleMaxMo),
    ("ED_TESTER_CONNEXION", Cle::AdmEdTesterConnexion),
    ("ED_TEST_EN_COURS", Cle::AdmEdTestEnCours),
    ("ED_TEXTE_ATTENDU", Cle::AdmEdTexteAttendu),
    ("ED_URL_SERVEUR", Cle::AdmEdUrlServeur),
    ("EMAIL", Cle::AdmEmail),
    ("EN_LIGNE", Cle::AdmEnLigne),
    ("ERREUR_SELECTIONNER_PAGE", Cle::AdmErreurSelectionnerPage),
    ("EXT_ACTION_BTN", Cle::AdmExtActionBtn),
    ("EXT_ACTIVEE_MSG", Cle::AdmExtActiveeMsg),
    ("EXT_ACTIVER_IMMEDIATEMENT", Cle::AdmExtActiverImmediatement),
    ("EXT_ACTUALISER", Cle::AdmExtActualiser),
    ("EXT_APP_DE_BASE_BADGE", Cle::AdmExtAppDeBaseBadge),
    ("EXT_APP_DU_MENU", Cle::AdmExtAppDuMenu),
    ("EXT_AUCUNE_ACTION_DECLAREE", Cle::AdmExtAucuneActionDeclaree),
    ("EXT_AUCUNE_EXTENSION", Cle::AdmExtAucuneExtension),
    ("EXT_AUCUNE_SORTIE", Cle::AdmExtAucuneSortie),
    ("EXT_AUCUN_FICHIER_SELECTIONNE", Cle::AdmExtAucunFichierSelectionne),
    ("EXT_AUTORISATIONS_INTERNES", Cle::AdmExtAutorisationsInternes),
    ("EXT_AUTORISATIONS_SUB", Cle::AdmExtAutorisationsSub),
    ("EXT_CHANGER_CATALOGUE", Cle::AdmExtChangerCatalogue),
    ("EXT_CHARGEMENT_CATALOGUE", Cle::AdmExtChargementCatalogue),
    ("EXT_CHOISISSEZ_FICHIER", Cle::AdmExtChoisissezFichier),
    ("EXT_COMPILATION_EN_COURS", Cle::AdmExtCompilationEnCours),
    ("EXT_COMPILEE_BADGE", Cle::AdmExtCompileeBadge),
    ("EXT_COMPILEE_TITLE", Cle::AdmExtCompileeTitle),
    ("EXT_CONFIRM_MAJ_GITHUB", Cle::AdmExtConfirmMajGithub),
    ("EXT_CONFIRM_RELANCER", Cle::AdmExtConfirmRelancer),
    ("EXT_CONFIRM_RETIRER", Cle::AdmExtConfirmRetirer),
    ("EXT_CONFIRM_SUPPR_FICHIERS", Cle::AdmExtConfirmSupprFichiers),
    ("EXT_DEMARREE_A", Cle::AdmExtDemarreeA),
    ("EXT_DERNIERE_COMPILATION", Cle::AdmExtDerniereCompilation),
    ("EXT_DESACTIVEE_MSG", Cle::AdmExtDesactiveeMsg),
    ("EXT_ECHOUEE", Cle::AdmExtEchouee),
    ("EXT_ERREUR_EXTENSION_RS", Cle::AdmExtErreurExtensionRs),
    ("EXT_ERREUR_FICHIER_TROP_GROS", Cle::AdmExtErreurFichierTropGros),
    ("EXT_EXTENSIONS_INSTALLEES", Cle::AdmExtExtensionsInstallees),
    ("EXT_EXTENSION_BADGE", Cle::AdmExtExtensionBadge),
    ("EXT_FICHIER_DOIT_EXPOSER_DESC", Cle::AdmExtFichierDoitExposerDesc),
    ("EXT_FICHIER_ECRIT_DESC", Cle::AdmExtFichierEcritDesc),
    ("EXT_FICHIER_SOURCE_RUST", Cle::AdmExtFichierSourceRust),
    ("EXT_ID_EXTENSION", Cle::AdmExtIdExtension),
    ("EXT_ID_REQUIS", Cle::AdmExtIdRequis),
    ("EXT_IMPOSSIBLE_SUPPRIMER_BASE", Cle::AdmExtImpossibleSupprimerBase),
    ("EXT_INFOS_TUILE_ADMIN", Cle::AdmExtInfosTuileAdmin),
    ("EXT_INSTALLEE_AVEC_MOTIFS_MSG", Cle::AdmExtInstalleeAvecMotifsMsg),
    ("EXT_INSTALLER_DEPUIS_FICHIER", Cle::AdmExtInstallerDepuisFichier),
    ("EXT_INSTALLER_RS", Cle::AdmExtInstallerRs),
    ("EXT_JSON_INVALIDE_LIB", Cle::AdmExtJsonInvalideLib),
    ("EXT_METTRE_A_JOUR_DESC", Cle::AdmExtMettreAJourDesc),
    ("EXT_METTRE_A_JOUR_GITHUB", Cle::AdmExtMettreAJourGithub),
    ("EXT_MODELE_RS", Cle::AdmExtModeleRs),
    ("EXT_PARAMETRES_INVALIDES", Cle::AdmExtParametresInvalides),
    ("EXT_PARAMETRES_JSON", Cle::AdmExtParametresJson),
    ("EXT_PARCOURIR", Cle::AdmExtParcourir),
    ("EXT_PLACEHOLDER_ACTION", Cle::AdmExtPlaceholderAction),
    ("EXT_PLACEHOLDER_VIDE_TOUS", Cle::AdmExtPlaceholderVideTous),
    ("EXT_PLANS_AUTORISES", Cle::AdmExtPlansAutorises),
    ("EXT_PLANS_AUTORISES_SUB", Cle::AdmExtPlansAutorisesSub),
    ("EXT_PLANS_VIRGULE", Cle::AdmExtPlansVirgule),
    ("EXT_PRIVILEGE_MAX", Cle::AdmExtPrivilegeMax),
    ("EXT_PRIVILEGE_MIN_REQUIS", Cle::AdmExtPrivilegeMinRequis),
    ("EXT_PRIVILEGE_MIN_SUB", Cle::AdmExtPrivilegeMinSub),
    ("EXT_RECHERCHER_INSTALLER", Cle::AdmExtRechercherInstaller),
    ("EXT_RECOMPILER_VEX", Cle::AdmExtRecompilerVex),
    ("EXT_REMPLACER_CODE", Cle::AdmExtRemplacerCode),
    ("EXT_REMPLACER_RS", Cle::AdmExtRemplacerRs),
    ("EXT_REUPLOADEZ_DESC", Cle::AdmExtReuploadezDesc),
    ("EXT_REUSSIE", Cle::AdmExtReussie),
    ("EXT_SAUVER_PERMISSIONS", Cle::AdmExtSauverPermissions),
    ("EXT_SORTIE", Cle::AdmExtSortie),
    ("EXT_SOURCES_NON_ENREGISTREES", Cle::AdmExtSourcesNonEnregistrees),
    ("EXT_SOURCE_ABSENTE_BADGE", Cle::AdmExtSourceAbsenteBadge),
    ("EXT_SOURCE_ABSENTE_TITLE", Cle::AdmExtSourceAbsenteTitle),
    ("EXT_STATIQUE_BADGE", Cle::AdmExtStatiqueBadge),
    ("EXT_STATIQUE_OK", Cle::AdmExtStatiqueOk),
    ("EXT_STATIQUE_TITLE", Cle::AdmExtStatiqueTitle),
    ("EXT_SUPERADMIN_UNIQUEMENT_TITLE", Cle::AdmExtSuperadminUniquementTitle),
    ("EXT_TITRE_RETIRER", Cle::AdmExtTitreRetirer),
    ("EXT_TITRE_SUPPR_FICHIERS", Cle::AdmExtTitreSupprFichiers),
    ("EXT_TUILE_DASHBOARD", Cle::AdmExtTuileDashboard),
    ("EXT_UPLOADER", Cle::AdmExtUploader),
    ("FICHIER_COL", Cle::AdmFichierCol),
    ("FREE", Cle::AdmFree),
    ("HORS_LIGNE", Cle::AdmHorsLigne),
    ("LOGS_ADMIN", Cle::AdmLogsAdmin),
    ("LOGS_AUCUNE_ACTION_VEXIA", Cle::AdmLogsAucuneActionVexia),
    ("LOGS_AUCUN_DISPONIBLE", Cle::AdmLogsAucunDisponible),
    ("LOGS_DETAIL", Cle::AdmLogsDetail),
    ("LOGS_FICHIERS_DE_LOGS", Cle::AdmLogsFichiersDeLogs),
    ("LOGS_OUTIL", Cle::AdmLogsOutil),
    ("LOGS_RESERVE_SUPERADMINS", Cle::AdmLogsReserveSuperadmins),
    ("LOGS_RESULTAT", Cle::AdmLogsResultat),
    ("LOGS_SCOPED", Cle::AdmLogsScoped),
    ("LOGS_TIER", Cle::AdmLogsTier),
    ("LOGS_UTILISATEUR", Cle::AdmLogsUtilisateur),
    ("LOGS_VIDER_CONFIRM", Cle::AdmLogsViderConfirm),
    ("MK_AUCUNE_CORRESPOND", Cle::AdmMkAucuneCorrespond),
    ("MK_A_COMPILER_BADGE", Cle::AdmMkACompilerBadge),
    ("MK_CATALOGUE_INDISPONIBLE", Cle::AdmMkCatalogueIndisponible),
    ("MK_CONFIRM_INSTALLER_MSG", Cle::AdmMkConfirmInstallerMsg),
    ("MK_ELEMENT_INTROUVABLE", Cle::AdmMkElementIntrouvable),
    ("MK_INSTALLATION_IMPOSSIBLE", Cle::AdmMkInstallationImpossible),
    ("MK_INSTALLEE_BADGE", Cle::AdmMkInstalleeBadge),
    ("MK_INSTALLER", Cle::AdmMkInstaller),
    ("MK_INSTALLER_QUAND_MEME", Cle::AdmMkInstallerQuandMeme),
    ("MK_INTERROGATION_GITHUB", Cle::AdmMkInterrogationGithub),
    ("MK_REINSTALLER", Cle::AdmMkReinstaller),
    ("MK_TELECHARGEMENT_DE", Cle::AdmMkTelechargementDe),
    ("MK_TEL_DOT", Cle::AdmMkTelDot),
    ("MK_TITRE_INSTALLER_DISTANTE", Cle::AdmMkTitreInstallerDistante),
    ("MK_TOUTES_INSTALLEES", Cle::AdmMkToutesInstallees),
    ("MK_TYPE_ARCHIVE", Cle::AdmMkTypeArchive),
    ("MK_TYPE_EXTENSION_COMPLETE", Cle::AdmMkTypeExtensionComplete),
    ("MK_TYPE_FICHIER_DEFAULT", Cle::AdmMkTypeFichierDefault),
    ("MK_TYPE_FICHIER_RS", Cle::AdmMkTypeFichierRs),
    ("NOM", Cle::AdmNom),
    ("N_LIGNES", Cle::AdmNLignes),
    ("P2P_ANNUAIRE_COPIE", Cle::AdmP2pAnnuaireCopie),
    ("P2P_AUCUN_NOEUD", Cle::AdmP2pAucunNoeud),
    ("P2P_AUCUN_TRANSFERT", Cle::AdmP2pAucunTransfert),
    ("P2P_AUCUN_UTILISATEUR", Cle::AdmP2pAucunUtilisateur),
    ("P2P_CHUNKS", Cle::AdmP2pChunks),
    ("P2P_COL_CLE_PUB", Cle::AdmP2pColClePub),
    ("P2P_COL_STATUT", Cle::AdmP2pColStatut),
    ("P2P_COL_URL_VEX", Cle::AdmP2pColUrlVex),
    ("P2P_COL_VER", Cle::AdmP2pColVer),
    ("P2P_COL_VU_LE", Cle::AdmP2pColVuLe),
    ("P2P_CONFIRM_SUPPRIMER_NOEUD", Cle::AdmP2pConfirmSupprimerNoeud),
    ("P2P_DE_VERS", Cle::AdmP2pDeVers),
    ("P2P_KICK", Cle::AdmP2pKick),
    ("P2P_NOEUDS", Cle::AdmP2pNoeuds),
    ("P2P_SYNC_BOOTSTRAP", Cle::AdmP2pSyncBootstrap),
    ("P2P_SYNC_EN_COURS", Cle::AdmP2pSyncEnCours),
    ("PRIV_ADMIN", Cle::AdmPrivAdmin),
    ("PRIV_AUCUN", Cle::AdmPrivAucun),
    ("PRIV_BAN", Cle::AdmPrivBan),
    ("PRIV_BETA_TESTEUR", Cle::AdmPrivBetaTesteur),
    ("PRIV_FONDATEUR", Cle::AdmPrivFondateur),
    ("PRIV_MODERATEUR", Cle::AdmPrivModerateur),
    ("PRIV_SUPER_ADMIN", Cle::AdmPrivSuperAdmin),
    ("PRIV_SUPER_MODERATEUR", Cle::AdmPrivSuperModerateur),
    ("PRIV_UTILISATEUR_CERTIFIE", Cle::AdmPrivUtilisateurCertifie),
    ("PRIV_VERIFICATEUR", Cle::AdmPrivVerificateur),
    ("ROLES_NOMS_SAUVEGARDES", Cle::AdmRolesNomsSauvegardes),
    ("ROLES_NOM_AFFICHE", Cle::AdmRolesNomAffiche),
    ("ROLES_PLACEHOLDER_NOM_ROLE", Cle::AdmRolesPlaceholderNomRole),
    ("ROLES_PLANS_SAUVEGARDES", Cle::AdmRolesPlansSauvegardes),
    ("ROLES_PRIVILEGE_LABEL", Cle::AdmRolesPrivilegeLabel),
    ("ROLES_PRIX_EUROS_MOIS", Cle::AdmRolesPrixEurosMois),
    ("ROLES_SAUVEGARDER_PLANS", Cle::AdmRolesSauvegarderPlans),
    ("ROLES_STOCKAGE", Cle::AdmRolesStockage),
    ("ROLES_SUPPRIMER_CE_PLAN", Cle::AdmRolesSupprimerCePlan),
    ("SAUVEGARDER", Cle::AdmSauvegarder),
    ("SB_EXTENSIONS_PLUGIN", Cle::AdmSbExtensionsPlugin),
    ("SB_UTILISATEURS", Cle::AdmSbUtilisateurs),
    ("SERVER_CLIQUEZ_VERIFIER", Cle::AdmServerCliquezVerifier),
    ("SERVER_DISQUE_LABEL", Cle::AdmServerDisqueLabel),
    ("SERVER_LECTURE_SEULE", Cle::AdmServerLectureSeule),
    ("SERVER_MAJ_RUST_DISPO", Cle::AdmServerMajRustDispo),
    ("SERVER_MISES_A_JOUR_SYSTEME", Cle::AdmServerMisesAJourSysteme),
    ("SERVER_NON_DISPONIBLE_PLATEFORME", Cle::AdmServerNonDisponiblePlateforme),
    ("SERVER_NOYAU", Cle::AdmServerNoyau),
    ("SERVER_OS_ARCH", Cle::AdmServerOsArch),
    ("SERVER_PAQUETS_A_METTRE_A_JOUR", Cle::AdmServerPaquetsAMettreAJour),
    ("SERVER_RUST_LABEL", Cle::AdmServerRustLabel),
    ("SERVER_SUR_LE_SERVEUR", Cle::AdmServerSurLeServeur),
    ("SERVER_SYSTEME_A_JOUR", Cle::AdmServerSystemeAJour),
    ("SERVER_SYSTEME_EXPLOITATION", Cle::AdmServerSystemeExploitation),
    ("SERVER_TOOLCHAIN_A_JOUR", Cle::AdmServerToolchainAJour),
    ("SERVER_UPTIME", Cle::AdmServerUptime),
    ("SERVER_VERIFIER", Cle::AdmServerVerifier),
    ("SERVER_VERSION_VEX", Cle::AdmServerVersionVex),
    ("SQL_AUCUN_RESULTAT", Cle::AdmSqlAucunResultat),
    ("SQL_COLONNE_PLUR", Cle::AdmSqlColonnePlur),
    ("SQL_COLONNE_SING", Cle::AdmSqlColonneSing),
    ("SQL_ERREUR_TITRE", Cle::AdmSqlErreurTitre),
    ("SQL_EXECUTER", Cle::AdmSqlExecuter),
    ("SQL_EXECUTION_EN_COURS", Cle::AdmSqlExecutionEnCours),
    ("SQL_LIGNE_AFFECTEE_PLUR", Cle::AdmSqlLigneAffecteePlur),
    ("SQL_LIGNE_AFFECTEE_SING", Cle::AdmSqlLigneAffecteeSing),
    ("SQL_LIGNE_RETOURNEE_PLUR", Cle::AdmSqlLigneRetourneePlur),
    ("SQL_LIGNE_RETOURNEE_SING", Cle::AdmSqlLigneRetourneeSing),
    ("SQL_LIMITE_ATTEINTE", Cle::AdmSqlLimiteAtteinte),
    ("SQL_LIMITE_ATTEINTE_MSG", Cle::AdmSqlLimiteAtteinteMsg),
    ("SQL_REQUETE_AUCUNE_LIGNE", Cle::AdmSqlRequeteAucuneLigne),
    ("SQL_REQUETE_VIDE", Cle::AdmSqlRequeteVide),
    ("STAT_BASE_DONNEES", Cle::AdmStatBaseDonnees),
    ("STAT_FICHIERS", Cle::AdmStatFichiers),
    ("STAT_PAGES_SITEC", Cle::AdmStatPagesSitec),
    ("STAT_SESSIONS", Cle::AdmStatSessions),
    ("SUPPRIMER", Cle::AdmSupprimer),
    ("SUPPRIMER_FULL", Cle::AdmSupprimerFull),
    ("TAILLE_COL", Cle::AdmTailleCol),
    ("TITRE_SUPPRIMER_UTILISATEUR", Cle::AdmTitreSupprimerUtilisateur),
    ("TOUS_LES_UTILISATEURS_OPTION", Cle::AdmTousLesUtilisateursOption),
    ("USERS_CHANGER_ROLE", Cle::AdmUsersChangerRole),
    ("USERS_N_COMPTES", Cle::AdmUsersNComptes),
    ("USERS_ROLE_ACTUEL", Cle::AdmUsersRoleActuel),
    ("USER_HASH_FALLBACK", Cle::AdmUserHashFallback),
    ("VIP_FREE_OPTION", Cle::AdmVipFreeOption),
];

fn vex_pages() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/login/login", "Login"),
        ("/login/dashboard", "Dashboard"),
        ("/login/account", "Mon compte"),
        ("/tel/", "ExoDrive — liste"),
        ("/edite1/", "Éditeur — liste"),
        ("/sitec", "SiteC (tout)"),
        ("/meet", "Meet (tout)"),
        ("/mess", "Messagerie (tout)"),
        ("/admin/admin", "Admin Panel"),
        ("/node", "Node (tout)"),
        ("/onlyoffice", "OnlyOffice (tout)"),
        ("/ext/", "Extensions (toutes)"),
    ]
}

fn log_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("log")
}

fn log_files() -> Vec<std::path::PathBuf> {
    let _ = std::fs::create_dir_all(log_dir());
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(log_dir()) {
        let mut collected: Vec<_> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let is_vex_log = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| name.starts_with("vex_") && name.ends_with(".log"))
                    .unwrap_or(false);
                is_vex_log.then_some(path)
            })
            .collect();
        collected.sort();
        files.extend(collected);
    }

    if files.is_empty() {
        let legacy = std::env::temp_dir().join("onlyoffice-callback.log");
        if legacy.exists() {
            files.push(legacy);
        }
    }

    files
}

fn read_log_content() -> (bool, String, Vec<String>) {
    let mut parts = Vec::new();
    let mut files = Vec::new();
    for path in log_files() {
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("log")
            .to_string();
        files.push(label.clone());
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                parts.push(format!("===== {} =====\n{}", label, trimmed));
            }
        }
    }
    (parts.is_empty(), parts.join("\n\n"), files)
}

pub fn handle_request(
    request: Request,
    pool: &DbPool,
    config: &VexConfig,
    config_path: &str,
    remote_full: &str,
) {
    let remote_ip = strip_port(remote_full);
    let url = request.url().to_string();
    let method = request.method().to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    let session_minutes = config.users.session_expiration_minutes as u32;
    let user_info = verifier_connexion_avec_expiration(
        pool,
        &cookie_val,
        &remote_ip,
        &user_agent,
        session_minutes,
    );
    let user = match &user_info {
        Some(u) => u,
        None => {
            let _ = request.respond(
                Response::empty(302)
                    .with_header(tiny_http::Header::from_bytes("Location", "/login").unwrap()),
            );
            return;
        }
    };

    let user_id = user.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);

    if privilege > PRIVILEGE_MAX {
        let _ = request.respond(Response::from_string(
            r#"<!DOCTYPE html><html><body style="font-family:monospace;display:flex;align-items:center;justify-content:center;height:100vh;background:#0a0a0a;color:#4caf50">
<div style="text-align:center"><div style="font-size:6rem;font-weight:900">403</div>
<p style="opacity:.6">Accès réservé aux administrateurs</p>
<a href="/" style="color:#4caf50;display:block;margin-top:1rem">← Retour</a></div>
</body></html>"#)
            .with_status_code(403)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()));
        return;
    }

    let query = parse_query(&url);
    let prefs = get_user_preferences(pool, user_id);
    let theme = if prefs.teme == 1 { "dark" } else { "light" };
    let lang = get_user_language(pool, Some(user_id), None, None);

    let path = url.split('?').next().unwrap_or(&url).to_string();
    if path.starts_with("/api/admin") {
        handle_api(
            request,
            pool,
            config_path,
            &query,
            &method,
            user_id,
            privilege,
            &cookie_val,
            &remote_ip,
            &user_agent,
            &lang,
        );
        return;
    }

    let nav_ctx = NavContext {
        pool,
        user_id: Some(user_id),
        page_key: "admin",
        cookie_val: &cookie_val,
        remote_ip: &remote_ip,
        user_agent: &user_agent,
        query_id: None,
        apps: vec![],
        admin_apps: vec![],
    };
    let nav_html = build_nav_html(&nav_ctx);

    let html = match std::fs::read_to_string(HTML_PATH) {
        Ok(s) => {
            let s = s
                .replace("__NAV_HTML__", &nav_html)
                .replace("__LANG__", &lang)
                .replace("__THEME__", theme);
            let s = i18n::appliquer_traductions(&s, &lang, &[
                ("{{T_TITRE_ONGLET}}", Cle::AdmTitreOnglet),
                ("{{T_SB_ADMIN_PANEL}}", Cle::AdmSbAdminPanel),
                ("{{T_SB_PRINCIPAL}}", Cle::AdmSbPrincipal),
                ("{{T_SB_DASHBOARD}}", Cle::AdmSbDashboard),
                ("{{T_SB_UTILISATEURS}}", Cle::AdmSbUtilisateurs),
                ("{{T_SB_BLOCAGES}}", Cle::AdmSbBlocages),
                ("{{T_SB_SYSTEME}}", Cle::AdmSbSysteme),
                ("{{T_SB_BASE_DE_DONNEES}}", Cle::AdmSbBaseDeDonnees),
                ("{{T_SB_CONFIGURATION}}", Cle::AdmSbConfiguration),
                ("{{T_SB_APPLICATIONS}}", Cle::AdmSbApplications),
                ("{{T_SB_EXTENSIONS_PLUGIN}}", Cle::AdmSbExtensionsPlugin),
                ("{{T_SB_ROLES_PLANS}}", Cle::AdmSbRolesPlans),
                ("{{T_SB_SERVEUR}}", Cle::AdmSbServeur),
                ("{{T_SB_LOGS}}", Cle::AdmSbLogs),
                ("{{T_SB_EDITEUR_EN_LIGNE}}", Cle::AdmSbEditeurEnLigne),
                ("{{T_SB_P2P_ANONNET}}", Cle::AdmSbP2pAnonnet),
                ("{{T_SB_SAUVEGARDES}}", Cle::AdmSbSauvegardes),
                ("{{T_CHARGEMENT}}", Cle::AdmChargement),
                ("{{T_DASH_DERNIERS_INSCRITS}}", Cle::AdmDashDerniersInscrits),
                ("{{T_USERS_TOUS_LES_COMPTES}}", Cle::AdmUsersTousLesComptes),
                ("{{T_RECHERCHE_PLACEHOLDER}}", Cle::AdmRecherchePlaceholder),
                ("{{T_BLOCKS_AJOUTER_BLOCAGE}}", Cle::AdmBlocksAjouterBlocage),
                ("{{T_BLOCKS_CIBLE}}", Cle::AdmBlocksCible),
                ("{{T_BLOCKS_TOUS_OPTION}}", Cle::AdmBlocksTousOption),
                ("{{T_BLOCKS_BLOQUER_SI_PRIVILEGE}}", Cle::AdmBlocksBloquerSiPrivilege),
                ("{{T_BLOCKS_PAGES_BLOQUEES}}", Cle::AdmBlocksPagesBloquees),
                ("{{T_BLOCKS_AJOUTER_LE_BLOCAGE}}", Cle::AdmBlocksAjouterLeBlocage),
                ("{{T_BLOCKS_TOUT_COCHER}}", Cle::AdmBlocksToutCocher),
                ("{{T_BLOCKS_ACTIFS}}", Cle::AdmBlocksActifs),
                ("{{T_SQL_REQUETE}}", Cle::AdmSqlRequete),
                ("{{T_SQL_SUPERADMIN}}", Cle::AdmSqlSuperadmin),
                ("{{T_SQL_META_TEXT}}", Cle::AdmSqlMetaText),
                ("{{T_SQL_EXECUTER}}", Cle::AdmSqlExecuter),
                ("{{T_SQL_EFFACER}}", Cle::AdmSqlEffacer),
                ("{{T_DB_MODE_EDITION_ACTIF}}", Cle::AdmDbModeEditionActif),
                ("{{T_DB_CLIQUEZ_POUR_MODIFIER}}", Cle::AdmDbCliquezPourModifier),
                ("{{T_SQL_RETOUR_EXPLORATEUR}}", Cle::AdmSqlRetourExplorateur),
                ("{{T_EXT_STRUCTURE_ATTENDUE}}", Cle::AdmExtStructureAttendue),
                ("{{T_EXT_FICHIER_UPLOADE_COMMENT}}", Cle::AdmExtFichierUploadeComment),
                ("{{T_EXT_CONVENTION}}", Cle::AdmExtConvention),
                ("{{T_EXT_CONVENTION_DESC}}", Cle::AdmExtConventionDesc),
                ("{{T_EXT_APP_DE_BASE_BADGE}}", Cle::AdmExtAppDeBaseBadge),
                ("{{T_EXT_APP_DE_BASE_DESC}}", Cle::AdmExtAppDeBaseDesc),
                ("{{T_EXT_EXTENSION_BADGE}}", Cle::AdmExtExtensionBadge),
                ("{{T_EXT_EXTENSION_DESC}}", Cle::AdmExtExtensionDesc),
                ("{{T_ROLES_NOMS_ROLES}}", Cle::AdmRolesNomsRoles),
                ("{{T_ROLES_SAUVEGARDER_NOMS}}", Cle::AdmRolesSauvegarderNoms),
                ("{{T_ROLES_PLANS_TARIFAIRES}}", Cle::AdmRolesPlansTarifaires),
                ("{{T_ROLES_NOUVEAU_PLAN}}", Cle::AdmRolesNouveauPlan),
                ("{{T_SERVER_MONITORING}}", Cle::AdmServerMonitoring),
                ("{{T_BACKUP_SUPERADMIN_UNIQUEMENT}}", Cle::AdmBackupSuperadminUniquement),
                ("{{T_BACKUP_DUMP_COMPLET}}", Cle::AdmBackupDumpComplet),
                ("{{T_BACKUP_LANCER_SAUVEGARDE}}", Cle::AdmBackupLancerSauvegarde),
                ("{{T_BACKUP_DESCRIPTION_TEXT}}", Cle::AdmBackupDescriptionText),
                ("{{T_BACKUP_SAUVEGARDES_EXISTANTES}}", Cle::AdmBackupSauvegardesExistantes),
                ("{{T_RAFRAICHIR}}", Cle::AdmRafraichir),
                ("{{T_LOGS_SERVEUR}}", Cle::AdmLogsServeur),
                ("{{T_LOGS_VEX_TITRE}}", Cle::AdmLogsVexTitre),
                ("{{T_LOGS_VIDER}}", Cle::AdmLogsVider),
                ("{{T_LOGS_BAS}}", Cle::AdmLogsBas),
                ("{{T_LOGS_ACTIONS_VEXIA}}", Cle::AdmLogsActionsVexia),
                ("{{T_LOGS_OUTILS_EXECUTES}}", Cle::AdmLogsOutilsExecutes),
                ("{{T_ED_CHOIX_SUITE}}", Cle::AdmEdChoixSuite),
                ("{{T_P2P_TOR_ACTIF}}", Cle::AdmP2pTorActif),
                ("{{T_P2P_SYNC_BOOTSTRAP}}", Cle::AdmP2pSyncBootstrap),
                ("{{T_P2P_CLE_PUBLIQUE}}", Cle::AdmP2pClePublique),
                ("{{T_P2P_BOOTSTRAP_LABEL}}", Cle::AdmP2pBootstrapLabel),
                ("{{T_P2P_CHUNK_LABEL}}", Cle::AdmP2pChunkLabel),
                ("{{T_P2P_NOEUDS}}", Cle::AdmP2pNoeuds),
                ("{{T_P2P_UTILISATEURS_P2P}}", Cle::AdmP2pUtilisateursP2p),
                ("{{T_P2P_TRANSFERTS}}", Cle::AdmP2pTransferts),
                ("{{T_P2P_ANNUAIRE}}", Cle::AdmP2pAnnuaire),
                ("{{T_P2P_NOEUDS_CONNUS}}", Cle::AdmP2pNoeudsConnus),
                ("{{T_P2P_UTILISATEURS_P2P_CONNUS}}", Cle::AdmP2pUtilisateursP2pConnus),
                ("{{T_P2P_COPIER}}", Cle::AdmP2pCopier),
            ]);
            s.replacen("{{I18N_JS}}", &i18n::objet_js(&lang, &ADMIN_I18N_JS_KEYS), 1)
        }
        Err(e) => {
            eprintln!("[admin] Impossible de lire {} : {}", HTML_PATH, e);
            format!("<h1>Erreur</h1><p>Fichier introuvable : {}</p>", HTML_PATH)
        }
    };

    let _ = request.respond(Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
    ));
}

fn handle_api(
    mut request: Request,
    pool: &DbPool,
    config_path: &str,
    query: &HashMap<String, String>,
    method: &str,
    user_id: i64,
    privilege: i64,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    langue: &str,
) {
    let full_url = request.url().to_string();
    let path = full_url.split('?').next().unwrap_or(&full_url).to_string();
    let sub = path.trim_start_matches("/api/admin");

    let body = if method == "POST" {
        read_body(&mut request)
    } else {
        HashMap::new()
    };

    let superadmin_routes = [
        "/db/cell/edit",
        "/db/row/delete",
        "/db/row/add",
        "/db/sql",
        "/p2p",
        "/p2p/peers",
        "/p2p/kick",
        "/p2p/users",
        "/p2p/transfers",
        "/p2p/annuaire",
        "/p2p/sync_now",
        "/onlyoffice/start",
        "/onlyoffice/stop",
        // Uploader un .rs revient a executer du code sur le serveur
        // apres recompilation : reserve aux superadmins.
        "/extensions/upload",
        "/extensions/delete",
        "/extensions/rebuild",
        "/extensions/update",
        "/editor/start",
        "/editor/stop",
        // Installer depuis GitHub = executer du code distant.
        "/marketplace/install",
        "/marketplace/source",
        // Visualiser les actions admin declenchees par VexIA (toutes,
        // pas seulement les siennes) est aussi sensible que les executer.
        "/vexia/audit",
    ];
    let needs_super = sub.starts_with("/p2p")
        || sub.starts_with("/backup")
        || superadmin_routes.iter().any(|r| sub.starts_with(r));
    if needs_super && privilege > PRIVILEGE_SUPER {
        return respond_json(
            request,
            json!({"success":false,"error":i18n::t(langue, Cle::AdmP2pErrReserveSuperadmins)}),
        );
    }

    let resp: Value = match sub {
        s if s.starts_with("/p2p") => {
            let cfg = load_config(config_path);
            let vex_url = cfg
                .extra
                .get("server")
                .and_then(|s| s.get("public_url"))
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080")
                .to_string();
            let p2p_cfg = P2pConfig::from_vex_config(&cfg);
            let node_state = Arc::new(RwLock::new(NodeState::init(&vex_url, p2p_cfg.clone())));
            p2p_admin_handle_api(pool, s, &body, method, &node_state)
        }

        "/dashboard" => {
            let vex_cfg = read_config(config_path);
            let sess_min = vex_cfg["users"]["session_expiration_minutes"]
                .as_u64()
                .unwrap_or(60) as u32;
            // Etat de l'editeur actif (quel que soit le provider choisi)
            let (ed_id, ed_p, ed_ok, ed_ms, _ed_detail) = editor_actif_complet(config_path);
            let ed_nom = ed_p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(ed_id.as_str())
                .to_string();
            // ── Filtrer le fondateur (privilege=1) des derniers inscrits ──
            let last_raw = selectionner(
                pool,
                "login",
                &[],
                &["id", "nom", "email", "privilege", "vip"],
                Some("id DESC"),
                Some(20),
            );
            let last: Vec<_> = last_raw.into_iter()
                .filter(|u| u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99) != 1)
                .take(5)
                .collect();
            json!({ "success": true, "data": {
                "nb_users":    compter_lignes(pool, "login",    &[]),
                "nb_fichiers": compter_lignes(pool, "fichiers", &[]),
                "nb_sessions": compter_sessions_actives(pool, sess_min),
                "nb_pages":    compter_lignes(pool, "sitec",    &[]),
                "db_size_mb":  get_taille_db(pool),
                // "onlyoffice" conserve pour compatibilite ascendante
                "onlyoffice":  { "online": ed_ok, "ms": ed_ms },
                "editor":      { "online": ed_ok, "ms": ed_ms, "id": ed_id, "name": ed_nom },
                "is_super":    privilege <= PRIVILEGE_SUPER,
                "last_users":  last.iter().map(|u| json!({
                    "id":        u.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    "nom":       u.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
                    "email":     u.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                    "privilege": u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(0),
                    // Stocke en base "0"/"1" (voir /users/vip) ; le front attend
                    // l'identifiant de plan "vip" pour presélectionner l'option.
                    "vip":       u.get("vip").and_then(|v| v.as_str()).filter(|s| !s.is_empty() && *s != "0").map(|_| "vip").unwrap_or(""),
                })).collect::<Vec<_>>(),
            }})
        }

        "/users" => {
            let users = selectionner(
                pool,
                "login",
                &[],
                &["id", "nom", "email", "privilege", "vip"],
                Some("privilege ASC, nom ASC"),
                None,
            );
            // ── Filtrer les fondateurs (privilege=1), sauf l'appelant lui-meme :
            // un fondateur doit pouvoir se voir dans sa propre liste d'utilisateurs.
            json!({ "success": true, "data": users.iter()
                .filter(|u| {
                    let priv_u = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
                    let id_u = u.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    priv_u != 1 || id_u == user_id
                })
                .map(|u| json!({
                "id":        u.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "nom":       u.get("nom").and_then(|v| v.as_str()).unwrap_or(""),
                "email":     u.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                "privilege": u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(0),
                "vip":       u.get("vip").and_then(|v| v.as_str()).filter(|s| !s.is_empty() && *s != "0").map(|_| "vip").unwrap_or(""),
            })).collect::<Vec<_>>() })
        }

        "/users/privilege" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let priv_val = body
                .get("privilege")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);

            match crate::admin::actions::set_user_privilege(pool, user_id, privilege, tid, priv_val, langue) {
                Ok(v) => v,
                Err(e) => return respond_json(request, json!({"success":false,"error":e})),
            }
        }

        "/users/vip" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            // La colonne `vip` (VARCHAR) n'est lue qu'en booleen partout
            // ailleurs (i64 : 0=free, non-zero=vip) -- voir access_control::
            // plan_autorise. Le formulaire admin envoie l'identifiant du plan
            // choisi (ex: "vip"), pas un entier : on normalise ici pour rester
            // compatible avec tout le code qui lit `vip` comme un i64.
            let vip_brut = body.get("vip").map(|v| v.as_str()).unwrap_or("0");
            let vip = if vip_brut.is_empty() || vip_brut == "0" { "0" } else { "1" };
            inserer_ou_modifier(
                pool,
                "login",
                &[("vip", mysql::Value::from(vip))],
                &[("id", mysql::Value::from(tid))],
            );
            json!({"success":true,"message":i18n::t(langue, Cle::AdmUsersMsgVipModifie)})
        }

        "/users/delete" => {
            let tid = body
                .get("uid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            match crate::admin::actions::delete_user(pool, user_id, privilege, tid, langue) {
                Ok(v) => v,
                Err(e) => return respond_json(request, json!({"success":false,"error":e})),
            }
        }

        "/blocks" => {
            let blocks = selectionner(
                pool,
                "bloqpage",
                &[],
                &["id", "iduserb", "priviautro", "iduserquiab", "pageb"],
                Some("id DESC"),
                None,
            );
            json!({ "success": true, "data": blocks.iter().map(|b| json!({
                "id":          b.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "iduserb":     b.get("iduserb").and_then(|v| v.as_str()).unwrap_or("all"),
                "priviautro":  b.get("priviautro").and_then(|v| v.as_i64()).unwrap_or(0),
                "iduserquiab": b.get("iduserquiab").and_then(|v| v.as_i64()).unwrap_or(0),
                "pageb":       b.get("pageb").and_then(|v| v.as_str()).unwrap_or(""),
            })).collect::<Vec<_>>() })
        }

        "/blocks/add" => {
            let iduserb = body.get("iduserb").cloned().unwrap_or_else(|| "all".into());
            let priviautro = body
                .get("priviautro")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(10);
            let pageb = body.get("pageb").cloned().unwrap_or_default();
            if pageb.is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(langue, Cle::AdmErreurSelectionnerPage)}),
                );
            }
            inserer_ou_modifier(
                pool,
                "bloqpage",
                &[
                    ("iduserb", mysql::Value::from(iduserb.as_str())),
                    ("priviautro", mysql::Value::from(priviautro)),
                    ("iduserquiab", mysql::Value::from(user_id)),
                    ("pageb", mysql::Value::from(pageb.as_str())),
                ],
                &[],
            );
            json!({"success":true,"message":i18n::t(langue, Cle::AdmBlocksMsgAjoute)})
        }

        "/blocks/delete" => {
            let bid = body
                .get("bid")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            supprimer_ligne(pool, "bloqpage", "id", mysql::Value::from(bid));
            json!({"success":true,"message":i18n::t(langue, Cle::AdmBlocksMsgSupprime)})
        }

        "/pages" => {
            json!({"success":true,"data": vex_pages().iter().map(|(p,l)| json!([p,l])).collect::<Vec<_>>()})
        }

        "/db/tables" => {
            json!({"success":true,"data":{"tables":lister_tables(pool),"sizes":get_tailles_tables(pool)}})
        }

        s if s.starts_with("/db/table/") && s.ends_with("/schema") => {
            let table_raw = s
                .trim_start_matches("/db/table/")
                .trim_end_matches("/schema");
            let table = url_decode(table_raw);
            let schema = decrire_table(pool, &table);
            if schema.is_empty() {
                json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrTableIntrouvable)})
            } else {
                json!({"success":true,"data":{"table":table,"schema":schema}})
            }
        }

        s if s.starts_with("/db/table/") && !s.contains("/cell") && !s.contains("/row") => {
            let table_raw = s.trim_start_matches("/db/table/");
            let table = url_decode(table_raw);
            let page = query
                .get("page")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1)
                .max(1);
            let per_page = 30u64;
            let total = compter_lignes(pool, &table, &[]);
            let offset = (page - 1) * per_page;
            let tri_col = query.get("sort_col").cloned();
            let tri_dir = query.get("sort_dir").cloned().unwrap_or_else(|| "asc".into());
            let tri = tri_col.as_deref().map(|c| (c, tri_dir.as_str()));

            match lire_lignes_table(pool, &table, per_page, offset, tri) {
                Some(data) => json!({"success":true,"data":{
                    "table": table, "total": total,
                    "page": page,
                    "total_pages": if total == 0 { 1u64 } else { ((total as f64) / (per_page as f64)).ceil() as u64 },
                    "cols": data["cols"], "rows": data["rows"],
                    "is_super": privilege <= PRIVILEGE_SUPER,
                }}),
                None => json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrTableIntrouvable)}),
            }
        }

        "/db/cell/edit" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let col = body.get("col").cloned().unwrap_or_default();
            let val = body.get("val").cloned().unwrap_or_default();
            let pk_col = body.get("pk_col").cloned().unwrap_or_else(|| "id".into());
            let pk_val = body.get("pk_val").cloned().unwrap_or_default();

            if table == "login" && col == "privilege" {
                let new_priv = val.parse::<i64>().unwrap_or(99);
                if new_priv < PRIVILEGE_MIN_SET {
                    return respond_json(
                        request,
                        json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrPrivilege1Interdit)}),
                    );
                }
            }

            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrTableInconnue)}));
            }
            inserer_ou_modifier(
                pool,
                &table,
                &[(&col, mysql::Value::from(val.as_str()))],
                &[(&pk_col, mysql::Value::from(pk_val.as_str()))],
            );
            json!({"success":true,"message":i18n::t(langue, Cle::AdmDbMsgCelluleModifiee)})
        }

        "/db/row/delete" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let pk_col = body.get("pk_col").cloned().unwrap_or_else(|| "id".into());
            let pk_val = body.get("pk_val").cloned().unwrap_or_default();

            if table == "login" && pk_col == "id" {
                let target = selectionner(
                    pool,
                    "login",
                    &[("id", mysql::Value::from(pk_val.as_str()))],
                    &["privilege"],
                    None,
                    Some(1),
                );
                let tp = target
                    .first()
                    .and_then(|r| r.get("privilege"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(99);
                if tp <= PRIVILEGE_SUPER {
                    return respond_json(
                        request,
                        json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrImpossibleSupprSuperadmin)}),
                    );
                }
            }

            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrTableInconnue)}));
            }
            supprimer_ligne(pool, &table, &pk_col, mysql::Value::from(pk_val.as_str()));
            json!({"success":true,"message":i18n::t(langue, Cle::AdmDbMsgLigneSupprimee)})
        }

        "/db/row/add" => {
            let table = body.get("table").cloned().unwrap_or_default();
            let tables_ok = lister_tables(pool);
            if !tables_ok.contains(&table) {
                return respond_json(request, json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrTableInconnue)}));
            }
            let cols_raw = body.get("cols").cloned().unwrap_or_default();
            let vals_raw = body.get("vals").cloned().unwrap_or_default();
            let cols: Vec<String> = serde_json::from_str(&cols_raw).unwrap_or_default();
            let vals: Vec<String> = serde_json::from_str(&vals_raw).unwrap_or_default();
            if cols.is_empty() || cols.len() != vals.len() {
                return respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrColonnesValeursInvalides)}),
                );
            }
            if table == "login" {
                for (c, v) in cols.iter().zip(vals.iter()) {
                    if c == "privilege" {
                        if v.parse::<i64>().unwrap_or(99) < PRIVILEGE_MIN_SET {
                            return respond_json(
                                request,
                                json!({"success":false,"error":i18n::t(langue, Cle::AdmDbErrColonnesValeursInvalidesJs)}),
                            );
                        }
                    }
                }
            }
            let pairs: Vec<(&str, mysql::Value)> = cols
                .iter()
                .zip(vals.iter())
                .map(|(c, v)| (c.as_str(), mysql::Value::from(v.as_str())))
                .collect();
            let new_id = inserer_ou_modifier(pool, &table, &pairs, &[]);
            json!({"success":true,"message":i18n::t(langue, Cle::AdmDbMsgLigneAjoutee),"id":new_id})
        }

        "/db/sql" => {
            let raw_sql = body.get("sql").cloned().unwrap_or_default();
            executer_sql_admin(pool, cookie_val, remote_ip, user_agent, &raw_sql)
        }

        "/p2p/peers" => {
            let peers = selectionner(
                pool,
                "p2p_peers",
                &[],
                &["id", "node_id", "ip", "port", "last_seen", "status"],
                Some("last_seen DESC"),
                None,
            );
            let total = compter_lignes(pool, "p2p_peers", &[]);
            let online = compter_lignes(
                pool,
                "p2p_peers",
                &[("status", mysql::Value::from("online"))],
            );
            json!({"success":true,"data":{
                "peers": peers.iter().map(|p| json!({
                    "id":        p.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    "node_id":   p.get("node_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "ip":        p.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                    "port":      p.get("port").and_then(|v| v.as_i64()).unwrap_or(0),
                    "last_seen": p.get("last_seen").and_then(|v| v.as_str()).unwrap_or(""),
                    "status":    p.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                })).collect::<Vec<_>>(),
                "total":  total,
                "online": online,
            }})
        }

        "/p2p/kick" => {
            let peer_id = body
                .get("peer_id")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            supprimer_ligne(pool, "p2p_peers", "id", mysql::Value::from(peer_id));
            json!({"success":true,"message":i18n::t(langue, Cle::AdmP2pMsgPeerSupprime)})
        }

        "/config" => {
            if method == "POST" {
                let new_cfg_str = body.get("config").cloned().unwrap_or_default();
                match serde_json::from_str::<Value>(&new_cfg_str) {
                    Ok(v) => {
                        let _ = std::fs::write(
                            config_path,
                            serde_json::to_string_pretty(&v).unwrap_or_default(),
                        );
                        json!({"success":true,"message":i18n::t(langue, Cle::AdmConfigSauvegardee)})
                    }
                    Err(e) => json!({"success":false,"error":i18n::t(langue, Cle::AdmConfigJsonInvalide).replace("{e}", &e.to_string())}),
                }
            } else {
                json!({"success":true,"data": read_config(config_path)})
            }
        }

        "/server" => {
            let (free, total, used, pct) = disk_info();
            json!({"success":true,"data":{
                "vex_version": env!("CARGO_PKG_VERSION"),
                "os":          std::env::consts::OS,
                "arch":        std::env::consts::ARCH,
                "uptime_sec":  uptime_sec(),
                "disk": {"free_gb":free,"total_gb":total,"used_gb":used,"used_pct":pct},
                "disks": disks_info(),
            }})
        }

        // Verification en lecture seule (aucune commande d'application/mise
        // a jour n'est exposee ici -- volontairement, l'admin applique lui-meme
        // via SSH une fois informe de ce qui est disponible).
        "/server/updates" => {
            json!({"success":true,"data":verifier_maj_systeme()})
        }

        // ══════════════════════════════════════════════════════════
        // SAUVEGARDES — dump complet de la base (fichiers inclus, ils
        // sont stockes en base64 dans la table `fichiers`) via
        // mysqldump | gzip. Reserve aux superadmins (voir
        // needs_super plus haut) : un dump contient tout, mots de
        // passe hashes/verifiers SRP compris.
        // ══════════════════════════════════════════════════════════
        "/backup/list" => {
            json!({"success":true,"data":lister_backups()})
        }

        "/backup/status" => {
            json!({"success":true,"data":lire_backup_status()})
        }

        "/backup/create" => {
            let db = match crate::config_loader::load_db_config(config_path) {
                Ok(d) => d,
                Err(e) => return respond_json(request, json!({"success":false,"error":e})),
            };
            match lancer_backup(&db, langue) {
                Ok(_) => json!({"success":true,"message":i18n::t(langue, Cle::AdmBackupMsgLancee)}),
                Err(e) => json!({"success":false,"error":e}),
            }
        }

        "/backup/delete" => {
            let nom = body.get("nom").cloned().unwrap_or_default();
            if !backup_nom_valide(&nom) {
                json!({"success":false,"error":i18n::t(langue, Cle::AdmBackupErrNomInvalide)})
            } else {
                let _ = std::fs::remove_file(format!("{}/{}", BACKUP_DIR, nom));
                json!({"success":true,"message":i18n::t(langue, Cle::AdmBackupMsgSupprimee)})
            }
        }

        "/backup/download" => {
            let nom = query.get("nom").cloned().unwrap_or_default();
            if !backup_nom_valide(&nom) {
                return respond_json(request, json!({"success":false,"error":i18n::t(langue, Cle::AdmBackupErrNomInvalide)}));
            }
            let chemin = format!("{}/{}", BACKUP_DIR, nom);
            match std::fs::read(&chemin) {
                Ok(octets) => {
                    let reponse = Response::from_data(octets)
                        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/gzip").unwrap())
                        .with_header(tiny_http::Header::from_bytes(
                            "Content-Disposition",
                            format!("attachment; filename=\"{}\"", nom),
                        ).unwrap());
                    let _ = request.respond(reponse);
                    return;
                }
                Err(_) => return respond_json(request, json!({"success":false,"error":i18n::t(langue, Cle::AdmBackupErrFichierIntrouvable)})),
            }
        }

        "/logs" => {
            let (empty, content, files) = read_log_content();
            if empty || content.trim().is_empty() {
                json!({"success":true,"data":{"empty":true,"files":files,"root":"log"}})
            } else {
                let lines: Vec<&str> = content.lines().collect();
                let trimmed = lines
                    .iter()
                    .rev()
                    .take(200)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                json!({"success":true,"data":{"empty":false,"content":trimmed,"files":files,"root":"log"}})
            }
        }

        "/logs/clear" => {
            for path in log_files() {
                let _ = std::fs::write(&path, "");
            }
            json!({"success":true,"message":i18n::t(langue, Cle::AdmLogsMsgVides)})
        }

        "/vexia/audit" => {
            let rows = selectionner(pool, "vexia_audit", &[], &[], Some("id DESC"), Some(200));
            let items: Vec<Value> = rows.iter().map(|r| json!({
                "id":         r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "user_id":    r.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0),
                "tool_name":  r.get("tool_name").and_then(|v| v.as_str()).unwrap_or(""),
                "tier":       r.get("tier").and_then(|v| v.as_str()).unwrap_or(""),
                "args_json":  r.get("args_json").and_then(|v| v.as_str()).unwrap_or(""),
                "success":    r.get("success").and_then(|v| v.as_i64()).unwrap_or(0) == 1,
                "error":      r.get("error").and_then(|v| v.as_str()).unwrap_or(""),
                "created_at": r.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
            })).collect();
            json!({"success":true,"data":items})
        }

        // ── Compatibilite : /onlyoffice* pilote desormais le provider actif ──
        "/onlyoffice" => {
            let cfg = read_config(config_path);
            let providers = editor_providers(&cfg);
            let id = editor_actif(&cfg, &providers);
            let p = providers.get(&id).cloned().unwrap_or(json!({}));
            let (online, ms, detail) = editor_check(&p);
            json!({"success":true,"data":{
                "online":   online,
                "ms":       ms,
                "detail":   detail,
                "url":      p.get("server_url").and_then(|v| v.as_str()).unwrap_or(""),
                "enabled":  p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                "version":  p.get("kind").and_then(|v| v.as_str()).unwrap_or("?"),
                "provider": id,
                "name":     p.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "params":   p.clone(),
                "server":   p,
            }})
        }

        "/onlyoffice/start" | "/onlyoffice/stop" => {
            let demarrage = sub.ends_with("/start");
            let cfg = read_config(config_path);
            let providers = editor_providers(&cfg);
            let id = editor_actif(&cfg, &providers);
            let p = providers.get(&id).cloned().unwrap_or(json!({}));
            if demarrage && !p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                return respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(langue, Cle::AdmEdErrDesactiveConfig)}),
                );
            }
            let cle = if demarrage { "start_cmd" } else { "stop_cmd" };
            let cmd = p.get(cle).and_then(|v| v.as_str()).unwrap_or("");
            if cmd.trim().is_empty() {
                return respond_json(
                    request,
                    json!({"success":false,"error":i18n::t(langue, Cle::AdmEdErrAucuneCommande).replace("{cle}", cle).replace("{id}", &id)}),
                );
            }
            let (cmd_ok, sortie) = run_shell_command(cmd);
            if demarrage {
                let wait_ms = p.get("wait_boot_ms").and_then(|v| v.as_u64()).unwrap_or(8000);
                if cmd_ok {
                    std::thread::sleep(std::time::Duration::from_millis(wait_ms.min(20_000)));
                }
                let (online, ms, _) = editor_check(&p);
                json!({"success":online,"cmd_ok":cmd_ok,"output":sortie,"ping_ms":ms,"online":online})
            } else {
                json!({"success":cmd_ok,"output":sortie})
            }
        }

        // ══════════════════════════════════════════════════════════
        // EXTENSIONS — upload .rs, permissions, compilation
        // ══════════════════════════════════════════════════════════
        "/extensions" => {
            let cfg = read_config(config_path);
            let ep = cfg["extensions"]["extension_params"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            let compilees: Vec<String> = crate::extensions::compiled_ids()
                .iter()
                .map(|s| s.to_string())
                .collect();

            let mut liste = Vec::new();
            for (id, e) in ep.iter() {
                if id.starts_with('_') {
                    continue;
                }
                liste.push(json!({
                    "id": id,
                    "enabled": e.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                    "version": e.get("version").and_then(|v| v.as_str()).unwrap_or("?"),
                    "privilege_min": e.get("privilege_min").and_then(|v| v.as_i64()).unwrap_or(10),
                    "plans_autorises": e.get("plans_autorises").cloned().unwrap_or(json!([])),
                    "params": e.get("params").cloned().unwrap_or(json!({})),
                    "permissions": e.get("permissions").cloned().unwrap_or(json!({})),
                    "nav_app": e.get("nav_app").cloned().unwrap_or(json!(null)),
                    "dashboard_tile": e.get("dashboard_tile").cloned().unwrap_or(json!(null)),
                    "admin_infos": e.get("admin_infos").cloned().unwrap_or(json!(null)),
                    "base_app": EXT_BASE_APPS.contains(&id.as_str()),
                    "compiled": compilees.contains(id),
                    "disque": ext_etat(id),
                }));
            }

            // Sources présentes sur le disque mais absentes de config.json
            let orphelins: Vec<String> = ext_ids_sur_disque()
                .into_iter()
                .filter(|i| !ep.contains_key(i))
                .collect();

            json!({"success": true, "data": {
                "extensions":   liste,
                "orphelins":    orphelins,
                "compiled":     compilees,
                "build_cmd":    ext_build_cmd(&cfg),
                "build_status": lire_build_status(),
                "src_root":     EXT_SRC_ROOT,
                "static_root":  EXT_STATIC_ROOT,
                "max_bytes":    EXT_MAX_BYTES,
                "is_super":     privilege <= PRIVILEGE_SUPER,
                "plans":        cfg["plans"]["available_plans"].clone(),
            }})
        }

        "/extensions/template" => {
            json!({"success": true, "data": {"code": EXT_TEMPLATE, "filename": "mod.rs"}})
        }

        "/extensions/upload" => {
            use base64::Engine as _;

            let id = body
                .get("id")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if !ext_id_valide(&id) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrIdInvalideLettre)}));
            }
            if EXT_BASE_APPS.contains(&id.as_str()) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrAppDeBaseChoisir).replace("{id}", &id)}));
            }

            let filename = body
                .get("filename")
                .cloned()
                .unwrap_or_else(|| "mod.rs".into());
            if !filename.to_lowercase().ends_with(".rs") {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrFichierRs)}));
            }

            let b64 = body.get("code_b64").cloned().unwrap_or_default();
            let brut = match base64::engine::general_purpose::STANDARD.decode(b64.as_bytes()) {
                Ok(b) => b,
                Err(_) => {
                    return respond_json(
                        request,
                        json!({"success": false, "error": i18n::t(langue, Cle::AdmExtErrContenuInvalide)}),
                    )
                }
            };
            if brut.len() > EXT_MAX_BYTES {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrTropVolumineux)
                        .replace("{ko}", &(brut.len() / 1024).to_string())
                        .replace("{maxko}", &(EXT_MAX_BYTES / 1024).to_string())}));
            }
            let code = match String::from_utf8(brut) {
                Ok(s) => s,
                Err(_) => {
                    return respond_json(request, json!({"success": false, "error":
                        i18n::t(langue, Cle::AdmExtErrPasUtf8)}))
                }
            };
            if code.trim().is_empty() {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmExtErrFichierVide)}),
                );
            }
            if !code.contains("pub fn handle") {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrDoitExposerHandle)}));
            }

            // ── Analyse des motifs sensibles ──────────────────────
            let risques = ext_scan_risques(&code, langue);
            let confirme = body
                .get("confirm_risques")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);
            if !risques.is_empty() && !confirme {
                return respond_json(request, json!({
                    "success": false,
                    "need_confirm": true,
                    "risques": risques,
                    "error": i18n::t(langue, Cle::AdmExtErrMotifsDetectes).replace("{n}", &risques.len().to_string()),
                }));
            }

            // ── Écriture des fichiers ─────────────────────────────
            let dossier_src = std::path::Path::new(EXT_SRC_ROOT).join(&id);
            if let Err(e) = std::fs::create_dir_all(&dossier_src) {
                return respond_json(request, json!({"success": false, "error":
                    format!("Création de {:?} : {}", dossier_src, e)}));
            }
            let chemin_src = dossier_src.join("mod.rs");
            if let Err(e) = std::fs::write(&chemin_src, &code) {
                return respond_json(request, json!({"success": false, "error":
                    format!("Écriture de {:?} : {}", chemin_src, e)}));
            }
            let dossier_static = std::path::Path::new(EXT_STATIC_ROOT).join(&id);
            let _ = std::fs::create_dir_all(&dossier_static);

            // ── Enregistrement + permissions dans config.json ─────
            let version = body
                .get("version")
                .cloned()
                .unwrap_or_else(|| "0.1".into())
                .trim()
                .to_string();
            let priv_min = body
                .get("privilege_min")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(10)
                .clamp(1, 12);
            let plans: Vec<String> = body
                .get("plans_autorises")
                .map(|s| {
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let active = body
                .get("enabled")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(true);

            let mut cfg = read_config(config_path);
            if !cfg["extensions"].is_object() {
                cfg["extensions"] = json!({"enabled": true, "extension_params": {}});
            }
            if !cfg["extensions"]["extension_params"].is_object() {
                cfg["extensions"]["extension_params"] = json!({});
            }
            let params_existants = cfg["extensions"]["extension_params"]
                .get(id.as_str())
                .and_then(|e| e.get("params"))
                .cloned()
                .unwrap_or(json!({}));
            let perms_existantes = cfg["extensions"]["extension_params"]
                .get(id.as_str())
                .and_then(|e| e.get("permissions"))
                .cloned()
                .unwrap_or(json!({}));
            let deja_present = cfg["extensions"]["extension_params"]
                .get(id.as_str())
                .is_some();

            cfg["extensions"]["extension_params"][id.as_str()] = json!({
                "enabled":         active,
                "version":         if version.is_empty() { "0.1".to_string() } else { version },
                "privilege_min":   priv_min,
                "plans_autorises": plans,
                "params":          params_existants,
                // Autorisations internes a l'extension (action -> regle)
                "permissions":     perms_existantes,
            });
            if let Err(e) = ecrire_config(config_path, &cfg) {
                return respond_json(request, json!({"success": false, "error": e}));
            }

            // ── Registre compilable ───────────────────────────────
            let ids = match ext_regenerer_registre() {
                Ok(v) => v,
                Err(e) => {
                    return respond_json(request, json!({"success": false, "error":
                        i18n::t(langue, Cle::AdmExtErrRegistreExtensions).replace("{e}", &e)}))
                }
            };

            // Compilation lancee automatiquement : l'admin n'a rien a faire.
            // En attendant qu'elle aboutisse, /ext/<id> sert deja le dossier
            // static/extensions/<id>/ (repli dans access_control.rs).
            let build_lance = ext_lancer_build(&ext_build_cmd(&cfg)).is_ok();

            json!({"success": true,
                "message": format!("« {} » {}{}",
                                   id,
                                   if deja_present { i18n::t(langue, Cle::AdmExtMsgInstalleeMiseAJour) } else { i18n::t(langue, Cle::AdmExtMsgInstalleeInstallee) },
                                   if build_lance { i18n::t(langue, Cle::AdmExtMsgCompilationLanceeAuto) }
                                   else { i18n::t(langue, Cle::AdmExtMsgCompilationDejaEnCours) }),
                "data": {
                    "id":            id,
                    "src_path":      chemin_src.to_string_lossy(),
                    "static_path":   dossier_static.to_string_lossy(),
                    "risques":       risques,
                    "registre_ids":  ids,
                    "build_lance":   build_lance,
                }})
        }

        "/extensions/permissions" => {
            let id = body.get("id").cloned().unwrap_or_default();
            let mut cfg = read_config(config_path);
            if !cfg["extensions"]["extension_params"]
                .get(id.as_str())
                .is_some()
            {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmExtErrExtensionInconnue)}),
                );
            }
            if let Some(v) = body.get("privilege_min").and_then(|v| v.parse::<i64>().ok()) {
                cfg["extensions"]["extension_params"][id.as_str()]["privilege_min"] =
                    json!(v.clamp(1, 12));
            }
            if let Some(v) = body.get("plans_autorises") {
                let plans: Vec<String> = v
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                cfg["extensions"]["extension_params"][id.as_str()]["plans_autorises"] =
                    json!(plans);
            }
            if let Some(v) = body.get("enabled") {
                cfg["extensions"]["extension_params"][id.as_str()]["enabled"] =
                    json!(v == "1" || v == "true");
            }
            if let Some(v) = body.get("version") {
                if !v.trim().is_empty() {
                    cfg["extensions"]["extension_params"][id.as_str()]["version"] =
                        json!(v.trim());
                }
            }
            // Integration interface : app du menu, tuile du dashboard,
            // lignes injectees dans la tuile Administration.
            for cle in ["nav_app", "dashboard_tile", "admin_infos"] {
                if let Some(v) = body.get(cle) {
                    if v.trim().is_empty() {
                        if let Some(o) = cfg["extensions"]["extension_params"][id.as_str()]
                            .as_object_mut()
                        {
                            o.remove(cle);
                        }
                        continue;
                    }
                    match serde_json::from_str::<Value>(v) {
                        Ok(j) => {
                            cfg["extensions"]["extension_params"][id.as_str()][cle] = j
                        }
                        Err(e) => {
                            return respond_json(request, json!({"success": false, "error":
                                i18n::t(langue, Cle::AdmExtJsonInvalideLib).replace("{lib}", cle).replace("{msg}", &e.to_string())}))
                        }
                    }
                }
            }
            if let Some(v) = body.get("permissions") {
                match serde_json::from_str::<Value>(v) {
                    Ok(pm) if pm.is_object() => {
                        cfg["extensions"]["extension_params"][id.as_str()]["permissions"] = pm
                    }
                    Ok(_) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmExtErrAutorisationsObjet)}))
                    }
                    Err(e) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmExtErrAutorisationsInvalides).replace("{e}", &e.to_string())}))
                    }
                }
            }
            if let Some(v) = body.get("params") {
                match serde_json::from_str::<Value>(v) {
                    Ok(p) if p.is_object() => {
                        cfg["extensions"]["extension_params"][id.as_str()]["params"] = p
                    }
                    Ok(_) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmExtErrParametresObjet)}))
                    }
                    Err(e) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmExtErrParametresInvalides2).replace("{e}", &e.to_string())}))
                    }
                }
            }
            match ecrire_config(config_path, &cfg) {
                Ok(_) => json!({"success": true, "message": i18n::t(langue, Cle::AdmExtMsgPermissionsEnregistrees)}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/extensions/delete" => {
            let id = body.get("id").cloned().unwrap_or_default();
            if EXT_BASE_APPS.contains(&id.as_str()) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmExtErrImpossibleSupprBase)}));
            }
            if !ext_id_valide(&id) {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmExtErrIdInvalide)}),
                );
            }
            let purge = body
                .get("purge")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

            let mut cfg = read_config(config_path);
            if let Some(o) = cfg["extensions"]["extension_params"].as_object_mut() {
                o.remove(&id);
            }
            if let Err(e) = ecrire_config(config_path, &cfg) {
                return respond_json(request, json!({"success": false, "error": e}));
            }

            let mut supprimes = Vec::new();
            if purge {
                let src = std::path::Path::new(EXT_SRC_ROOT).join(&id);
                let stat = std::path::Path::new(EXT_STATIC_ROOT).join(&id);
                if src.is_dir() && std::fs::remove_dir_all(&src).is_ok() {
                    supprimes.push(src.to_string_lossy().to_string());
                }
                if stat.is_dir() && std::fs::remove_dir_all(&stat).is_ok() {
                    supprimes.push(stat.to_string_lossy().to_string());
                }
            }
            let ids = ext_regenerer_registre().unwrap_or_default();
            json!({"success": true,
                "message": if purge {
                    i18n::t(langue, Cle::AdmExtMsgSupprimeeConfigFichiers).replace("{id}", &id)
                } else {
                    i18n::t(langue, Cle::AdmExtMsgRetireeConfig).replace("{id}", &id)
                },
                "data": {"supprimes": supprimes, "registre_ids": ids, "needs_rebuild": purge}})
        }

        "/extensions/rebuild" => {
            let cfg = read_config(config_path);
            let cmd = ext_build_cmd(&cfg);
            match ext_lancer_build(&cmd) {
                Ok(_) => json!({"success": true, "message":
                    i18n::t(langue, Cle::AdmExtMsgCompilationLanceeRedemarrez),
                    "data": {"cmd": cmd}}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/extensions/rebuild/status" => {
            json!({"success": true, "data": lire_build_status()})
        }

        // Tire le depot git (origin/main) puis relance la compilation --
        // meme mecanisme de suivi que /extensions/rebuild, juste une
        // commande differente (git pull d'abord).
        "/extensions/update" => {
            let cfg = read_config(config_path);
            let build_cmd = ext_build_cmd(&cfg);
            let cmd = format!("git pull origin main 2>&1 && {}", build_cmd);
            match ext_lancer_build(&cmd) {
                Ok(_) => json!({"success": true, "message":
                    i18n::t(langue, Cle::AdmExtMsgMajGithubCompilation),
                    "data": {"cmd": cmd}}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        // ══════════════════════════════════════════════════════════
        // ÉDITEUR EN LIGNE — choix de la suite bureautique
        // ══════════════════════════════════════════════════════════
        "/editor" => {
            let mut cfg = read_config(config_path);
            let providers = editor_providers(&cfg);

            // Première visite : on matérialise la section dans config.json
            if !cfg["editor"]["providers"].is_object() {
                if !cfg["editor"].is_object() {
                    cfg["editor"] = json!({});
                }
                cfg["editor"]["providers"] = Value::Object(providers.clone());
                if !cfg["editor"]["provider"].is_string() {
                    cfg["editor"]["provider"] = json!("onlyoffice");
                }
                let _ = ecrire_config(config_path, &cfg);
            }

            let actif = editor_actif(&cfg, &providers);
            let p = providers.get(&actif).cloned().unwrap_or(json!({}));
            let (online, ms, detail) = editor_check(&p);

            json!({"success": true, "data": {
                "active":    actif,
                "providers": Value::Object(providers),
                "presets":   editor_presets(),
                "status":    {"online": online, "ms": ms, "detail": detail},
                "online_editing_enabled": cfg["editor"]["online_editing_enabled"].as_bool().unwrap_or(true),
                "is_super":  privilege <= PRIVILEGE_SUPER,
            }})
        }

        "/editor/select" => {
            let id = body.get("id").cloned().unwrap_or_default();
            let mut cfg = read_config(config_path);
            let providers = editor_providers(&cfg);
            if !providers.contains_key(&id) {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmEdErrProviderInconnu)}),
                );
            }
            if !cfg["editor"].is_object() {
                cfg["editor"] = json!({});
            }
            cfg["editor"]["providers"] = Value::Object(providers.clone());
            cfg["editor"]["provider"] = json!(id);
            // On garde editor.supported_formats aligné sur le provider actif
            if let Some(f) = providers[&id].get("supported_formats") {
                cfg["editor"]["supported_formats"] = f.clone();
            }
            if let Some(c) = providers[&id].get("collaborative_editing") {
                cfg["editor"]["collaborative_editing"] = c.clone();
            }
            if let Some(v) = body.get("online_editing_enabled") {
                cfg["editor"]["online_editing_enabled"] = json!(v == "1" || v == "true");
            }
            match ecrire_config(config_path, &cfg) {
                Ok(_) => json!({"success": true,
                    "message": i18n::t(langue, Cle::AdmEdMsgEditeurActif).replace("{name}",
                        providers[&id].get("name").and_then(|v| v.as_str()).unwrap_or(&id))}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/editor/add" => {
            let id = body
                .get("id")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if !ext_id_valide(&id) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrIdInvalide2a32)}));
            }
            let kind = body
                .get("kind")
                .cloned()
                .unwrap_or_else(|| "custom".into());
            let mut cfg = read_config(config_path);
            let mut providers = editor_providers(&cfg);
            if providers.contains_key(&id) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrProviderExisteDeja)}));
            }
            let mut p = editor_defauts(&kind);
            if let Some(nom) = body.get("name") {
                if !nom.trim().is_empty() {
                    p["name"] = json!(nom.trim());
                }
            }
            if let Some(url) = body.get("server_url") {
                if !url.trim().is_empty() {
                    p["server_url"] = json!(url.trim());
                }
            }
            providers.insert(id.clone(), p);
            if !cfg["editor"].is_object() {
                cfg["editor"] = json!({});
            }
            cfg["editor"]["providers"] = Value::Object(providers);
            match ecrire_config(config_path, &cfg) {
                Ok(_) => json!({"success": true,
                    "message": i18n::t(langue, Cle::AdmEdMsgProviderAjoute).replace("{id}", &id), "data": {"id": id}}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/editor/save" => {
            let id = body.get("id").cloned().unwrap_or_default();
            let brut = body.get("provider").cloned().unwrap_or_default();
            let recu: Value = match serde_json::from_str(&brut) {
                Ok(v) => v,
                Err(e) => {
                    return respond_json(request, json!({"success": false, "error":
                        i18n::t(langue, Cle::AdmEdErrJsonProviderInvalide).replace("{e}", &e.to_string())}))
                }
            };
            if !recu.is_object() {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrProviderDoitEtreObjet)}));
            }
            let mut cfg = read_config(config_path);
            let mut providers = editor_providers(&cfg);
            if !providers.contains_key(&id) {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmEdErrProviderInconnu)}),
                );
            }
            let mut fusionne = providers[&id].clone();
            if let (Some(dst), Some(src)) = (fusionne.as_object_mut(), recu.as_object()) {
                for (k, v) in src {
                    dst.insert(k.clone(), v.clone());
                }
            }
            let fusionne = editor_normaliser(&fusionne);
            providers.insert(id.clone(), fusionne.clone());
            if !cfg["editor"].is_object() {
                cfg["editor"] = json!({});
            }
            cfg["editor"]["providers"] = Value::Object(providers.clone());
            // Le provider actif pilote aussi les champs globaux editor.*
            if editor_actif(&cfg, &providers) == id {
                if let Some(f) = fusionne.get("supported_formats") {
                    cfg["editor"]["supported_formats"] = f.clone();
                }
                if let Some(c) = fusionne.get("collaborative_editing") {
                    cfg["editor"]["collaborative_editing"] = c.clone();
                }
            }
            match ecrire_config(config_path, &cfg) {
                Ok(_) => {
                    let (online, ms, detail) = editor_check(&fusionne);
                    json!({"success": true, "message": i18n::t(langue, Cle::AdmEdMsgParametresEnregistres),
                        "data": {"status": {"online": online, "ms": ms, "detail": detail}}})
                }
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/editor/delete" => {
            let id = body.get("id").cloned().unwrap_or_default();
            let mut cfg = read_config(config_path);
            let mut providers = editor_providers(&cfg);
            if !providers.contains_key(&id) {
                return respond_json(
                    request,
                    json!({"success": false, "error": i18n::t(langue, Cle::AdmEdErrProviderInconnu)}),
                );
            }
            if providers.len() <= 1 {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrImpossibleSupprDernier)}));
            }
            providers.remove(&id);
            if !cfg["editor"].is_object() {
                cfg["editor"] = json!({});
            }
            cfg["editor"]["providers"] = Value::Object(providers.clone());
            let actif = editor_actif(&cfg, &providers);
            cfg["editor"]["provider"] = json!(actif);
            match ecrire_config(config_path, &cfg) {
                Ok(_) => json!({"success": true,
                    "message": i18n::t(langue, Cle::AdmEdMsgProviderSupprime).replace("{id}", &id),
                    "data": {"active": actif}}),
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/editor/test" => {
            // Soit un provider enregistré (id), soit une URL saisie à la volée.
            let url = body.get("server_url").cloned().unwrap_or_default();
            let (online, ms, detail) = if url.trim().is_empty() {
                let cfg = read_config(config_path);
                let providers = editor_providers(&cfg);
                let id = body
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| editor_actif(&cfg, &providers));
                match providers.get(&id) {
                    Some(p) => editor_check(p),
                    None => {
                        return respond_json(
                            request,
                            json!({"success": false, "error": i18n::t(langue, Cle::AdmEdErrProviderInconnu)}),
                        )
                    }
                }
            } else {
                let path = body
                    .get("healthcheck_path")
                    .cloned()
                    .unwrap_or_else(|| "/".into());
                let expect = body.get("healthcheck_expect").cloned().unwrap_or_default();
                http_check(&url, &path, &expect)
            };
            json!({"success": true, "data": {"online": online, "ms": ms, "detail": detail}})
        }

        "/editor/start" | "/editor/stop" => {
            let demarrage = sub.ends_with("/start");
            let cfg = read_config(config_path);
            let providers = editor_providers(&cfg);
            let id = body
                .get("id")
                .cloned()
                .unwrap_or_else(|| editor_actif(&cfg, &providers));
            let p = match providers.get(&id) {
                Some(p) => p.clone(),
                None => {
                    return respond_json(
                        request,
                        json!({"success": false, "error": i18n::t(langue, Cle::AdmEdErrProviderInconnu)}),
                    )
                }
            };
            if demarrage && !p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrProviderDesactive)}));
            }
            let cle = if demarrage { "start_cmd" } else { "stop_cmd" };
            let cmd = p.get(cle).and_then(|v| v.as_str()).unwrap_or("");
            if cmd.trim().is_empty() {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmEdErrAucuneCommande).replace("{cle}", cle).replace("{id}", &id)}));
            }
            let (cmd_ok, sortie) = run_shell_command(cmd);
            if demarrage {
                let wait_ms = p
                    .get("wait_boot_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(8000);
                if cmd_ok {
                    std::thread::sleep(std::time::Duration::from_millis(wait_ms.min(20_000)));
                }
                let (online, ms, detail) = editor_check(&p);
                json!({"success": online, "cmd_ok": cmd_ok, "output": sortie,
                    "online": online, "ping_ms": ms, "detail": detail})
            } else {
                json!({"success": cmd_ok, "cmd_ok": cmd_ok, "output": sortie})
            }
        }

        // ══════════════════════════════════════════════════════════
        // PLACE DE MARCHE — recherche et installation depuis GitHub
        // ══════════════════════════════════════════════════════════
        "/marketplace" => {
            let cfg = read_config(config_path);
            let forcer = query.get("refresh").map(|v| v == "1").unwrap_or(false);
            let recherche = query
                .get("q")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            match market_release(&cfg, forcer, langue) {
                Ok(rel) => {
                    let installees: Vec<String> = cfg["extensions"]["extension_params"]
                        .as_object()
                        .map(|o| o.keys().cloned().collect())
                        .unwrap_or_default();
                    let compilees: Vec<String> = crate::extensions::compiled_ids()
                        .iter()
                        .map(|s| s.to_string())
                        .collect();

                    let mut items = Vec::new();
                    if let Some(assets) = rel["assets"].as_array() {
                        for a in assets {
                            let nom = a["name"].as_str().unwrap_or("").to_string();
                            let bas = nom.to_lowercase();
                            // Trois formes acceptees :
                            //   <id>.extension.json  manifeste + fichiers separes
                            //   <id>.zip             archive complete
                            //   <id>.rs              extension d'un seul fichier
                            let manifeste = bas.ends_with(".extension.json");
                            if !(manifeste || bas.ends_with(".rs") || bas.ends_with(".zip")) {
                                continue;
                            }
                            let id = if manifeste {
                                nom.trim_end_matches(".extension.json").to_lowercase()
                            } else {
                                market_id_depuis_nom(&nom)
                            };
                            if !recherche.is_empty()
                                && !bas.contains(&recherche)
                                && !id.contains(&recherche)
                            {
                                continue;
                            }
                            items.push(json!({
                                "nom":        nom,
                                "id":         id,
                                "forme":      if manifeste { "manifeste" }
                                              else if bas.ends_with(".zip") { "archive" }
                                              else { "fichier" },
                                "taille":     a["size"].as_u64().unwrap_or(0),
                                "url":        a["browser_download_url"].as_str().unwrap_or(""),
                                "maj":        a["updated_at"].as_str().unwrap_or(""),
                                "telechargements": a["download_count"].as_u64().unwrap_or(0),
                                "installee":  installees.contains(&id),
                                "compilee":   compilees.contains(&id),
                            }));
                        }
                    }
                    json!({"success": true, "data": {
                        "release":  rel["name"].as_str().or(rel["tag_name"].as_str()).unwrap_or(""),
                        "tag":      rel["tag_name"].as_str().unwrap_or(""),
                        "page":     rel["html_url"].as_str().unwrap_or(""),
                        "publiee":  rel["published_at"].as_str().unwrap_or(""),
                        "notes":    rel["body"].as_str().unwrap_or(""),
                        "items":    items,
                        "source":   cfg["extensions"]["marketplace_url"].as_str().unwrap_or(""),
                        "is_super": privilege <= PRIVILEGE_SUPER,
                    }})
                }
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/marketplace/source" => {
            let url = body.get("url").cloned().unwrap_or_default().trim().to_string();
            if !url.is_empty()
                && !url.starts_with("https://github.com/")
                && !url.starts_with("https://api.github.com/")
            {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmMkErrAdresseRefusee)}));
            }
            let mut cfg = read_config(config_path);
            if !cfg["extensions"].is_object() {
                cfg["extensions"] = json!({});
            }
            cfg["extensions"]["marketplace_url"] = json!(url);
            match ecrire_config(config_path, &cfg) {
                Ok(_) => {
                    let _ = std::fs::remove_file(MARKET_CACHE); // le cache pointait ailleurs
                    json!({"success": true, "message": i18n::t(langue, Cle::AdmMkMsgCatalogueMaj)})
                }
                Err(e) => json!({"success": false, "error": e}),
            }
        }

        "/marketplace/install" => {
            let url = body.get("url").cloned().unwrap_or_default();
            if !url.starts_with("https://github.com/")
                && !url.starts_with("https://api.github.com/")
                && !url.starts_with("https://objects.githubusercontent.com/")
            {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmMkErrSourceRefusee)}));
            }
            let nom = body.get("nom").cloned().unwrap_or_default();
            let id = body
                .get("id")
                .cloned()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| market_id_depuis_nom(&nom));
            if !ext_id_valide(&id) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmMkErrIdDeduitInvalide).replace("{id}", &id)}));
            }
            if EXT_BASE_APPS.contains(&id.as_str()) {
                return respond_json(request, json!({"success": false, "error":
                    i18n::t(langue, Cle::AdmMkErrIdReserveBase)}));
            }

            let donnees = match market_telecharger(&url) {
                Ok(d) => d,
                Err(e) => return respond_json(request, json!({"success": false, "error": e})),
            };

            // ── Pose des fichiers ─────────────────────────────────
            let mut risques = Vec::new();
            let fichiers: Vec<String> = if nom.to_lowercase().ends_with(".extension.json") {
                // Manifeste : il liste les assets a recuperer un par un.
                let manif: Value = match serde_json::from_slice(&donnees) {
                    Ok(v) => v,
                    Err(e) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmMkErrManifesteIllisible).replace("{e}", &e.to_string())}))
                    }
                };
                let cfg_rel = read_config(config_path);
                let rel = match market_release(&cfg_rel, false, langue) {
                    Ok(r) => r,
                    Err(e) => return respond_json(request, json!({"success": false, "error": e})),
                };
                match market_poser_fichiers(&id, &manif, &rel) {
                    Ok((poses, code)) => {
                        risques = ext_scan_risques(&code, langue);
                        poses
                    }
                    Err(e) => return respond_json(request, json!({"success": false, "error": e})),
                }
            } else if nom.to_lowercase().ends_with(".zip") {
                match market_extraire_zip(&id, &donnees) {
                    Ok(v) => {
                        // On relit le mod.rs extrait pour l'analyser.
                        let src = std::path::Path::new(EXT_SRC_ROOT).join(&id).join("mod.rs");
                        if let Ok(code) = std::fs::read_to_string(&src) {
                            risques = ext_scan_risques(&code, langue);
                        }
                        v
                    }
                    Err(e) => return respond_json(request, json!({"success": false, "error": e})),
                }
            } else {
                let code = match String::from_utf8(donnees) {
                    Ok(c) => c,
                    Err(_) => {
                        return respond_json(request, json!({"success": false, "error":
                            i18n::t(langue, Cle::AdmMkErrFichierPasUtf8)}))
                    }
                };
                if !code.contains("pub fn handle") {
                    return respond_json(request, json!({"success": false, "error":
                        i18n::t(langue, Cle::AdmMkErrPasExtensionVex)}));
                }
                risques = ext_scan_risques(&code, langue);
                let dossier = std::path::Path::new(EXT_SRC_ROOT).join(&id);
                if let Err(e) = std::fs::create_dir_all(&dossier) {
                    return respond_json(request, json!({"success": false, "error":
                        format!("Creation de {:?} : {}", dossier, e)}));
                }
                let cible = dossier.join("mod.rs");
                if let Err(e) = std::fs::write(&cible, &code) {
                    return respond_json(request, json!({"success": false, "error":
                        format!("Ecriture de {:?} : {}", cible, e)}));
                }
                let _ = std::fs::create_dir_all(std::path::Path::new(EXT_STATIC_ROOT).join(&id));
                vec![cible.to_string_lossy().to_string()]
            };

            // ── Confirmation si le code touche a des choses sensibles ──
            let confirme = body
                .get("confirm_risques")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);
            if !risques.is_empty() && !confirme {
                // Les fichiers sont poses mais l'extension n'est pas activee :
                // l'admin voit le detail avant de decider.
                return respond_json(request, json!({
                    "success": false, "need_confirm": true, "risques": risques,
                    "error": i18n::t(langue, Cle::AdmMkErrMotifsCode).replace("{n}", &risques.len().to_string()),
                }));
            }

            // ── Enregistrement dans config.json ───────────────────
            let mut cfg = read_config(config_path);
            if !cfg["extensions"]["extension_params"].is_object() {
                cfg["extensions"]["extension_params"] = json!({});
            }
            let ancien = cfg["extensions"]["extension_params"]
                .get(id.as_str())
                .cloned()
                .unwrap_or(json!({}));

            // Manifeste facultatif livre dans l'archive : il evite a
            // l'admin de ressaisir nom, version, privileges et
            // integration interface apres l'installation.
            let manifeste: Value = std::fs::read_to_string(
                std::path::Path::new(EXT_STATIC_ROOT).join(&id).join("extension.json"),
            )
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(json!({}));

            // Priorite : reglages deja en place > manifeste > defaut.
            let garde = |cle: &str, defaut: Value| -> Value {
                ancien
                    .get(cle)
                    .cloned()
                    .or_else(|| manifeste.get(cle).cloned())
                    .unwrap_or(defaut)
            };
            let mut entree = json!({
                "enabled":         true,
                "version":         garde("version", json!("0.1")),
                "privilege_min":   garde("privilege_min", json!(10)),
                "plans_autorises": garde("plans_autorises", json!(["free", "vip"])),
                "params":          garde("params", json!({})),
                "permissions":     garde("permissions", json!({})),
                "source":          json!(url),
            });
            // Blocs d'integration : uniquement s'ils ne sont pas deja
            // regles a la main dans la configuration.
            for cle in ["nav_app", "dashboard_tile", "admin_infos"] {
                if let Some(v) = ancien.get(cle).cloned().or_else(|| manifeste.get(cle).cloned()) {
                    entree[cle] = v;
                }
            }
            cfg["extensions"]["extension_params"][id.as_str()] = entree;
            if let Err(e) = ecrire_config(config_path, &cfg) {
                return respond_json(request, json!({"success": false, "error": e}));
            }

            let ids = match ext_regenerer_registre() {
                Ok(v) => v,
                Err(e) => {
                    return respond_json(request, json!({"success": false, "error":
                        i18n::t(langue, Cle::AdmExtErrRegistreExtensions).replace("{e}", &e)}))
                }
            };
            let build_lance = ext_lancer_build(&ext_build_cmd(&cfg)).is_ok();

            json!({"success": true,
                "message": i18n::t(langue, Cle::AdmMkMsgInstalleeDepuisGithub).replace("{id}", &id).replace("{suffix}",
                    if build_lance { i18n::t(langue, Cle::AdmExtMsgCompilationLanceeAuto) } else { i18n::t(langue, Cle::AdmExtMsgCompilationDejaEnCours) }),
                "data": {"id": id, "fichiers": fichiers, "risques": risques,
                         "registre_ids": ids, "build_lance": build_lance}})
        }

        _ => json!({"success":false,"error":"Route inconnue."}),
    };

    respond_json(request, resp);
}

fn respond_json(request: Request, body: Value) {
    let _ = request.respond(Response::from_string(body.to_string()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    ));
}

fn read_config(path: &str) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}

fn read_body(request: &mut Request) -> HashMap<String, String> {
    let mut s = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut s);
    let mut m = HashMap::new();
    for pair in s.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            m.insert(url_decode(k), url_decode(v));
        }
    }
    m
}

fn run_shell_command(cmd: &str) -> (bool, String) {
    if cmd.trim().is_empty() {
        return (false, "Commande vide".into());
    }
    #[cfg(windows)]
    let mut c = Command::new("cmd");
    #[cfg(windows)]
    {
        c.arg("/C").arg(cmd);
    }
    #[cfg(not(windows))]
    let mut c = Command::new("sh");
    #[cfg(not(windows))]
    {
        c.arg("-c").arg(cmd);
    }
    match c.output() {
        Ok(out) => {
            let mut s = String::new();
            if !out.stdout.is_empty() {
                s.push_str(&String::from_utf8_lossy(&out.stdout));
            }
            if !out.stderr.is_empty() {
                s.push_str(&String::from_utf8_lossy(&out.stderr));
            }
            (out.status.success(), s.trim().to_string())
        }
        Err(e) => (false, format!("{}", e)),
    }
}

/// Ancien point d'entree (disque racine uniquement), conserve pour
/// compatibilite -- utilise disks_info() en interne.
fn disk_info() -> (f64, f64, f64, u64) {
    disks_info()
        .into_iter()
        .find(|d| d.get("mount").and_then(|v| v.as_str()) == Some("/"))
        .map(|d| {
            (
                d.get("free_gb").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("total_gb").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("used_gb").and_then(|v| v.as_f64()).unwrap_or(0.0),
                d.get("used_pct").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0))
}

/// Liste tous les disques "reels" montes (exclut tmpfs/devtmpfs et les
/// petites partitions systeme type /boot) -- un Pi avec un disque externe
/// branche (ex: /media/.../E) doit voir les deux dans le dashboard, pas
/// seulement la racine.
fn disks_info() -> Vec<Value> {
    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("df")
            .args(["-B1", "--output=fstype,size,used,avail,target"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut disks = Vec::new();
            for line in s.lines().skip(1) {
                let p: Vec<&str> = line.split_whitespace().collect();
                if p.len() < 5 {
                    continue;
                }
                let fstype = p[0];
                if matches!(fstype, "tmpfs" | "devtmpfs" | "overlay" | "squashfs") {
                    continue;
                }
                let t: u64 = p[1].parse().unwrap_or(0);
                let u: u64 = p[2].parse().unwrap_or(0);
                let f: u64 = p[3].parse().unwrap_or(0);
                let mount = p[4..].join(" ");
                // Ignore les petites partitions systeme (ex: /boot/firmware, ~500 Mo)
                if t < 5_000_000_000 {
                    continue;
                }
                disks.push(json!({
                    "mount": mount,
                    "free_gb": (f as f64 / 1e9 * 10.0).round() / 10.0,
                    "total_gb": (t as f64 / 1e9 * 10.0).round() / 10.0,
                    "used_gb": (u as f64 / 1e9 * 10.0).round() / 10.0,
                    "used_pct": if t > 0 { u * 100 / t } else { 0 },
                }));
            }
            if !disks.is_empty() {
                return disks;
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(out) = std::process::Command::new("wmic")
            .args(["logicaldisk", "where", "DeviceID='C:'", "get", "Size,FreeSpace", "/format:csv"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines().skip(2) {
                let p: Vec<&str> = line.split(',').collect();
                if p.len() >= 3 {
                    let f = p[1].trim().parse::<u64>().unwrap_or(0);
                    let t = p[2].trim().parse::<u64>().unwrap_or(0);
                    let u = t.saturating_sub(f);
                    return vec![json!({
                        "mount": "C:",
                        "free_gb": f as f64 / 1e9,
                        "total_gb": t as f64 / 1e9,
                        "used_gb": u as f64 / 1e9,
                        "used_pct": if t > 0 { u * 100 / t } else { 0 },
                    })];
                }
            }
        }
    }
    vec![json!({"mount": "/", "free_gb": 0.0, "total_gb": 0.0, "used_gb": 0.0, "used_pct": 0})]
}

fn uptime_sec() -> u64 {
    #[cfg(unix)]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/uptime") {
            if let Some(v) = s.split_whitespace().next() {
                return v.parse::<f64>().unwrap_or(0.0) as u64;
            }
        }
    }
    0
}

/// Etat des mises a jour disponibles (OS, noyau, toolchain Rust).
/// Purement informatif : ne lance jamais de commande d'installation --
/// c'est a l'administrateur de les appliquer lui-meme, en connaissance
/// de cause, via SSH.
fn verifier_maj_systeme() -> Value {
    #[cfg(windows)]
    {
        return json!({
            "os_nom": "Windows", "kernel": "", "paquets_dispo": 0,
            "rustc_version": "", "rust_maj_disponible": false, "rust_detail": "",
            "supporte": false,
        });
    }
    #[cfg(unix)]
    {
        let (_, os_nom) = run_shell_command(
            "grep -m1 PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '\"'",
        );
        let (_, kernel) = run_shell_command("uname -r");
        let (_, apt_out) = run_shell_command(
            "apt list --upgradable 2>/dev/null | tail -n +2 | wc -l",
        );
        let paquets_dispo: u64 = apt_out.trim().parse().unwrap_or(0);
        let (_, rustc_version) = run_shell_command(". $HOME/.cargo/env 2>/dev/null; rustc --version 2>/dev/null");
        let (_, rustup_check) = run_shell_command(". $HOME/.cargo/env 2>/dev/null; rustup check 2>&1");
        let rust_maj_disponible = rustup_check.to_lowercase().contains("update available");

        json!({
            "os_nom": os_nom.trim(),
            "kernel": kernel.trim(),
            "paquets_dispo": paquets_dispo,
            "rustc_version": rustc_version.trim(),
            "rust_maj_disponible": rust_maj_disponible,
            "rust_detail": rustup_check.trim(),
            "supporte": true,
        })
    }
}
// ══════════════════════════════════════════════════════════════════
// EXTENSIONS — upload de fichiers .rs, permissions, compilation
// ══════════════════════════════════════════════════════════════════

const EXT_SRC_ROOT: &str = "src/extensions";
const EXT_STATIC_ROOT: &str = "static/extensions";
const EXT_MAX_BYTES: usize = 512 * 1024;
const BUILD_STATUS_PATH: &str = "log/build_status.json";

/// Apps compilées en dur dans le binaire : leur id ne peut pas être
/// réutilisé par une extension uploadée.
const EXT_BASE_APPS: [&str; 7] = ["meet", "onlyoffice", "sitec", "vexmail", "mess", "p2p", "viso"];

static BUILD_EN_COURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Modèle fourni à l'admin pour écrire une extension valide.
const EXT_TEMPLATE: &str = r####"// ══════════════════════════════════════════════════════════════════
// Extension VEX — modele
//
// Placez ce fichier dans src/extensions/<id>/mod.rs
// (ou uploadez-le depuis le panel admin, section Extensions).
//
// L'extension est servie sur :
//     /ext/<id>       -> page HTML
//     /api/ext/<id>/… -> API JSON
//
// Le privilege et le plan sont deja verifies par VEX avant l'appel :
// si `handle` est appele, l'utilisateur a le droit d'etre la.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::c::SessionInfo;
use serde_json::json;
use std::io::Cursor;
use tiny_http::{Header, Request, Response};

pub fn handle(
    pool: &DbPool,
    session: &SessionInfo,
    req: &mut Request,
) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();

    // ── API JSON ──────────────────────────────────────────────────
    if path.starts_with("/api/ext/") {
        let corps = json!({
            "success": true,
            "message": "Bonjour depuis l'extension !",
            "user_id": session.user_id,
            "user_nom": session.user_nom,
            "privilege": session.user_privilege,
        });
        return Response::from_string(corps.to_string()).with_header(
            Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
        );
    }

    // ── Page HTML ─────────────────────────────────────────────────
    // Les fichiers statiques de l'extension sont servis depuis
    // /static/extensions/<id>/… si vous en ajoutez.
    let html = format!(
        "<!DOCTYPE html><html lang=\"fr\"><head><meta charset=\"UTF-8\">\
         <title>Mon extension</title></head><body style=\"font-family:sans-serif;padding:40px\">\
         <h1>Mon extension</h1><p>Connecte en tant que <strong>{}</strong> (privilege {}).</p>\
         </body></html>",
        session.user_nom, session.user_privilege
    );

    let _ = pool; // le pool DB est disponible via appeldb::selectionner(...)

    Response::from_string(html)
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
}
"####;

/// Un id d'extension est un nom de module Rust valide.
fn ext_id_valide(id: &str) -> bool {
    if id.len() < 2 || id.len() > 32 {
        return false;
    }
    match id.chars().next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return false;
    }
    // mots-clés Rust qui casseraient `pub mod <id>;`
    !matches!(
        id,
        "crate" | "self" | "super" | "mod" | "use" | "type" | "impl" | "match" | "move" | "loop"
    )
}

/// Repère les constructions sensibles d'un code uploadé.
/// Ne bloque rien tout seul : la liste part à l'admin qui confirme ou non.
fn ext_scan_risques(code: &str, langue: &str) -> Vec<Value> {
    let motifs: [(&str, &'static str); 11] = [
        ("Command::new", i18n::t(langue, Cle::AdmRisqueCommandeSysteme)),
        ("std::process", i18n::t(langue, Cle::AdmRisqueAccesProcessus)),
        ("unsafe", i18n::t(langue, Cle::AdmRisqueBlocUnsafe)),
        ("remove_dir_all", i18n::t(langue, Cle::AdmRisqueSupprRecursive)),
        ("remove_file", i18n::t(langue, Cle::AdmRisqueSupprFichiers)),
        ("executer_sql", i18n::t(langue, Cle::AdmRisqueSqlBrut)),
        ("destruction_totale", i18n::t(langue, Cle::AdmRisqueDestructionTotale)),
        ("\"privilege\"", i18n::t(langue, Cle::AdmRisqueEcriturePrivilege)),
        ("extern \"C\"", i18n::t(langue, Cle::AdmRisqueLiaisonNative)),
        ("libloading", i18n::t(langue, Cle::AdmRisqueChargementBiblio)),
        ("include_bytes!", i18n::t(langue, Cle::AdmRisqueInclusionBinaire)),
    ];
    let mut sorties = Vec::new();
    for (i, ligne) in code.lines().enumerate() {
        if ligne.trim_start().starts_with("//") {
            continue;
        }
        for (motif, label) in motifs.iter() {
            if ligne.contains(motif) {
                let extrait: String = ligne.trim().chars().take(160).collect();
                sorties.push(json!({
                    "ligne": i + 1,
                    "motif": label,
                    "extrait": extrait,
                }));
                break;
            }
        }
        if sorties.len() >= 60 {
            break;
        }
    }
    sorties
}

/// Extensions présentes sur le disque (src/extensions/<id>/mod.rs).
fn ext_ids_sur_disque() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(EXT_SRC_ROOT) {
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let nom = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !ext_id_valide(&nom) {
                continue;
            }
            if p.join("mod.rs").is_file() {
                ids.push(nom);
            }
        }
    }
    ids.sort();
    ids
}

/// État disque d'une extension : source présente, taille, date, statique.
fn ext_etat(id: &str) -> Value {
    let src = std::path::Path::new(EXT_SRC_ROOT).join(id).join("mod.rs");
    let stat = std::path::Path::new(EXT_STATIC_ROOT).join(id);
    let (existe, taille, modifie) = match std::fs::metadata(&src) {
        Ok(m) => {
            let secs = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (true, m.len(), secs)
        }
        Err(_) => (false, 0, 0),
    };
    json!({
        "src_path": src.to_string_lossy(),
        "src_existe": existe,
        "src_taille": taille,
        "src_modifie_unix": modifie,
        "static_path": stat.to_string_lossy(),
        "static_existe": stat.is_dir(),
    })
}

/// Contenu du registre src/extensions/mod.rs pour la liste d'ids donnée.
fn ext_registre_contenu(ids: &[String]) -> String {
    let mut mods = String::new();
    let mut arms = String::new();
    let mut liste = String::new();
    for id in ids {
        mods.push_str(&format!("pub mod {};\n", id));
        arms.push_str(&format!(
            "        \"{}\" => Some({}::handle(pool, session, req)),\n",
            id, id
        ));
        liste.push_str(&format!("\"{}\", ", id));
    }
    let liste = liste.trim_end_matches(", ").to_string();
    let bloc_mods = if mods.is_empty() {
        String::new()
    } else {
        format!("{}\n", mods)
    };

    let mut out = String::new();
    out.push_str("// ══════════════════════════════════════════════════════════════════\n");
    out.push_str("// extensions/mod.rs — FICHIER GENERE AUTOMATIQUEMENT\n");
    out.push_str("// Reecrit par le panel admin VEX (section Extensions) a chaque\n");
    out.push_str("// ajout ou suppression d'extension. Ne pas editer a la main.\n");
    out.push_str("//\n");
    out.push_str("// Convention : chaque extension vit dans src/extensions/<id>/mod.rs\n");
    out.push_str("// et expose :\n");
    out.push_str("//     pub fn handle(pool: &DbPool, session: &SessionInfo,\n");
    out.push_str("//                   req: &mut Request) -> Response<Cursor<Vec<u8>>>\n");
    out.push_str("// Elle est ensuite servie sur /ext/<id> et /api/ext/<id>, apres\n");
    out.push_str("// verification du privilege et du plan definis dans config.json.\n");
    out.push_str("// ══════════════════════════════════════════════════════════════════\n\n");
    out.push_str("#![allow(unused_imports, unused_variables, dead_code)]\n\n");
    out.push_str("use crate::appeldb::DbPool;\n");
    out.push_str("use crate::c::SessionInfo;\n");
    out.push_str("use std::io::Cursor;\n");
    out.push_str("use tiny_http::{Request, Response};\n\n");
    out.push_str(&bloc_mods);
    out.push_str("/// Extensions reellement compilees dans ce binaire.\n");
    out.push_str("pub fn compiled_ids() -> &'static [&'static str] {\n");
    out.push_str(&format!("    &[{}]\n", liste));
    out.push_str("}\n\n");
    out.push_str("/// Route /ext/<id> vers l'extension. None si l'id n'est pas compile.\n");
    out.push_str("pub fn dispatch(\n");
    out.push_str("    id: &str,\n");
    out.push_str("    pool: &DbPool,\n");
    out.push_str("    session: &SessionInfo,\n");
    out.push_str("    req: &mut Request,\n");
    out.push_str(") -> Option<Response<Cursor<Vec<u8>>>> {\n");
    out.push_str("    match id {\n");
    out.push_str(&arms);
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

/// Réécrit src/extensions/mod.rs à partir de ce qui existe sur le disque.
fn ext_regenerer_registre() -> Result<Vec<String>, String> {
    std::fs::create_dir_all(EXT_SRC_ROOT).map_err(|e| format!("mkdir {} : {}", EXT_SRC_ROOT, e))?;
    let ids = ext_ids_sur_disque();
    let contenu = ext_registre_contenu(&ids);
    let chemin = std::path::Path::new(EXT_SRC_ROOT).join("mod.rs");
    std::fs::write(&chemin, contenu).map_err(|e| format!("écriture {:?} : {}", chemin, e))?;
    Ok(ids)
}

/// Commande de compilation configurable (extensions.build_cmd).
/// Source ~/.cargo/env par défaut : run_shell_command() lance `sh -c`, qui
/// n'hérite pas forcément du PATH mis à jour par rustup (le process vex
/// est démarré hors shell de login) — sans ça, "cargo: not found".
fn ext_build_cmd(cfg: &Value) -> String {
    cfg["extensions"]["build_cmd"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(". $HOME/.cargo/env 2>/dev/null; cargo build --release")
        .to_string()
}

fn maintenant() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn ecrire_build_status(v: Value) {
    let _ = std::fs::create_dir_all(log_dir());
    let _ = std::fs::write(
        BUILD_STATUS_PATH,
        serde_json::to_string_pretty(&v).unwrap_or_default(),
    );
}

fn lire_build_status() -> Value {
    std::fs::read_to_string(BUILD_STATUS_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({ "running": false, "jamais_lance": true }))
}

/// Lance la compilation dans un thread : le serveur HTTP mono-thread
/// ne doit pas être bloqué plusieurs minutes par cargo.
fn ext_lancer_build(cmd: &str) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if BUILD_EN_COURS.swap(true, Ordering::SeqCst) {
        return Err("Une compilation est déjà en cours.".into());
    }
    let cmd = cmd.to_string();
    ecrire_build_status(json!({
        "running": true,
        "cmd": cmd,
        "started_at": maintenant(),
        "output": "",
    }));
    std::thread::spawn(move || {
        let (ok, out) = run_shell_command(&cmd);
        let out_court: String = {
            let lignes: Vec<&str> = out.lines().collect();
            let debut = lignes.len().saturating_sub(400);
            lignes[debut..].join("\n")
        };
        ecrire_build_status(json!({
            "running": false,
            "cmd": cmd,
            "success": ok,
            "finished_at": maintenant(),
            "output": out_court,
        }));
        BUILD_EN_COURS.store(false, Ordering::SeqCst);
    });
    Ok(())
}

// ══════════════════════════════════════════════════════════════════
// SAUVEGARDES — mysqldump | gzip vers backups/, dans un thread (meme
// raison que ext_lancer_build : serveur HTTP mono-thread).
// ══════════════════════════════════════════════════════════════════
const BACKUP_DIR: &str = "backups";
const BACKUP_STATUS_PATH: &str = "log/backup_status.json";
static BACKUP_EN_COURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn ecrire_backup_status(v: Value) {
    let _ = std::fs::create_dir_all(log_dir());
    let _ = std::fs::write(
        BACKUP_STATUS_PATH,
        serde_json::to_string_pretty(&v).unwrap_or_default(),
    );
}

fn lire_backup_status() -> Value {
    std::fs::read_to_string(BACKUP_STATUS_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({ "running": false, "jamais_lance": true }))
}

/// Whitelist stricte : seuls les noms au format attendu sont acceptes,
/// pour /backup/delete et /backup/download (pas de traversee de chemin).
fn backup_nom_valide(nom: &str) -> bool {
    nom.len() < 100
        && nom.starts_with("vex-backup-")
        && nom.ends_with(".sql.gz")
        && nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
}

fn lister_backups() -> Vec<Value> {
    let _ = std::fs::create_dir_all(BACKUP_DIR);
    let mut fichiers: Vec<Value> = std::fs::read_dir(BACKUP_DIR)
        .map(|it| {
            it.filter_map(Result::ok)
                .filter_map(|e| {
                    let nom = e.file_name().to_str()?.to_string();
                    if !backup_nom_valide(&nom) {
                        return None;
                    }
                    let meta = e.metadata().ok()?;
                    let modifie = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    Some(json!({
                        "nom": nom,
                        "taille_mo": (meta.len() as f64 / 1_048_576.0 * 100.0).round() / 100.0,
                        "modifie": modifie,
                    }))
                })
                .collect()
        })
        .unwrap_or_default();
    fichiers.sort_by(|a, b| {
        b["modifie"].as_u64().unwrap_or(0).cmp(&a["modifie"].as_u64().unwrap_or(0))
    });
    fichiers
}

/// Lance un dump complet (structure + donnees, fichiers inclus car
/// stockes en base64 dans `fichiers`) dans un thread separe.
fn lancer_backup(db: &crate::config_loader::DbConfig, langue: &str) -> Result<(), String> {
    use std::process::Stdio;
    use std::sync::atomic::Ordering;
    if BACKUP_EN_COURS.swap(true, Ordering::SeqCst) {
        return Err(i18n::t(langue, Cle::AdmBackupErrDejaEnCours).to_string());
    }
    let _ = std::fs::create_dir_all(BACKUP_DIR);
    let horodatage = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let nom = format!("vex-backup-{}.sql.gz", horodatage);
    let chemin = format!("{}/{}", BACKUP_DIR, nom);
    let db = db.clone();
    let langue = langue.to_string();
    ecrire_backup_status(json!({"running": true, "started_at": maintenant(), "fichier": nom}));

    std::thread::spawn(move || {
        let resultat: Result<u64, String> = (|| {
            let mut dump = Command::new("mysqldump")
                .args([
                    "--single-transaction",
                    "--quick",
                    "-h", &db.host,
                    "-P", &db.port.to_string(),
                    "-u", &db.user,
                    &db.dbname,
                ])
                .env("MYSQL_PWD", &db.password)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("mysqldump introuvable : {e}"))?;
            let dump_out = dump.stdout.take().ok_or("pipe mysqldump indisponible")?;
            let fichier = std::fs::File::create(&chemin)
                .map_err(|e| format!("Création du fichier de sauvegarde : {e}"))?;
            let gzip_status = Command::new("gzip")
                .arg("-c")
                .stdin(Stdio::from(dump_out))
                .stdout(Stdio::from(fichier))
                .status()
                .map_err(|e| format!("gzip introuvable : {e}"))?;
            let dump_status = dump.wait().map_err(|e| format!("{e}"))?;
            if !dump_status.success() {
                let mut err = String::new();
                if let Some(mut s) = dump.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut s, &mut err);
                }
                return Err(format!("mysqldump a échoué : {}", err.trim()));
            }
            if !gzip_status.success() {
                return Err(i18n::t(&langue, Cle::AdmBackupErrGzipEchoue).to_string());
            }
            std::fs::metadata(&chemin).map(|m| m.len()).map_err(|e| format!("{e}"))
        })();

        match resultat {
            Ok(taille) => ecrire_backup_status(json!({
                "running": false, "success": true, "finished_at": maintenant(),
                "fichier": nom, "taille_mo": (taille as f64 / 1_048_576.0 * 100.0).round() / 100.0,
            })),
            Err(e) => {
                let _ = std::fs::remove_file(&chemin);
                ecrire_backup_status(json!({
                    "running": false, "success": false, "finished_at": maintenant(),
                    "fichier": nom, "erreur": e,
                }));
            }
        }
        BACKUP_EN_COURS.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn ecrire_config(path: &str, cfg: &Value) -> Result<(), String> {
    let txt = serde_json::to_string_pretty(cfg).map_err(|e| format!("Sérialisation : {}", e))?;
    std::fs::write(path, txt).map_err(|e| format!("Écriture de {} : {}", path, e))
}

// ══════════════════════════════════════════════════════════════════
// ÉDITEUR EN LIGNE — providers multiples (OnlyOffice, Collabora, …)
// ══════════════════════════════════════════════════════════════════

/// Valeurs par défaut d'un provider selon la suite choisie.
fn editor_defauts(kind: &str) -> Value {
    match kind {
        "onlyoffice" => json!({
            "kind": "onlyoffice",
            "name": "OnlyOffice Document Server",
            "enabled": true,
            "server_url": "http://127.0.0.1:8080",
            "healthcheck_path": "/healthcheck",
            "healthcheck_expect": "true",
            "editor_api_js": "/web-apps/apps/api/documents/api.js",
            "wopi_discovery_path": "",
            "callback_path": "/api/fchier/onlyoffice/callback",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 8000,
            "jwt_enabled": true,
            "jwt_secret": "",
            "default_format": "docx",
            "supported_formats": ["docx", "xlsx", "pptx", "odt", "ods", "odp"],
            "max_file_size_mb": 100,
            "auto_save": true,
            "collaborative_editing": true
        }),
        "collabora" => json!({
            "kind": "collabora",
            "name": "Collabora Online (CODE)",
            "enabled": true,
            "server_url": "http://127.0.0.1:9980",
            "healthcheck_path": "/hosting/discovery",
            "healthcheck_expect": "wopi",
            "editor_api_js": "",
            "wopi_discovery_path": "/hosting/discovery",
            "callback_path": "/api/fchier/wopi",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 10000,
            "jwt_enabled": false,
            "jwt_secret": "",
            "default_format": "odt",
            "supported_formats": ["odt", "ods", "odp", "docx", "xlsx", "pptx", "rtf", "csv"],
            "max_file_size_mb": 100,
            "auto_save": true,
            "collaborative_editing": true
        }),
        "etherpad" => json!({
            "kind": "etherpad",
            "name": "Etherpad (texte collaboratif)",
            "enabled": true,
            "server_url": "http://127.0.0.1:9001",
            "healthcheck_path": "/api",
            "healthcheck_expect": "currentVersion",
            "editor_api_js": "",
            "wopi_discovery_path": "",
            "callback_path": "",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 6000,
            "jwt_enabled": false,
            "jwt_secret": "",
            "default_format": "txt",
            "supported_formats": ["txt", "md", "html"],
            "max_file_size_mb": 20,
            "auto_save": true,
            "collaborative_editing": true
        }),
        "cryptpad" => json!({
            "kind": "cryptpad",
            "name": "CryptPad (chiffré de bout en bout)",
            "enabled": true,
            "server_url": "http://127.0.0.1:3000",
            "healthcheck_path": "/api/config",
            "healthcheck_expect": "",
            "editor_api_js": "",
            "wopi_discovery_path": "",
            "callback_path": "",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 8000,
            "jwt_enabled": false,
            "jwt_secret": "",
            "default_format": "md",
            "supported_formats": ["md", "txt", "docx", "xlsx", "pptx"],
            "max_file_size_mb": 50,
            "auto_save": true,
            "collaborative_editing": true
        }),
        "externe" => json!({
            "kind": "externe",
            "name": "Visionneuse web externe",
            "enabled": true,
            "server_url": "https://view.officeapps.live.com",
            "healthcheck_path": "/",
            "healthcheck_expect": "",
            "editor_api_js": "",
            "wopi_discovery_path": "",
            "callback_path": "",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 0,
            "jwt_enabled": false,
            "jwt_secret": "",
            "default_format": "pdf",
            "supported_formats": ["pdf", "docx", "xlsx", "pptx"],
            "max_file_size_mb": 25,
            "auto_save": false,
            "collaborative_editing": false
        }),
        _ => json!({
            "kind": "custom",
            "name": "Serveur personnalisé",
            "enabled": true,
            "server_url": "http://127.0.0.1:8000",
            "healthcheck_path": "/",
            "healthcheck_expect": "",
            "editor_api_js": "",
            "wopi_discovery_path": "",
            "callback_path": "",
            "start_cmd": "",
            "stop_cmd": "",
            "wait_boot_ms": 5000,
            "jwt_enabled": false,
            "jwt_secret": "",
            "default_format": "txt",
            "supported_formats": ["txt"],
            "max_file_size_mb": 50,
            "auto_save": false,
            "collaborative_editing": false
        }),
    }
}

/// Suites proposées dans le sélecteur du panel admin.
fn editor_presets() -> Value {
    json!([
        {
            "kind": "onlyoffice",
            "name": "OnlyOffice Document Server",
            "desc": "Suite bureautique complète (docx/xlsx/pptx), édition collaborative, JWT.",
            "defaults": editor_defauts("onlyoffice")
        },
        {
            "kind": "collabora",
            "name": "Collabora Online (CODE)",
            "desc": "LibreOffice en ligne via WOPI, formats ODF natifs.",
            "defaults": editor_defauts("collabora")
        },
        {
            "kind": "etherpad",
            "name": "Etherpad",
            "desc": "Édition de texte collaborative temps réel, très léger.",
            "defaults": editor_defauts("etherpad")
        },
        {
            "kind": "cryptpad",
            "name": "CryptPad",
            "desc": "Suite chiffrée de bout en bout, orientée vie privée.",
            "defaults": editor_defauts("cryptpad")
        },
        {
            "kind": "externe",
            "name": "Visionneuse web externe",
            "desc": "Aperçu via un service web public, aucun serveur à héberger.",
            "defaults": editor_defauts("externe")
        },
        {
            "kind": "custom",
            "name": "Serveur personnalisé",
            "desc": "N'importe quel serveur : vous fixez l'URL et tous les paramètres.",
            "defaults": editor_defauts("custom")
        }
    ])
}

/// Complète un provider avec les valeurs par défaut de sa suite.
fn editor_normaliser(p: &Value) -> Value {
    let kind = p.get("kind").and_then(|v| v.as_str()).unwrap_or("custom");
    let mut base = editor_defauts(kind);
    if let (Some(b), Some(o)) = (base.as_object_mut(), p.as_object()) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

/// Liste des providers, avec migration depuis l'ancienne config
/// (onlyoffice_server + extensions.extension_params.onlyoffice).
fn editor_providers(cfg: &Value) -> serde_json::Map<String, Value> {
    if let Some(obj) = cfg["editor"]["providers"].as_object() {
        if !obj.is_empty() {
            let mut m = serde_json::Map::new();
            for (k, v) in obj {
                m.insert(k.clone(), editor_normaliser(v));
            }
            return m;
        }
    }

    // ── Migration depuis l'ancienne configuration OnlyOffice ─────
    let mut oo = editor_defauts("onlyoffice");
    let srv = cfg["onlyoffice_server"].clone();
    let params = cfg["extensions"]["extension_params"]["onlyoffice"]["params"].clone();

    {
        let obj = oo.as_object_mut().unwrap();
        let mut reprendre = |cle: &str, val: Option<&Value>| {
            if let Some(v) = val {
                if !v.is_null() {
                    obj.insert(cle.to_string(), v.clone());
                }
            }
        };
        reprendre("server_url", srv.get("server_url"));
        reprendre("healthcheck_path", srv.get("healthcheck_path"));
        reprendre("start_cmd", srv.get("start_cmd"));
        reprendre("stop_cmd", srv.get("stop_cmd"));
        reprendre("wait_boot_ms", srv.get("wait_boot_ms"));
        reprendre("enabled", srv.get("enabled"));
        reprendre("jwt_enabled", params.get("jwt_enabled"));
        reprendre("jwt_secret", params.get("jwt_secret"));
        reprendre("default_format", params.get("default_format"));
        reprendre("max_file_size_mb", params.get("max_file_size_mb"));
        reprendre("auto_save", params.get("auto_save"));
        reprendre(
            "collaborative_editing",
            cfg["editor"].get("collaborative_editing"),
        );
        if let Some(f) = cfg["editor"]["supported_formats"].as_array() {
            if !f.is_empty() {
                obj.insert("supported_formats".to_string(), json!(f));
            }
        }
    }

    let mut m = serde_json::Map::new();
    m.insert("onlyoffice".to_string(), oo);
    m
}

/// Id du provider actif, garanti présent dans la liste.
fn editor_actif(cfg: &Value, providers: &serde_json::Map<String, Value>) -> String {
    let demande = cfg["editor"]["provider"].as_str().unwrap_or("").to_string();
    if providers.contains_key(&demande) {
        return demande;
    }
    providers
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "onlyoffice".to_string())
}

/// Ping HTTP générique : `expect` vide = tout code 2xx/3xx suffit.
fn http_check(base_url: &str, path: &str, expect: &str) -> (bool, u64, String) {
    if base_url.trim().is_empty() {
        return (false, 0, "URL du serveur non définie.".into());
    }
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let t0 = std::time::Instant::now();
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .call();
    let ms = t0.elapsed().as_millis() as u64;
    match resp {
        Ok(r) => {
            let code = r.status();
            let corps = r.into_string().unwrap_or_default();
            let ok = if expect.trim().is_empty() {
                (200..400).contains(&code)
            } else {
                corps.to_lowercase().contains(&expect.trim().to_lowercase())
            };
            let extrait: String = corps.trim().chars().take(200).collect();
            (ok, ms, format!("HTTP {} — {}", code, extrait))
        }
        Err(e) => (false, ms, format!("{}", e)),
    }
}

/// Ping d'un provider à partir de sa configuration.
fn editor_check(p: &Value) -> (bool, u64, String) {
    let url = p.get("server_url").and_then(|v| v.as_str()).unwrap_or("");
    let path = p
        .get("healthcheck_path")
        .and_then(|v| v.as_str())
        .unwrap_or("/");
    let expect = p
        .get("healthcheck_expect")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    http_check(url, path, expect)
}

/// Provider actif + son état, utilisé par le dashboard et les routes
/// de compatibilité /onlyoffice*.
fn editor_actif_complet(config_path: &str) -> (String, Value, bool, u64, String) {
    let cfg = read_config(config_path);
    let providers = editor_providers(&cfg);
    let id = editor_actif(&cfg, &providers);
    let p = providers.get(&id).cloned().unwrap_or(json!({}));
    let (online, ms, detail) = editor_check(&p);
    (id, p, online, ms, detail)
}

// ══════════════════════════════════════════════════════════════════
// PLACE DE MARCHE — extensions publiees sur une release GitHub
//
// La liste vient de l'API GitHub (assets de la release), elle est
// mise en cache 5 minutes pour ne pas marteler l'API. L'installation
// telecharge l'asset et le pose exactement au meme endroit qu'un
// upload manuel, puis relance la compilation.
//   .rs  -> src/extensions/<id>/mod.rs
//   .zip -> src/extensions/<id>/  et  static/extensions/<id>/
// Reserve aux superadmins : installer, c'est executer du code.
// ══════════════════════════════════════════════════════════════════

const MARKET_CACHE: &str = "log/marketplace_cache.json";
const MARKET_CACHE_SECS: u64 = 300;
const MARKET_MAX_BYTES: usize = 8 * 1024 * 1024;

/// URL de la release configuree, convertie en URL d'API GitHub.
/// Accepte une page release ("/releases/tag/X") ou un depot nu.
fn market_api_url(cfg: &Value) -> Option<String> {
    let brut = cfg["extensions"]["marketplace_url"]
        .as_str()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
        .to_string();
    if brut.is_empty() {
        return None;
    }
    if brut.starts_with("https://api.github.com/") {
        return Some(brut);
    }
    let reste = brut.strip_prefix("https://github.com/")?;
    let mut bouts = reste.split('/');
    let proprio = bouts.next()?;
    let depot = bouts.next()?;
    // .../releases/tag/<tag>  ->  release precise ; sinon la derniere.
    let tag = reste.split("/releases/tag/").nth(1).map(|t| {
        t.split('/').next().unwrap_or(t).to_string()
    });
    Some(match tag {
        Some(t) => format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            proprio, depot, t
        ),
        None => format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            proprio, depot
        ),
    })
}

fn market_lire_cache() -> Option<Value> {
    let brut = std::fs::read_to_string(MARKET_CACHE).ok()?;
    let v: Value = serde_json::from_str(&brut).ok()?;
    let age = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs()
        .saturating_sub(v.get("_cache_ts")?.as_u64()?);
    if age < MARKET_CACHE_SECS {
        Some(v)
    } else {
        None
    }
}

fn market_ecrire_cache(mut v: Value) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    v["_cache_ts"] = json!(ts);
    let _ = std::fs::create_dir_all(log_dir());
    let _ = std::fs::write(MARKET_CACHE, v.to_string());
}

/// Recupere la release depuis GitHub (ou le cache).
fn market_release(cfg: &Value, forcer: bool, langue: &str) -> Result<Value, String> {
    if !forcer {
        if let Some(v) = market_lire_cache() {
            return Ok(v);
        }
    }
    let configuree = cfg["extensions"]["marketplace_url"].as_str().unwrap_or("").trim();
    let url = market_api_url(cfg).ok_or_else(|| {
        if configuree.is_empty() {
            i18n::t(langue, Cle::AdmMkAucunCatalogueConfigure).to_string()
        } else {
            i18n::t(langue, Cle::AdmMkPasUrlRelease).replace("{url}", configuree)
        }
    })?;
    let rep = ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "VEX")
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .map_err(|e| format!("GitHub injoignable : {}", e))?;
    let v: Value = rep
        .into_json()
        .map_err(|e| format!("Reponse GitHub illisible : {}", e))?;
    market_ecrire_cache(v.clone());
    Ok(v)
}

/// Un id d'extension deduit du nom de l'asset ("monchat-0.2.zip" -> "monchat").
fn market_id_depuis_nom(nom: &str) -> String {
    let base = nom
        .rsplit('/')
        .next()
        .unwrap_or(nom)
        .trim_end_matches(".zip")
        .trim_end_matches(".rs");
    let coupe = base
        .split(|c: char| c == '-' || c == '_')
        .take_while(|m| !m.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .collect::<Vec<_>>()
        .join("_");
    let brut = if coupe.is_empty() { base.to_string() } else { coupe };
    brut.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
        .collect()
}

/// Telecharge un asset, en refusant tout ce qui depasse la limite.
fn market_telecharger(url: &str) -> Result<Vec<u8>, String> {
    let rep = ureq::get(url)
        .set("Accept", "application/octet-stream")
        .set("User-Agent", "VEX")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| format!("Telechargement impossible : {}", e))?;
    let mut donnees = Vec::new();
    let mut lecteur = std::io::Read::take(
        std::io::BufReader::new(rep.into_reader()),
        (MARKET_MAX_BYTES + 1) as u64,
    );
    std::io::Read::read_to_end(&mut lecteur, &mut donnees)
        .map_err(|e| format!("Lecture interrompue : {}", e))?;
    if donnees.len() > MARKET_MAX_BYTES {
        return Err(format!(
            "Archive trop volumineuse (max {} Mo).",
            MARKET_MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok(donnees)
}

/// Installe une extension livree en fichiers separes.
/// Le manifeste indique, pour chaque fichier, l'asset a telecharger et
/// sa destination relative ("src/..." ou "static/...").
/// Rend les chemins ecrits et le contenu du mod.rs (pour l'analyse).
fn market_poser_fichiers(
    id: &str,
    manifeste: &Value,
    release: &Value,
) -> Result<(Vec<String>, String), String> {
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| "Release sans asset.".to_string())?;
    let url_de = |nom: &str| -> Option<String> {
        assets
            .iter()
            .find(|a| a["name"].as_str() == Some(nom))
            .and_then(|a| a["browser_download_url"].as_str())
            .map(|s| s.to_string())
    };

    let liste = manifeste["fichiers"]
        .as_array()
        .ok_or_else(|| "Le manifeste ne contient pas de liste « fichiers ».".to_string())?;
    if liste.is_empty() {
        return Err("Le manifeste ne declare aucun fichier.".into());
    }

    let racine_src = std::path::Path::new(EXT_SRC_ROOT).join(id);
    let racine_static = std::path::Path::new(EXT_STATIC_ROOT).join(id);
    let mut ecrits = Vec::new();
    let mut code_mod = String::new();

    for f in liste {
        let asset = f["asset"].as_str().unwrap_or("");
        let cible = f["cible"].as_str().unwrap_or("");
        if asset.is_empty() || cible.is_empty() {
            return Err("Entree de manifeste incomplete (asset/cible).".into());
        }
        // Aucune remontee de dossier, aucun chemin absolu.
        if cible.contains("..") || cible.starts_with('/') || cible.contains('\\') {
            return Err(format!("Destination refusee : {}", cible));
        }
        let (racine, reste) = if let Some(r) = cible.strip_prefix("src/") {
            (&racine_src, r)
        } else if let Some(r) = cible.strip_prefix("static/") {
            (&racine_static, r)
        } else {
            (&racine_static, cible)
        };
        if reste.is_empty() {
            return Err(format!("Destination vide pour {}", asset));
        }

        let url = url_de(asset)
            .ok_or_else(|| format!("Asset « {} » absent de la release.", asset))?;
        let donnees = market_telecharger(&url)?;

        let mut chemin = racine.clone();
        for seg in reste.split('/') {
            if seg.is_empty() || seg == "." {
                continue;
            }
            chemin.push(seg);
        }
        if let Some(parent) = chemin.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?} : {}", parent, e))?;
        }
        std::fs::write(&chemin, &donnees).map_err(|e| format!("Ecriture {:?} : {}", chemin, e))?;
        if reste == "mod.rs" {
            code_mod = String::from_utf8_lossy(&donnees).to_string();
        }
        ecrits.push(chemin.to_string_lossy().to_string());
    }

    if code_mod.is_empty() {
        return Err("Le manifeste ne fournit pas src/mod.rs.".into());
    }
    if !code_mod.contains("pub fn handle") {
        return Err("Le mod.rs telecharge n'expose pas `pub fn handle`.".into());
    }
    // On garde le manifeste sur place : il alimente la configuration.
    let _ = std::fs::create_dir_all(&racine_static);
    let _ = std::fs::write(
        racine_static.join("extension.json"),
        serde_json::to_string_pretty(manifeste).unwrap_or_default(),
    );
    Ok((ecrits, code_mod))
}

/// Extrait une archive d'extension. Les chemins sont normalises :
/// tout ce qui ressemble a du code va dans src/, le reste dans static/.
fn market_extraire_zip(id: &str, donnees: &[u8]) -> Result<Vec<String>, String> {
    let curseur = std::io::Cursor::new(donnees);
    let mut archive =
        zip::ZipArchive::new(curseur).map_err(|e| format!("Archive illisible : {}", e))?;
    let racine_src = std::path::Path::new(EXT_SRC_ROOT).join(id);
    let racine_static = std::path::Path::new(EXT_STATIC_ROOT).join(id);
    let mut ecrits = Vec::new();

    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Entree {} illisible : {}", i, e))?;
        if f.is_dir() {
            continue;
        }
        // enclosed_name refuse les chemins absolus et les "..".
        let interne = match f.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let bouts: Vec<String> = interne
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
            .collect();
        if bouts.is_empty() {
            continue;
        }
        // On retire un eventuel dossier racine et les prefixes src/ static/
        let mut reste: Vec<String> = bouts.clone();
        let vers_src = reste.iter().any(|b| b == "src") || interne.extension().map(|e| e == "rs").unwrap_or(false);
        while matches!(reste.first().map(|s| s.as_str()), Some("src") | Some("static") | Some("extensions"))
            || (reste.len() > 1 && reste[0] == id)
        {
            reste.remove(0);
        }
        if reste.is_empty() {
            continue;
        }
        let cible = if vers_src {
            racine_src.join(reste.join("/"))
        } else {
            racine_static.join(reste.join("/"))
        };
        if let Some(parent) = cible.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?} : {}", parent, e))?;
        }
        let mut contenu = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut contenu)
            .map_err(|e| format!("Lecture de {:?} : {}", interne, e))?;
        std::fs::write(&cible, &contenu).map_err(|e| format!("Ecriture {:?} : {}", cible, e))?;
        ecrits.push(cible.to_string_lossy().to_string());
    }
    if ecrits.is_empty() {
        return Err("Archive vide ou sans fichier exploitable.".into());
    }
    Ok(ecrits)
}
