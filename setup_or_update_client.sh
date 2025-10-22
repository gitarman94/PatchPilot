#!/bin/bash
set -e

# === Config ===
INSTALL_DIR="/opt/patchpilot_client"
REPO_URL="https://github.com/gitarman94/PatchPilot.git"
CLIENT_SRC_DIR="/tmp/patchpilot_client_src"
RUST_CLIENT_SUBDIR="patchpilot_client_rust"

# Check if running as root or set sudo prefix
if [[ $EUID -eq 0 ]]; then
  SUDO=""
else
  SUDO="sudo"
fi

# Helper: Install Rust toolchain if missing
install_rust() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust toolchain not found. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
}

# Helper: Clone or update client source code
update_source() {
  if [[ -d "$CLIENT_SRC_DIR" ]]; then
    echo "Updating existing client source..."
    git -C "$CLIENT_SRC_DIR" pull
  else
    echo "Cloning client source..."
    git clone "$REPO_URL" "$CLIENT_SRC_DIR"
  fi
}

# Build Rust client binary
build_client() {
  echo "Building Rust client binary..."
  cd "$CLIENT_SRC_DIR/$RUST_CLIENT_SUBDIR"
  cargo build --release
}

# Install client files
install_files() {
  echo "Installing client files..."

  $SUDO mkdir -p "$INSTALL_DIR"
  
  # Copy binary
  $SUDO cp "$CLIENT_SRC_DIR/$RUST_CLIENT_SUBDIR/target/release/patchpilot_client" "$INSTALL_DIR/"
  $SUDO chmod +x "$INSTALL_DIR/patchpilot_client"

  # Copy config.json if exists in repo root or elsewhere
  # Adjust this path if your config.json is somewhere else
  if [[ -f "$CLIENT_SRC_DIR/config.json" ]]; then
    $SUDO cp "$CLIENT_SRC_DIR/config.json" "$INSTALL_DIR/"
  else
    # Create an empty config.json if missing
    echo "{}" | $SUDO tee "$INSTALL_DIR/config.json" > /dev/null
  fi

  # Generate client_id.txt if missing
  if [[ ! -f "$INSTALL_DIR/client_id.txt" ]]; then
    echo "Generating client ID..."
    uuidgen | $SUDO tee "$INSTALL_DIR/client_id.txt" > /dev/null
  fi
}

# Save server URL (append /api automatically)
save_server_url() {
  local input_url="$1"
  input_url="${input_url#http://}"
  input_url="${input_url#https://}"
  if [[ "$input_url" != */api ]]; then
    input_url="${input_url}/api"
  fi
  echo "Saving server URL: $input_url"
  echo "$input_url" | $SUDO tee "$INSTALL_DIR/server_url.txt" > /dev/null
}

# Setup cron jobs
setup_cron() {
  echo "[*] Setting up cron jobs..."

  # Remove old jobs for patchpilot_client (adjust if your client uses other scripts)
  $SUDO crontab -l 2>/dev/null | grep -v 'patchpilot_client' | $SUDO crontab -

  # Add new cron job to run the Rust binary every 10 mins
  # (adjust command if client requires arguments or a wrapper script)
  ( $SUDO crontab -l 2>/dev/null; echo "*/10 * * * * $INSTALL_DIR/patchpilot_client" ) | $SUDO crontab -
}

# Uninstall client
uninstall_client() {
  echo "Uninstalling PatchPilot client..."

  # Remove cron jobs
  $SUDO crontab -l 2>/dev/null | grep -v 'patchpilot_client' | $SUDO crontab -

  # Remove install dir
  $SUDO rm -rf "$INSTALL_DIR"

  # Remove source dir
  rm -rf "$CLIENT_SRC_DIR"

  echo "Uninstall complete."
}

# === Main ===
if [[ "$1" == "-u" || "$1" == "--uninstall" ]]; then
  uninstall_client
  exit 0
fi

if [[ -f "$INSTALL_DIR/patchpilot_client" ]]; then
  echo "Existing installation detected. Running update..."

  install_rust
  update_source
  build_client
  install_files

  echo "PatchPilot client updated."
else
  echo "No installation detected. Running full install..."

  install_rust
  update_source
  build_client
  install_files

  # Prompt for server IP (without port)
  read -rp "Enter the patch server IP (without port): " server_ip
  save_server_url "$server_ip"

  setup_cron

  echo "PatchPilot client installed."
fi
