#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════
# Met Plume (static/plume/) en test sur le Pi, depuis votre PC.
#
#   ./deploy-plume-pi.sh                    # copie les pages de Plume
#   ./deploy-plume-pi.sh --complet          # + recompile VEX (collaboration)
#   ./deploy-plume-pi.sh utilisateur@hote   # autre machine que le Pi habituel
#
# A lancer a la racine du depot vex (Linux, macOS, ou « Git Bash » sous
# Windows). Il faut pouvoir se connecter au Pi en SSH.
#
# Sans option : seuls les fichiers de static/plume/ sont copies. VEX les
# sert tout de suite (aucun redemarrage) a l'adresse /static/plume/.
# Tout Plume fonctionne, sauf la collaboration en temps reel, qui a
# besoin de la nouvelle route /api/plume/collab, donc d'un VEX recompile.
#
# --complet : le Pi passe sur la branche de test, recompile VEX
# (plusieurs minutes sur un Pi), garde l'ancien binaire dans
# ./vex.precedent et redemarre le service. Pour revenir en arriere :
#   git checkout main && cp -f vex.precedent vex && sudo systemctl restart vex
# ══════════════════════════════════════════════════════════════════
set -euo pipefail

CIBLE="clement@vex.hopto.org"
COMPLET=0
BRANCHE="${BRANCHE:-claude/friendly-keller-sy5rja}"
for arg in "$@"; do
  case "$arg" in
    --complet) COMPLET=1 ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *@*) CIBLE="$arg" ;;
    *) echo "Option inconnue : $arg" >&2; exit 2 ;;
  esac
done

if [ ! -f static/plume/texte.html ]; then
  echo "static/plume/ est introuvable : lancez ce script a la racine du depot vex," >&2
  echo "sur la branche $BRANCHE (git fetch origin $BRANCHE && git checkout $BRANCHE)." >&2
  exit 1
fi

# Dossier de VEX sur le Pi : celui du service systemd, sinon /opt/vex.
DOSSIER="${VEX_DIR:-$(ssh "$CIBLE" 'd=$(systemctl show -p WorkingDirectory --value vex 2>/dev/null); if [ -n "$d" ] && [ -d "$d/static" ]; then echo "$d"; elif [ -d /opt/vex/static ]; then echo /opt/vex; fi')}"
if [ -z "$DOSSIER" ]; then
  echo "Dossier de VEX introuvable sur $CIBLE. Indiquez-le : VEX_DIR=/chemin/vers/vex $0" >&2
  exit 1
fi
echo "VEX est dans $DOSSIER sur $CIBLE."

if [ "$COMPLET" = 1 ]; then
  echo "Passage sur la branche $BRANCHE, compilation et redemarrage (plusieurs minutes)…"
  ssh -t "$CIBLE" "set -e; cd '$DOSSIER'; . \$HOME/.cargo/env 2>/dev/null || true; \
    git fetch origin '$BRANCHE'; git checkout '$BRANCHE'; git pull --ff-only origin '$BRANCHE'; \
    cargo build --release; \
    if [ -f ./vex ]; then cp -f ./vex ./vex.precedent; fi; \
    cp -f target/release/vex ./vex; \
    sudo systemctl restart vex; sleep 2; systemctl is-active vex"
  echo "Termine : Plume et la collaboration sont en test."
else
  echo "Copie de static/plume/ …"
  tar -C static -czf - plume | ssh "$CIBLE" "mkdir -p '$DOSSIER/static' && tar -C '$DOSSIER/static' -xzf -"
  echo "Termine : Plume est en ligne (sans la collaboration ; pour l'activer : $0 --complet)."
fi
HOTE="${CIBLE#*@}"
echo "Adresse : https://$HOTE/static/plume/"
