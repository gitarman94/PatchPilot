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
PASSWORD_FILE="${APP_DIR}/postgresql_pwd.txt"  # Path to save the password

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

    # Kill running instances of PatchPilot
    echo "☠️ Killing all running patchpilot server.py instances..."
    PIDS=$(pgrep -f "server.py" || true)
    if [ -n "$PIDS" ]; then
        for pid in $PIDS; do
            kill -9 "$pid" || true
        done
    fi

    # Remove PostgreSQL database and user
    echo "🧹 Removing PostgreSQL database and user..."
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

# === PostgreSQL Setup ===
if [ "$FORCE_REINSTALL" = true ] || [ ! -f "$APP_DIR/server.py" ]; then
    echo "🔄 Setting up PostgreSQL..."

    # Generate a random password for PostgreSQL
    PG_PASSWORD=$(openssl rand -base64 16 | tr -d '[:space:]')

    # Ensure the password file is clean and write the password to it
    echo -n "$PG_PASSWORD" > "$PASSWORD_FILE"

    # Confirm if the password was written to the file
    if [ -f "$PASSWORD_FILE" ]; then
        echo "✔️ Password successfully written to: $PASSWORD_FILE"
    else
        echo "❌ Failed to write password to $PASSWORD_FILE"
    fi

    # Create PostgreSQL user and database with the generated password
    sudo -u postgres psql -c "CREATE USER patchpilot_user WITH PASSWORD '$PG_PASSWORD';" || true
    sudo -u postgres psql -c "CREATE DATABASE patchpilot_db;" || true
    sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE patchpilot_db TO patchpilot_user;" || true
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

# === Virtual environment setup ===
if [ ! -d "$VENV_DIR" ]; then
    echo "🐍 Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

echo "⬆️  Activating venv and installing Python dependencies..."
source "${VENV_DIR}/bin/activate"
python -m ensurepip --upgrade
pip install --upgrade pip setuptools wheel
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

cd "$APP_DIR"

# === Update configuration for database ===
echo "📄 Updating database configuration..."

# Read password from file
DB_PASSWORD=$(cat "$PASSWORD_FILE")

# Update server.py to use the correct PostgreSQL password
sed -i "s|postgresql://patchpilot_user:.*@localhost/patchpilot_db|postgresql://patchpilot_user:${DB_PASSWORD}@localhost/patchpilot_db|" "${APP_DIR}/server.py"

# === Clean up unnecessary files ===
echo "🧹 Cleaning up unnecessary files..."

# Remove client setup files
rm -f "${APP_DIR}/setup_or_update_client.ps1"
rm -f "${APP_DIR}/setup_or_update_client.sh"
rm -f "${APP_DIR}/setup_or_update_server.sh"

# Remove the client source code (rust code and other unused files)
rm -rf "${APP_DIR}/patchpilot_client_rust"

# Remove README and LICENSE files (optional, if you want to keep the server clean)
rm -f "${APP_DIR}/LICENSE"
rm -f "${APP_DIR}/README.md"

# Remove the templates directory if not needed
rm -rf "${APP_DIR}/templates"

# === Systemd service setup ===
echo "⚙️  Enabling systemd service for PatchPilot..."
cp "${APP_DIR}/patchpilot_server.service" "$SYSTEMD_DIR/"
systemctl enable "$SERVICE_NAME"
systemctl start "$SERVICE_NAME"

# === Final message with URL ===
SERVER_IP=$(hostname -I | awk '{print $1}')   # Grabs the server's IP
echo "✅ Installation complete! Visit: http://${SERVER_IP}:8080 to view the PatchPilot dashboard."
