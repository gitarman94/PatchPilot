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
SERVICE_NAME="patch_server.service"
SELF_UPDATE_SCRIPT="linux_server_self_update.sh"
SELF_UPDATE_SERVICE="patch_server_update.service"
SELF_UPDATE_TIMER="patch_server_update.timer"
SYSTEMD_DIR="/etc/systemd/system"

# === Flags ===
FORCE_REINSTALL=false
UPGRADE_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --force) FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE_ONLY=true ;;
    esac
done

if [ "$FORCE_REINSTALL" = true ]; then
    echo "⚠️  Force reinstallation enabled: previous installation will be deleted."
fi

if [ "$UPGRADE_ONLY" = true ]; then
    echo "⬆️  Upgrade mode: preserving configs and database."
fi

# === System dependencies ===
echo "📦 Installing system packages (python3, venv, pip, curl, unzip)..."
if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y python3 python3-venv python3-pip curl unzip
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y python3 python3-venv python3-pip curl unzip
elif command -v yum >/dev/null 2>&1; then
    yum install -y python3 python3-venv python3-pip curl unzip
else
    echo "❌ Unsupported OS / package manager. Please install dependencies manually."
    exit 1
fi

# === Optional force cleanup ===
if [ "$FORCE_REINSTALL" = true ] && [ -d "$APP_DIR" ]; then
    echo "🧹 Removing previous installation at $APP_DIR..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl stop "$SELF_UPDATE_TIMER" 2>/dev/null || true
    rm -rf "$APP_DIR"
fi

# === Prepare directories ===
echo "📁 Ensuring application directory exists at ${APP_DIR}"
mkdir -p "${APP_DIR}"

# === Virtual environment setup ===
if [ ! -d "$VENV_DIR" ]; then
    echo "🐍 Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

echo "⬆️  Activating venv and installing Python dependencies..."
source "${VENV_DIR}/bin/activate"
pip install --upgrade pip
pip install Flask Flask-SQLAlchemy flask_cors

# === Download/update repo only if not upgrade mode ===
if [ "$UPGRADE_ONLY" != true ]; then
    echo "⬇️  Downloading repository ZIP from GitHub and extracting..."
    TMPDIR=$(mktemp -d)
    cd "${TMPDIR}"
    curl -L "${ZIP_URL}" -o latest.zip
    unzip -o latest.zip
    EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
    if [ -z "${EXTRACTED_DIR}" ]; then
        echo "❌ Failed to locate extracted repo directory."
        exit 1
    fi
    echo "📂 Copying extracted files into ${APP_DIR}"
    cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"
    cd /
    rm -rf "${TMPDIR}"
fi

# === Permissions ===
echo "🛠️  Setting permissions on key files"
chmod +x "${APP_DIR}/server.py"
if [ -f "${APP_DIR}/${SELF_UPDATE_SCRIPT}" ]; then
    chmod +x "${APP_DIR}/${SELF_UPDATE_SCRIPT}"
else
    echo "⚠️  Warning: Self-update script '${SELF_UPDATE_SCRIPT}' not found. Skipping."
fi

# === Systemd service ===
echo "🛎️  Creating systemd service: ${SERVICE_NAME}"
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
ExecStart=/bin/bash -c 'cd ${APP_DIR} && ${VENV_DIR}/bin/python server.py'
Restart=always

[Install]
WantedBy=multi-user.target
EOF

# === Self-update timer ===
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

# === Finalize systemd ===
echo "🔄 Reloading systemd daemon"
systemctl daemon-reload

echo "🚀 Enabling & starting services"
systemctl enable --now "${SERVICE_NAME}"
systemctl enable --now "${SELF_UPDATE_TIMER}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation/Upgrade complete! Visit: http://${SERVER_IP}:8080 to view dashboard."
