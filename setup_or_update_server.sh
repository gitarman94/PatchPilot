#!/bin/bash
set -e

# === Configuration ===
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"

RAW_BASE="https://raw.githubusercontent.com/${GITHUB_USER}/${GITHUB_REPO}/${BRANCH}"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
SELF_UPDATE_SCRIPT="linux_server_self_update.sh"
SELF_UPDATE_SERVICE="patchpilot_server_update.service"
SELF_UPDATE_TIMER="patchpilot_server_update.timer"
SYSTEMD_DIR="/etc/systemd/system"

FORCE_REINSTALL=false
UPGRADE=false
for arg in "$@"; do
    case "$arg" in
        --force)
            FORCE_REINSTALL=true
            echo "⚠️  Force reinstallation enabled: previous installation will be deleted."
            ;;
        --upgrade)
            UPGRADE=true
            echo "⬆️  Upgrade mode enabled: keeping configs but updating software."
            ;;
    esac
done

# === Install system deps omitted for brevity ===

# === Stop services & kill processes ===
echo "🛑 Stopping and disabling systemd services..."
systemctl stop "$SERVICE_NAME" 2>/dev/null || true
systemctl disable "$SERVICE_NAME" 2>/dev/null || true
rm -f "${SYSTEMD_DIR}/${SERVICE_NAME}"

systemctl stop "$SELF_UPDATE_TIMER" 2>/dev/null || true
systemctl disable "$SELF_UPDATE_TIMER" 2>/dev/null || true
rm -f "${SYSTEMD_DIR}/${SELF_UPDATE_TIMER}"
rm -f "${SYSTEMD_DIR}/${SELF_UPDATE_SERVICE}"

echo "☠️ Killing all running patchpilot server.py instances..."
pkill -f "/opt/patchpilot_server/server.py" || true

for pid in $(pgrep -f "/opt/patchpilot_server/server.py" || true); do
    if [ "$pid" != $$ ]; then
        kill -9 "$pid" || true
    fi
done

# === Handle FORCE reinstall ===
if [ "$FORCE_REINSTALL" = true ]; then
    echo "🧹 Removing previous installation at $APP_DIR..."
    rm -rf "$APP_DIR"
fi

# === Create directories ===
mkdir -p "${APP_DIR}"

# === Virtual environment setup & install packages ===
# ... (your existing logic here) ...

# === Download & extract repo ===
# ... (your existing logic here) ...

# === Permissions ===
chmod +x "${APP_DIR}/server.py"
if [ -f "${APP_DIR}/${SELF_UPDATE_SCRIPT}" ]; then
    chmod +x "${APP_DIR}/${SELF_UPDATE_SCRIPT}"
else
    echo "⚠️  Warning: Self-update script '${SELF_UPDATE_SCRIPT}' not found. Skipping."
fi

# === Write systemd service files ===
echo "🛎️  Creating systemd service: ${SERVICE_NAME}"
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=root
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
ExecStart=${VENV_DIR}/bin/python ${APP_DIR}/server.py
Restart=always

[Install]
WantedBy=multi-user.target
EOF

echo "📅 Creating self-update service & timer for daily updates"
cat > "${SYSTEMD_DIR}/${SELF_UPDATE_SERVICE}" <<EOF
[Unit]
Description=Patch Server Self-Update
After=network.target

[Service]
Type=oneshot
ExecStart=${APP_DIR}/${SELF_UPDATE_SCRIPT}
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
EOF

cat > "${SYSTEMD_DIR}/${SELF_UPDATE_TIMER}" <<EOF
[Unit]
Description=Run Patch Server Self-Update Daily

[Timer]
OnCalendar=*-*-* 02:00:00
Persistent=true

[Install]
WantedBy=timers.target
EOF

# === Reload and enable services ===
echo "🔄 Reloading systemd daemon"
systemctl daemon-reload

echo "🚀 Enabling & starting services"
systemctl enable --now "${SERVICE_NAME}"
systemctl enable --now "${SELF_UPDATE_TIMER}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete! Visit: http://${SERVER_IP}:8080 to view dashboard."
