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
SYSTEMD_DIR="/etc/systemd/system"
PG_USER="patchpilot_user"
PG_DB="patchpilot_db"
PG_PASSWORD_FILE="/opt/patchpilot_client/postgres_password.txt"
PG_HBA_PATH="/etc/postgresql/15/main/pg_hba.conf"

# === Flags ===
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

# === System dependencies ===
echo "📦 Installing system packages (python3, venv, pip, curl, unzip, postgresql, libpq-dev)..."
if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    apt-get install -y python3 python3-venv python3-pip curl unzip postgresql postgresql-contrib libpq-dev
else
    echo "❌ Unsupported OS / package manager. Please install dependencies manually."
    exit 1
fi

# === Modify pg_hba.conf to allow passwordless authentication for user 'postgres' ===
echo "🛠️ Modifying pg_hba.conf to allow passwordless authentication for user 'postgres'..."

if [ -f "$PG_HBA_PATH" ]; then
    # Backup the original file first
    cp "$PG_HBA_PATH" "$PG_HBA_PATH.bak"
    echo "🔙 Backed up the original pg_hba.conf to pg_hba.conf.bak."

    # Replace all instances of 'scram-sha-256' with 'peer' for local connections
    sed -i 's/scram-sha-256/peer/g' "$PG_HBA_PATH"
    echo "⚙️ Updated pg_hba.conf to use peer authentication for local connections."

    # Reload PostgreSQL service to apply the changes
    systemctl reload postgresql
    echo "✅ PostgreSQL reloaded successfully to apply changes."
else
    echo "❌ pg_hba.conf not found at ${PG_HBA_PATH}. Please check your PostgreSQL installation."
    exit 1
fi

# === Automatically Generate a Secure Password ===
echo "🛠️ Generating a secure password for PostgreSQL user 'patchpilot_user'..."
PG_PASSWORD=$(openssl rand -base64 32)

# === PostgreSQL Setup ===
echo "🛠️  Creating PostgreSQL user and database..."

# Create the application directory before attempting to access it
mkdir -p "${APP_DIR}"

# Change to the application directory before running the PostgreSQL setup
cd "${APP_DIR}"

# Create the .pgpass file for automated authentication (without re-entering password)
PGPASSFILE="/tmp/.pgpass"
echo "localhost:5432:*:${PG_USER}:${PG_PASSWORD}" > $PGPASSFILE
chmod 600 $PGPASSFILE

# Ensure PostgreSQL commands are run by the 'postgres' user
runuser -u postgres -- bash -c "
psql -d postgres <<EOF
-- Step 1: Check if the 'patchpilot_user' role exists, create it if necessary
SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = '${PG_USER}';
EOF
"

# Create the user role if it doesn't exist
runuser -u postgres -- bash -c "
psql -d postgres -c \"DO \$\$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = '${PG_USER}') THEN
        CREATE ROLE ${PG_USER} WITH LOGIN PASSWORD '${PG_PASSWORD}';
    END IF;
END
\$\$;\"
"

# Step 2: Check if the 'patchpilot_db' database exists, create it if necessary
runuser -u postgres -- bash -c "
psql -d postgres -c \"SELECT 1 FROM pg_catalog.pg_database WHERE datname = '${PG_DB}'\"
"

# If database doesn't exist, create it
runuser -u postgres -- bash -c "
psql -d postgres -c \"CREATE DATABASE ${PG_DB} OWNER ${PG_USER};\"
"

# Clean up the .pgpass file
rm -f $PGPASSFILE

# === Save PostgreSQL password securely (Non-encrypted) ===
echo "[*] Saving PostgreSQL password to ${PG_PASSWORD_FILE} ..."

mkdir -p "$(dirname "$PG_PASSWORD_FILE")"
chmod 700 "$(dirname "$PG_PASSWORD_FILE")"  # Secure the directory
echo "$PG_PASSWORD" > "$PG_PASSWORD_FILE"
chmod 600 "$PG_PASSWORD_FILE"  # Only root and postgres can read the file

echo "✅ Password saved successfully. Only 'root' and 'postgres' can access it."

# === Optional cleanup ===
if [ "$FORCE_REINSTALL" = true ]; then
    echo "🛑 Stopping and disabling systemd services..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    systemctl disable "$SERVICE_NAME" 2>/dev/null || true

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

if [ "$UPGRADE" = true ] && [ -d "$VENV_DIR" ]; then
    if [ ! -f "${VENV_DIR}/bin/activate" ]; then
        echo "⚠️  Existing venv is broken, recreating..."
        rm -rf "$VENV_DIR"
    fi
fi

if [ ! -d "$VENV_DIR" ]; then
    echo "🐍 Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

# Check if pip is installed in venv, if not, install it
if [ ! -f "${VENV_DIR}/bin/pip" ]; then
    echo "⚠️ Pip not found, installing pip..."
    ${VENV_DIR}/bin/python -m ensurepip --upgrade
fi

# Check if pip works properly, otherwise fix it
if ! ${VENV_DIR}/bin/pip --version > /dev/null 2>&1; then
    echo "❌ Pip installation failed, trying to reinstall pip..."
    ${VENV_DIR}/bin/python -m pip install --upgrade pip setuptools wheel
fi

echo "⬆️  Activating venv and installing Python dependencies..."
source "${VENV_DIR}/bin/activate"

# Upgrade pip and setuptools
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

cd /  # Clean up temporary directory
rm -rf "${TMPDIR}"

# === Systemd service creation ===
echo "⚙️  Creating systemd service for PatchPilot..."
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=root
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
ExecStart=${VENV_DIR}/bin/gunicorn -w 4 -b 0.0.0.0:8080 server:app
Restart=always

[Install]
WantedBy=multi-user.target
EOF

# === Finalizing Installation ===
echo "🔄 Reloading systemd daemon..."
systemctl daemon-reload

echo "🚀 Enabling & starting PatchPilot service..."
systemctl enable --now "${SERVICE_NAME}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete! Visit: http://${SERVER_IP}:8080 to view the PatchPilot dashboard."
