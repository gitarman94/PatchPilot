#!/usr/bin/env bash
set -euo pipefail

# Configuration
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_RE}//heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

# Flags (optional)
FORCE_REINSTALL=false
UPGRADE=false

for arg in "$@"; do
    case "$arg" in
        --force)   FORCE_REINSTALL=true;   echo "⚠️  Force reinstall enabled." ;;
        --upgrade) UPGRADE=true;           echo "⬆️  Upgrade mode enabled." ;;
    esac
done

# OS check – only allow Debian‑derived systems
if [[ -f /etc/os-release ]]; then

    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;   # allowed
        *) echo "❌ This installer works only on Debian‑based systems."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS – /etc/os-release missing."; exit 1
fi

# Install required Debian packages (no stray words)
export DEBIAN_FRONTEND=noninteractive
echo "📦 Installing required packages..."
apt-get update -qq
apt-get install -y -qq \
    python3 python3-venv python3-pip curl unzip

# Optional force‑reinstall cleanup
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Removing any previous installation..."
    systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    systemctl disable "${SERVICE_NAME}" 2>/dev/null || true

    pids=$(pgrep -f "server.py" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "Terminating pid $pid"
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    rm -rf "${APP_DIR}"   # removes everything, including a stale venv
fi

# Create required directories
mkdir -p "${APP_DIR}"
mkdir -p "${APP_DIR}/updates"

# Create a fresh virtual environment
# At this point ${VENV_DIR} definitely does NOT exist (either never created or removed above)
echo "🐍 Creating Python virtual environment..."
python3 -m venv "${VENV_DIR}"

# Ensure pip works inside the venv
if [[ ! -x "${VENV_DIR}/bin/pip" ]]; then
    echo "Installing pip into venv..."
    "${VENV_DIR}/bin/python" -m ensurepip --upgrade
fi
"${VENV_DIR}/bin/pip" install --upgrade pip setuptools wheel

# Install Python dependencies (SQLite only)
source "${VENV_DIR}/bin/activate"
pip install --upgrade Flask Flask-SQLAlchemy flask_cors gunicorn

# Pull latest source from GitHub
TMPmktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

echo "⬇️  Downloading repository ZIP..."
curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

# The extracted folder is named "<repo>-<branch>"
EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
if [[ -z "$EXTRACTED_DIR" ]]; then
    echo "❌ Failed to locate extracted repo directory."
    exit 1
fi

echo "Copying files to ${APP_DIR}..."
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"
chmod +x "${APP_DIR}/server.py"

# Create unprivileged service user (if missing)
if ! id -u patchpilot >/dev/null 2>&1; then
    echo "Creating service user 'patchpilot'..."
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "${APP_DIR}"

# SQLite DB file creation and permission
SQLITE_DB="${APP_DIR}/patchpilot.db"
if [[ ! -f "$SQLITE_DB" ]]; then
    echo "Creating empty SQLite DB file..."
    touch "$SQLITE_DB"
fi
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Generate admin token (if not already present)
TOKEN_FILE="${APP_DIR}/admin_token.txt"
if [[ ! -f "$TOKEN_FILE" ]]; then
    echo "Generating admin token..."
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -d '=+/')
    echo "${ADMIN_TOKEN}" > "${TOKEN_FILE}"
    chmod 600TOKEN_FILE}"
else
    ADMIN_TOKEN=$(cat "${TOKEN_FILE}")
fi

# Systemd service definition (runs as unprivileged user)
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin" "ADMIN_TOKEN=${ADMIN_TOKEN}"
ExecStart=${VENV_DIR}/bin/gunicorn -w 4 -b 0.0.0.0:8080 server:app
ExecReload=/bin/kill -s HUP \$MAINPID
Restart=always

[Install]
WantedBy=multi-user.target
EOF

# Enable & start the service
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo "Enabling and starting ${SERVICE_NAME}..."
systemctl enable --now "${SERVICE_NAME}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete! Dashboard: http://${SERVER_IP}:8080"
echo "🔐 Admin token (keep it safe): ${ADMIN_TOKEN}"
