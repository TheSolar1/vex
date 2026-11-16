# Plume dans VEX (en test)

Copie compilée de la suite bureautique **Plume** (Texte, Présentation,
Tableur), servie telle quelle par VEX :

    https://<votre-vex>/static/plume/

Aucun lien n'y mène depuis l'interface de VEX : c'est une mise en test.

- Tout tourne dans le navigateur ; les documents restent sur l'appareil.
- Seule la **collaboration en temps réel** de Plume Texte passe par VEX :
  route `POST /api/plume/collab` (`src/plume/collab.rs`), réservée aux
  comptes connectés, en mémoire uniquement.
- Ne pas modifier ces fichiers à la main : ils sont générés depuis le
  dépôt `plume` (`node build.mjs`), puis copiés ici (`index.html`,
  `texte.html`, `presentation.html`, `tableur.html`, `dist/*.js`,
  `dist/*.css`).

Source : dépôt `plume`, commit 0dfaa75.
