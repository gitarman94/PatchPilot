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
PASSWORD_FILE="${APP_DIR}/postgresql_pwd.txt"  # New password file

# === Flags ===
FORCE_REINSTALL=false
UNINSTALL=false

for arg in "$@"; do
    case "$arg" in
        --force)
            FORCE_REINSTALL=true
            echo "⚠️  Force reinstallation enabled: removing previous installation and reinstalling."
            ;;
        --uninstall)
            UNINSTALL=true
            echo "🛑 Uninstall mode enabled: removing PatchPilot and all dependencies."
            ;;
    esac
done

# === Uninstall Process ===
if [ "$UNINSTALL" = true ]; then
    echo "🛑 Uninstalling PatchPilot..."

    # Stop and disable systemd services if running
    echo "🛑 Stopping and disabling systemd services..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl disable "$SERVICE_NAME" 2>/dev/null || true
    systemctl stop "$SELF_UPDATE_TIMER" 2>/dev/null || true
    systemctl disable "$SELF_UPDATE_TIMER" 2>/dev/null || true

    # Kill running instances of PatchPilot
    echo "☠️ Killing all running patchpilot server.py instances..."
    PIDS=$(pgrep -f "server.py" | grep -v "^$$\$" || true)
    if [ -n "$PIDS" ]; then
        for pid in $PIDS; do
            if [ "$pid" -eq "$$" ]; then
                continue
            fi
            echo "Sending SIGTERM to pid $pid"
            set +e
            kill -15 "$pid" || true
            sleep 2
            if kill -0 "$pid" 2>/dev/null; then
                echo "Pid $pid still alive after SIGTERM, sending SIGKILL"
                kill -9 "$pid" || true
            else
                echo "Pid $pid terminated cleanly"
            fi
            set -e
        done
    else
        echo "No running patchpilot server.py processes found."
    fi

    # Remove PostgreSQL database and user
    echo "🧹 Uninstalling PostgreSQL database and user..."
    sudo -u postgres psql -c "DROP DATABASE IF EXISTS patchpilot_db;" || true
    sudo -u postgres psql -c "DROP USER IF EXISTS patchpilot_user;" || true

    # Remove the application directory
    echo "🧹 Removing PatchPilot installation at $APP_DIR..."
    rm -rf "$APP_DIR"

    # Clean up virtual environment
    echo "🧹 Removing virtual environment..."
    rm -rf "$VENV_DIR"

    echo "✅ Uninstallation complete."
    exit 0
fi

# === System dependencies ===
echo "📦 Installing system packages (python3, venv, pip, curl, unzip, postgresql, libpq-dev)..."
if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y python3 python3-venv python3-pip curl unzip postgresql postgresql-contrib libpq-dev
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y python3 python3-venv python3-pip curl unzip postgresql-server postgresql-contrib libpq-dev
elif command -v yum >/dev/null 2>&1; then
    yum install -y python3 python3-venv python3-pip curl unzip postgresql postgresql-contrib libpq-dev
else
    echo "❌ Unsupported OS / package manager. Please install dependencies manually."
    exit 1
fi

# === PostgreSQL Setup ===
if [ "$FORCE_REINSTALL" = true ] || [ ! -f "$APP_DIR/server.py" ]; then
    # Generate a random password for the PostgreSQL user
    PASSWORD=$(openssl rand -base64 16)

    # Save the password to the file for later reference
    echo $PASSWORD > "$PASSWORD_FILE"
    echo "⚠️  The password for the PostgreSQL user 'patchpilot_user' has been saved to: $PASSWORD_FILE"

    echo "🔄 Setting up PostgreSQL..."

    # Create PostgreSQL user and database with the generated password
    sudo -u postgres psql -c "CREATE USER patchpilot_user WITH PASSWORD '$PASSWORD';" || true
    sudo -u postgres psql -c "CREATE DATABASE patchpilot_db;" || true
    sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE patchpilot_db TO patchpilot_user;" || true
