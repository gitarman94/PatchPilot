#!/bin/bash
set -e

# Detect if running as root
if [[ $EUID -eq 0 ]]; then
  SUDO=""
else
  SUDO="sudo"
fi

INSTALL_DIR="/opt/patchpilot_client"
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"

RAW_BASE="https://raw.githubusercontent.com/$GITHUB_USER/$GITHUB_REPO/$BRANCH/linux-client"

FILES_TO_UPDATE=(
  "patchpilot_client"
  "patchpilot_updater"
  "config.json"
  "patchpilot_client.sh"
)

CLIENT_ID_FILE="$INSTALL_DIR/client_id.txt"
SERVER_URL_FILE="$INSTALL_DIR/server_url.txt"

# Helper: Download a file
download_file() {
  local url=$1
  local dest=$2
  curl -sSL "$url" -o "$dest"
}

# Helper: Compute SHA256 hash of a file
file_hash() {
  sha256sum "$1" | awk '{print $1}'
}

# Update files if changed
update_files() {
  echo "🔍 Checking for client updates..."

  updated=false

  for file in "${FILES_TO_UPDATE[@]}"; do
    local local_path="$INSTALL_DIR/$file"
    local temp_remote="/tmp/$file.remote"

    local remote_url="$RAW_BASE/$file"
    echo "📁 Checking $file"

    download_file "$remote_url" "$temp_remote"

    local remote_hash
    remote_hash=$(file_hash "$temp_remote")

    local local_hash=""
    if [[ -f "$local_path" ]]; then
      local_hash=$(file_hash "$local_path")
    fi

    if [[ "$remote_hash" != "$local_hash" ]]; then
      echo "⬆️  $file is outdated. Updating..."
      cp "$temp_remote" "$local_path"
      chmod +x "$local_path"
      updated=true
    else
      echo "✅ $file is up to date."
    fi

    rm -f "$temp_remote"
  done

  if $updated; then
    echo "🔁 Client files updated."
  else
    echo "🚀 No updates detected."
  fi
}

# Full install
install_client() {
  echo "[*] Installing dependencies..."

  # Install jq if missing
  if ! command -v jq >/dev/null 2>&1; then
    echo "Installing jq..."
    if command -v apt-get >/dev/null 2>&1; then
      $SUDO apt-get update
      $SUDO apt-get install -y jq
    elif command -v yum >/dev/null 2>&1; then
      $SUDO yum install -y jq
    else
      echo "Please install jq manually."
      exit 1
    fi
  fi

  # Install build tools (gcc, make, etc) required for Rust builds
  if ! command -v cc >/dev/null 2>&1; then
    echo "Installing build tools..."

    if command -v apt-get >/dev/null 2>&1; then
      $SUDO apt-get update
      $SUDO apt-get install -y build-essential
    elif command -v yum >/dev/null 2>&1; then
      $SUDO yum groupinstall -y "Development Tools"
    elif command -v dnf >/dev/null 2>&1; then
      $SUDO dnf groupinstall -y "Development Tools"
    else
      echo "Please install C compiler and build tools manually."
      exit 1
    fi
  fi

  # Verify compiler installed
  if ! command -v cc >/dev/null 2>&1; then
    echo "Error: C compiler 'cc' not found after installing build tools."
    echo "Please install it manually and re-run this script."
    exit 1
  fi

  echo "[*] Creating install directory..."
  $SUDO rm -rf "$INSTALL_DIR"
  $SUDO mkdir -p "$INSTALL_DIR"

  echo "[*] Cloning Rust client source..."
  $SUDO rm -rf /tmp/patchpilot_client_src
  git clone --depth 1 https://github.com/$GITHUB_USER/$GITHUB_REPO.git /tmp/patchpilot_client_src

  echo "[*] Building Rust client binary..."
  pushd /tmp/patchpilot_client_src/patchpilot_client_rust >/dev/null
  $SUDO cargo build --release
  popd >/dev/null

  echo "[*] Copying built client to install directory..."
  $SUDO cp /tmp/patchpilot_client_src/patchpilot_client_rust/target/release/patchpilot_client "$INSTALL_DIR/"
  $SUDO chmod +x "$INSTALL_DIR/patchpilot_client"

  # Generate client_id.txt if missing
  if [[ ! -f "$CLIENT_ID_FILE" ]]; then
    echo "Generating client ID..."
    uuidgen | $SUDO tee "$CLIENT_ID_FILE" >/dev/null
  fi

  # Prompt for server URL if not provided as env var
  if [[ -z "$SERVER_URL" ]]; then
    read -rp "Enter the patch server IP (without port): " input_ip
  else
    input_ip="$SERVER_URL"
  fi

  # Append default port and /api path
  input_url="${input_ip}:8080/api"

  echo "Saving server URL: $input_url"
  echo "$input_url" | $SUDO tee "$SERVER_URL_FILE" >/dev/null

  echo "[✓] Installation complete."
}

# Uninstall client
uninstall_client() {
  echo "Uninstalling PatchPilot client..."

  $SUDO rm -rf "$INSTALL_DIR"

  echo "Uninstall complete."
}

# === Main ===
if [[ "$1" == "-u" || "$1" == "--uninstall" ]]; then
  uninstall_client
  exit 0
fi

if [[ -f "$INSTALL_DIR/patchpilot_client" ]]; then
  echo "Existing installation detected. Running update..."
  update_files
else
  echo "No installation detected. Running full install..."
  install_client
fi
