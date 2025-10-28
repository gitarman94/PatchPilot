#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_PATH="$INSTALL_DIR/rust_patch_client"
UPDATER_PATH="$INSTALL_DIR/patchpilot_updater"
CONFIG_PATH="$INSTALL_DIR/config.json"
SERVER_URL_FILE="$INSTALL_DIR/server_url.txt"
SERVICE_FILE="/etc/systemd/system/patchpilot_client.service"

# --- Auto-detect the server ---
detect_server() {
  echo "Attempting to discover server on the local network..."

  # Define your local network range (adjust to match your network)
  NETWORK="192.168.1"  # Replace with your network address (e.g., 192.168.1)

  # Loop through the possible IP addresses in the subnet (1-254)
  for ip in $(seq 1 254); do
    target="${NETWORK}.$ip"
    echo "Pinging $target..."

    # Ping each IP address once with a timeout of 1 second
    if ping -c 1 -W 1 $target &>/dev/null; then
      # Server found, set the discovered server URL
      echo "Found server at $target"
      DISCOVERED_SERVER="http://$target:8080/api"
      return 0  # Success, return the discovered server
    fi
  done

  # No server found
  echo "No server found on the local network"
  return 1  # Failure
}

# --- Load Rust environment ---
load_rust_env() {
  # Check if the Rust environment file exists for the current user
  if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
  elif [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found"
  fi
}

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
  crontab -l | grep -v 'patchpilot_client' | crontab - || true
  rm -rf "$INSTALL_DIR"
  echo "Uninstalled."
}

common_install_update() {
  echo "[*] Installing dependencies..."
  apt-get update -y
  apt-get install -y curl git build-essential pkg-config libssl-dev nmap

  echo "[*] Installing Rust toolchain if missing..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  # Load Rust environment for the current user
  load_rust_env

  echo "[*] Cloning client source..."
  if [ -d "$SRC_DIR" ]; then
    rm -rf "$SRC_DIR"
  fi
  mkdir -p "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"
  cd "$SRC_DIR/patchpilot_client_rust"

  export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
  export OPENSSL_INCLUDE_DIR=/usr/include
  export OPENSSL_DIR=/usr
  export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig"

  echo "[*] Building PatchPilot client..."
  cargo build --release
}

install() {
  echo "Installing PatchPilot client..."

  # Call common steps for install and update
  common_install_update

  # Install the client binary
  echo "[*] Installing client to $CLIENT_PATH..."
  mkdir -p "$INSTALL_DIR"
  cp target/release/rust_patch_client "$CLIENT_PATH"

  # Try auto-detecting the server
  echo "[*] Attempting to auto-discover the PatchPilot server on the local network..."
  if detect_server; then
    final_url="$DISCOVERED_SERVER"
    echo "[+] Auto-discovered server: $final_url"
  else
    read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip
    input_ip="${input_ip#http://}"
    input_ip="${input_ip#https://}"
    input_ip="${input_ip%%/*}"
    final_url="http://${input_ip}:8080/api"
  fi

  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  # Setup systemd service
  echo "[*] Setting up systemd service..."
  cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=PatchPilot Client
After=network.target

[Service]
ExecStart=$CLIENT_PATH
WorkingDirectory=$INSTALL_DIR
Restart=always
User=root
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reload
  systemctl enable patchpilot_client.service
  systemctl start patchpilot_client.service

  echo "[+] Installation complete!"
}

update() {
  echo "Updating PatchPilot client..."

  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "Error: Installation not found at $INSTALL_DIR"
    echo "Attempting to install PatchPilot client..."
    install
    return
  fi

  # Call common steps for install and update
  common_install_update

  # Install the client binary
  echo "[*] Installing client to $CLIENT_PATH..."
  cp target/release/rust_patch_client "$CLIENT_PATH"

  # Try auto-detecting the server (same logic as in install)
  echo "[*] Attempting to auto-discover the PatchPilot server on the local network..."
  if detect_server; then
    final_url="$DISCOVERED_SERVER"
    echo "[+] Auto-discovered server: $final_url"
  else
    read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip
    input_ip="${input_ip#http://}"
    input_ip="${input_ip#https://}"
    input_ip="${input_ip%%/*}"
    final_url="http://${input_ip}:8080/api"
  fi

  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  # Setup systemd service (same as install)
  echo "[*] Setting up systemd service..."
  cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=PatchPilot Client
After=network.target

[Service]
ExecStart=$CLIENT_PATH
WorkingDirectory=$INSTALL_DIR
Restart=always
User=root
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reload
  systemctl enable patchpilot_client.service
  systemctl start patchpilot_client.service

  echo "[+] Update complete!"
}

# Main script logic
case "$1" in
  --uninstall)
    uninstall
    ;;
  --update)
    update
    ;;
  --reinstall)
    uninstall
    install
    ;;
  *)
    show_usage
    ;;
esac
