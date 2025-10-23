#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/patchpilot_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVER_URL_FILE="$INSTALL_DIR/server_url.txt"
SERVICE_FILE="/etc/systemd/system/patchpilot_client.service"

show_usage() {
  echo "Usage: $0 [--uninstall] [--update] [--reinstall]"
  exit 1
}

uninstall() {
  echo "Uninstalling PatchPilot client..."
  systemctl stop patchpilot_client.service 2>/dev/null || true
  systemctl disable patchpilot_client.service 2>/dev/null || true
  rm -f "$SERVICE_FILE"
  systemctl daemon-reload
  # Remove cron jobs related to patchpilot_client (if any)
  crontab -l | grep -v 'patchpilot_client' | crontab - || true
  rm -rf "$INSTALL_DIR"
  echo "Uninstalled."
}

update() {
  echo "Updating PatchPilot client..."
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "Error: Installation not found at $INSTALL_DIR"
    exit 1
  fi

  # Prompt for server IP (not full URL)
  read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip

  # Strip protocol and port if somehow included
  input_ip="${input_ip#http://}"
  input_ip="${input_ip#https://}"
  input_ip="${input_ip%%/*}"  # Remove trailing slash or paths

  final_url="http://${input_ip}:8080/api"
  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  echo "[*] Installing dependencies..."
  apt-get update
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain if missing..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  # Load Rust environment for root
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
  cp target/release/rust_patch_client "$CLIENT_PATH"
  cp target/release/patchpilot_updater "$UPDATER_PATH"
  chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

  # Update config.json with new server URL but keep client_id intact if exists
  if [[ -f "$CONFIG_PATH" ]]; then
    client_id=$(jq -r '.client_id // empty' "$CONFIG_PATH")
  else
    client_id=""
  fi

  echo "[*] Updating config.json..."
  cat > "$CONFIG_PATH" <<EOF
{
  "server_ip": "$final_url",
  "client_id": "$client_id"
}
EOF

  echo "[*] Starting service..."
  systemctl start patchpilot_client.service || true

  echo "Update complete."
}

reinstall() {
  echo "Reinstalling PatchPilot client..."
  uninstall
  update
}

install() {
  echo "Installing PatchPilot client..."

  # Prompt for server IP (not full URL)
  read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip

  # Strip protocol and port if somehow included
  input_ip="${input_ip#http://}"
  input_ip="${input_ip#https://}"
  input_ip="${input_ip%%/*}"  # Remove trailing slash or paths

  final_url="http://${input_ip}:8080/api"
  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  echo "[*] Installing dependencies..."
  apt-get update
  apt-get install -y curl git build-essential pkg-config libssl-dev

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
  "server_ip": "$final_url",
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
}

# Check if we're root
if [[ $(id -u) -ne 0 ]]; then
  echo "Please run as root."
  exit 1
fi

# Handle options
if [[ "$1" == "--uninstall" ]]; then
  uninstall
  exit 0
fi

if [[ "$1" == "--update" ]]; then
  update
  exit 0
fi

if [[ "$1" == "--reinstall" ]]; then
  reinstall
  exit 0
fi

# If no flag is provided, check if it's an update or new installation
if [[ -d "$INSTALL_DIR" ]]; then
  echo "Existing installation detected."
  read -rp "Do you want to [u]pdate or [r]einstall? (u/r): " action
  if [[ "$action" == "u" ]]; then
    update
  elif [[ "$action" == "r" ]]; then
    reinstall
  else
    echo "Invalid choice, exiting."
    exit 1
  fi
else
  echo "No installation detected. Running full install..."
  install
fi

exit 0
