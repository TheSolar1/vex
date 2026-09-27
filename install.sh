#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════
# install.sh — installeur VEX pour Linux (Debian 12+, Ubuntu 22.04+,
# Raspberry Pi OS 64 bits)
#
#   curl -fsSL https://raw.githubusercontent.com/TheSolar1/vex/main/install.sh | sudo bash
#
# Questions posees (menus) :
#   - acces : domaine public (HTTPS Let's Encrypt) / IP publique /
#     reseau local / Tor uniquement, + option adresse .onion en plus
#   - editeur de documents en ligne : OnlyOffice ou aucun
#
# Met en place : MariaDB, VEX (compile depuis GitHub) en service
# systemd, Caddy (reverse proxy + HTTPS), Docker + OnlyOffice, Tor.
#
# Relancer le script = mettre a jour VEX et reconfigurer l'acces /
# l'editeur. La base, db.json et les secrets deja generes sont gardes.
#
# La fin de l'installation (compte administrateur) se fait dans le
# navigateur : /login/first_setup.
#
# Variables optionnelles : VEX_REPO, VEX_BRANCH.
# ══════════════════════════════════════════════════════════════════
set -euo pipefail

VEX_REPO="${VEX_REPO:-https://github.com/TheSolar1/vex.git}"
VEX_BRANCH="${VEX_BRANCH:-main}"
VEX_USER=vex
VEX_DIR=/opt/vex
APP_DIR="$VEX_DIR/app"
CFG="$APP_DIR/config.json"
PORT=8080       # VEX (tiny_http)
TOR_PORT=8081   # Caddy, ecoute locale pour le service .onion
OO_PORT=8084    # OnlyOffice Document Server (conteneur)
OO_CONTENEUR=vex-onlyoffice
# Chemin code en dur dans src/fchier/onlyoffice.rs (DOCUMENTS_DIR).
OO_DOCUMENTS=/var/www/html/onlyoffice/documents
TOR_DIR=/var/lib/tor/vex
CADDYFILE=/etc/caddy/Caddyfile
MARQUEUR_CADDY="# Genere par install.sh (VEX)"
TITRE="Installation de VEX"

log()  { printf '\n\033[1;32m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m[!] %s\033[0m\n' "$*" >&2; }
die()  { printf '\033[1;31m[x] %s\033[0m\n' "$*" >&2; exit 1; }

# Chaine aleatoire alphanumerique de $1 caracteres (max ~60).
rand() { head -c 64 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | cut -c1-"$1"; }

# Menus : lus depuis /dev/tty pour fonctionner aussi en `curl | bash`
# (stdin est alors le script lui-meme).
menu() { whiptail --title "$TITRE" "$@" 3>&1 1>&2 2>&3 </dev/tty; }
annule() { die "Installation annulee."; }

jq_edit() {
    local tmp
    tmp=$(mktemp)
    jq "$@" "$CFG" >"$tmp"
    mv "$tmp" "$CFG"
    chown "$VEX_USER:$VEX_USER" "$CFG"
    chmod 600 "$CFG"
}

en_tant_que_vex() { runuser -u "$VEX_USER" -- env HOME="$VEX_DIR" "$@"; }

# ── Verifications ────────────────────────────────────────────────
verifications() {
    [[ $EUID -eq 0 ]] || die "Lance ce script en root : sudo bash install.sh"
    command -v apt-get >/dev/null || die "Seules les distributions basees sur Debian/Ubuntu sont prises en charge."
    [[ -r /dev/tty ]] || die "Terminal interactif requis (les menus sont lus depuis /dev/tty)."
    case "$(uname -m)" in
        x86_64|aarch64) ;;
        *) die "Architecture $(uname -m) non prise en charge (x86_64 ou aarch64 requis)." ;;
    esac
    if ! command -v whiptail >/dev/null || ! command -v jq >/dev/null || ! command -v curl >/dev/null; then
        log "Installation des outils de l'installeur"
        apt-get update -qq
        DEBIAN_FRONTEND=noninteractive apt-get install -y -qq whiptail jq curl ca-certificates >/dev/null
    fi
}

