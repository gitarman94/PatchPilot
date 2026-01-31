#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/patchpilot_server"
INSTALL_LOG="${APP_DIR}/install.log"
SERVICE_NAME="patchpilot_server.service"
SOCKET_NAME="patchpilot_server.socket"
SYSTEMD_DIR="/etc/systemd/system"

mkdir -p "$APP_DIR"
echo "Starting PatchPilot server setup at $(date)..." >&2

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
  case "$ID" in debian|ubuntu|linuxmint|pop|raspbian) ;;
  *) echo "Only Debian-based systems supported." >&2; exit 1 ;;
  esac
else
  echo "Cannot determine OS." >&2; exit 1
fi

# Stop any running server
systemctl stop "$SERVICE_NAME" 2>/dev/null || true
systemctl stop "$SOCKET_NAME" 2>/dev/null || true
systemctl disable "$SERVICE_NAME" 2>/dev/null || true
systemctl disable "$SOCKET_NAME" 2>/dev/null || true

# Ensure minimal tools available before using unzip/curl
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config sqlite3 libsqlite3-dev openssl

# Ensure patchpilot user exists early (so we can chown files safely)
if ! id -u patchpilot >/dev/null 2>&1; then
  useradd -r -m -d /home/patchpilot -s /usr/sbin/nologin patchpilot
fi
mkdir -p /home/patchpilot/.cargo /home/patchpilot/.rustup
chown -R patchpilot:patchpilot /home/patchpilot
chmod 700 /home/patchpilot /home/patchpilot/.cargo /home/patchpilot/.rustup

# Prepare a safe temporary extraction directory
TMPDIR="$(mktemp -d /tmp/patchpilot_install.XXXXXX)"
trap 'rm -rf "$TMPDIR"' EXIT

cd "$TMPDIR"
curl -L "$ZIP_URL" -o latest.zip
unzip -q latest.zip -d "$TMPDIR"

EXTRACT_DIR=$(find "$TMPDIR" -maxdepth 1 -type d -name "${GITHUB_REPO}-*" -print -quit)
if [ -z "$EXTRACT_DIR" ]; then
  echo "Extraction failed: could not find ${GITHUB_REPO}-* directory" >&2
  exit 1
fi

# Use patchpilot_server subdir if present, otherwise top-level extracted dir
if [ -d "${EXTRACT_DIR}/patchpilot_server" ]; then
  SRC_DIR="${EXTRACT_DIR}/patchpilot_server"
else
  SRC_DIR="${EXTRACT_DIR}"
fi

# Ensure target app dir exists and is writable by current user for copy operations
mkdir -p "$APP_DIR"
chown -R "$(id -u):$(id -g)" "$APP_DIR"

# Remove old app subdirectories that will be replaced
rm -rf "$APP_DIR/src" "$APP_DIR/templates" "$APP_DIR/static"

# Copy directories (cp -a available on base system)
for dir in src templates static; do
  if [ -d "${SRC_DIR}/${dir}" ]; then
    cp -a "${SRC_DIR}/${dir}" "$APP_DIR/"
  fi
done

# Copy server_test.sh if present
if [ -f "${SRC_DIR}/server_test.sh" ]; then
  cp "${SRC_DIR}/server_test.sh" "$APP_DIR/"
  chmod +x "$APP_DIR/server_test.sh"
fi

# Copy other top-level files (Cargo.toml, Cargo.lock, Rocket.toml, static files that might be top-level)
for file in "${SRC_DIR}"/*; do
  basename="$(basename "$file")"
  case "$basename" in
    src|templates|static|server_test.sh) continue ;;
    *) cp -a "$file" "$APP_DIR/" ;;
  esac
done

# Ensure APP_DIR has Rocket.toml and templates exist (warn if missing)
if [ ! -f "${APP_DIR}/Rocket.toml" ]; then
  cat > "$APP_DIR/Rocket.toml" <<'EOF'
[default]
address = "0.0.0.0"
port = 8080
log_level = "normal"

[release]
log_level = "critical"

[debug]
address = "0.0.0.0"
port = 8080
log_level = "normal"
EOF
  chmod 600 "$APP_DIR/Rocket.toml"
fi

if [ ! -d "${APP_DIR}/templates" ]; then
  echo "Warning: templates directory not found in $APP_DIR" >&2
fi

# Ensure admin token exists
TOKEN_FILE="${APP_DIR}/admin_token.txt"
if [ ! -f "$TOKEN_FILE" ]; then
  openssl rand -base64 32 | head -c 44 > "$TOKEN_FILE"
  chmod 600 "$TOKEN_FILE"
fi

# Ensure database exists and owned by patchpilot
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Set ownership for app files to patchpilot
chown -R patchpilot:patchpilot "$APP_DIR"
find "$APP_DIR" -type d -exec chmod 755 {} \;
find "$APP_DIR" -type f -exec chmod 644 {} \;
chmod +x "${APP_DIR}/server_test.sh" 2>/dev/null || true

# Rust installation (self-contained)
export CARGO_HOME="${APP_DIR}/.cargo"
export RUSTUP_HOME="${APP_DIR}/.rustup"
export PATH="${CARGO_HOME}/bin:${PATH}"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

if [[ ! -x "${CARGO_HOME}/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | HOME=/root CARGO_HOME="$CARGO_HOME" RUSTUP_HOME="$RUSTUP_HOME" sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi

"$CARGO_HOME/bin/rustup" install stable
"$CARGO_HOME/bin/rustup" default stable

# Build the server
cd "$APP_DIR"
echo "Building PatchPilot server (${BUILD_MODE})..." >&2
if [ "$BUILD_MODE" = "release" ]; then
  "$CARGO_HOME/bin/cargo" build --release
  EXE_PATH="${APP_DIR}/target/release/patchpilot_server"
else
  "$CARGO_HOME/bin/cargo" build
  EXE_PATH="${APP_DIR}/target/debug/patchpilot_server"
fi

chown patchpilot:patchpilot "$EXE_PATH"
chmod +x "$EXE_PATH"

# Create systemd socket and service
cat > "${SYSTEMD_DIR}/${SOCKET_NAME}" <<EOF
[Unit]
Description=PatchPilot Server Socket

[Socket]
ListenStream=8080
Accept=no

[Install]
WantedBy=sockets.target
EOF

cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=PatchPilot Server
After=network.target ${SOCKET_NAME}
Requires=${SOCKET_NAME}

[Service]
User=patchpilot
Group=patchpilot
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_DIR}/.env
Environment=PATH=${CARGO_HOME}/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ExecStart=${EXE_PATH}
Restart=on-failure
RestartSec=5s
LimitNOFILE=65535
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "${SOCKET_NAME}" "${SERVICE_NAME}"

SERVER_IP=$(hostname -I | awk '{print $1}')
echo "Installation complete"
echo "Dashboard: http://${SERVER_IP}:8080"
echo "Admin token: ${TOKEN_FILE}"
