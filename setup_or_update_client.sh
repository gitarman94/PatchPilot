#!/bin/bash
set -e

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
  "patchpilot_ping.sh"
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
  # Install dependencies (jq, curl, git) if missing
  if ! command -v jq >/dev/null 2>&1; then
    echo "Installing jq..."
    if command -v apt-get >/dev/null 2>&1; then
      sudo apt-get update && sudo apt-get install -y jq
    elif command -v yum >/dev/null 2>&1; then
      sudo yum install -y jq
    else
      echo "Please install jq manually."
      exit 1
    fi
  fi

  echo "[*] Creating install directory..."
  sudo rm -rf "$INSTALL_DIR"
  sudo mkdir -p "$INSTALL_DIR"

  echo "[*] Downloading client files..."
  for file in "${FILES_TO_UPDATE[@]}"; do
    local url="$RAW_BASE/$file"
    local dest="$INSTALL_DIR/$file"
    echo "Downloading $file..."
    sudo curl -sSL "$url" -o "$dest"
    sudo chmod +x "$dest"
  done

  # Generate client_id.txt if missing
  if [[ ! -f "$CLIENT_ID_FILE" ]]; then
    echo "Generating client ID..."
    uuidgen | sudo tee "$CLIENT_ID_FILE" >/dev/null
  fi

  # Prompt for server URL if not provided as env var
  if [[ -z "$SERVER_URL" ]]; then
    read -rp "Enter the patch server URL (e.g., 192.168.1.100:8080): " input_url
  else
    input_url="$SERVER_URL"
  fi

  # Strip protocol if present
  input_url="${input_url#http://}"
  input_url="${input_url#https://}"

  # Append /api if not already there
  if [[ "$input_url" != */api ]]; then
    input_url="${input_url}/api"
  fi

  echo "Saving server URL: $input_url"
  echo "$input_url" | sudo tee "$SERVER_URL_FILE" >/dev/null

  # Setup cron jobs (client and ping)
  echo "[*] Setting up cron jobs..."

  # Remove old jobs
  sudo crontab -l 2>/dev/null | grep -v 'patchpilot_client.sh' | grep -v 'patchpilot_ping.sh' | sudo crontab -

  # Add new cron jobs: client every 10 mins, ping every 5 mins
  (sudo crontab -l 2>/dev/null; echo "*/10 * * * * $INSTALL_DIR/patchpilot_client.sh") | sudo crontab -
  (sudo crontab -l 2>/dev/null; echo "*/5 * * * * $INSTALL_DIR/patchpilot_ping.sh") | sudo crontab -

  echo "[✓] Installation complete."
}

# Uninstall client
uninstall_client() {
  echo "Uninstalling PatchPilot client..."

  # Remove cron jobs
  sudo crontab -l 2>/dev/null | grep -v 'patchpilot_client.sh' | grep -v 'patchpilot_ping.sh' | sudo crontab -

  # Remove files and directory
  sudo rm -rf "$INSTALL_DIR"

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
