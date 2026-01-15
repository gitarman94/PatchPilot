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
BUILD_MODE="debug"

for arg in "$@"; do
    case "$arg" in
        --force) FORCE_REINSTALL=true ;;
        --upgrade) UPGRADE=true ;;
        --debug) BUILD_MODE="debug" ;;
        --release) BUILD_MODE="release" ;;
    esac
done

if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "❌ Only Debian-based systems supported."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS."; exit 1
fi

if [[ "$FORCE_REINSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."
    systemctl stop "${SERVICE_NAME}" || true
    systemctl disable "${SERVICE_NAME}" || true
    pkill -f "^${APP_DIR}/target/${BUILD_MODE}/patchpilot_server$" || true
    rm -rf "${APP_DIR}" /opt/patchpilot_install*
    mkdir -p /opt/patchpilot_install "$APP_DIR"
fi

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

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config sqlite3 libsqlite3-dev openssl

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

export PATH="${CARGO_HOME}/bin:$PATH"
"${CARGO_HOME}/bin/rustup" default stable || true

SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chmod 755 "$SQLITE_DB"

cd "$APP_DIR"
echo "🛠️ Building PatchPilot server (${BUILD_MODE})..."
"${CARGO_HOME}/bin/cargo" build $([[ "$BUILD_MODE" == "release" ]] && echo "--release")

cat > "${APP_DIR}/Rocket.toml" <<EOF
[default]
address = "0.0.0.0"
port = 8080
log_level = "normal"

[production]
log_level = "critical"

[dev]
log_level = "normal"
address = "0.0.0.0"
port = 8080
EOF

APP_ENV_FILE="${APP_DIR}/.env"
cat > "${APP_ENV_FILE}" <<EOF
DATABASE_URL=sqlite:///${APP_DIR}/patchpilot.db
RUST_LOG=info
ROCKET_ADDRESS=0.0.0.0
ROCKET_PORT=8080
ROCKET_PROFILE=dev
ROCKET_INSECURE_ALLOW_DEV=true
EOF

ROCKET_SECRET_KEY=$(openssl rand -base64 48 | head -c 64)
echo "ROCKET_SECRET_KEY=${ROCKET_SECRET_KEY}" >> "$APP_ENV_FILE"
chmod 600 "$APP_ENV_FILE"

TOKEN_FILE="${APP_DIR}/admin_token.txt"
if [[ ! -f "$TOKEN_FILE" ]]; then
    openssl rand -base64 32 | head -c 44 > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
fi

if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -m -d /home/patchpilot -s /usr/sbin/nologin patchpilot
fi

mkdir -p /home/patchpilot
chown patchpilot:patchpilot /home/patchpilot
chmod 700 /home/patchpilot

chown -R patchpilot:patchpilot "$APP_DIR"
find "$APP_DIR" -type d -exec chmod 755 {} \;
find "$APP_DIR" -type f -exec chmod 755 {} \;

chmod +x "$APP_DIR/target/${BUILD_MODE}/patchpilot_server"
chmod +x "$APP_DIR/server_test.sh" 2>/dev/null || true

cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=PatchPilot Server
After=network.target

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_ENV_FILE}
ExecStart=${APP_DIR}/target/${BUILD_MODE}/patchpilot_server
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
