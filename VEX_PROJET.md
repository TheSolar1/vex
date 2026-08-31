# VEX — Description du projet

## Vue d'ensemble

VEX est un serveur web auto-hébergé écrit en Rust, alternative légère à Nextcloud. Fichiers (ExoDrive), utilisateurs, admin, messagerie chiffrée, appels vidéo, P2P inter-nœuds. Chiffrement client-side : le serveur ne voit jamais les données en clair.

**Version actuelle :** alpha-0.3
**Langage :** Rust (serveur), HTML/JS/CSS (client)
**Base de données :** MySQL 8 / MariaDB
**Framework HTTP :** tiny-http
**Port par défaut :** 8080

---

## Architecture des fichiers

```
/
├── Cargo.toml / Cargo.lock
├── config.json          — Config serveur (DB, sécurité, plans, extensions)
├── static/               — Fichiers statiques (HTML, CSS, JS, images)
└── src/                  — Code source Rust
```

### Source Rust (`src/`)

| Fichier | Rôle |
|---|---|
| `main.rs` | Point d'entrée. Init DB, pool MySQL, nœud P2P, serveur HTTP, routage global. |
| `config_loader.rs` | Charge/sauvegarde `config.json` (`VexConfig`, `DbConfig`). |
| `appeldb.rs` | Couche DB unique — `selectionner()`, `inserer_ou_modifier()`, `supprimer_ligne()`, `compter_lignes()`, `verifier_connexion()`. 0 SQL brut ailleurs. |
| `db_init.rs` | Création auto de la base + tables au démarrage, migrations `ADD COLUMN`. |
| `c.rs` | Sessions : `verifier_session()` (cookie+IP+UA+expiration), `verifier_blocage()`, `is_recent_local()`. |
| `access_control.rs` | Lecture cookies/headers HTTP. |
| `function.rs` | `build_nav_html()` (nav standard + grille d'apps, cf. capture "Accueil / Mail / Exodrive / Éditeur de fichiers / Vidéos / Sitec / Administration"), `get_theme_attr()`, `get_privilege_details()`, `html_escape()`. |
| `utils.rs` | `strip_port()`, `parse_query()`, `url_decode()`. |

### Modules applicatifs (`src/<module>/`)

| Module | Fichier | Rôle |
|---|---|---|
| Connexion | `login/login.rs` | Connexion/inscription. MDP jamais en clair (PBKDF2-SHA256 côté client, 100k itérations). |
| Connexion | `login/first_setup.rs` | Création du compte admin initial. |
| Connexion | `login/dashboard.rs` | Accueil post-connexion (stats, widgets). |
| Connexion | `login/account.rs` | Compte utilisateur (MDP, préférences). |
| Connexion | `login/autologin.rs` | Connexion par token URL. |
| Connexion | `login/logout.rs` | Déconnexion (purge `loginc`). |
| ExoDrive | `fchier/fchier.rs` | Fichiers : upload (AES-256-GCM client), download, dossiers, renommage, déplacement, partage, envoi P2P. |
| Messagerie | `mess.rs` | Messagerie E2E (ECDH), gestion sessions via `c::verifier_session()`. |
| Visio | `viso.rs` | Appels audio/vidéo. Serveur = relais de signalisation uniquement (SDP/ICE chiffrés AES-256-GCM via ECDH P-256+HKDF par paire, flux DTLS-SRTP direct). Voir section dédiée ci-dessous. |
| Admin | `admin/admin.rs` | Panneau admin : utilisateurs, blocages, SQL runner, config, extensions, rôles/plans, serveur, logs, OnlyOffice, P2P. Accessible privilege ≤ 2. |
| P2P | `p2p/p2p.rs` | Nœuds VEX inter-instances, clés Ed25519, sync bootstrap (`vex.hopto.org`), support Tor optionnel. |
| Sitec | `sitec.rs` *(à confirmer)* | Créateur de sites (pages `sitec` / `sitecdos`). |

### Fichiers statiques (`static/`)

| Dossier | Contenu |
|---|---|
| `static/login/` | `login.html`, `dashboard.html`, `account.html`, `autologin.html`, `logout.html`, `first_setup.html` |
| `static/fchier/` | `fchier.html` — ExoDrive |
| `static/mess/` | `mess.html` — messagerie |
| `static/viso/` | `viso.html` — appels vidéo (grille de tuiles style Discord) |
| `static/admin/` | Interface admin (sidebar + sections dynamiques JS) |
| `static/img/` | Icônes SVG (Font Awesome solid), favicon, logo |
| `static/css/` | Styles globaux |
| `static/js/` | `crypto.js` — PBKDF2, AES-256-GCM, HKDF côté client |

---

## Routes HTTP (vue d'ensemble)

Toutes les pages suivent le même schéma : `GET /<page>` → rend le HTML avec nav injectée (`{{NAV_HTML}}`) + thème (`{{THEME}}` / `data-theme`) ; `POST /api/<page>` → routeur d'actions JSON (`action=...`), 0 SQL hors `appeldb.rs`.

| Page (grille d'apps) | Route HTML | Route API | Base apps ? |
|---|---|---|---|
| Accueil | `GET /dashboard` | `POST /api/dashboard` | oui |
| Mail | `GET /vexmail` | `POST /api/vexmail` | oui |
| Exodrive | `GET /fchier` | `POST /api/fchier` | oui |
| Éditeur de fichiers | `GET /mess` (ou dédié OnlyOffice) | `POST /api/mess` | oui |
| Vidéos (Visio) | `GET /viso`, `GET /viso/` | `POST /api/viso` | oui |
| Sitec | `GET /sitec` | `POST /api/sitec` | à confirmer |
| Administration | `GET /admin` | `POST /api/admin/*` (sous-routes : `/dashboard`, `/users`, `/blocks`, `/db/*`, `/config`, `/extensions`, `/roles`, `/server`, `/logs`, `/onlyoffice`, `/p2p/*`) | oui |

`BASE_APPS` (non suppressibles, cf. `admin.rs`) : `meet` (viso), `onlyoffice`, `sitec`, `vexmail`, `mess`, `p2p`.
→ **`viso` doit être ajouté à `extension_params` dans `config.json` au même titre que les autres apps de base**, avec `privilege_min` et `plans_autorises`, sinon il n'apparaîtra pas correctement géré dans l'onglet Extensions du panneau admin.

---

## Module Visio (`viso.rs` / `viso.html`)

- **Modèle de sécurité** : le serveur relaie uniquement des signaux chiffrés (offer/answer/ice/state/bye), jamais le flux média ni les SDP en clair.
- **Crypto** : ECDH P-256 éphémère par appareil + HKDF-SHA256 → clé AES-256-GCM partagée par paire de participants.
- **Auth découplée** (`auth_viso` / `session_active`) : les actions de signalisation en boucle (poster_signal, recuperer_signaux, heartbeat, lister_participants) ne revalident que `session_id` actif, pas IP/UA strict, pour éviter les faux "Session invalide" en cas de changement réseau.
- **UI (mise à jour)** : grille de tuiles façon Discord — fond sombre uniforme, avatar rond avec initiales quand la caméra est coupée, nom + icône micro en overlay bas-gauche, compteur de participants dans l'en-tête, purge automatique des tuiles fantômes (participant parti sans signal `bye`), état micro/caméra propagé aux autres participants via un nouveau type de signal `state`.
- **Partage d'écran** : piste ajoutée via `addTrack` dédié + `onnegotiationneeded` gère la renégociation automatiquement (plus de conflit avec le sender caméra).

---

## Base de données

| Table | Rôle |
|---|---|
| `login` | Comptes (nom, email, hash PBKDF2, privilege, vip) |
| `loginc` | Sessions actives (cookie, IP, UA, date création) |
| `autologin` | Tokens d'autologin |
| `fichiers` | Fichiers ExoDrive (contenu chiffré, type, taille, visibilité, partage) |
| `pref` | Préférences (thème, langue, style nav, icône profil) |
| `tag-user` | Restrictions par utilisateur |
| `bloqpage` | Blocages de pages par privilege |
| `sus-hac` | Log activités suspectes |
| `sitec` / `sitecdos` | Pages / dossiers Sitec |
| `p2p_peers` / `p2p_nodes` / `p2p_users` / `p2p_messages` / `p2p_transfers` | P2P inter-nœuds |
| `meet_rooms` | Salles d'appel Visio (code, titre, mdp, max participants) |
| `meet_participants` | Participants actifs par salle (session_id, clé publique ECDH) |
| `meet_signaling` | File de signaux chiffrés (offer/answer/ice/state/bye), purgée après lecture / 10 min |
| `conxiont` | (Réservé) connexions externes |

---

## Système de privileges

| Niveau | Rôle |
|---|---|
| 1 | Fondateur (masqué de toutes les listes admin) |
| 2 | Super-admin (SQL runner, accès total) |
| 3 | Admin |
| 4-5 | Modérateur / super-modérateur |
| 6-9 | Utilisateur avancé / certifié / bêta-testeur |
| 10 | Utilisateur standard |
| 11-12 | Banni |

Noms de rôles personnalisables dans Admin → Rôles & Plans (`config.privilege_labels`).

---

## Chiffrement (côté client uniquement)

- **Auth** : PBKDF2-SHA256(mdp, email, 100 000 itérations) → hash hex 64 car.
- **Fichiers** : AES-256-GCM, IV 96 bits dérivé (HKDF) de l'ID fichier + email, pas d'IV stocké.
- **Messagerie** : E2E via ECDH.
- **Visio** : ECDH P-256 + HKDF par paire → AES-256-GCM pour la signalisation ; flux média en DTLS-SRTP direct navigateur↔navigateur.
- Le serveur ne peut déchiffrer ni fichiers, ni messages, ni signalisation d'appel.

---

## Lancer le serveur

```
cargo run
```

Init auto de la base MySQL via `db_init.rs`. Config DB par défaut (`config_loader.rs`), surchargeable via section `"db"` dans `config.json` :
- host: localhost, port: 3306
- user: orsql, password: iDq]25F0u8v*z[1d
- database: user

---

## TODO / à faire suite à cette mise à jour

- [ ] Ajouter `viso` dans `config.json → extensions.extension_params` (voir tableau routes ci-dessus).
- [ ] Confirmer le nom de fichier réel du module Sitec (`sitec.rs` supposé).
- [ ] Vérifier que `build_nav_html()` génère bien la tuile "Vidéos" pointant vers `/viso` (grille d'accueil, cf. capture app).
- [ ] Tester `viso.html` mis à jour (tuiles Discord-style) en conditions réelles multi-participants.