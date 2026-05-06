#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/commandpilot"
SERVICE_NAME="pilot-core.service"
SYSTEMD_DIR="/etc/systemd/system"

GITHUB_USER="gitarman94"
GITHUB_REPO="CommandPilot"
BRANCH="main"
REPO_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}.git"

echo "Starting CommandPilot (pilot-core) setup..."

# --- OS check ---
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "Only Debian-based systems supported."; exit 1 ;;
    esac
else
    echo "Cannot determine OS."; exit 1
fi

# --- stop old service ---
systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
systemctl disable "${SERVICE_NAME}" 2>/dev/null || true

# --- install dependencies ---
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl git build-essential sqlite3 libsqlite3-dev ca-certificates

# --- install Go if missing ---
if ! command -v go >/dev/null 2>&1; then
    echo "Installing Go..."
    curl -LO https://go.dev/dl/go1.22.5.linux-amd64.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf go1.22.5.linux-amd64.tar.gz
fi

export PATH=$PATH:/usr/local/go/bin

# --- create system user ---
if ! id -u commandpilot >/dev/null 2>&1; then
    useradd -r -m -d /home/commandpilot -s /usr/sbin/nologin commandpilot || true
fi

# --- fetch source ---
echo "Fetching CommandPilot source..."
rm -rf "$APP_DIR"
git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$APP_DIR"

# --- build ---
echo "Building pilot-core..."
cd "$APP_DIR/pilot-core"

# IMPORTANT: use repo's go.mod (do NOT create one)
go mod tidy
go build -o pilot-core

# --- environment ---
cat > "${APP_DIR}/.env" <<EOF
DATABASE_PATH=${APP_DIR}/commandpilot.db
SERVER_ADDRESS=0.0.0.0
SERVER_PORT=8080
EOF

# --- permissions ---
chown -R commandpilot:commandpilot "$APP_DIR"
chmod +x "${APP_DIR}/pilot-core/pilot-core"

# --- systemd ---
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=CommandPilot pilot-core
After=network.target

[Service]
User=commandpilot
Group=commandpilot
WorkingDirectory=${APP_DIR}/pilot-core
EnvironmentFile=${APP_DIR}/.env
Environment=PATH=/usr/local/go/bin:/usr/bin:/bin
ExecStart=${APP_DIR}/pilot-core/pilot-core
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

# --- start service ---
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

# --- verify ---
if ! systemctl is-active --quiet "${SERVICE_NAME}"; then
    echo "pilot-core failed to start. Logs:"
    journalctl -u "${SERVICE_NAME}" -n 50 --no-pager
    exit 1
fi

IP=$(hostname -I | awk '{print $1}')

echo "----------------------------------------"
echo "CommandPilot is running"
echo "URL: http://${IP}:8080"
echo "Service: ${SERVICE_NAME}"
echo "----------------------------------------"