#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/patchpilot_server"
INSTALL_LOG="${APP_DIR}/install.log"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

mkdir -p "$APP_DIR" /opt/patchpilot_install

echo "🛠️ Starting PatchPilot server setup at $(date)..."

GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

FORCE_REINSTALL=false
UPGRADE=false

# Parse args
for arg in "$@"; do
    case "$arg" in
        --force) FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE=true ;;
    esac
done

# OS check
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "❌ Only Debian-based systems supported."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS."
    exit 1
fi

# Cleanup if --force
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."
    systemctl stop "${SERVICE_NAME}" || true
    systemctl disable "${SERVICE_NAME}" || true

    pkill -f "^${APP_DIR}/target/release/patchpilot_server$" || true
    rm -rf "${APP_DIR}" /opt/patchpilot_install*
    mkdir -p /opt/patchpilot_install "$APP_DIR"
fi

# Download & unpack latest release
cd /opt/patchpilot_install
curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

cd "${APP_DIR}"
mv /opt/patchpilot_install/PatchPilot-main/patchpilot_server/* "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/templates "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/server_test.sh "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/static "$APP_DIR"
chmod +x "$APP_DIR/server_test.sh"
rm -rf /opt/patchpilot_install

# Install required packages
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config sqlite3 libsqlite3-dev

# Rust environment
export CARGO_HOME="${APP_DIR}/.cargo"
export RUSTUP_HOME="${APP_DIR}/.rustup"
export PATH="${CARGO_HOME}/bin:$PATH"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

if [[ ! -x "${CARGO_HOME}/bin/cargo" ]]; then
    echo "🛠️ Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi

# DB setup
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chmod 755 "$SQLITE_DB"

# Build Rust app
cd "$APP_DIR"
"${CARGO_HOME}/bin/cargo" build --release

# Rocket config
cat > "${APP_DIR}/Rocket.toml" <<EOF
[default]
address = "0.0.0.0"
port = 8080
log = "normal"

[release]
log = "critical"
EOF

# Environment for systemd
APP_ENV_FILE="${APP_DIR}/.env"
cat > "${APP_ENV_FILE}" <<EOF
DATABASE_URL=sqlite:${APP_DIR}/patchpilot.db
RUST_LOG=info
ROCKET_ADDRESS=0.0.0.0
ROCKET_PORT=8080
EOF

ROCKET_SECRET_KEY=$(openssl rand -base64 48 | tr -d '=+/')
echo "ROCKET_SECRET_KEY=${ROCKET_SECRET_KEY}" >> "$APP_ENV_FILE"
chmod 755 "$APP_ENV_FILE"

# Admin token
TOKEN_FILE="${APP_DIR}/admin_token.txt"
if [[ ! -f "$TOKEN_FILE" ]]; then
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -d '=+/')
    echo "$ADMIN_TOKEN" > "$TOKEN_FILE"
    chmod 755 "$TOKEN_FILE"
else
    ADMIN_TOKEN=$(cat "$TOKEN_FILE")
fi

# Ensure patchpilot user exists
if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "$APP_DIR"

# Permissions
find "$APP_DIR" -type d -exec chmod 755 {} \;
find "$APP_DIR" -type f -exec chmod 755 {} \;
chmod +x "$APP_DIR/target/release/patchpilot_server"

# Setup systemd service
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=PatchPilot Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_ENV_FILE}
ExecStart=${APP_DIR}/target/release/patchpilot_server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "$SERVICE_NAME"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete!"
echo "🌐 Dashboard: http://${SERVER_IP}:8080"
echo "🔑 Admin token stored at ${TOKEN_FILE}"
