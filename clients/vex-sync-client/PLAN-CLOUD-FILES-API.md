# Plan — Intégration de l'API Windows Cloud Files dans vex-sync-client

## Objectif

Remplacer le modèle actuel de vex-sync-client (copie complète des fichiers en
local, synchronisation par comparaison de taille/date) par le même modèle que
OneDrive/Dropbox/Google Drive : des **fichiers-fantômes** (placeholders)
visibles dans l'Explorateur sans être vraiment téléchargés, une colonne
**"Statut"** qui apparaît automatiquement, et un téléchargement **à la
demande** dès qu'un fichier est ouvert.

C'est un projet à part entière — plusieurs jours de travail, pas une suite
rapide d'une soirée. Ce document sert de point de départ pour une session
dédiée, qui que ce soit qui la mène.

## Pourquoi ce n'est pas juste "une colonne à ajouter"

`IColumnProvider` (l'API "simple" pour ajouter une colonne à l'Explorateur)
a été retirée par Microsoft depuis Windows Vista. La colonne "Statut" que
montre OneDrive n'est pas un composant à part — elle apparaît automatiquement
dès qu'un dossier est enregistré comme "racine de synchro" via l'API Cloud
Files. Il n'y a pas de raccourci : pour avoir la colonne, il faut toute
l'architecture placeholder/hydratation.

## Prérequis techniques

- **Windows 10 version 1709 (build 16299) ou supérieur** — l'API Cloud Files
  n'existe pas avant.
