#!/bin/bash
set -e

# === Configuration – adjust these for your repo ===
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"

# Validation to prevent running with placeholder values
if [[ "$GITHUB_USER" == "<your-github-username>" || "$GITHUB_REPO" == "<your-repo-name>" ]]; then
  echo "❌ Please update GITHUB_USER and GITHUB_REPO variables in the script before running."
  exit 1
fi

# The “raw” base URL for individual files
RAW_BASE="https://raw.githubusercontent.com/${GITHUB_USER}/${GITHUB_REPO}/${BRANCH}"
# The “archive zip” URL for entire repo
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

# Local paths
APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patch_server.service"
SELF_UPDATE_SCRIPT="linux_server_self_update.sh"
SELF_UPDATE_SERVICE="patch_server_update.service"
SELF_UPDATE_TIMER="patch_server_update.timer"

SYSTEMD_DIR="/etc/systemd/system"

echo "📦 Installing system packages (python3, venv, pip, curl, unzip)..."

if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y python3 python3-venv python3-pip curl unzip
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y python3 python3-venv python3-pip curl unzip
elif command -v yum >/dev/null 2>&1; then
    yum install -y python3 python3-venv python3-pip curl unzip
else
    echo "❌ Unsupported OS / package manager. Please install Python3, pip, curl, unzip manually."
    exit 1
fi

echo "📁 Creating application directory at ${APP_DIR}"
mkdir -p "${APP_DIR}"

echo "🐍 Creating Python virtual environment..."
python3 -m venv "${VENV_DIR}"

echo "⬆️ Activating venv and installing Python dependencies..."
source "${VENV_DIR}/bin/activate"
pip install --upgrade pip
pip install Flask Flask-SQLAlchemy flask_wtf

echo "⬇️ Downloading repository ZIP from GitHub and extracting to ${APP_DIR}"
TMPDIR=$(mktemp -d)
cd "${TMPDIR}"
curl -L "${ZIP_URL}" -o latest.zip
unzip -o latest.zip
# The zip extracts to something like <repo-name>-<branch>, identify it:
EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
if [ -z "${EXTRACTED_DIR}" ]; then
    echo "❌ Failed to locate extracted repo directory."
    exit 1
fi

echo "📂 Copying extracted files into ${APP_DIR}"
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"

echo "🛠️ Setting permissions on key files"
chmod +x "${APP_DIR}/server.py"
chmod +x "${APP_DIR}/${SELF_UPDATE_SCRIPT}"

cd /
rm -rf "${TMPDIR}"

echo "🛎️ Creating systemd service: ${SERVICE_NAME}"
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

echo "🔄 Reloading systemd daemon"
systemctl daemon-reload

echo "🚀 Enabling & starting services"
systemctl enable --now "${SERVICE_NAME}"
systemctl enable --now "${SELF_UPDATE_TIMER}"

echo "✅ Installation complete! Visit: http://<server-ip>:5000 to view dashboard."