# ── Questions ────────────────────────────────────────────────────
questions() {
    MODE=$(menu --radiolist "Comment VEX sera-t-il joint ?\n(Espace pour choisir, Entree pour valider)" 16 78 4 \
        domaine "Domaine public + HTTPS Let's Encrypt (recommande)" ON \
        ip      "IP publique, sans domaine (certificat auto-signe)" OFF \
        local   "Reseau local uniquement (certificat auto-signe)" OFF \
        tor     "Tor uniquement : adresse .onion, rien d'expose" OFF) || annule

    DOMAINE="" EMAIL_LE="" ADRESSE_IP=""
    case "$MODE" in
        domaine)
            DOMAINE=$(menu --inputbox "Nom de domaine (il doit deja pointer vers ce serveur, ports 80 et 443 ouverts) :" 10 78 "") || annule
            DOMAINE=${DOMAINE,,}
            [[ $DOMAINE =~ ^[a-z0-9.-]+\.[a-z]{2,}$ ]] || die "Nom de domaine invalide : '$DOMAINE'"
            EMAIL_LE=$(menu --inputbox "Adresse e-mail pour Let's Encrypt (alertes d'expiration) :" 10 78 "") || annule
            [[ $EMAIL_LE == *@*.* ]] || die "Adresse e-mail invalide : '$EMAIL_LE'"
            ;;
        ip)
            local ip_detectee
            ip_detectee=$(curl -fsS --max-time 8 https://api.ipify.org || true)
            ADRESSE_IP=$(menu --inputbox "Adresse IP publique du serveur (ports 80 et 443 ouverts) :" 10 78 "$ip_detectee") || annule
            ;;
        local)
            ADRESSE_IP=$(menu --inputbox "Adresse IP du serveur sur le reseau local :" 10 78 "$(hostname -I | awk '{print $1}')") || annule
            ;;
    esac
    if [[ $MODE == ip || $MODE == local ]]; then
        [[ $ADRESSE_IP =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] || die "Adresse IP invalide : '$ADRESSE_IP'"
    fi

    AVEC_TOR=0
    if [[ $MODE == tor ]]; then
        AVEC_TOR=1
    elif menu --defaultno --yesno "Rendre VEX accessible AUSSI via Tor (adresse .onion) ?" 8 78; then
        AVEC_TOR=1
    fi

    EDITEUR=$(menu --radiolist "Editeur de documents en ligne (docx, xlsx, pptx...) :" 14 78 2 \
        onlyoffice "OnlyOffice Document Server (Docker, ~4 Go de RAM conseilles)" ON \
        aucun      "Aucun editeur en ligne" OFF) || annule

    local resume="Acces   : $MODE"
    [[ -n $DOMAINE ]] && resume+=" ($DOMAINE)"
    [[ -n $ADRESSE_IP ]] && resume+=" ($ADRESSE_IP)"
    resume+="\nTor     : $([[ $AVEC_TOR == 1 ]] && echo oui || echo non)"
    resume+="\nEditeur : $EDITEUR"
    resume+="\n\nDossier : $APP_DIR\nSource  : $VEX_REPO ($VEX_BRANCH)"
    resume+="\n\nLa compilation de VEX peut prendre 10 a 30 minutes sur un Raspberry Pi."
    menu --yesno "$resume\n\nLancer l'installation ?" 18 78 || annule
}

# Caddy doit pouvoir prendre les ports 80/443 (sauf mode Tor seul).
verifier_ports() {
    [[ $MODE == tor ]] && return 0
    local occupe
    occupe=$(ss -Hltnp '( sport = :80 or sport = :443 )' 2>/dev/null | grep -v '"caddy"' || true)
    if [[ -n $occupe ]]; then
        die "Les ports 80/443 sont deja utilises par un autre service (Apache, Nginx...) :
$occupe
Arrete-le ou desactive-le, puis relance ce script."
    fi
}

# ── Paquets ──────────────────────────────────────────────────────
installer_paquets() {
    log "Installation des paquets systeme"
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
        git build-essential pkg-config libssl-dev mariadb-server gnupg >/dev/null

    if ! command -v caddy >/dev/null; then
        log "Installation de Caddy"
        curl -fsSL 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
            | gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
        curl -fsSL 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
            >/etc/apt/sources.list.d/caddy-stable.list
        apt-get update -qq
        DEBIAN_FRONTEND=noninteractive apt-get install -y -qq caddy >/dev/null
    fi

    if [[ $EDITEUR == onlyoffice ]] && ! command -v docker >/dev/null; then
        log "Installation de Docker"
        curl -fsSL https://get.docker.com | sh
    fi

    if [[ $AVEC_TOR == 1 ]] && ! command -v tor >/dev/null; then
        log "Installation de Tor"
        DEBIAN_FRONTEND=noninteractive apt-get install -y -qq tor >/dev/null
    fi
}