fi

# === Optional cleanup (if not uninstall) ===
if [ "$FORCE_REINSTALL" = true ] || [ ! -f "$APP_DIR/server.py" ]; then
    # If force or nothing installed, stop previous services and remove old files
    echo "🛑 Stopping and disabling systemd services..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl disable "$SERVICE_NAME" 2>/dev/null || true
    systemctl stop "$SELF_UPDATE_TIMER" 2>/dev/null || true
    systemctl disable "$SELF_UPDATE_TIMER" 2>/dev/null || true

    echo "☠️ Killing all running patchpilot server.py instances..."
    PIDS=$(pgrep -f "server.py" | grep -v "^$$\$" || true)
    if [ -n "$PIDS" ]; then
        for pid in $PIDS; do
            if [ "$pid" -eq "$$" ]; then
                continue
            fi
            echo "Sending SIGTERM to pid $pid"
            set +e
            kill -15 "$pid" || true
            sleep 2
            if kill -0 "$pid" 2>/dev/null; then
                echo "Pid $pid still alive after SIGTERM, sending SIGKILL"
                kill -9 "$pid" || true
            else
                echo "Pid $pid terminated cleanly"
            fi
            set -e
        done
    else
        echo "No running patchpilot server.py processes found."
    fi

    echo "🧹 Removing previous installation at $APP_DIR..."
    rm -rf "$APP_DIR"
fi

# === Create directories ===
mkdir -p "${APP_DIR}"

# === Virtual environment setup ===
if [ "$FORCE_REINSTALL" = true ] && [ -d "$VENV_DIR" ]; then
    echo "🧹 Removing old virtual environment..."
    rm -rf "$VENV_DIR"
fi

if [ ! -d "$VENV_DIR" ]; then
    echo "🐍 Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

echo "⬆️  Activating venv and installing Python dependencies..."
source "${VENV_DIR}/bin/activate"

# Ensure pip/bootstrap exists
python -m ensurepip --upgrade
pip install --upgrade pip setuptools wheel

# Install/update core dependencies
pip install --upgrade Flask Flask-SQLAlchemy flask_cors gunicorn psycopg2

# === Download repo ===
TMPDIR=$(mktemp -d)
cd "${TMPDIR}"
echo "⬇️  Downloading repository ZIP from GitHub..."
curl -L "${ZIP_URL}" -o latest.zip

unzip -o latest.zip
EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")

if [ -z "${EXTRACTED_DIR}" ]; then
    echo "❌ Failed to locate extracted repo directory."
    exit 1
fi

echo "📂 Copying files into ${APP_DIR}"
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"

# === Permissions ===
chmod +x "${APP_DIR}/server.py"
if [ -f "${APP_DIR}/${SELF_UPDATE_SCRIPT}" ]; then
    chmod +x "${APP_DIR}/${SELF_UPDATE_SCRIPT}"
else
    echo "⚠️  Warning: Self-update script '${SELF_UPDATE_SCRIPT}' not found. Skipping."
fi

cd /
rm -rf "${TMPDIR}"

# === Initialize Database ===
if [ ! -f "${APP_DIR}/server.py" ]; then
    echo "🔄 Checking if database exists and initializing if needed..."
    source "${VENV_DIR}/bin/activate"

    # Change to the app directory before running the Python command
    cd "${APP_DIR}"

    # Now run the python command with the correct context
    python -c "
from server import app, db
with app.app_context():
    db.create_all()
"
fi

# === Systemd service ===
echo "🛎️  Creating systemd service: ${SERVICE_NAME}"
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=root
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
ExecStart=${VENV_DIR}/bin/gunicorn --bind 0.0.0.0:8080 server:app
Restart=always

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "${SERVICE_NAME}"
systemctl start "${SERVICE_NAME}"

echo "✅ Installation complete."
