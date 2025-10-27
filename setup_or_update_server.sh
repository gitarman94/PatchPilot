#!/usr/bin/env bash
set -euo pipefail

# Config
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

# Optional flags
FORCE_REINSTALL=false
UPGRADE=false

for arg in "$@"; do
    case "$arg" in
        --force)   FORCE_REINSTALL=true;   echo "⚠️  Force reinstall enabled." ;;
        --upgrade) UPGRADE=true;           echo "⬆️  Upgrade mode enabled." ;;
    esac
done

# OS check – allow only Debian‑based systems
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;   # allowed
        *) echo "❌ This installer works only on Debian‑based systems."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS – /etc/os-release missing."
    exit 1
fi

# Install required Debian packages
export DEBIAN_FRONTEND=noninteractive
echo "📦 Installing required packages..."
apt-get update -qq
apt-get install -y -qq \
    python3 python3-venv python3-pip curl unzip jq

# Force-reinstall cleanup (if requested)
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Removing any previous installation..."

    # Stop and disable the systemd service
    systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    systemctl disable "${SERVICE_NAME}" 2>/dev/null || true

    # Kill stray server.py processes
    pids=$(pgrep -f "server.py" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "Terminating pid $pid"
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    # Remove previous application directory and its contents
    echo "Removing previous installation files..."
    rm -rf "${APP_DIR}"

    # Remove systemd service file
    echo "Removing systemd service definition..."
    rm -f "${SYSTEMD_DIR}/${SERVICE_NAME}"

    # Remove virtual environment
    echo "Removing virtual environment..."
    rm -rf "${VENV_DIR}"

    # Optionally remove SQLite DB
    SQLITE_DB="${APP_DIR}/patchpilot.db"
    if [[ -f "$SQLITE_DB" ]]; then
        echo "Removing SQLite database..."
        rm -f "$SQLITE_DB"
    fi

    # Optionally remove the patchpilot service user (if not used elsewhere)
    if id -u patchpilot >/dev/null 2>&1; then
        echo "Removing patchpilot service user..."
        userdel patchpilot || true
    fi
fi

# Create required directories
mkdir -p "${APP_DIR}"
mkdir -p "${APP_DIR}/updates"

# Create a fresh virtual environment (always runs)
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

# Pull the latest source from GitHub
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

echo "⬇️  Downloading repository ZIP..."
curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

# Extracted folder is named "<repo>-<branch>"
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

# SQLite DB creation and permission
SQLITE_DB="${APP_DIR}/patchpilot.db"
if [[ ! -f "$SQLITE_DB" ]]; then
    echo "Creating empty SQLite DB file..."
    touch "$SQLITE_DB"
fi
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Generate admin token (saved in project directory)
TOKEN_FILE="${APP_DIR}/admin_token.txt"
ENV_FILE="${APP_DIR}/admin_token.env"

if [[ ! -f "$TOKEN_FILE" ]]; then
    echo "Generating admin token..."
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -d '=+/')
    echo "$ADMIN_TOKEN" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
else
    ADMIN_TOKEN=$(cat "$TOKEN_FILE")
fi

# Environment file for systemd
printf "ADMIN_TOKEN=%s\n" "$ADMIN_TOKEN" > "$ENV_FILE"
chmod 600 "$ENV_FILE"

echo "✅ Admin token saved to ${TOKEN_FILE}"
echo "   (systemd will read it from ${ENV_FILE})"

# Systemd service definition (runs as unprivileged user)
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${ENV_FILE}
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
echo "🔐 Admin token is stored at ${TOKEN_FILE}"
