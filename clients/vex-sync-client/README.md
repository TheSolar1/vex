# VEX Sync Client (v3)

Synchronise un ou plusieurs dossiers locaux (Windows) avec `fchier` sur
un serveur VEX, **récursivement**, en reproduisant fidèlement :
- l'authentification **SRP-6a** du serveur (`src/srp.rs` / `login.html`) ;
- le chiffrement de fichiers **AES-256-GCM + HKDF** (`static/crypto.js`).

Le mot de passe en clair n'est **jamais écrit sur disque**.

## Ce qui a changé depuis la v2 : plus de console

L'appli tourne maintenant **sans fenêtre de commande** (`windows_subsystem
= "windows"`). Toute l'interaction passe par :
- une **page web locale stylée** (servie par l'appli elle-même sur
  `127.0.0.1:<port aléatoire>`, ouverte automatiquement dans le
  navigateur par défaut à chaque lancement) pour se connecter et choisir
  les dossiers à synchroniser — **Documents, Images, Vidéos, Musique,
  Bureau, Téléchargements** détectés automatiquement (vrais chemins
  Windows via `directories::UserDirs`), plus un bouton **"Parcourir…"**
  (sélecteur de dossier natif via `rfd`, pas de chemin à taper à la main) ;
- une **icône dans la barre des tâches** une fois connecté : "Ouvrir VEX
  Sync" (rouvre la page, sert aussi à reconfigurer les dossiers),
  "Synchroniser maintenant", "Quitter".
- chaque dossier configuré est automatiquement **épinglé aux "Accès
  rapides" de l'Explorateur** (même verbe Shell.Application qu'utilise
  Explorer pour "Épingler aux accès rapides" au clic droit — pas une
  vraie extension d'espace de noms comme OneDrive, mais l'approche
  réaliste et sans risque : aucun enregistrement COM/DLL/registre, juste
  l'API que l'Explorateur expose déjà pour ça). Vérifié en listant
  `shell:::{679f85cb-0220-4080-b29b-5540cc05aab6}` (le dossier virtuel
  "Accès rapides") après connexion : le dossier y apparaît bien.

Chaque dossier sélectionné devient un dossier de même nom à la racine de
fchier (créé automatiquement s'il n'existe pas). Plusieurs dossiers sont
synchronisés en parallèle, chacun avec son propre état de suivi (pas de
mélange entre "Documents" et "Images" si un fichier porte le même nom
dans les deux).

Comme avant, le mot de passe n'est jamais enregistré : il est redemandé
via la page web à chaque lancement de l'appli (identique au comportement
console précédent, juste dans un formulaire au lieu d'un prompt).

Testé de bout en bout (`src/bin/guitest.rs`) : lancement de l'appli,
requête HTTP sur `/api/connecter` comme le ferait la page web, vérifie
que le dossier distant est créé et qu'un fichier déposé *avant* la
connexion est bien remonté par la première synchro automatique — succès
contre le serveur réel.

Bug trouvé et corrigé en vérifiant visuellement (capture d'écran) que la
page s'ouvrait vraiment : `cmd /C start` combiné à `CREATE_NO_WINDOW`
(nécessaire pour ne jamais flasher de fenêtre console) échouait
SILENCIEUSEMENT à lancer le navigateur — le "start" interne de cmd.exe a
besoin d'une console pour fonctionner. Remplacé par `explorer.exe <url>`
(programme GUI normal, compatible avec `CREATE_NO_WINDOW`), revérifié
par capture d'écran : la page s'affiche bien, thème sombre correct,
champs pré-remplis.

## Fonctionnalités héritées de la v2

- **Sous-dossiers** : toute l'arborescence de fchier est synchronisée,
  pas seulement la racine (création automatique du dossier manquant,
  dans un sens comme dans l'autre).
- **Détection de conflit réelle** : un état persistant
  (`%APPDATA%/vex/vex-sync-client/vex-sync-state.json`) retient, pour
  chaque fichier, sa taille/date de modification locale et la date
  connue côté serveur au moment de la dernière synchro réussie. Ça
  permet de distinguer "rien n'a changé", "un seul côté a changé"
  (transfert normal) et "les deux côtés ont changé" (vrai conflit).
  En cas de conflit, **le fichier local n'est jamais écrasé** : la
  version serveur est téléchargée à côté sous forme de copie
  `nom (conflit-serveur-<horodatage>).ext`.
- **Suppression réellement bidirectionnelle** : un fichier déjà
  synchronisé au moins une fois, qui disparaît d'un côté (supprimé
  localement, ou via l'interface web), est supprimé de l'autre côté au
  prochain passage. ⚠️ Ça change le comportement par rapport à avant :
  supprimer un fichier local synchronisé ne le "redemande" plus au
  serveur, ça le supprime aussi côté serveur.
- L'état de suivi est namespacé par **compte + nom de dossier** (pas
  seulement le nom) : reconfigurer l'appli avec un AUTRE compte VEX tout
  en réutilisant le même nom de dossier (ex. "Documents") ne risque plus
  de lire une baseline périmée d'un compte différent et de supprimer des
  fichiers locaux à tort. Bug trouvé et corrigé pendant les tests de la
  v3 (reproduit puis vérifié corrigé avec deux comptes jetables
  successifs sur le même mapping).
- Le moteur de synchro est maintenant dans `src/sync.rs` (testable
  indépendamment du CLI/tray) et validé par un test d'intégration réel
  (`src/bin/synctest.rs`) qui exerce sous-dossiers, conflit, et
  suppression dans les deux sens, contre le serveur en production avec
  un compte jetable.
- Correctif serveur associé (`../3/src/fchier/fchier.rs` +
  `db_init.rs`) : la colonne `date` des fichiers est passée de `DATE`
  (jour près) à `DATETIME` (seconde près), et `api_edit_content` la met
  à jour — sans ça, un conflit survenant le même jour qu'une synchro
  précédente aurait été invisible pour ce client.

## Limitations connues (v3)

- Au plus **100 fichiers par compte** (pas par dossier) : limite de
  l'API serveur `/api/fchier/data`, pas de pagination côté client.
- Pas encore de suppression pour les **dossiers** (seulement les
  fichiers).
- Pas d'icône de statut façon OneDrive sur les fichiers individuels
  (coche verte) — nécessiterait une vraie extension shell Windows
  (COM, `IShellIconOverlayIdentifier`, enregistrement DLL/registre) :
  hors de portée ici (impossible à tester/vérifier visuellement dans ce
  format de session, et risqué à déployer sans retour visuel). Les
  dossiers configurés sont épinglés aux Accès rapides à la place (voir
  plus haut) — visible, mais pas au niveau fichier.
- Trouvé en cours de route, pas corrigé (hors sujet de ce projet) :
  `/autologin/api/generer` côté serveur VEX revérifie le mot de passe
  via l'ancien hash bcrypt/MD5 (`motdepass`), qui est vide pour tout
  compte créé via le flux SRP actuel — l'autologin ne peut donc jamais
  être activé sur un compte moderne. Bug serveur préexistant, pas
  provoqué par ce client.

## Utilisation

```
cargo run --release
```

Une page s'ouvre dans le navigateur : adresse du serveur, email, mot de
passe, sélection des dossiers. Une fois connecté, tout se passe en tâche
de fond (icône dans la barre des tâches). Chaque dossier est surveillé
récursivement (`notify`), et le serveur est revérifié en entier toutes
les 60 secondes.

## Tests

```
cargo run --release --bin selftest -- https://vex.hopto.org
cargo run --release --bin synctest -- https://vex.hopto.org
cargo run --release --bin guitest  -- http://serveur:port <port-local-affiche-au-lancement>
```

Les trois créent un compte jetable (`*-@example.invalid`), l'utilisent
pour valider le protocole de bout en bout, et le laissent en base
(nettoyage manuel via l'admin VEX si besoin). `synctest` couvre
sous-dossiers/conflit/suppression ; `guitest` couvre la page de
configuration locale et la première synchro automatique après connexion
(lancer `vex-sync-client` d'abord, relever le port dans son fichier de
log, puis lancer `guitest` avec ce port).

## Architecture

- `src/srp.rs` — port du protocole SRP-6a (identique à `../3/src/srp.rs`).
- `src/filecrypto.rs` — port du chiffrement fichier (`../3/static/crypto.js`).
- `src/api.rs` — client HTTP (login, dossiers/fichiers fchier, requêtes
  authentifiées génériques pour diagnostic).
- `src/sync.rs` — moteur de synchronisation récursive + conflits +
  suppressions, indépendant du reste (testable via `src/bin/synctest.rs`).
- `assets/setup.html` — page de configuration (autonome, aucune
  ressource externe).
- `src/main.rs` — pas de console (`windows_subsystem = "windows"`) ;
  serveur HTTP local (`tiny_http`) servant la page de config, icône
  barre des tâches, surveillance récursive multi-dossiers (`notify`),
  poll périodique (60s), journal dans `%APPDATA%/vex/vex-sync-client/`.
