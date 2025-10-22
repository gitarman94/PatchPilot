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
  "patchpilot_ping.sh"
)

CLIENT_ID_FILE="$INSTALL_DIR/client_id.txt"
SERVER_URL_FILE="$INSTALL_DIR/server_url.txt"

# === FUNCTIONS ===

download_file() {
  url=$1
  dest=$2
  curl -sSL "$url" -o "$dest"
}

file_hash() {
  sha256sum "$1" | awk '{print $1}'
}

update_files() {
  echo "🔍 Checking for client updates..."
  updated=false

  for file in "${FILES_TO_UPDATE[@]}"; do
    local_path="$INSTALL_DIR/$file"
    temp_remote="/tmp/$file.remote"
    remote_url="$RAW_BASE/$file"

    echo "📁 Checking $file"
    download_file "$remote_url" "$temp_remote"

    remote_hash=$(file_hash "$temp_remote")
    local_hash=""
    [[ -f "$local_path" ]] && local_hash=$(file_hash "$local_path")

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

install_client() {
  echo "[*] Installing dependencies..."
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

  echo "[*] Creating install directory..."
  $SUDO rm -rf "$INSTALL_DIR"
  $SUDO mkdir -p "$INSTALL_DIR"

  echo "[*] Downloading client files..."
  for file in "${FILES_TO_UPDATE[@]}"; do
    url="$RAW_BASE/$file"
    dest="$INSTALL_DIR/$file"
    echo "Downloading $file..."
    $SUDO curl -sSL "$url" -o "$dest"
    $SUDO chmod +x "$dest"
  done

  if [[ ! -f "$CLIENT_ID_FILE" ]]; then
    echo "Generating client ID..."
    uuidgen | $SUDO tee "$CLIENT_ID_FILE" >/dev/null
  fi

  if [[ -z "$SERVER_URL" ]]; then
    read -rp "Enter the patch server URL (e.g., 192.168.1.100:8080): " input_url
  else
    input_url="$SERVER_URL"
  fi

  input_url="${input_url#http://}"
  input_url="${input_url#https://}"

  [[ "$input_url" != */api ]] && input_url="${input_url}/api"

  echo "Saving server URL: $input_url"
  echo "$input_url" | $SUDO tee "$SERVER_URL_FILE" >/dev/null

  echo "[*] Setting up cron jobs..."

  # Remove old jobs
  crontab_tmp=$(mktemp)
  crontab -l 2>/dev/null | grep -v 'patchpilot_client.sh' | grep -v 'patchpilot_ping.sh' > "$crontab_tmp" || true
  echo "*/10 * * * * $INSTALL_DIR/patchpilot_client.sh" >> "$crontab_tmp"
  echo "*/5 * * * * $INSTALL_DIR/patchpilot_ping.sh" >> "$crontab_tmp"
  $SUDO crontab "$crontab_tmp"
  rm "$crontab_tmp"

  echo "[✓] Installation complete."
}

uninstall_client() {
  echo "Uninstalling PatchPilot client..."

  crontab_tmp=$(mktemp)
  crontab -l 2>/dev/null | grep -v 'patchpilot_client.sh' | grep -v 'patchpilot_ping.sh' > "$crontab_tmp" || true
  $SUDO crontab "$crontab_tmp"
  rm "$crontab_tmp"

  $SUDO rm -rf "$INSTALL_DIR"

  echo "Uninstall complete."
}

# === MAIN ===

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
