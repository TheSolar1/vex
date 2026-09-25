# Journal des modifications — VEX

## alpha-0.4 (0.4.0) — 2026-09-25

### Nouveautés
- **ExoDrive — corbeille** : les fichiers et dossiers supprimés sont restaurables pendant 30 jours. Un dossier part et revient avec tout son contenu.
- **ExoDrive — lien de partage public** chiffré de bout en bout (la clé est dans le `#` du lien, jamais envoyée au serveur), avec mot de passe, date d'expiration et nombre maximal de téléchargements.
- **ExoDrive — aperçu** des images, PDF, vidéos, sons et textes, déchiffrés dans le navigateur (flèches ← → pour naviguer).
- **ExoDrive — favoris, récents, export ZIP** d'un dossier ou de tout le drive (déchiffré dans le navigateur).
- **Messagerie** : accusés de lecture (✓ Envoyé / ✓✓ Lu), restauration depuis la corbeille, notifications (badge, titre d'onglet, notification bureau).
- **Visio** : serveurs STUN/TURN configurables (`config.json` → `visio`), identifiants TURN temporaires compatibles coturn ; un appel survit aux micro-coupures réseau (redémarrage ICE).
- **Interface** : thème « Automatique » (suit l'appareil), raccourcis clavier (Ctrl+K ou `/` : recherche, `?` : aide), page 404, meilleur affichage mobile d'ExoDrive, contour de focus clavier.
- **Admin** : sauvegardes automatiques avec rotation, retour arrière **automatique** si une mise à jour ne démarre pas (et bouton de retour manuel), nettoyage des builds (`target/debug`).
- **Logs** : IP réelle du client derrière un proxy, journal d'accès séparé (`log/acces_<date>.log`), filtre et choix du journal dans Admin > Logs, alerte sur les requêtes lentes.

### Performances
- Serveur HTTP **multi-thread** (8 threads par défaut, `server.threads`) : une requête lente ne bloque plus tout le monde.
- Compression **gzip** des pages, CSS, JS et JSON (ex. ExoDrive 121 Ko → 29 Ko).
- Cache navigateur par **ETag / 304**.
- **Index SQL** sur les sessions et les fichiers (la session était recherchée par parcours complet de table à chaque page).

### Corrections
- Dates des logs fausses de plusieurs semaines ; rotation quotidienne des logs inexistante.
- Barre de navigation blanche et illisible en thème clair.
- Supprimer un dossier laissait ses fichiers réapparaître à la racine ; les fichiers supprimés n'étaient jamais effacés du disque.
- Messagerie : supprimer un message côté expéditeur le supprimait aussi chez le destinataire.
- ExoDrive : au-delà de 100 fichiers, certains dossiers apparaissaient vides ; `is_owner` toujours faux ; menu contextuel coupé en bas d'écran.
- Tableau de bord jamais affiché en thème sombre.

### Sécurité
- Messagerie : toutes les requêtes SQL sont paramétrées (fin de l'échappement manuel).
- En-têtes `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy` sur les pages.
- Jetons d'autologin et de partage masqués dans les journaux.

### Migration
Automatique au démarrage : nouvelles tables `fchier_corbeille`, `fchier_liens`, `fchier_favoris` et nouveaux index. Aucune action manuelle requise.
