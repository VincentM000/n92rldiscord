#!/usr/bin/env bash
# Script de déploiement unifié sur le VPS (bots Rust + base documentaire Polly sous 'hermes')
# Usage :
#   bash deploy/deploy.sh [branche] [--reset-sessions]
# Exemples :
#   bash deploy/deploy.sh main
#   bash deploy/deploy.sh develop --reset-sessions

set -euo pipefail

BRANCH="${1:-main}"
RESET_SESSIONS=false
for arg in "$@"; do
    if [ "$arg" = "--reset-sessions" ]; then
        RESET_SESSIONS=true
    fi
done

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HERMES_USER="${HERMES_USER:-hermes}"

echo "🔄 [1/4] Mise à jour du dépôt sur la branche '${BRANCH}'..."
cd "$REPO_DIR"
git fetch origin
git checkout -B "$BRANCH" "origin/$BRANCH"

echo "🦀 [2/4] Compilation release de tout le workspace..."
cargo build --release --workspace

echo "🦜 [3/4] Synchronisation de la base documentaire Polly (utilisateur: ${HERMES_USER})..."
if id -u "$HERMES_USER" >/dev/null 2>&1 && [ -d "/home/${HERMES_USER}/.hermes" ]; then
    HERMES_HOME_DIR="/home/${HERMES_USER}/.hermes"
    HERMES_UID="$(id -u "$HERMES_USER")"

    mkdir -p "$HERMES_HOME_DIR/skills/nanterre-pirates-faq"
    cp "$REPO_DIR/polly/SOUL.md" "$HERMES_HOME_DIR/SOUL.md"
    cp "$REPO_DIR/polly/MEMORY.md" "$HERMES_HOME_DIR/MEMORY.md"
    cp -r "$REPO_DIR/polly/skills/nanterre-pirates-faq/." "$HERMES_HOME_DIR/skills/nanterre-pirates-faq/"
    rm -f "$HERMES_HOME_DIR/skills/nanterre-pirates-faq/references/matches.txt"
    cp "$REPO_DIR/matches.txt" "$HERMES_HOME_DIR/skills/nanterre-pirates-faq/references/matches.txt"
    chown -R "${HERMES_USER}:${HERMES_USER}" "$HERMES_HOME_DIR"

    if [ "$RESET_SESSIONS" = true ]; then
        bash "$REPO_DIR/deploy/reset-polly.sh"
    else
        sudo -u "$HERMES_USER" \
            XDG_RUNTIME_DIR="/run/user/${HERMES_UID}" \
            DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${HERMES_UID}/bus" \
            PATH="/home/${HERMES_USER}/.local/bin:/usr/local/bin:/usr/bin:/bin" \
            bash -lc 'hermes gateway restart' || true
    fi
else
    echo "ℹ️  Utilisateur '${HERMES_USER}' ou /home/${HERMES_USER}/.hermes introuvable, étape Polly ignorée."
fi

echo "🚀 [4/4] Redémarrage des services systemd Rust..."
sudo systemctl restart \
    nrl-tracker \
    super-league-tracker \
    super-xiii-tracker \
    live-match-monitor \
    rss-feed-notifier

sudo systemctl enable --now weather-notifier.timer

echo "✅ Déploiement terminé !"
sudo systemctl status \
    nrl-tracker \
    super-league-tracker \
    super-xiii-tracker \
    live-match-monitor \
    rss-feed-notifier \
    weather-notifier.timer \
    --no-pager