- Crate Rust : [`cloud-filter`](https://crates.io/crates/cloud-filter) (fork
  actif de `wincs`, dépôt : https://github.com/ho-229/cloud-filter-rs).
  Wrapper sûr/idiomatique au-dessus de l'API Windows brute
  (`windows::Win32::Storage::CloudFilters`).
- Exemple de référence à étudier en premier :
  `cloud-filter-rs/examples/sftp.rs` (~350 lignes) — synchronise un serveur
  SFTP distant avec le même modèle placeholder. La structure est presque
  directement transposable à VEX (remplacer les appels SFTP par les appels
  déjà existants dans `src/api.rs`).

## Ce qui doit changer dans vex-sync-client

L'architecture actuelle (`sync.rs`, comparaison taille/date, copie complète)
devient obsolète pour les dossiers gérés en mode Cloud Files — ce sera un
**mode différent**, pas une évolution de l'existant :

1. **Enregistrement de la racine de synchro** (une fois, à la connexion) :
   `CfRegisterSyncRoot` avec un identifiant de fournisseur unique ("VEX"),
   nom d'affichage, icône. Voir `CF_SYNC_REGISTRATION` /
   `CF_SYNC_POLICIES` dans le crate.

2. **Création des placeholders** : pour chaque fichier/dossier distant
   (`client.lister_dossier()` déjà existant), créer un placeholder local
   via le crate (pas d'écriture du contenu réel — juste métadonnées : nom,
   taille, date, identifiant de fichier distant en `file_id` opaque).

3. **Implémenter `SyncFilter`** (13 méthodes de rappel) — mapping proposé
   vers le code existant :
   | Méthode du trait | Ce qu'elle doit faire pour VEX |
   |---|---|
   | `fetch_data` | `client.telecharger(id)` (déjà écrit) + déchiffrement (`filecrypto::dechiffrer_fichier`, déjà écrit), renvoyé par blocs |
   | `fetch_placeholders` | `client.lister_dossier(id)` (déjà écrit) pour peupler un sous-dossier ouvert la première fois |
   | `validate_data` | Vérifier la taille reçue contre celle annoncée par `FileEntry` |
   | `cancel_fetch_data` / `cancel_fetch_placeholders` | Annulation propre, libérer les ressources |
   | `delete` / `deleted` | `client.supprimer_fichier(id)` (déjà écrit, ajouté pour la suppression bidirectionnelle) |
   | `rename` / `renamed` | Pas d'équivalent direct dans `api.rs` actuellement — **à ajouter** (endpoint `/api/fchier/rename` existe côté serveur, non exposé cote client Rust) |
   | `dehydrate` / `dehydrated` | Rien de special a faire cote VEX (le fichier redevient "cloud-only") |
   | `opened` / `closed` | Logging/statut uniquement pour V1 |
   | `state_changed` | Logging/statut uniquement pour V1 |

4. **Chiffrement/déchiffrement en flux** : contrairement au modèle actuel
   (fichier entier en mémoire), `fetch_data` doit répondre par blocs
   (l'exemple SFTP aligne sur 4096 octets). Le chiffrement VEX actuel
   (`filecrypto.rs`) traite le fichier entier d'un coup (AES-256-GCM,
   authentifié globalement) — **incompatible tel quel avec un flux par
   blocs**. Deux options a trancher en debut de session dediee :
   - (a) Télécharger tout le blob chiffré d'un coup en interne (comme
     aujourd'hui), le déchiffrer entièrement en mémoire, puis ne renvoyer
     que des tranches de ce buffer déchiffré au fur et à mesure que
     `fetch_data` est appelé (simple, mais perd l'intérêt "vrai flux" pour
     les gros fichiers).
   - (b) Passer à un chiffrement par blocs authentifiés séparément
     (nécessiterait de changer le format de blob VEX cote serveur/web
     aussi — gros impact, à éviter pour une V1).
   Recommandation : commencer par (a), c'est suffisant pour valider
   l'architecture avant d'optimiser.

5. **Boucle de connexion** (`CfConnectSyncRoot`) tournant en tâche de fond,
   en plus (pas à la place) de la boucle de poll existante — les nouveaux
   fichiers distants doivent quand même être détectés côté serveur
   (`fetch_placeholders` ne se déclenche que quand l'utilisateur ouvre un
   dossier ; il faut un mécanisme séparé pour faire apparaître les
   nouveaux fichiers sans que l'utilisateur ouvre le dossier — reprendre
   le poll 60s existant pour appeler la création de placeholders sur les
   dossiers déjà connus).

## Étapes concrètes pour démarrer

1. `cargo add cloud-filter` dans `vex-sync-client` (ou un nouveau crate
   séparé `vex-cloudsync`, comme `vex-overlay` a été séparé — même logique
   de risque : composant qui touche à Explorer/le système de fichiers,
   à tester isolément avant tout usage réel).
2. Compiler et lancer `examples/sftp.rs` du crate tel quel (avec un vrai
   petit serveur SFTP de test, ou adapté minimalement) pour comprendre le
   cycle de vie réel AVANT de commencer à adapter à VEX.
3. Écrire un module `src/cloudsync.rs` calqué sur `sftp.rs`, en remplaçant
   les appels SFTP par `VexClient` (déjà tout écrit dans `api.rs`).
4. Tester avec UN SEUL dossier, quelques fichiers, avant de brancher sur
   la configuration multi-dossiers existante.
5. Ajouter la désinscription propre (`CfUnregisterSyncRoot`) au moment de
   la déconnexion/désinstallation — sans ça, Windows garde le dossier
   enregistré même après la fermeture de l'app.

## Risques identifiés (à traiter avec la même prudence que l'overlay COM)

- Composant chargé/actif tant que la racine est enregistrée — un bug dans
  les callbacks peut geler des opérations Explorer sur ce dossier précis
  (moins grave qu'un plantage d'Explorer entier, mais gênant).
- Toujours tester en isolation (dossier de test jetable, jamais sur
  Documents/Images réels) avant tout usage sur des données importantes.
- Prévoir un mécanisme de "sortie de secours" : `CfUnregisterSyncRoot` +
  suppression du dossier placeholder si quelque chose tourne mal, documenté
  clairement (comme le nettoyage overlay fait ce soir).

## État au moment de la rédaction de ce plan

- **Confirmé visuellement par capture d'écran** : la colonne "Statut" avec
  icônes nuage apparaît automatiquement dans l'Explorateur, exactement comme
  OneDrive, sans code dédié — gratuit avec l'enregistrement de la racine.
- **Premier prototype écrit et testé avec succès** dans `vex-cloudsync/`
  (nouveau crate séparé, dépend de `vex-sync-client` en path pour réutiliser
  `VexClient`/`filecrypto`/`srp` tels quels).
- Testé de bout en bout contre le serveur réel, avec un compte jetable et un
  dossier local jetable : racine de synchro enregistrée, `fetch_placeholders`
  déclenché automatiquement par Windows, placeholder créé, `fetch_data`
  déclenché à l'ouverture du fichier, contenu téléchargé ET déchiffré
  (AES-256-GCM via le code existant) correctement du premier coup. Explorer
  resté sain (`Responding: True`) pendant tout le test.
- Simplifications encore en place (voir haut de `vex-cloudsync/src/main.rs`) :
  `rename`/`delete` de dossier renvoient `NotSupported` (méthodes manquantes
  côté `VexClient`, faciles à ajouter), fichier entier déchiffré en mémoire
  avant découpage en tranches (option (a) du plan).
- **Compatibilité** : vérification de la version Windows au démarrage
  (build >= 16299 requis), message clair si trop ancien au lieu d'un échec
  cryptique. Testé sur build 26200 : détection correcte.
- **`mark_in_sync` implémenté et testé** : un dossier local qui contient
  déjà de vrais fichiers (pas vide) est maintenant correctement pris en
  compte au démarrage — conversion en placeholders sans dupliquer ni
  corrompre le contenu (vérifié avec 2 fichiers + 1 sous-dossier
  pré-remplis). La colonne "Statut" reflète correctement l'état réel :
  coche verte pour un fichier déjà ouvert/hydraté, nuage pour les autres.
- **Icône VEX personnalisée** générée et enregistrée
  (`vex-cloudsync/vex-icon.ico`, carré vert arrondi avec "V" blanc,
  cohérent avec le logo de la page de configuration web) — enregistrement
  sans erreur, mais pas visible dans la liste de fichiers elle-même (les
  icônes de fichiers restent génériques par type ; l'icône de fournisseur
  apparaît ailleurs dans Windows, ex. paramètres de stockage — pas vérifié
  plus loin).
- **Confirmé : pas de présence dans la barre latérale de l'Explorateur**
  (contrairement à OneDrive) — un sync root Cloud Files enregistré sur un
  chemin arbitraire (ex. sous Documents) n'obtient pas d'entrée dédiée
  dans le panneau de navigation. La présence de OneDrive dans la barre
  latérale vient d'un mécanisme séparé (dossier connu / bibliothèque),
  pas d'un effet automatique de l'API Cloud Files. La demande initiale
  ("espace VEX dans la barre latérale") reste donc non résolue par cette
  approche.
- **Raccourci Bureau ajouté et vérifié visuellement** (capture d'écran) :
  `creer_raccourci_bureau()` pose un `VEX.lnk` sur le Bureau au démarrage,
  avec l'icône VEX personnalisée — mécanisme standard (COM
  `WScript.Shell`), sans registre système ni droits admin, exactement ce
  que fait n'importe quel logiciel à l'installation. Confirmé à l'écran :
  icône verte "V" bien visible et intégrée parmi les autres raccourcis.
  Ce n'est pas une entrée de barre latérale (voir point précédent), mais
  ça répond à la demande "une icône sur le Bureau à côté de Ce PC" (la
  position exacte n'est pas contrôlable, Windows range lui-même la grille).
- **Pas encore testé** : le sens montée (créer un fichier local et le voir
  apparaître côté serveur) — l'API Cloud Files ne fournit pas de callback
  direct pour "nouveau fichier local détecté" ; ça nécessite de combiner
  avec la surveillance `notify` déjà existante dans `sync.rs` (regarder les
  nouveaux fichiers *réels* — pas les placeholders — et les uploader via
  `client.uploader_dans()`, déjà écrit).
- **Outillage installation/désinstallation** (`src/bin/status.rs`,
  `src/bin/uninstall.rs`) :
  - `status` : vérifie sans rien modifier si la racine de synchro est
    enregistrée et si le raccourci Bureau existe (avec sa cible).
  - `uninstall` : désinscrit la racine + retire le raccourci. **Exige le
    chemin du dossier en argument et refuse de continuer si une copie de
    secours préalable échoue.**
  - **Découverte importante, testée et comprise** : désinscrire une racine
    de synchro (`CfUnregisterSyncRoot`) supprime les placeholders locaux
    qui n'ont **jamais été téléchargés** (encore "nuage") — logique une
    fois comprise : un placeholder non hydraté n'a aucune donnée réelle
    sur le disque, ce n'est qu'une référence vers le fournisseur ; si le
    fournisseur (le processus `vex-cloudsync`) est déjà arrêté, cette
    référence ne peut plus être résolue et Windows la retire. **Ce n'est
    pas une perte de données** au sens propre : le contenu réel reste sur
    le serveur VEX (la source de vérité), récupérable en resynchronisant.
    Un fichier déjà hydraté (coche verte, ouvert au moins une fois) N'EST
    PAS affecté et survit à la désinscription tel quel.
  - Conséquence pratique implémentée dans `uninstall` : avant de
    désinscrire, copie tout le dossier local vers `<dossier>-backup-<ts>`.
    Si le fournisseur est déjà mort, copier un fichier jamais hydraté
    échoue explicitement (erreur claire, testée : "le fournisseur de
    fichiers cloud s'est fermé de manière inattendue") — dans ce cas
    `uninstall` **annule** la désinscription plutôt que de continuer en
    silence. Pour désinstaller proprement y compris les fichiers jamais
    ouverts, il faut que `vex-cloudsync` tourne encore au moment de lancer
    `uninstall` (testé et confirmé : la copie réussit alors).
- Ce qui existe et reste réutilisable tel quel : `api.rs` (VexClient complet,
  déjà testé), `filecrypto.rs` (chiffrement/déchiffrement), `srp.rs`.
- Le composant `vex-overlay` (icône de surcouche, terminé et fonctionnel,
  voir son propre dossier) devient probablement inutile si ce chantier
  aboutit : l'API Cloud Files fournit sa PROPRE colonne de statut et ses
  propres icônes, sans besoin d'un overlay séparé.
