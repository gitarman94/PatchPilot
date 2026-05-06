#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/commandpilot"
SERVICE_NAME="pilot-core.service"
SYSTEMD_DIR="/etc/systemd/system"

GITHUB_USER="gitarman94"
GITHUB_REPO="commandpilot"
BRANCH="main"
REPO_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}.git"

echo "Starting KentroCore setup..."

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
apt-get install -y -qq curl git build-essential sqlite3 libsqlite3-dev

# --- install Go if missing ---
if ! command -v go >/dev/null 2>&1; then
    echo "Installing Go..."
    curl -LO https://go.dev/dl/go1.22.5.linux-amd64.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf go1.22.5.linux-amd64.tar.gz
fi

export PATH=$PATH:/usr/local/go/bin

# --- create system user ---
if ! id -u kentro >/dev/null 2>&1; then
    useradd -r -m -d /home/kentro -s /usr/sbin/nologin kentro || true
fi

# --- fetch source ---
echo "Fetching Kentro source..."
rm -rf "$APP_DIR"
git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$APP_DIR"

cd "$APP_DIR"

# If you keep code in a subdir, adjust here
if [[ -d "kentrocore" ]]; then
    cd kentrocore
fi

# --- ensure go module ---
if [[ ! -f "go.mod" ]]; then
    go mod init kentro
fi

# --- dependencies ---
echo "Fetching Go dependencies..."
go mod tidy

# --- build ---
echo "Building KentroCore..."
go build -o kentrocore

# --- environment ---
cat > .env <<EOF
DATABASE_PATH=${APP_DIR}/kentro.db
SERVER_ADDRESS=0.0.0.0
SERVER_PORT=8080
EOF

# --- permissions ---
chown -R kentro:kentro "$APP_DIR"
chmod +x kentrocore

# --- systemd ---
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=KentroCore Server
After=network.target

[Service]
User=kentro
Group=kentro
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_DIR}/.env
Environment=PATH=/usr/local/go/bin:/usr/bin:/bin
ExecStart=${APP_DIR}/kentrocore
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
    echo "KentroCore failed to start. Logs:"
    journalctl -u "${SERVICE_NAME}" -n 50 --no-pager
    exit 1
fi

IP=$(hostname -I | awk '{print $1}')

echo "----------------------------------------"
echo "KentroCore is running"
echo "URL: http://${IP}:8080"
echo "----------------------------------------"