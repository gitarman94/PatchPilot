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
  # Placeholder for update logic: e.g., git pull + rebuild
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

# Source Rust environment (root user, so .cargo/env in /root)
if [ -f "/root/.cargo/env" ]; then
  source "/root/.cargo/env"
else
  echo "Warning: Rust environment file not found at /root/.cargo/env"
fi

echo "[*] Cloning client source..."
rm -rf "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

echo "[*] Building Rust client binary..."

echo "[*] PATH: $PATH"
which pkg-config || echo "pkg-config not found"
pkg-config --version || echo "pkg-config version check failed"
which cargo || echo "cargo not found"
cargo --version || echo "cargo version check failed"

# Set OpenSSL env vars - usually /usr works fine if libssl-dev is installed
export OPENSSL_DIR="/usr"
# Add common pkgconfig paths (adjust if your distro differs)
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"

# Change dir to rust client source (adjust if path is different)
cd "$SRC_DIR/patchpilot_client_rust"

export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include
export OPENSSL_DIR=/usr
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
