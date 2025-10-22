#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVICE_FILE="/etc/systemd/system/patchpilot_client.service"
RUST_ENV_FILE="/root/.cargo/env"

show_usage() {
  echo "Usage: $0 [--uninstall]"
  exit 1
}

uninstall() {
  echo "[*] Uninstalling PatchPilot client..."
  systemctl stop patchpilot_client.service 2>/dev/null || true
  systemctl disable patchpilot_client.service 2>/dev/null || true
  rm -f "$SERVICE_FILE"
  rm -rf "$INSTALL_DIR"
  rm -rf "$SRC_DIR"
  systemctl daemon-reexec
  systemctl daemon-reload
  echo "[*] Uninstalled successfully."
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
  echo "[*] Existing installation detected. Running update..."
  # Placeholder for update logic
  echo "No updates detected."
  exit 0
fi

echo "[*] No installation detected. Running full install..."

echo "[*] Installing system dependencies..."
apt-get update
apt-get install -y curl git build-essential pkg-config libssl-dev

echo "[*] Installing Rust toolchain..."
if ! command -v rustc >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

# Source Rust environment for root user
if [ -f "$RUST_ENV_FILE" ]; then
  source "$RUST_ENV_FILE"
else
  echo "Rust environment file not found at $RUST_ENV_FILE"
  exit 1
fi

echo "[*] Creating install directory..."
mkdir -p "$INSTALL_DIR"

echo "[*] Cloning client source..."
rm -rf "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

echo "[*] Building Rust client binary..."
cd "$SRC_DIR/patchpilot_client_rust"

export OPENSSL_DIR="/usr"
export OPENSSL_LIB_DIR="/usr/lib/x86_64-linux-gnu"
export OPENSSL_INCLUDE_DIR="/usr/include"
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

cargo clean
cargo build --release

if [ ! -f "target/release/rust_patch_client" ]; then
  echo "Error: Built binary not found!"
  exit 1
fi

echo "[*] Copying binary to install directory..."
cp target/release/rust_patch_client "$CLIENT_PATH"
chmod +x "$CLIENT_PATH"

echo "[*] Creating default config.json..."
cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "127.0.0.1",
  "client_id": ""
}
EOF

echo "[*] Creating systemd service..."
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=PatchPilot Client Service
After=network.target

[Service]
Type=simple
ExecStart=$CLIENT_PATH
Restart=on-failure
WorkingDirectory=$INSTALL_DIR
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

echo "[*] Enabling and starting systemd service..."
systemctl daemon-reload
systemctl enable patchpilot_client.service
systemctl start patchpilot_client.service

echo "[✔] Installation complete. PatchPilot client is running and will start on boot."
exit 0
