#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/patchpilot_server"
INSTALL_LOG="${APP_DIR}/install.log"
SERVICE_NAME="patchpilot_server.service"
SOCKET_NAME="patchpilot_server.socket"
SYSTEMD_DIR="/etc/systemd/system"

mkdir -p "$APP_DIR" /opt/patchpilot_install

echo "🛠️ Starting PatchPilot server setup at $(date)..."

GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

FORCE_REINSTALL=false
UPGRADE=false
BUILD_MODE="debug"

for arg in "$@"; do
    case "$arg" in
        --force) FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE=true ;;
        --debug) BUILD_MODE="debug" ;;
        --release) BUILD_MODE="release" ;;
    esac
done

# Only Debian-based systems supported
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "❌ Only Debian-based systems supported."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS."; exit 1
fi

# Stop and remove any running instances before reinstall
echo "🛑 Ensuring no running PatchPilot server instances..."
# Be tolerant if units don't exist; don't call tools that may be absent
systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
systemctl stop "${SOCKET_NAME}" 2>/dev/null || true
systemctl disable "${SERVICE_NAME}" 2>/dev/null || true
systemctl disable "${SOCKET_NAME}" 2>/dev/null || true

rm -rf /opt/patchpilot_install*
mkdir -p /opt/patchpilot_install "$APP_DIR"

# Download latest source
cd /opt/patchpilot_install
curl -L "$ZIP_URL" -o latest.zip
unzip -o latest.zip

# Remove existing folders to allow mv to succeed
rm -rf "$APP_DIR/src" "$APP_DIR/templates" "$APP_DIR/static"

# Move new files into place
cd "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/patchpilot_server/* "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/templates "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/server_test.sh "$APP_DIR"
mv /opt/patchpilot_install/PatchPilot-main/static "$APP_DIR"
chmod +x "$APP_DIR/server_test.sh"
rm -rf /opt/patchpilot_install

# System dependencies
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config sqlite3 libsqlite3-dev openssl

# Rust self-contained installation
export CARGO_HOME="${APP_DIR}/.cargo"
export RUSTUP_HOME="${APP_DIR}/.rustup"
export PATH="${CARGO_HOME}/bin:$PATH"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

if [[ ! -x "${CARGO_HOME}/bin/rustup" ]]; then
    echo "🛠️ Installing Rust (self-contained)..."
    export RUSTUP_INIT_SKIP_PATH_CHECK=yes
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | HOME=/root CARGO_HOME="${CARGO_HOME}" RUSTUP_HOME="${RUSTUP_HOME}" sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi

# Explicitly install and set latest stable Rust
export PATH="${CARGO_HOME}/bin:$PATH"
/opt/patchpilot_server/.cargo/bin/rustup install stable
/opt/patchpilot_server/.cargo/bin/rustup default stable

# Verify Rust installation
echo " Rust version:"
/opt/patchpilot_server/.cargo/bin/rustc --version
/opt/patchpilot_server/.cargo/bin/cargo --version

# Ensure database exists and secure permissions
SQLITE_DB="${APP_DIR}/patchpilot.db"
if [[ ! -f "$SQLITE_DB" ]]; then
    touch "$SQLITE_DB"
fi
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

## Build the server
cd "$APP_DIR"

echo "🛠️ Performing full rebuild of PatchPilot server (${BUILD_MODE})..."
"${CARGO_HOME}/bin/cargo" clean
if ! "${CARGO_HOME}/bin/cargo" build $([[ "$BUILD_MODE" == "release" ]] && echo "--release"); then
    echo "❌ Cargo build failed! Check the output above."
    exit 1
fi

# Rocket configuration
cat > "${APP_DIR}/Rocket.toml" <<EOF
[default]
address = "0.0.0.0"
port = 8080
log_level = "normal"

[release]
log_level = "critical"

[dev]
log_level = "normal"
address = "0.0.0.0"
port = 8080
EOF

# Environment file — preserve existing, otherwise create minimal default
APP_ENV_FILE="${APP_DIR}/.env"
if [[ ! -f "$APP_ENV_FILE" ]]; then
cat > "${APP_ENV_FILE}" <<EOF
DATABASE_URL=sqlite:///${APP_DIR}/patchpilot.db
RUST_LOG=info
ROCKET_ADDRESS=0.0.0.0
ROCKET_PORT=8080
ROCKET_PROFILE=dev
ROCKET_INSECURE_ALLOW_DEV=true
HOME=/home/patchpilot
EOF
chmod 600 "$APP_ENV_FILE"
fi

# Generate Rocket secret key
if ! grep -q "^ROCKET_SECRET_KEY=" "$APP_ENV_FILE" || \
   ! grep -E "^ROCKET_SECRET_KEY=([A-Za-z0-9+/]{43}=|[A-Fa-f0-9]{64})$" "$APP_ENV_FILE"; then
    echo "Generating valid Rocket secret key"
    sed -i '/^ROCKET_SECRET_KEY=/d' "$APP_ENV_FILE"
    echo "ROCKET_SECRET_KEY=$(openssl rand -base64 32)" >> "$APP_ENV_FILE"
fi
chmod 600 "$APP_ENV_FILE"

# Admin token
TOKEN_FILE="${APP_DIR}/admin_token.txt"
if [[ ! -f "$TOKEN_FILE" ]]; then
    openssl rand -base64 32 | head -c 44 > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
fi

# Ensure patchpilot user exists
if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -m -d /home/patchpilot -s /usr/sbin/nologin patchpilot
fi
mkdir -p /home/patchpilot/.cargo /home/patchpilot/.rustup
chown -R patchpilot:patchpilot /home/patchpilot
chmod 700 /home/patchpilot
chmod 700 /home/patchpilot/.cargo /home/patchpilot/.rustup

mkdir -p /opt/patchpilot_server/migrations
chown -R patchpilot:patchpilot /opt/patchpilot_server/migrations
chown -R patchpilot:patchpilot "$APP_DIR"
find "$APP_DIR" -type d -exec chmod 755 {} \;
find "$APP_DIR" -type f -exec chmod 755 {} \;

chmod +x "$APP_DIR/target/${BUILD_MODE}/patchpilot_server"
chmod +x "$APP_DIR/server_test.sh" 2>/dev/null || true

# Systemd socket unit (8080)
cat > "${SYSTEMD_DIR}/${SOCKET_NAME}" <<EOF
[Unit]
Description=PatchPilot Server Socket
After=network.target

[Socket]
ListenStream=8080
Accept=no

[Install]
WantedBy=sockets.target
EOF

# Systemd service unit (Rocket will use fd 3 provided by systemd if present)
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=PatchPilot Server
After=network.target ${SOCKET_NAME}
Requires=${SOCKET_NAME}

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_ENV_FILE}
Environment=PATH=${CARGO_HOME}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ExecStart=${APP_DIR}/target/${BUILD_MODE}/patchpilot_server
Restart=on-failure
RestartSec=5s
LimitNOFILE=65535
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Enable and start socket + service robustly
systemctl daemon-reload
systemctl enable "${SOCKET_NAME}" --now 2>/dev/null || true
systemctl enable "${SERVICE_NAME}" --now 2>/dev/null || true

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "✅ Installation complete!"
echo "🌐 Dashboard: http://${SERVER_IP}:8080"
echo "🔑 Admin token stored at ${TOKEN_FILE}"