# ── VEX : utilisateur, source, compilation ───────────────────────
installer_vex() {
    if ! id "$VEX_USER" >/dev/null 2>&1; then
        log "Creation de l'utilisateur systeme '$VEX_USER'"
        useradd --system --home-dir "$VEX_DIR" --create-home --shell /usr/sbin/nologin "$VEX_USER"
    fi
    mkdir -p "$VEX_DIR"
    chown "$VEX_USER:$VEX_USER" "$VEX_DIR"

    if [[ -d $APP_DIR/.git ]]; then
        log "Mise a jour du code de VEX"
        en_tant_que_vex git -C "$APP_DIR" pull --ff-only
    else
        log "Telechargement de VEX ($VEX_REPO)"
        en_tant_que_vex git clone --depth 1 -b "$VEX_BRANCH" "$VEX_REPO" "$APP_DIR"
    fi

    if [[ ! -x $VEX_DIR/.cargo/bin/cargo ]]; then
        log "Installation de Rust (pour l'utilisateur $VEX_USER)"
        en_tant_que_vex sh -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
    fi

    log "Compilation de VEX (patience...)"
    en_tant_que_vex sh -c "cd '$APP_DIR' && '$VEX_DIR/.cargo/bin/cargo' build --release"
    install -o "$VEX_USER" -g "$VEX_USER" -m 755 "$APP_DIR/target/release/vex" "$APP_DIR/vex.new"
    mv "$APP_DIR/vex.new" "$APP_DIR/vex"
}

# ── Base de donnees ──────────────────────────────────────────────
configurer_db() {
    systemctl enable --now mariadb >/dev/null 2>&1
    if [[ -f $APP_DIR/db.json ]]; then
        log "db.json deja present, base conservee"
        return 0
    fi
    log "Creation de la base MariaDB"
    local mdp
    mdp=$(rand 32)
    mysql <<SQL
CREATE DATABASE IF NOT EXISTS vex CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'vex'@'localhost' IDENTIFIED BY '$mdp';
ALTER USER 'vex'@'localhost' IDENTIFIED BY '$mdp';
GRANT ALL PRIVILEGES ON vex.* TO 'vex'@'localhost';
FLUSH PRIVILEGES;
SQL
    jq -n --arg p "$mdp" '{host: "127.0.0.1", port: 3306, user: "vex", password: $p, database: "vex"}' \
        >"$APP_DIR/db.json"
    chown "$VEX_USER:$VEX_USER" "$APP_DIR/db.json"
    chmod 600 "$APP_DIR/db.json"
}

# ── OnlyOffice ───────────────────────────────────────────────────
installer_onlyoffice() {
    mkdir -p "$OO_DOCUMENTS"
    chown "$VEX_USER:$VEX_USER" "$OO_DOCUMENTS"
    chmod 750 "$OO_DOCUMENTS"

    log "Demarrage d'OnlyOffice Document Server (premier telechargement : plusieurs Go)"
    systemctl enable --now docker >/dev/null 2>&1
    docker rm -f "$OO_CONTENEUR" >/dev/null 2>&1 || true
    mkdir -p "$VEX_DIR/onlyoffice/data" "$VEX_DIR/onlyoffice/logs"
    # host.docker.internal : le conteneur joint VEX directement
    # (server.internal_url), sans passer par l'adresse publique.
    # ALLOW_PRIVATE_IP_ADDRESS : sinon OnlyOffice refuse de telecharger
    # depuis une IP privee (la passerelle Docker).
    docker run -d --name "$OO_CONTENEUR" --restart unless-stopped \
        -p "127.0.0.1:$OO_PORT:80" \
        --add-host host.docker.internal:host-gateway \
        -e JWT_ENABLED=true \
        -e JWT_SECRET="$OO_JWT" \
        -e JWT_HEADER=Authorization \
        -e JWT_IN_BODY=true \
        -e ALLOW_PRIVATE_IP_ADDRESS=true \
        -v "$VEX_DIR/onlyoffice/data:/var/www/onlyoffice/Data" \
        -v "$VEX_DIR/onlyoffice/logs:/var/log/onlyoffice" \
        onlyoffice/documentserver >/dev/null
}

supprimer_onlyoffice() {
    if command -v docker >/dev/null && docker ps -a --format '{{.Names}}' | grep -qx "$OO_CONTENEUR"; then
        log "Arret du conteneur OnlyOffice (editeur desactive)"
        docker rm -f "$OO_CONTENEUR" >/dev/null
    fi
}

