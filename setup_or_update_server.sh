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

# Cleanup old install first if --force is used
if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."

    # Stop and disable systemd service if it exists
    if systemctl list-units --full -all | grep -q "^${SERVICE_NAME}"; then
        echo "🛑 Stopping systemd service ${SERVICE_NAME}..."
        systemctl stop "${SERVICE_NAME}" || true
        systemctl disable "${SERVICE_NAME}" || true
    fi

    # Kill any running processes in the application directory
    pids=$(pgrep -f "^${APP_DIR}/target/release/patchpilot_server$" || true)
    if [[ -n "$pids" ]]; then
        for pid in $pids; do
            echo "🛑 Terminating running process $pid..."
            kill -15 "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    echo "🧹 Removing old files..."
    rm -rf /opt/patchpilot_server
    rm -rf /opt/patchpilot_install*
    rm -rf .rustup
fi

# Create the required directories before usage
mkdir -p /opt/patchpilot_install
mkdir -p /opt/patchpilot_server
chown -R patchpilot:patchpilot /opt/patchpilot_server
chmod -R 644 /opt/patchpilot_server

# Download latest release from GitHub (no token required for public repo)
cd /opt/patchpilot_install
curl -L "$ZIP_URL" -o latest.zip

# Check if the ZIP file was downloaded successfully
if [[ ! -f latest.zip ]]; then
    echo "❌ Download failed! Please check the URL."
    exit 1
fi

unzip -o latest.zip
cd "${APP_DIR}"
mv -f "/opt/patchpilot_install/PatchPilot-main/patchpilot_server" "${APP_DIR}"
mv -f "/opt/patchpilot_install/PatchPilot-main/templates" "${APP_DIR}"
mv -f "/opt/patchpilot_install/PatchPilot-main/server_test.sh" "${APP_DIR}"
rm -rf "/opt/patchpilot_install"

# Install system packages
echo "📦 Installing required packages..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config libsqlite3-dev

# Install Rust if not installed (directly in /opt/patchpilot_server)
if ! command -v cargo >/dev/null 2>&1; then
    echo "⚙️ Installing Rust in /opt/patchpilot_server..."
    mkdir -p "${APP_DIR}/.cargo" "${APP_DIR}/.rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
    export CARGO_HOME="${APP_DIR}/.cargo"
    export RUSTUP_HOME="${APP_DIR}/.rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
    # Set default toolchain to stable
    "${CARGO_HOME}/bin/rustup" default stable
else
    echo "✅ Rust is already installed."
    export CARGO_HOME="${APP_DIR}/.cargo"
    export RUSTUP_HOME="${APP_DIR}/.rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi

# Set up SQLite database
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Set up log file and permissions
touch /opt/patchpilot_server/server.log

# Setup admin token
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

# Set ownership of the entire directory to patchpilot
chown -R patchpilot:patchpilot "${APP_DIR}"

# Build the Rust application
cd "${APP_DIR}"
echo "🔨 Building the Rust application..."
cargo build --release

# Setup systemd service
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Patch Management Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${ENV_FILE}

ExecStart=${APP_DIR}/target/release/patchpilot_server
ExecReload=/bin/kill -s HUP \$MAINPID
Restart=always
RestartSec=10
StandardOutput=append:${APP_DIR}/server.log
StandardError=append:${APP_DIR}/server.log

[Install]
WantedBy=multi-user.target
EOF

# Start the service
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

# Clean up the temporary client files
rm -r /opt/patchpilot_server/patchpilot_client_rust/
rm /opt/patchpilot_server/setup_or_update_client*

# Output success message
SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete!"
echo "🌐 Dashboard: http://${SERVER_IP}:8080"
echo "🔑 Admin token is stored at ${TOKEN_FILE}"
