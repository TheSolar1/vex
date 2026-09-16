# Règles pour ce dépôt

## Paiement (Stripe) — ne jamais intégrer à VEX ni pousser sur GitHub

Le système de paiement réel (Stripe Checkout + webhook) doit rester un
service **totalement séparé** de VEX :
- Code dans `paiement-pi/` à la racine du dépôt — ce dossier est dans
  `.gitignore` et ne doit **jamais** être commité ni poussé sur GitHub.
- Aucune route, dépendance ou logique de paiement dans `src/` (le code
  VEX lui-même) — la page `static/login/paiement.html` reste une maquette
  visuelle non fonctionnelle, jamais branchée à un vrai paiement.
- `paiement-pi/` tourne comme son propre processus, sur son propre port,
  déployé à la main sur le Pi (jamais via le mécanisme de mise à jour
  Git de VEX).

Raison : limiter les abus et garder les clés Stripe / la logique de
paiement hors du dépôt partagé — demande explicite de l'utilisateur.

Voir `paiement-pi/README.md` pour le déploiement.
