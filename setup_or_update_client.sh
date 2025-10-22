#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVICE_FILE="/etc/systemd/system/patchpilot_client.service"

show_usage() {
  echo "Usage: $0 [--uninstall] [--update]"
  exit 1
}

uninstall() {
  echo "Uninstalling PatchPilot client..."
  systemctl stop patchpilot_client.service 2>/dev/null || true
  systemctl disable patchpilot_client.service 2>/dev/null || true
  rm -f "$SERVICE_FILE"
  systemctl daemon-reload
  # Remove any cron jobs related to patchpilot_client if exist
  crontab -l | grep -v 'patchpilot_client' | crontab - || true
  rm -rf "$INSTALL_DIR"
  echo "Uninstalled."
  exit 0
}

update() {
  echo "Updating PatchPilot client..."
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "Error: Installation not found at $INSTALL_DIR"
    exit 1
  fi

  echo "[*] Installing dependencies..."
  apt-get update
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain if missing..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  # Source Rust environment (assuming root)
  if [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found at /root/.cargo/env"
  fi

  echo "[*] Cloning client source..."
  rm -rf "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"

  cd "$SRC_DIR/patchpilot_client_rust"

  export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
  export OPENSSL_INCLUDE_DIR=/usr/include
  export OPENSSL_DIR=/usr
  export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

  cargo clean
  cargo build --release

  echo "[*] Stopping service to update binaries..."
  systemctl stop patchpilot_client.service || true

  echo "[*] Copying binaries to install directory..."
  # Safely replace binaries by moving old files before copy
  mv "$CLIENT_PATH" "$CLIENT_PATH.old" 2>/dev/null || true
  mv "$UPDATER_PATH" "$UPDATER_PATH.old" 2>/dev/null || true

  cp target/release/rust_patch_client "$CLIENT_PATH"
  cp target/release/patchpilot_updater "$UPDATER_PATH"
  chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

  echo "[*] Starting service..."
  systemctl start patchpilot_client.service || true

  echo "Update complete."
  exit 0
}

if [[ "$1" == "--uninstall" ]]; then
  uninstall
fi

if [[ "$1" == "--update" ]]; then
  update
fi

if [[ $(id -u) -ne 0 ]]; then
  echo "Please run as root."
  exit 1
fi

if [[ -d "$INSTALL_DIR" ]]; then
  echo "Existing installation detected. Use --update to rebuild or --uninstall to remove."
  exit 0
fi

echo "No installation detected. Running full install..."

echo "[*] Installing dependencies..."
apt-get update
apt-get install -y curl git build-essential pkg-config libssl-dev

echo "[*] Creating install directory..."
mkdir -p "$INSTALL_DIR"

echo "[*] Installing Rust toolchain..."
if ! command -v rustc >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

# Source Rust environment (root user)
if [ -f "/root/.cargo/env" ]; then
  source "/root/.cargo/env"
else
  echo "Warning: Rust environment file not found at /root/.cargo/env"
fi

echo "[*] Cloning client source..."
rm -rf "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

echo "[*] Building Rust client binary..."
cd "$SRC_DIR/patchpilot_client_rust"

export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include
export OPENSSL_DIR=/usr
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

cargo clean
cargo build --release

echo "[*] Copying binaries to install directory..."
cp target/release/rust_patch_client "$CLIENT_PATH"
cp target/release/patchpilot_updater "$UPDATER_PATH"

chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

echo "[*] Creating default config.json..."
cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "",
  "client_id": ""
}
EOF

echo "[*] Creating systemd service..."
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=PatchPilot Client
After=network.target

[Service]
ExecStart=$CLIENT_PATH
Restart=always
User=root
WorkingDirectory=$INSTALL_DIR

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable patchpilot_client.service
systemctl start patchpilot_client.service

echo "[✔] Installation complete. PatchPilot client is running and will start on boot."

exit 0
