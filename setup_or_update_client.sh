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
  echo "Uninstalled."
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
  # Placeholder for update logic, currently just checks files
  # Could add git pull and rebuild logic here if needed
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

# Source Rust environment for this script/session
if [ -f "$HOME/.cargo/env" ]; then
  source "$HOME/.cargo/env"
else
  echo "Warning: Rust environment file not found at $HOME/.cargo/env"
fi

echo "[*] Cloning client source..."
rm -rf "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

echo "[*] Building Rust client binary..."

# Print environment info for debug
echo "[*] PATH: $PATH"
which pkg-config || echo "pkg-config not found"
pkg-config --version || echo "pkg-config version check failed"
which cargo || echo "cargo not found"
cargo --version || echo "cargo version check failed"

# Export OpenSSL environment vars if necessary (adjust path if needed)
export OPENSSL_DIR="/usr/lib/ssl"
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/share/pkgconfig"

cd "$SRC_DIR/patchpilot_client_rust"
cargo build --release

echo "[*] Copying binaries to install directory..."
cp target/release/patchpilot_client "$CLIENT_PATH"
cp target/release/patchpilot_updater "$UPDATER_PATH"

chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

echo "[*] Creating default config.json..."
cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "127.0.0.1",
  "client_id": ""
}
EOF

echo "Installation complete. You can now run $CLIENT_PATH"

exit 0
