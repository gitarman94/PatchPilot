#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVICE_NAME="patchpilot_client.service"

show_usage() {
  echo "Usage: $0 [--uninstall | --update]"
  exit 1
}

uninstall() {
  echo "Uninstalling PatchPilot client..."
  systemctl stop $SERVICE_NAME 2>/dev/null || true
  systemctl disable $SERVICE_NAME 2>/dev/null || true
  rm -rf "$INSTALL_DIR"
  rm -f /etc/systemd/system/$SERVICE_NAME
  systemctl daemon-reload
  echo "Uninstalled."
  exit 0
}

update() {
  echo "Updating PatchPilot client..."

  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "PatchPilot is not installed. Please run full install."
    exit 1
  fi

  echo "[*] Pulling latest source code..."
  rm -rf "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"

  echo "[*] Building Rust client binary..."

  if [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found at /root/.cargo/env"
  fi

  cd "$SRC_DIR/patchpilot_client_rust"

  export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
  export OPENSSL_INCLUDE_DIR=/usr/include
  export OPENSSL_DIR=/usr

  cargo clean
  cargo build --release

  echo "[*] Copying binaries to install directory..."
  cp target/release/rust_patch_client "$CLIENT_PATH"
  cp target/release/patchpilot_updater "$UPDATER_PATH"

  chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

  echo "[*] Restarting PatchPilot service..."
  systemctl restart $SERVICE_NAME

  echo "Update complete."
  exit 0
}

if [[ "$1" == "--uninstall" ]]; then
  uninstall
elif [[ "$1" == "--update" ]]; then
  update
elif [[ -n "$1" ]]; then
  show_usage
fi

if [[ $(id -u) -ne 0 ]]; then
  echo "Please run as root."
  exit 1
fi

if [[ -d "$INSTALL_DIR" ]]; then
  echo "Existing installation detected. Run '$0 --update' to update."
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
cat > /etc/systemd/system/$SERVICE_NAME <<EOF
[Unit]
Description=PatchPilot Client
After=network.target

[Service]
Type=simple
ExecStart=$CLIENT_PATH
Restart=always
User=root
WorkingDirectory=$INSTALL_DIR

[Install]
WantedBy=multi-user.target
EOF

echo "[*] Enabling and starting systemd service..."
systemctl daemon-reload
systemctl enable $SERVICE_NAME
systemctl start $SERVICE_NAME

echo "[✔] Installation complete. PatchPilot client is running and will start on boot."

exit 0
