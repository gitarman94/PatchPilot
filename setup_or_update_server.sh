#!/usr/bin/env bash
set -euo pipefail

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

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq python3 python3-venv python3-pip curl unzip openssl

if [[ "$FORCE_REINSTALL" = true ]]; then
    systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    systemctl disable "${SERVICE_NAME}" 2>/dev/null || true
    pids=$(pgrep -f "server.py" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi
    rm -rf "${APP_DIR}"
fi

mkdir -p "${APP_DIR}/updates"

python3 -m venv "${VENV_DIR}"
"${VENV_DIR}/bin/pip" install --upgrade pip setuptools wheel
source "${VENV_DIR}/bin/activate"
pip install --upgrade Flask Flask-SQLAlchemy flask_cors gunicorn

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"
chmod +x "${APP_DIR}/server.py"

if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "${APP_DIR}"

SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

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

systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "Installation complete. Dashboard: http://${SERVER_IP}:8080"
echo "Admin token is stored at ${TOKEN_FILE}"
