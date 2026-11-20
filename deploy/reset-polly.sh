#!/usr/bin/env bash
# Réinitialise silencieusement toutes les sessions Discord de Polly (sans rien afficher dans les salons)
# Usage : sudo bash deploy/reset-polly.sh

set -euo pipefail

HERMES_USER="${HERMES_USER:-hermes}"
HERMES_UID="$(id -u "$HERMES_USER")"

echo "🧹 Réinitialisation silencieuse des sessions Polly (utilisateur: ${HERMES_USER})..."

sudo -u "$HERMES_USER" \
    XDG_RUNTIME_DIR="/run/user/${HERMES_UID}" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${HERMES_UID}/bus" \
    PATH="/home/${HERMES_USER}/.local/bin:/usr/local/bin:/usr/bin:/bin" \
    bash -lc 'hermes gateway stop && rm -rf ~/.hermes/sessions/* ~/.hermes/state.db* && hermes gateway start && hermes gateway status'

echo "✅ Sessions Polly réinitialisées et passerelle redémarrée !"
