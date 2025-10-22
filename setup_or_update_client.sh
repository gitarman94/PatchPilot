#!/bin/bash
set -e

# === CONFIG ===
GITHUB_USER="gitarman94"
GITHUB_REPO="PatchPilot"
BRANCH="main"

GITHUB_RAW_BASE="https://raw.githubusercontent.com/$GITHUB_USER/$GITHUB_REPO/$BRANCH/linux-client"

INSTALL_DIR="/opt/patchpilot_client"
SERVICE_CLIENT="patchpilot_client.service"
SERVICE_PING="patchpilot_ping.service"
SERVICE_CLIENT_TIMER="patchpilot_client.timer"
SERVICE_PING_TIMER="patchpilot_ping.timer"
PATCHPILOT_USER="patchpilot"
PATCHPILOT_GROUP="patchpilot"

FILES_TO_UPDATE=(
  "patchpilot_client"
  "patchpilot_updater"
  "config.json"
  "patchpilot_client.service"
  "patchpilot_client.timer"
  "patchpilot_ping.service"
  "patchpilot_ping.timer"
)

# === Helpers ===
function hash_file() {
    sha256sum "$1" 2>/dev/null | awk '{print $1}'
}

function create_patchpilot_user() {
    if ! id -u "$PATCHPILOT_USER" >/dev/null 2>&1; then
        echo "🛡️  Creating dedicated user and group '$PATCHPILOT_USER'..."
        groupadd --system "$PATCHPILOT_GROUP"
        useradd --system --gid "$PATCHPILOT_GROUP" --no-create-home --shell /usr/sbin/nologin "$PATCHPILOT_USER"
    else
        echo "✅ User '$PATCHPILOT_USER' already exists."
    fi
}

function set_permissions() {
    echo "🔒 Setting ownership and permissions on $INSTALL_DIR..."
    chown -R "$PATCHPILOT_USER":"$PATCHPILOT_GROUP" "$INSTALL_DIR"
    chmod -R 750 "$INSTALL_DIR"
}

function update_files() {
    echo "🔍 Checking for client updates via SHA256 hash..."

    local UPDATED=false

    for FILE in "${FILES_TO_UPDATE[@]}"; do
        local LOCAL_PATH="$INSTALL_DIR/$FILE"
        local TEMP_REMOTE="/tmp/$FILE.remote"

        echo "📁 Checking: $FILE"

        curl -fsSL "$GITHUB_RAW_BASE/$FILE" -o "$TEMP_REMOTE"

        local REMOTE_HASH
        REMOTE_HASH=$(hash_file "$TEMP_REMOTE")

        local LOCAL_HASH=""
        if [ -f "$LOCAL_PATH" ]; then
            LOCAL_HASH=$(hash_file "$LOCAL_PATH")
        fi

        if [ "$REMOTE_HASH" != "$LOCAL_HASH" ]; then
            echo "⬆️  $FILE is outdated. Updating..."
            cp "$TEMP_REMOTE" "$LOCAL_PATH"
            chmod +x "$LOCAL_PATH"
            UPDATED=true
        else
            echo "✅ $FILE is up to date."
        fi

        rm -f "$TEMP_REMOTE"
    done

    if [ "$UPDATED" = true ]; then
        set_permissions
        echo "🔁 Reloading systemd daemon and restarting client service..."
        systemctl daemon-reload
        systemctl restart "$SERVICE_CLIENT"
        echo "✅ Client update complete."
    else
        echo "🚀 No client updates detected. Everything is current."
    fi
}

function install_client() {
    echo "[*] Installing dependencies..."
    apt update
    apt install -y git curl jq systemd uuid-runtime build-essential pkg-config libssl-dev

    create_patchpilot_user

    # Install Rust if missing
    if ! command -v cargo >/dev/null 2>&1; then
      echo "[*] Installing Rust toolchain..."
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      source "$HOME/.cargo/env"
    fi

    echo "[*] Cloning repository..."
    rm -rf "/tmp/$GITHUB_REPO"
    git clone "https://github.com/$GITHUB_USER/$GITHUB_REPO.git" "/tmp/$GITHUB_REPO"

    echo "[*] Building Rust binaries..."
    pushd "/tmp/$GITHUB_REPO/linux-client" >/dev/null
    cargo build --release
    popd >/dev/null

    echo "[*] Creating install directory..."
    rm -rf "$INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"

    echo "[*] Copying client files..."
    cp "/tmp/$GITHUB_REPO/linux-client/target/release/patchpilot_client" "$INSTALL_DIR/"
    cp "/tmp/$GITHUB_REPO/linux-client/target/release/patchpilot_updater" "$INSTALL_DIR/"
    cp "/tmp/$GITHUB_REPO/linux-client/config.json" "$INSTALL_DIR/" 2>/dev/null || true

    # Generate client ID if missing
    if [ ! -f "$INSTALL_DIR/client_id.txt" ]; then
      uuidgen > "$INSTALL_DIR/client_id.txt"
    fi

    # Prompt for server URL if not provided as argument
    if [ -z "$1" ]; then
      read -rp "Enter the patch server URL (e.g., 192.168.1.100:8080): " SERVER_URL
    else
      SERVER_URL="$1"
    fi

    SERVER_URL="${SERVER_URL#http://}"
    SERVER_URL="${SERVER_URL#https://}"

    echo "$SERVER_URL" > "$INSTALL_DIR/server_url.txt"

    set_permissions

    echo "[*] Installing systemd unit files..."

    # Modify service files to run as patchpilot user/group
    for svcfile in patchpilot_client.service patchpilot_ping.service; do
        sed -i '/^\[Service\]/a User=patchpilot\nGroup=patchpilot' "/tmp/$GITHUB_REPO/linux-client/$svcfile"
        cp "/tmp/$GITHUB_REPO/linux-client/$svcfile" /etc/systemd/system/
    done

    # Timers usually don't need user set, just copy as is
    cp "/tmp/$GITHUB_REPO/linux-client/patchpilot_client.timer" /etc/systemd/system/
    cp "/tmp/$GITHUB_REPO/linux-client/patchpilot_ping.timer" /etc/systemd/system/

    echo "[*] Reloading systemd daemon and enabling timers..."
    systemctl daemon-reload
    systemctl enable --now patchpilot_client.timer
    systemctl enable --now patchpilot_ping.timer

    echo "[✓] Installation complete. Client is active."
}

# === Main ===

if [ "$(id -u)" -ne 0 ]; then
    echo "⚠️  Please run this script as root or with sudo."
    exit 1
fi

if [ -d "$INSTALL_DIR" ] && [ -f "$INSTALL_DIR/patchpilot_client" ]; then
    echo "[*] Detected existing client installation. Running update..."
    update_files
else
    echo "[*] No client installation detected. Running full install..."
    install_client "$1"
fi