# Adresse d'ecoute de VEX : jamais 0.0.0.0 (sinon VEX serait joignable en
# clair sur le port $PORT sans passer par Caddy ni Tor). Avec OnlyOffice,
# la passerelle du pont Docker (= host.docker.internal pour le conteneur).
adresse_ecoute() {
    VEX_BIND=127.0.0.1
    if [[ $EDITEUR == onlyoffice ]]; then
        VEX_BIND=$(docker network inspect bridge -f '{{(index .IPAM.Config 0).Gateway}}' 2>/dev/null || true)
        [[ -n $VEX_BIND ]] || die "Impossible de lire l'adresse du pont Docker (docker network inspect bridge)."
    fi
}

# ── Tor ──────────────────────────────────────────────────────────
configurer_tor() {
    ONION=""
    [[ $AVEC_TOR == 1 ]] || return 0
    log "Configuration du service Tor (.onion)"
    if ! grep -q "^HiddenServiceDir $TOR_DIR/" /etc/tor/torrc; then
        printf '\n# VEX (install.sh)\nHiddenServiceDir %s/\nHiddenServicePort 80 127.0.0.1:%s\n' \
            "$TOR_DIR" "$TOR_PORT" >>/etc/tor/torrc
    fi
    systemctl enable tor >/dev/null 2>&1
    systemctl restart tor

    for _ in $(seq 1 60); do
        [[ -s $TOR_DIR/hostname ]] && break
        sleep 1
    done
    [[ -s $TOR_DIR/hostname ]] || die "Tor n'a pas genere d'adresse .onion (voir : journalctl -u tor@default)."
    ONION=$(<"$TOR_DIR/hostname")
}

# ── config.json ──────────────────────────────────────────────────
configurer_vex() {
    if [[ ! -f $CFG ]]; then
        log "Creation de config.json"
        cp "$APP_DIR/config.example.json" "$CFG"
        jq_edit --arg s "$(rand 60)" '.autologin.server_secret = $s'
    fi

    case "$MODE" in
        domaine)  URL_PUBLIQUE="https://$DOMAINE" ;;
        ip|local) URL_PUBLIQUE="https://$ADRESSE_IP" ;;
        tor)      URL_PUBLIQUE="http://$ONION" ;;
    esac

    jq_edit --arg pub "$URL_PUBLIQUE" --argjson port "$PORT" --arg bind "$VEX_BIND" \
        '.server.public_url = $pub | .server.port = $port | .server.bind = $bind'

    if [[ $EDITEUR == onlyoffice ]]; then
        jq_edit --arg jwt "$OO_JWT" \
            --arg interne "http://host.docker.internal:$PORT" \
            --arg srv "http://127.0.0.1:$OO_PORT" '
            .server.internal_url = $interne
            | .editor.online_editing_enabled = true
            | .editor.provider = "onlyoffice"
            | .editor.providers.onlyoffice.enabled = true
            | .editor.providers.onlyoffice.jwt_enabled = true
            | .editor.providers.onlyoffice.jwt_secret = $jwt
            | .editor.providers.onlyoffice.server_url = $srv
            | .onlyoffice_server.enabled = true
            | .onlyoffice_server.server_url = $srv
            | .extensions.extension_params.onlyoffice.params.jwt_secret = $jwt'
    else
        jq_edit '
            .server.internal_url = ""
            | .editor.online_editing_enabled = false
            | .editor.providers.onlyoffice.enabled = false
            | .onlyoffice_server.enabled = false'
    fi

    if [[ $AVEC_TOR == 1 ]]; then
        jq_edit --arg onion "http://$ONION" '.p2p.use_tor = true | .p2p.tor_addr = $onion'
    else
        jq_edit '.p2p.use_tor = false | .p2p.tor_addr = ""'
    fi
}

# Secret JWT OnlyOffice : garde celui de config.json s'il existe deja.
secret_onlyoffice() {
    OO_JWT=""
    if [[ -f $CFG ]]; then
        OO_JWT=$(jq -r '.editor.providers.onlyoffice.jwt_secret // ""' "$CFG")
    fi
    if [[ -z $OO_JWT || $OO_JWT == change-me-secret ]]; then
        OO_JWT=$(rand 40)
    fi
}

