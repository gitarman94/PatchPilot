#!/usr/bin/env bash
set -euo pipefail

# Retrieve the GitHub token from the environment variable or command line arguments
if [[ -z "${GITHUB_TOKEN:-}" ]]; then
    echo "❌ GitHub token is required. Please set the GITHUB_TOKEN environment variable."
    exit 1
fi

GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

FORCE_REINSTALL=false
UPGRADE=false

for arg in "$@"; do
    case "$arg" in
        --force)   FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE=true ;;
    esac
done

if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "❌ This installer works only on Debian-based systems."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS – /etc/os-release missing."
    exit 1
fi

# Cleanup old install first if --force is used
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."

    # Stop and disable systemd service if it exists
    if systemctl list-units --full -all | grep -q "^${SERVICE_NAME}"; then
        echo "🛑 Stopping systemd service ${SERVICE_NAME}..."
        systemctl stop "${SERVICE_NAME}" || true
        systemctl disable "${SERVICE_NAME}" || true
    fi

    # Kill any running server.py instances in the application directory
    pids=$(pgrep -f "^${APP_DIR}/server.py$" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "🛑 Terminating running server.py process $pid..."
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    # Remove the old application directory
    rm -rf "${APP_DIR}"
fi

# Install system packages
echo "📦 Installing required packages..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq python3 python3-venv python3-pip curl unzip openssl

# Setup application directories
mkdir -p "${APP_DIR}/updates"

# Create Python virtual environment and install Python packages
echo "🐍 Creating Python virtual environment..."
python3 -m venv "${VENV_DIR}"
"${VENV_DIR}/bin/pip" install --upgrade pip setuptools wheel
source "${VENV_DIR}/bin/activate"

# Install required Python packages for Flask and additional extensions
echo "Installing Flask and extensions..."
pip install Flask Flask-SQLAlchemy Flask-Cors gunicorn \
            Flask-SocketIO Flask-Celery Flask-Login

# Install Celery's Redis broker (or your preferred broker)
echo "Installing Celery's Redis broker (optional but recommended)..."
pip install redis

# Download latest release using GitHub token passed in from the command line
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

curl -L -H "Authorization: token ${GITHUB_TOKEN}" "$ZIP_URL" -o latest.zip

# Check if the ZIP file was downloaded successfully
if [[ ! -f latest.zip ]]; then
    echo "❌ Download failed! Please check your GitHub token and URL."
    exit 1
fi

unzip -o latest.zip

EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"
chmod +x "${APP_DIR}/server.py"
chmod +x "${APP_DIR}/server_test.sh"

# Ensure patchpilot user exists
if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "${APP_DIR}"

# Setup SQLite database (no need for init_db here, it's handled by server.py)
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Setup admin token
TOKEN_FILE="${APP_DIR}/admin_token.txt"
ENV_FILE="${APP_DIR}/admin_token.env"
if [[ ! -f "$TOKEN_FILE" ]]; then
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -d '=+/')
    echo "$ADMIN_TOKEN" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
else
    ADMIN_TOKEN=$(cat "$TOKEN_FILE")
fi
printf "ADMIN_TOKEN=%s\n" "$ADMIN_TOKEN" > "$ENV_FILE"
chmod 600 "$ENV_FILE"

# Setup systemd service
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

# Start the service *after* database initialization (which is handled by server.py)
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

# Output success message
SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete!"
echo "🌐 Dashboard: http://${SERVER_IP}:8080"
echo "🔑 Admin token is stored at ${TOKEN_FILE}"
