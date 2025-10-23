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
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y python3 python3-venv python3-pip curl unzip postgresql-server postgresql-contrib libpq-dev
elif command -v yum >/dev/null 2>&1; then
    yum install -y python3 python3-venv python3-pip curl unzip postgresql postgresql-contrib libpq-dev
else
    echo "❌ Unsupported OS / package manager. Please install dependencies manually."
    exit 1
fi

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

# === PostgreSQL Setup ===
echo "🔐 Checking PostgreSQL setup..."

POSTGRES_PASSWORD_FILE="${APP_DIR}/postgresql_pwd.txt"
if [ ! -f "$POSTGRES_PASSWORD_FILE" ]; then
    echo "❌ PostgreSQL password file '$POSTGRES_PASSWORD_FILE' not found! Creating one now..."
    
    # Generate a random password and save it in the password file
    POSTGRES_PASSWORD=$(openssl rand -base64 12)
    echo "$POSTGRES_PASSWORD" > "$POSTGRES_PASSWORD_FILE"
    echo "Password for PostgreSQL created and saved to $POSTGRES_PASSWORD_FILE."
    
    # === Fix Authentication Issue ===
    echo "🔧 Fixing PostgreSQL authentication to allow password-based login with scram-sha-256..."

    # Attempt to locate pg_hba.conf in common locations
    PG_HBA_CONF=""
    if [ -d "/etc/postgresql" ]; then
        PG_HBA_CONF=$(find /etc/postgresql -name "pg_hba.conf" 2>/dev/null | head -n 1)
    fi
    if [ -z "$PG_HBA_CONF" ] && [ -d "/var/lib/pgsql" ]; then
        PG_HBA_CONF=$(find /var/lib/pgsql -name "pg_hba.conf" 2>/dev/null | head -n 1)
    fi
    
    if [ -z "$PG_HBA_CONF" ]; then
        echo "❌ pg_hba.conf file not found in common locations."
        exit 1
    fi

    echo "📂 Found pg_hba.conf at $PG_HBA_CONF"

    # Modify pg_hba.conf to use scram-sha-256 authentication for both local and host
    echo "📂 Modifying pg_hba.conf for password authentication using scram-sha-256..."
    
    # Use an alternative delimiter to avoid issues with slashes
    sed -i 's#^local\s*all\s*postgres\s*peer#local   all             postgres                                scram-sha-256#' "$PG_HBA_CONF"
    sed -i 's#^#host\s*all\s*postgres\s*127.0.0.1/32\s*peer#host    all             postgres        127.0.0.1/32            scram-sha-256#' "$PG_HBA_CONF"
    sed -i 's#^#host\s*all\s*postgres\s*::1/128\s*peer#host    all             postgres        ::1/128                 scram-sha-256#' "$PG_HBA_CONF"
    
    # Restart PostgreSQL to apply changes
    echo "🔄 Restarting PostgreSQL..."
    if command -v systemctl >/dev/null 2>&1; then
        systemctl restart postgresql
    else
        service postgresql restart
    fi

    # Update PostgreSQL password
    echo "Updating PostgreSQL password for user 'postgres'..."
    if [ "$(id -u)" -eq 0 ]; then
        psql -U postgres -c "ALTER USER postgres WITH PASSWORD '$POSTGRES_PASSWORD';"
        echo "PostgreSQL password has been updated for user 'postgres'."
    else
        if command -v sudo >/dev/null 2>&1; then
            sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD '$POSTGRES_PASSWORD';"
            echo "PostgreSQL password has been updated for user 'postgres' using sudo."
        else
            echo "❌ sudo is not available, and you're not running as root. Unable to update PostgreSQL password."
            exit 1
        fi
    fi
fi

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
cat <<EOF > "${SYSTEMD_DIR}/${SERVICE_NAME}"
[Unit]
Description=PatchPilot Server
After=network.target postgresql.service

[Service]
ExecStart=${APP_DIR}/venv/bin/python ${APP_DIR}/server.py
WorkingDirectory=${APP_DIR}
User=root
Group=root
Environment="PATH=${APP_DIR}/venv/bin"
Environment="FLASK_APP=${APP_DIR}/server.py"
Restart=always

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload

# === Start the service ===
echo "🚀 Starting PatchPilot server..."
systemctl enable --now "${SERVICE_NAME}"

echo "✅ Installation complete! The PatchPilot server is running as a systemd service."
