#!/usr/bin/env bash
set -euo pipefail

GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

APP_DIR="/opt/patchpilot_server"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"

FORCE_REINSTALL=false
UPGRADE=false

# Parse command-line arguments
for arg in "$@"; do
    case "$arg" in
        --force)   FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE=true ;;
    esac
done

# Check if the OS is supported (Debian-based systems)
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

# Cleanup old install if --force is used
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."
    if systemctl list-units --full -all | grep -q "^${SERVICE_NAME}"; then
        echo "🛑 Stopping systemd service ${SERVICE_NAME}..."
        systemctl stop "${SERVICE_NAME}" || true
        systemctl disable "${SERVICE_NAME}" || true
    fi

    sed -i '/CARGO_HOME/d' /etc/environment
    sed -i '/RUSTUP_HOME/d' /etc/environment
    sed -i '/PATH=.*\/opt\/patchpilot_server\/.cargo\/bin/d' /etc/environment
    
    pids=$(pgrep -f "^${APP_DIR}/target/release/patchpilot_server$" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "🛑 Terminating running process $pid..."
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    rm -rf /opt/patchpilot_server
    rm -rf /opt/patchpilot_install*
    rm -rf "$HOME/.cargo" "$HOME/.rustup"
    rm -f /usr/local/bin/cargo /usr/local/bin/rustup
fi

# Create required directories
mkdir -p /opt/patchpilot_install
mkdir -p /opt/patchpilot_server

# Download latest release
cd /opt/patchpilot_install
curl -L "$ZIP_URL" -o latest.zip
if [[ ! -f latest.zip ]]; then
    echo "❌ Download failed! Please check the URL."
    exit 1
fi

unzip -o latest.zip
cd "${APP_DIR}"
mv /opt/patchpilot_install/PatchPilot-main/patchpilot_server/* ${APP_DIR}
mv /opt/patchpilot_install/PatchPilot-main/templates ${APP_DIR}
mv /opt/patchpilot_install/PatchPilot-main/server_test.sh ${APP_DIR}
chmod +x ${APP_DIR}/server_test.sh
rm -rf "/opt/patchpilot_install"

# Install system packages
echo "📦 Installing required packages..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config libsqlite3-dev

# Install Rust if not installed
if ! command -v cargo >/dev/null 2>&1; then
    echo "🛠️ Setting up system-wide environment variables..."
    echo "CARGO_HOME=/opt/patchpilot_server/.cargo" >> /etc/environment
    echo "RUSTUP_HOME=/opt/patchpilot_server/.rustup" >> /etc/environment
    echo "PATH=\$CARGO_HOME/bin:\$PATH" >> /etc/environment
    
    echo "⚙️ Installing Rust..."
    mkdir -p "${APP_DIR}/.cargo" "${APP_DIR}/.rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path

    export CARGO_HOME="${APP_DIR}/.cargo"
    export RUSTUP_HOME="${APP_DIR}/.rustup"
    export PATH="$CARGO_HOME/bin:$PATH"

    "${CARGO_HOME}/bin/rustup" default stable
    "${CARGO_HOME}/bin/cargo" --version
else
    echo "✅ Rust is already installed."
    export CARGO_HOME="${APP_DIR}/.cargo"
    export RUSTUP_HOME="${APP_DIR}/.rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi

# SQLite database setup
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Log file
touch /opt/patchpilot_server/server.log

# Admin token
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

# Ensure patchpilot user exists
if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -s /usr/sbin/nologin patchpilot
fi

# Ownership
chown -R patchpilot:patchpilot "${APP_DIR}"

# Build Rust app
cd "${APP_DIR}"
echo "🔨 Building the Rust application..."
/opt/patchpilot_server/.cargo/bin/cargo build --release

# Create Rocket.toml
cat > "${APP_DIR}/Rocket.toml" <<EOF
[default]
address = "0.0.0.0"
port = 8080
log = "normal"

[release]
log = "critical"
EOF

# Create .env for systemd
APP_ENV_FILE="${APP_DIR}/.env"
cat > "${APP_ENV_FILE}" <<EOF
DATABASE_URL=sqlite://${APP_DIR}/patchpilot.db
RUST_LOG=info
EOF

# Permissions
chown -R patchpilot:patchpilot ${APP_DIR}
find /opt/patchpilot_server -type d -exec chmod 755 {} \;
find /opt/patchpilot_server -type f -exec chmod 644 {} \;
chmod +x /opt/patchpilot_server/target/release/patchpilot_server

# Setup systemd service (simplified)
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
StandardOutput=append:${APP_DIR}/server.log
StandardError=append:${APP_DIR}/server.log

[Install]
WantedBy=multi-user.target
EOF

# Reload and start service
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete!"
echo "🌐 Dashboard: http://${SERVER_IP}:8080"
echo "🔑 Admin token is stored at ${TOKEN_FILE}"