# ── Caddy ────────────────────────────────────────────────────────
configurer_caddy() {
    log "Configuration de Caddy"
    if [[ -f $CADDYFILE ]] && ! grep -qF "$MARQUEUR_CADDY" "$CADDYFILE"; then
        cp "$CADDYFILE" "$CADDYFILE.avant-vex.$(date +%s)"
    fi

    local routes_oo=""
    if [[ $EDITEUR == onlyoffice ]]; then
        # X-Forwarded-Host avec le prefixe : methode documentee par
        # OnlyOffice pour le servir sous un sous-chemin (/onlyoffice/).
        routes_oo="	handle_path /onlyoffice/* {
		reverse_proxy 127.0.0.1:$OO_PORT {
			header_up X-Forwarded-Host {host}/onlyoffice
		}
	}
	handle /cache/* {
		reverse_proxy 127.0.0.1:$OO_PORT
	}
"
    fi

    {
        echo "$MARQUEUR_CADDY -- relancer install.sh pour le regenerer."
        if [[ $MODE == domaine ]]; then
            printf '{\n\temail %s\n}\n' "$EMAIL_LE"
        fi
        printf '\n(vex) {\n%s\thandle {\n\t\treverse_proxy %s:%s\n\t}\n}\n' "$routes_oo" "$VEX_BIND" "$PORT"
        case "$MODE" in
            domaine)  printf '\n%s {\n\timport vex\n}\n' "$DOMAINE" ;;
            ip|local) printf '\nhttps://%s {\n\ttls internal\n\timport vex\n}\n' "$ADRESSE_IP" ;;
        esac
        if [[ $AVEC_TOR == 1 ]]; then
            printf '\nhttp://:%s {\n\tbind 127.0.0.1\n\timport vex\n}\n' "$TOR_PORT"
        fi
    } >"$CADDYFILE"

    caddy validate --config "$CADDYFILE" --adapter caddyfile >/dev/null 2>&1 \
        || die "Caddyfile invalide ($CADDYFILE) : caddy validate --config $CADDYFILE"
    systemctl enable caddy >/dev/null 2>&1
    systemctl restart caddy
}

# ── Service systemd ──────────────────────────────────────────────
installer_service() {
    log "Installation du service systemd 'vex'"
    cat >/etc/systemd/system/vex.service <<EOF
[Unit]
Description=VEX
After=network-online.target mariadb.service docker.service
Wants=network-online.target
Requires=mariadb.service

[Service]
User=$VEX_USER
Group=$VEX_USER
WorkingDirectory=$APP_DIR
ExecStart=$APP_DIR/vex
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    systemctl enable vex >/dev/null 2>&1
    systemctl restart vex

    local code=000
    for _ in $(seq 1 60); do
        code=$(curl -s -o /dev/null -w '%{http_code}' "http://$VEX_BIND:$PORT/login" || true)
        [[ $code != 000 ]] && break
        sleep 2
    done
    [[ $code != 000 ]] || warn "VEX ne repond pas encore sur le port $PORT (voir : journalctl -u vex -n 50)."
}

# ── Resume final ─────────────────────────────────────────────────
conclure() {
    local msg="VEX est installe.\n\n"
    msg+="Adresse : $URL_PUBLIQUE\n"
    [[ -n $ONION && $MODE != tor ]] && msg+="Tor     : http://$ONION\n"
    msg+="\nDERNIERE ETAPE, A FAIRE TOUT DE SUITE : ouvre\n  $URL_PUBLIQUE/login/first_setup\n"
    msg+="pour creer le compte administrateur (le premier compte cree le devient).\n"
    if [[ $MODE == ip || $MODE == local ]]; then
        msg+="\nCertificat auto-signe : le navigateur affichera un avertissement.\n"
        msg+="Pour l'eviter, installe sur tes appareils l'autorite racine de Caddy :\n"
        msg+="  /var/lib/caddy/.local/share/caddy/pki/authorities/local/root.crt\n"
    fi
    if [[ $EDITEUR == onlyoffice ]]; then
        msg+="\nOnlyOffice met 1 a 2 minutes a demarrer au premier lancement.\n"
    fi
    msg+="\nCommandes utiles : systemctl status vex | journalctl -u vex -f"
    menu --msgbox "$msg" 24 78 || true
    printf '%b\n' "$msg"
}

main() {
    verifications
    questions
    verifier_ports
    installer_paquets
    installer_vex
    configurer_db
    secret_onlyoffice
    if [[ $EDITEUR == onlyoffice ]]; then
        installer_onlyoffice
    else
        supprimer_onlyoffice
    fi
    adresse_ecoute
    configurer_tor
    configurer_vex
    configurer_caddy
    installer_service
    conclure
}

# Tout est dans main() : en `curl | bash`, bash lit ainsi le script en
# entier avant d'executer quoi que ce soit.
main "$@"
