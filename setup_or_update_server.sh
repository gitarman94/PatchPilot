#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/patchpilot_server"
SERVICE_NAME="patchpilot_server.service"
SOCKET_NAME="patchpilot_server.socket"
SYSTEMD_DIR="/etc/systemd/system"

mkdir -p "$APP_DIR"
echo "Starting PatchPilot server setup at $(date)..." >&2

GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/${BRANCH}.zip"

BUILD_MODE="debug"
for arg in "$@"; do
  case "$arg" in
    --debug) BUILD_MODE="debug" ;;
    --release) BUILD_MODE="release" ;;
  esac
done

# Ensure minimal tools present
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl unzip build-essential libssl-dev pkg-config sqlite3 libsqlite3-dev openssl

# Ensure patchpilot user exists early
if ! id -u patchpilot >/dev/null 2>&1; then
  useradd -r -m -d /home/patchpilot -s /usr/sbin/nologin patchpilot
fi
mkdir -p /home/patchpilot/.cargo /home/patchpilot/.rustup
chown -R patchpilot:patchpilot /home/patchpilot
chmod 700 /home/patchpilot /home/patchpilot/.cargo /home/patchpilot/.rustup

# Stop/disable previous units (safe)
systemctl stop "$SERVICE_NAME" 2>/dev/null || true
systemctl stop "$SOCKET_NAME" 2>/dev/null || true
systemctl disable "$SERVICE_NAME" 2>/dev/null || true
systemctl disable "$SOCKET_NAME" 2>/dev/null || true

# Prepare temp dir and download
TMPDIR="$(mktemp -d /tmp/patchpilot_install.XXXXXX)"
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"
curl -sL "$ZIP_URL" -o latest.zip
unzip -q latest.zip -d "$TMPDIR"

EXTRACT_DIR=$(find "$TMPDIR" -maxdepth 1 -type d -name "${GITHUB_REPO}-*" -print -quit)
if [ -z "$EXTRACT_DIR" ]; then
  echo "Extraction failed: could not find ${GITHUB_REPO}-* directory" >&2
  exit 1
fi

REPO_ROOT="${EXTRACT_DIR}"
# server sources may be under repo_root/patchpilot_server or at repo_root
if [ -d "${REPO_ROOT}/patchpilot_server" ]; then
  SRC_DIR="${REPO_ROOT}/patchpilot_server"
else
  SRC_DIR="${REPO_ROOT}"
fi

# Ensure APP_DIR exists and is writable
mkdir -p "$APP_DIR"
# copy rather than move to avoid leaving repo in inconsistent state
rm -rf "$APP_DIR/src" "$APP_DIR/static" "$APP_DIR/templates"

# Copy server source dir contents
# - copy any src dir from SRC_DIR (server code)
if [ -d "${SRC_DIR}/src" ]; then
  cp -a "${SRC_DIR}/src" "$APP_DIR/"
fi

# Copy templates: look in both repo root and src dir
if [ -d "${REPO_ROOT}/templates" ]; then
  cp -a "${REPO_ROOT}/templates" "$APP_DIR/"
fi
if [ -d "${SRC_DIR}/templates" ]; then
  cp -a "${SRC_DIR}/templates" "$APP_DIR/"
fi

# Copy static: look in both repo root and src dir
if [ -d "${REPO_ROOT}/static" ]; then
  cp -a "${REPO_ROOT}/static" "$APP_DIR/"
fi
if [ -d "${SRC_DIR}/static" ]; then
  cp -a "${SRC_DIR}/static" "$APP_DIR/"
fi

# Copy other top-level files from SRC_DIR (Cargo.toml, Rocket.toml, server_test.sh, etc.)
for file in "${SRC_DIR}"/*; do
  bn=$(basename "$file")
  case "$bn" in
    src|templates|static) continue ;;
    *) cp -a "$file" "$APP_DIR/" ;;
  esac
done

# Also copy top-level files from REPO_ROOT (if different)
if [ "$REPO_ROOT" != "$SRC_DIR" ]; then
  for file in "${REPO_ROOT}"/*; do
    bn=$(basename "$file")
    case "$bn" in
      patchpilot_server|src|templates|static) continue ;;
      *) cp -a "$file" "$APP_DIR/" ;;
    esac
  done
fi

# Ensure Rocket.toml present
if [ ! -f "${APP_DIR}/Rocket.toml" ]; then
  cat > "${APP_DIR}/Rocket.toml" <<'EOF'
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
  chmod 600 "${APP_DIR}/Rocket.toml"
fi

# Ensure admin token
if [ ! -f "${APP_DIR}/admin_token.txt" ]; then
  openssl rand -base64 32 | head -c 44 > "${APP_DIR}/admin_token.txt"
  chmod 600 "${APP_DIR}/admin_token.txt"
fi

# Ensure DB present
touch "${APP_DIR}/patchpilot.db"
chown patchpilot:patchpilot "${APP_DIR}/patchpilot.db"
chmod 600 "${APP_DIR}/patchpilot.db"

# Ensure ownership and permissions for app dir (readable by root and patchpilot)
chown -R patchpilot:patchpilot "$APP_DIR"
find "$APP_DIR" -type d -exec chmod 755 {} \;
find "$APP_DIR" -type f -exec chmod 644 {} \;
chmod +x "${APP_DIR}/server_test.sh" 2>/dev/null || true

# Install rustup/cargo into CARGO_HOME under APP_DIR (non-invasive)
export CARGO_HOME="${APP_DIR}/.cargo"
export RUSTUP_HOME="${APP_DIR}/.rustup"
export PATH="${CARGO_HOME}/bin:${PATH}"
mkdir -p "${CARGO_HOME}" "${RUSTUP_HOME}"

if [[ ! -x "${CARGO_HOME}/bin/rustup" ]]; then
  # install rustup as root but target CARGO_HOME
  curl -sSf https://sh.rustup.rs | HOME=/root CARGO_HOME="${CARGO_HOME}" RUSTUP_HOME="${RUSTUP_HOME}" sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi

# Ensure the cargo env is loaded for this script
if [ -f "${CARGO_HOME}/env" ]; then
  # shellcheck disable=SC1090
  . "${CARGO_HOME}/env"
fi

# Make sure executables exist and are executable
if [ -d "${CARGO_HOME}/bin" ]; then
  chmod -R a+rx "${CARGO_HOME}/bin" || true
fi
chmod -R a+rx "${RUSTUP_HOME}" || true

# Ensure stable toolchain exists
"${CARGO_HOME}/bin/rustup" install stable || true
"${CARGO_HOME}/bin/rustup" default stable || true

# Build the server
cd "$APP_DIR"
echo "Building PatchPilot server (${BUILD_MODE})..." >&2

if [ "$BUILD_MODE" = "release" ]; then
  "${CARGO_HOME}/bin/cargo" build --release
  EXE_PATH="${APP_DIR}/target/release/patchpilot_server"
else
  "${CARGO_HOME}/bin/cargo" build
  EXE_PATH="${APP_DIR}/target/debug/patchpilot_server"
fi

# Ensure binary exists and is executable
if [ ! -x "${EXE_PATH}" ]; then
  chmod +x "${EXE_PATH}" || true
fi
chown patchpilot:patchpilot "${EXE_PATH}" 2>/dev/null || true

# Create systemd socket and service (socket activation)
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
echo "Admin token: ${APP_DIR}/admin_token.txt"
