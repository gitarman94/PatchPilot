#!/usr/bin/env bash
set -euo pipefail

# Configuration
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
VENV_DIR="${APP_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

PG_USER="patchpilot_user"
PG_DB="patchpilot_db"
PG_PASSWORD_FILE="/opt/patchpilot_server/postgresql_pwd.txt"
PG_HBA_PATH="/etc/postgresql/15/main/pg_hba.conf"

# Flags
FORCE_REINSTALL=false
UPGRADE=false

for arg in "$@"; do
    case "$arg" in
        --force)   FORCE_REINSTALL=true;  echo "⚠️  Force reinstall enabled." ;;
        --upgrade) UPGRADE=true;          echo "⬆️  Upgrade mode enabled." ;;
    esac
done

# Install system dependencies (non‑interactive)
export DEBIAN_FRONTEND=noninteractive
echo "📦 Installing required packages..."
if command -v apt-get >/dev/null 2>&1; then
    apt-get update -qq
    apt-get install -y -qq \
        python3 python3-venv python3-pip curl unzip \
        postgresql postgresql-contrib libpq-dev
else
    echo "❌ Unsupported OS – apt-get not found."
    exit 1
fi

# Adjust pg_hba.conf – only the local line (scram‑sha‑256 → peer)
echo "🔧 Updating pg_hba.conf for password‑less local auth..."
if [[ -f "$PG_HBA_PATH" ]]; then
    cp "$PG_HBA_PATH" "${PG_HBA_PATH}.bak"
    echo "🔙 Backup created at ${PG_HBA_PATH}.bak"
    sed -i '/^local[[:space:]]\+all[[:space:]]\+all[[:space:]]\+scram-sha-256/s//peer/' "$PG_HBA_PATH"
    systemctl reload postgresql
    echo "✅ pg_hba.conf updated and PostgreSQL reloaded."
else
    echo "❌ $PG_HBA_PATH not found – aborting."
    exit 1
fi

# Generate a strong PostgreSQL password
echo "🔐 Generating a secure password for ${PG_USER}..."
PG_PASSWORD=$(openssl rand -base64 32 | tr -d '=+/')
sleep 5  # give PostgreSQL a moment to be ready

# PostgreSQL role / database creation (idempotent)
echo "🗄️  Setting up PostgreSQL role & database..."
mkdir -p "${APP_DIR}"
cd "${APP_DIR}"

# Temporary .pgpass for password‑less psql
PGPASSFILE="/tmp.$$"
echo "localhost:5432:*:${PG_USER}:${PG_PASSWORD}" > "${PGPASSFILE}"
chmod 600 "${PGPASSFILE}"
export PGPASSFILE="${PGPASSFILE}"

runuser -u postgres -- bash -c "
psql -v dbname='${PG_DB}' -v dbowner='${PG_USER}' <<'EOSQL'
DO \$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'dbowner') THEN
        CREATE ROLE :\"dbowner\" WITH LOGIN PASSWORD '${PG_PASSWORD}';
    END IF;
END
\$(\$)\$;

DO \$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = :'dbname') THEN
        EXECUTE format('CREATE DATABASE %I OWNER %I', :'dbname', :'dbowner');
    END IF;
END
\$(\$)\$;
EOSQL
"

unset PGPASSFILE
rm -f "${PGPASSFILE}"

# Persist the generated password (readable only by root & postgres)
echo "🔐 Storing PostgreSQL password at ${PG_PASSWORD_FILE}..."
mkdir -p "$(dirname "${PG_PASSWORD_FILE}")"
chmod 700 "$(dirname "${PG_PASSWORD_FILE}")"
echo "${PG_PASSWORD}" > "${PG_PASSWORD_FILE}"
chmod 600 "${PG_PASSWORD_FILE}"
echo "✅ Password stored securely."

# Optional force‑reinstall cleanup
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Removing previous installation..."
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

    echo "Removing ${APP_DIR}..."
    rm -rf "${APP_DIR}"
fi

# Create required directories
mkdir -p "${APP_DIR}"
mkdir -p "${VENV_DIR}"

# Virtual environment setup
if [[ "$FORCE_REINSTALL" = true && -d "$VENV_DIR" ]]; then
    echo "Removing old virtual environment..."
    rm -rf "$VENV_DIR"
fi

if [[ "$UPGRADE" = true && -d "$VENV_DIR" && ! -x "${VENV_DIR}/bin/activate" ]]; then
    echo "Existing venv broken – recreating..."
    rm -rf "$VENV_DIR"
fi

if [[ ! -d "$VENV_DIR" ]]; then
    echo "🐍 Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

# Ensure pip works inside the venv
if [[ ! -x "${VENV_DIR}/bin/pip" ]]; then
    echo "Installing pip into venv..."
    "${VENV_DIR}/bin/python" -m ensurepip --upgrade
fi

if ! "${VENV_DIR}/bin/pip" --version >/dev/null 2>&1; then
    echo "Reinstalling pip..."
    "${VENV_DIR}/bin/python" -m pip install --upgrade pip setuptools wheel
fi

# Install Python dependencies
echo "⬆️  Installing/upgrading Python packages..."
source "${VENV_DIR}/bin/activate"
pip install --upgrade pip setuptools wheel
pip install --upgrade Flask Flask-SQLAlchemy flask_cors gunicorn psycopg2-binary

# Pull latest source code from GitHub
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

echo "⬇️  Downloading repository ZIP..."
curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
if [[ -z "$EXTRACTED_DIR" ]]; then
    echo "❌ Failed to locate extracted repo directory."
    exit 1
fi

echo "Copying files to ${APP_DIR}..."
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"
chmod +x "${APP_DIR}/server.py"

# Create a dedicated, unprivileged service user (if it does not exist)
if ! id -u patchpilot >/dev/null 2>&1; then
    echo "Creating service user 'patchpilot'..."
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "${APP_DIR}"

# Systemd service definition (runs as unprivileged user)
echo "Creating systemd unit file..."
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
Environment="PATH=${VENV_DIR}/bin"
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
echo "✅ Installation complete! Access the dashboard at http://${SERVER_IP}:8080"
