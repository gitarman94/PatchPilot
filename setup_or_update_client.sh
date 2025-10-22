#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVICE_PATH="/etc/systemd/system/patchpilot_client.service"

show_usage() {
  echo "Usage: $0 [--uninstall]"
  exit 1
}

uninstall() {
  echo "[*] Uninstalling PatchPilot client..."
  systemctl stop patchpilot_client.service 2>/dev/null || true
  systemctl disable patchpilot_client.service 2>/dev/null || true
  rm -rf "$INSTALL_DIR"
  rm -f "$SERVICE_PATH"
  systemctl daemon-reload

  echo "[*] Cleaning Rust build artifacts and cargo cache..."
  rm -rf "$SRC_DIR"
  rm -rf /root/.cargo /root/.rustup
  rm -rf /root/.cargo/registry
  rm -rf /root/.cargo/git
  echo "Uninstalled and cleaned up."
  exit 0
}

if [[ "$1" == "--uninstall" ]]; then
  uninstall
fi

if [[ $(id -u) -ne 0 ]]; then
  echo "Please run as root."
  exit 1
fi

if [[ -d "$INSTALL_DIR" ]]; then
  echo "Existing installation detected. Running update..."
  # Placeholder for update logic
  echo "No updates detected."
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

# Source Rust environment
if [ -f "/root/.cargo/env" ]; then
  source "/root/.cargo/env"
else
  echo "Warning: Rust environment file not found at /root/.cargo/env"
fi

echo "[*] Cloning client source..."
rm -rf "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

echo "[*] Building Rust client binary..."
export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include
export OPENSSL_DIR=/usr
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

cd "$SRC_DIR/patchpilot_client_rust"
cargo clean
cargo build --release

echo "[*] Copying binaries to install directory..."
if [ -f target/release/rust_patch_client ]; then
  cp target/release/rust_patch_client "$CLIENT_PATH"
else
  echo "Error: rust_patch_client binary not found after build."
  exit 1
fi

if [ -f target/release/patchpilot_updater ]; then
  cp target/release/patchpilot_updater "$UPDATER_PATH"
else
  echo "Warning: Updater binary not found, skipping."
fi

chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

echo "[*] Creating default config.json..."
cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "127.0.0.1",
  "client_id": ""
}
EOF

echo "[*] Setting up systemd service..."
cat > "$SERVICE_PATH" <<EOF
[Unit]
Description=PatchPilot Client Service
After=network.target

[Service]
ExecStart=$CLIENT_PATH
Restart=always
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable patchpilot_client.service
systemctl start patchpilot_client.service

echo "Installation complete. Service is running and will auto-start on boot."
exit 0
