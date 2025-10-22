#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"

show_usage() {
  echo "Usage: $0 [--uninstall]"
  exit 1
}

uninstall() {
  echo "Uninstalling PatchPilot client..."
  systemctl stop patchpilot_client.service 2>/dev/null || true
  systemctl disable patchpilot_client.service 2>/dev/null || true
  rm -rf "$INSTALL_DIR"
  rm -f /etc/systemd/system/patchpilot_client.service
  systemctl daemon-reload

  echo "[*] Cleaning up Rust toolchain and temporary build files..."
  rm -rf "$SRC_DIR"
  rm -rf "$HOME/.cargo"
  rm -rf "$HOME/.rustup"
  rm -rf "$HOME/.cargo/registry"
  rm -rf "$HOME/.cargo/git"

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
  echo "No updates detected."  # Placeholder
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

# Build the Rust client
PROJECT_DIR="$SRC_DIR/patchpilot_client_rust"
cd "$PROJECT_DIR" || { echo "Failed to cd into source directory"; exit 1; }

echo "[*] Building Rust client binary..."
export OPENSSL_DIR="/usr"
export OPENSSL_LIB_DIR="/usr/lib/x86_64-linux-gnu"
export OPENSSL_INCLUDE_DIR="/usr/include"
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

cargo clean
cargo build --release

# Copy binaries using absolute paths
echo "[*] Copying binaries to install directory..."

CLIENT_BIN="$PROJECT_DIR/target/release/rust_patch_client"
UPDATER_BIN="$PROJECT_DIR/target/release/patchpilot_updater"

if [ -f "$CLIENT_BIN" ]; then
  cp "$CLIENT_BIN" "$CLIENT_PATH"
else
  echo "Error: Client binary not found at $CLIENT_BIN"
  exit 1
fi

if [ -f "$UPDATER_BIN" ]; then
  cp "$UPDATER_BIN" "$UPDATER_PATH"
else
  echo "Warning: Updater binary not found at $UPDATER_BIN"
fi

chmod +x "$CLIENT_PATH" "$UPDATER_PATH" 2>/dev/null || true

echo "[*] Creating default config.json..."
cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "127.0.0.1",
  "client_id": ""
}
EOF

echo "Installation complete. You can now run: $CLIENT_PATH"
exit 0
