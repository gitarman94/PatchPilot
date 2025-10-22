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
    # Optionally restart services or cron jobs here if needed
  else
    echo "🚀 No updates detected."
  fi
}

# Full install
install_client() {
  echo "[*] Installing dependencies..."

  # Check and install build-essential or dev tools for compiling Rust code
  if ! command -v cc >/dev/null 2>&1; then
    echo "C compiler (cc) not found. Installing build tools..."
    if command -v apt-get >/dev/null 2>&1; then
      $SUDO apt-get update
      $SUDO apt-get install -y build-essential
    elif command -v yum >/dev/null 2>&1; then
      $SUDO yum groupinstall -y "Development Tools"
    elif command -v dnf >/dev/null 2>&1; then
      $SUDO dnf groupinstall -y "Development Tools"
    else
      echo "Please install a C compiler toolchain (e.g. build-essential) manually."
      exit 1
    fi
  else
    echo "C compiler (cc) found."
  fi

  # Install dependencies (jq, curl) if missing
  if ! command -v jq >/dev/null 2>&1; then
    echo "Installing jq..."
    if command -v apt-get >/dev/null 2>&1; then
      $SUDO apt-get update && $SUDO apt-get install -y jq
    elif command -v yum >/dev/null 2>&1; then
      $SUDO yum install -y jq
    elif command -v dnf >/dev/null 2>&1; then
      $SUDO dnf install -y jq
    else
      echo "Please install jq manually."
      exit 1
    fi
  fi

  echo "[*] Creating install directory..."
  $SUDO rm -rf "$INSTALL_DIR"
  $SUDO mkdir -p "$INSTALL_DIR"

  echo "[*] Cloning client source..."
  $SUDO rm -rf /tmp/patchpilot_client_src
  git clone --depth=1 https://github.com/$GITHUB_USER/$GITHUB_REPO.git /tmp/patchpilot_client_src

  echo "[*] Building Rust client binary..."
  cd /tmp/patchpilot_client_src/patchpilot_client_rust
  $SUDO cargo build --release

  echo "[*] Copying built client binary..."
  $SUDO cp target/release/patchpilot_client "$INSTALL_DIR/patchpilot_client"
  $SUDO chmod +x "$INSTALL_DIR/patchpilot_client"

  # Also copy any other needed files (e.g. config.json, patchpilot_updater, patchpilot_client.sh)
  for file in "${FILES_TO_UPDATE[@]:1}"; do
    src="/tmp/patchpilot_client_src/linux-client/$file"
    if [[ -f "$src" ]]; then
      echo "Copying $file..."
      $SUDO cp "$src" "$INSTALL_DIR/$file"
      $SUDO chmod +x "$INSTALL_DIR/$file"
    fi
  done

  # Generate client_id.txt if missing or empty
  if [[ ! -s "$CLIENT_ID_FILE" ]]; then
    echo "Generating client ID..."
    uuidgen | $SUDO tee "$CLIENT_ID_FILE" >/dev/null
  fi

  # Prompt for server IP if not provided as env var
  if [[ -z "$SERVER_IP" ]]; then
    read -rp "Enter the patch server IP address (no port, e.g., 192.168.1.100): " input_ip
  else
    input_ip="$SERVER_IP"
  fi

  # Append port and /api path
  input_url="${input_ip}:8080/api"

  echo "Saving server URL: $input_url"
  echo "$input_url" | $SUDO tee "$SERVER_URL_FILE" >/dev/null

  # Setup cron job for running patchpilot_client every 10 minutes
  echo "[*] Setting up cron job..."
  $SUDO crontab -l 2>/dev/null | grep -v 'patchpilot_client' | $SUDO crontab -
  ( $SUDO crontab -l 2>/dev/null; echo "*/10 * * * * $INSTALL_DIR/patchpilot_client" ) | $SUDO crontab -

  echo "[✓] Installation complete."
}

# Uninstall client
uninstall_client() {
  echo "Uninstalling PatchPilot client..."

  # Remove cron jobs
  $SUDO crontab -l 2>/dev/null | grep -v 'patchpilot_client' | $SUDO crontab -

  # Remove files and directory
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
