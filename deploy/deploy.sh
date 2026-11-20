#!/usr/bin/env bash
# Script de déploiement unifié sur le VPS (bots Rust + base documentaire Polly)
# Usage : bash deploy/deploy.sh [branche]   (par défaut : main)

set -euo pipefail

BRANCH="${1:-main}"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HERMES_HOME_DIR="${HERMES_HOME:-$HOME/.hermes}"

echo "🔄 [1/4] Mise à jour du dépôt sur la branche '${BRANCH}'..."
cd "$REPO_DIR"
git fetch origin
git checkout -B "$BRANCH" "origin/$BRANCH"

echo "🦀 [2/4] Compilation release de tout le workspace..."
cargo build --release --workspace

echo "🦜 [3/4] Synchronisation de la base documentaire Polly (Hermes)..."
if [ -d "$HERMES_HOME_DIR" ]; then
    mkdir -p "$HERMES_HOME_DIR/skills"
    ln -sfn "$REPO_DIR/polly/SOUL.md" "$HERMES_HOME_DIR/SOUL.md"
    ln -sfn "$REPO_DIR/polly/MEMORY.md" "$HERMES_HOME_DIR/MEMORY.md"
    ln -sfn "$REPO_DIR/polly/skills/nanterre-pirates-faq" "$HERMES_HOME_DIR/skills/nanterre-pirates-faq"
    ln -sfn "$REPO_DIR/matches.txt" "$HERMES_HOME_DIR/skills/nanterre-pirates-faq/references/matches.txt"

    if command -v hermes >/dev/null 2>&1; then
        hermes gateway restart || true
    fi
else
    echo "ℹ️  Dossier $HERMES_HOME_DIR absent (si Hermes tourne sous un autre utilisateur, exécutez le symlink sous cet utilisateur)."
fi

echo "🚀 [4/4] Redémarrage des services systemd..."
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
