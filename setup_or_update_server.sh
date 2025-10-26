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
POSTGRES_DB="patchpilot_db"
POSTGRES_USER="patchpilot_user"
POSTGRES_PASSWORD_FILE="$INSTALL_DIR/postgres_password.txt"

show_usage() {
  echo "Usage: $0 [--uninstall] [--update] [--reinstall]"
  exit 1
}

generate_random_password() {
  # Generate a secure random password
  echo "$(openssl rand -base64 16)"
}

setup_postgresql() {
  echo "[*] Setting up PostgreSQL database..."

  # Check if PostgreSQL is installed
  if ! command -v psql >/dev/null 2>&1; then
    echo "PostgreSQL not found, installing..."
    # Assuming `apt-get` is available for package installation
    apt-get update
    apt-get install -y postgresql postgresql-contrib
  fi

  # Create database and user if they do not exist
  echo "[*] Ensuring PostgreSQL user and database exist..."

  PGPASSWORD=$(generate_random_password)
  echo "Generated PostgreSQL password: $PGPASSWORD"

  # Save the password to a file
  echo "$PGPASSWORD" > "$POSTGRES_PASSWORD_FILE"

  # Ensure the PostgreSQL service is running
  systemctl start postgresql || true

  # Create the user and database if they don't exist
  sudo -u postgres psql <<-EOF
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '$POSTGRES_USER') THEN
            CREATE ROLE $POSTGRES_USER WITH LOGIN PASSWORD '$PGPASSWORD';
        END IF;
    END
    \$\$;

    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = '$POSTGRES_DB') THEN
            CREATE DATABASE $POSTGRES_DB OWNER $POSTGRES_USER;
        END IF;
    END
    \$\$;
EOF
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

update() {
  echo "Updating PatchPilot client..."
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "Error: Installation not found at $INSTALL_DIR"
    echo "Attempting to install PatchPilot client..."
    install
    return
  fi

  echo "[*] Installing dependencies..."
  apt-get update -y
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain if missing..."
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
  
  cd "$SRC_DIR/patchpilot_client_rust"
  cargo clean
  cargo build --release

  systemctl stop patchpilot_client.service || true

  echo "[*] Copying binaries..."
  cp target/release/rust_patch_client "$CLIENT_PATH"
  cp target/release/patchpilot_updater "$UPDATER_PATH"
  chmod +x "$CLIENT_PATH" "$UPDATER_PATH"

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

  systemctl start patchpilot_client.service || true
  echo "Update complete."
}

install() {
  echo "Installing PatchPilot client..."

  echo "[*] Installing dependencies..."
  apt-get update
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  if [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found at /root/.cargo/env"
  fi

  setup_postgresql

  echo "[*] Cloning client source..."
  rm -rf "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"

  cd "$SRC_DIR/patchpilot_client_rust"
  cargo clean
  cargo build --release

  if [[ ! -d "$INSTALL_DIR" ]]; then
    mkdir -p "$INSTALL_DIR" || { echo "Error: Failed to create directory $INSTALL_DIR"; exit 1; }
  fi

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

  echo "[✔] Installation complete. PatchPilot client is running."
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

# Prompt for server IP (moved to the bottom)
read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip

input_ip="${input_ip#http://}"
input_ip="${input_ip#https://}"
input_ip="${input_ip%%/*}"

final_url="http://${input_ip}:8080/api"
echo "Saving server URL: $final_url"
echo "$final_url" > "$SERVER_URL_FILE"

exit 0
