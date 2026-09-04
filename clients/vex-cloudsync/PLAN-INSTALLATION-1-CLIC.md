# Plan — Installation "un clic" de vex-cloudsync depuis le site VEX

## Objectif (décrit par l'utilisateur)

Depuis l'interface web VEX (fchier), un bouton "Synchroniser" / "Se
connecter" qui :
1. Télécharge un exécutable **déjà configuré** avec la bonne URL du
   serveur VEX (l'utilisateur n'a rien à taper).
2. L'utilisateur lance l'exécutable, qui tourne en tâche de fond
   (démarrage automatique).
3. L'app se connecte, ouvre une page **sur le vrai serveur VEX**
   (`vex.hopto.org`, pas une page locale comme le fait
   `vex-sync-client`).
4. L'utilisateur approuve la connexion sur cette page.
5. C'est prêt — la synchro démarre.

C'est un changement de modèle par rapport à `vex-sync-client` (qui a sa
propre page de connexion **locale**, avec email/mot de passe tapés à la
main à chaque lancement) : ici, l'autorisation passe par le **serveur**,
pas par un formulaire local.

## Composants à construire

### 1. Packaging de l'exécutable "pré-configuré"

Le plus simple : ne PAS générer un exécutable différent par utilisateur.
À la place, l'exécutable générique (déjà compilé une fois) lit l'URL du
serveur depuis un petit fichier texte/JSON **livré à côté de lui** au
téléchargement (ex. `vex-cloudsync.exe` + `config.json` contenant
`{"base_url": "https://vex.hopto.org"}`), généré dynamiquement par le
serveur au moment du téléchargement (une route `/download/vex-cloudsync`
qui sert un `.zip` contenant les deux fichiers). Évite de recompiler un
binaire par utilisateur — inutile et lent.

### 2. Authentification par jeton, pas par mot de passe local

Le vrai changement de modèle. Au lieu de refaire un login SRP-6a complet
dans l'app desktop (mot de passe tapé localement), flux type "device
authorization" (comme utilisé par GitHub CLI, Docker Desktop, etc.) :

1. Au premier lancement, l'app génère un identifiant d'appareil aléatoire
   et l'affiche/l'envoie, puis ouvre
   `https://vex.hopto.org/autoriser-appareil?code=XXXXX` dans le
   navigateur par défaut de l'utilisateur (déjà logué sur le site).
2. **Nouveau endpoint serveur à créer** : une page qui affiche "Un
   appareil demande à accéder à tes fichiers VEX. Autoriser ?" avec
   Oui/Non — nécessite une session utilisateur active (donc protégé par
   l'auth existante du site).
3. Si l'utilisateur clique "Oui", le serveur génère un jeton d'accès
   longue durée pour ce device (à la manière du token autologin —
   **attention, ne pas réutiliser tel quel le mécanisme autologin
   existant : il a le bug bcrypt/MD5 documenté dans
   `vex-sync-client/README.md`, jamais corrigé ce soir**), et le
   marque "approuvé" côté serveur.
4. L'app desktop, qui poll un endpoint du style
   `/api/appareil/statut?code=XXXXX` toutes les 2-3 secondes, détecte
   l'approbation et récupère son jeton.
5. Toutes les requêtes suivantes (`lister_dossier`, `telecharger`, etc.)
   utilisent ce jeton au lieu d'un cookie de session classique — **nouveau
   mécanisme d'auth à ajouter côté serveur VEX**, distinct de SRP et
   distinct de l'autologin existant.

### 3. Démarrage automatique

Ajouter une entrée au démarrage Windows (`HKCU\Software\Microsoft\Windows\
CurrentVersion\Run`, pas besoin d'admin) pointant vers l'exécutable —
mécanisme standard, faible risque, à faire une fois l'auth par jeton en
place.

## Pourquoi c'est un gros morceau

- Nouveau mécanisme d'authentification **côté serveur** (jeton d'appareil)
  entièrement à concevoir et sécuriser — surface d'attaque nouvelle,
  demande une vraie réflexion sécurité (durée de vie du jeton, révocation,
  qu'est-ce qui se passe si le code est intercepté, etc.), pas juste du
  code.
- Nouvelle route de téléchargement + génération de bundle côté serveur.
- Nouvelle page web d'approbation d'appareil.
- Modifier `VexClient` (ou créer une variante) pour s'authentifier par
  jeton plutôt que SRP.
- Démarrage automatique Windows (petit, mais encore un point de contact
  avec le système à tester prudemment comme tout le reste ce soir).

## Ordre recommandé pour une session dédiée

1. Concevoir et sécuriser le mécanisme de jeton d'appareil côté serveur
   D'ABORD (le plus risqué et le plus structurant) avant tout code
   desktop.
2. Endpoint serveur : générer/valider/révoquer un jeton d'appareil.
3. Page web d'approbation (réutilise l'auth de session existante).
4. Route de téléchargement du bundle.
5. Côté `vex-cloudsync` : remplacer le login SRP par le flux de polling
   + jeton.
6. Démarrage automatique Windows, en dernier (le plus simple des points).

## État au moment de la rédaction

- Rien codé pour cette partie — uniquement ce plan.
- `vex-cloudsync` (Cloud Files API) et son icône/raccourci Bureau restent
  utilisables tels quels en attendant — ce plan ne les remplace pas, il
  change juste COMMENT l'utilisateur se connecte.
