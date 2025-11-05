#!/usr/bin/env bash
set -euo pipefail

# --- Configuration ---
APP_DIR="/opt/patchpilot_client"
SRC_DIR="/tmp/patchpilot_client_src"
RUST_REPO="https://github.com/gitarman94/PatchPilot.git"
CLIENT_BINARY="$APP_DIR/patchpilot_client"
SERVICE_NAME="patchpilot_client.service"
SYSTEMD_DIR="/etc/systemd/system"

FORCE_INSTALL=false
UPDATE=false

# --- Parse arguments ---
for arg in "$@"; do
    case "$arg" in
        --force) FORCE_INSTALL=true ;;
        --update) UPDATE=true ;;
        *) echo "Usage: $0 [--force] [--update]"; exit 1 ;;
    esac
done

# --- OS check ---
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "❌ This installer works only on Debian-based systems."; exit 1 ;;
    esac
else
    echo "❌ Cannot determine OS – /etc/os-release missing."
    exit 1
fi

# --- Cleanup for force install ---
if [[ "$FORCE_INSTALL" = true ]]; then
    echo "🧹 Cleaning up old installation..."
    
    # Check if the service exists before stopping or disabling
    if systemctl list-units --full --all | grep -q "$SERVICE_NAME"; then
        systemctl stop "$SERVICE_NAME" || true
        systemctl disable "$SERVICE_NAME" || true
        rm -f "${SYSTEMD_DIR}/${SERVICE_NAME}"
        systemctl daemon-reload
    else
        echo "⚠️ Service $SERVICE_NAME not found. Skipping service cleanup."
    fi

    # Remove Rust and Cargo installed under APP_DIR
    rm -rf "$APP_DIR" "$HOME/.cargo" "$HOME/.rustup"

    # Remove /etc/environment entries
    sed -i '/CARGO_HOME/d' /etc/environment || true
    sed -i '/RUSTUP_HOME/d' /etc/environment || true
    sed -i "/PATH=.*\/opt\/patchpilot_client\/.cargo\/bin/d" /etc/environment || true
fi

mkdir -p "$APP_DIR"

# --- Install dependencies ---
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl git build-essential pkg-config libssl-dev

# --- Install Rust locally ---
CARGO_HOME="$APP_DIR/.cargo"
RUSTUP_HOME="$APP_DIR/.rustup"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"
export CARGO_HOME RUSTUP_HOME PATH="$CARGO_HOME/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    echo "🛠️ Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi

"$CARGO_HOME/bin/rustup" default stable
"$CARGO_HOME/bin/cargo" --version

# --- Clone and build client ---
if [[ -d "$SRC_DIR" ]]; then rm -rf "$SRC_DIR"; fi
mkdir -p "$SRC_DIR"
git clone "$RUST_REPO" "$SRC_DIR"

cd "$SRC_DIR/patchpilot_client"

export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include
export OPENSSL_DIR=/usr
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig"

echo "🔨 Building PatchPilot client..."
"$CARGO_HOME/bin/cargo" build --release

# --- Copy binary ---
cp target/release/rust_patch_client "$CLIENT_BINARY"
chmod +x "$CLIENT_BINARY"

# --- Optional: patchpilot user ---
if ! id -u patchpilot >/dev/null 2>&1; then
    useradd -r -s /usr/sbin/nologin patchpilot
fi
chown -R patchpilot:patchpilot "$APP_DIR"
chmod -R 755 "$APP_DIR"
find "$APP_DIR" -type f -exec chmod 755 {} \;

# --- Prompt for server URL ---
read -rp "Enter the PatchPilot server IP (e.g., 192.168.1.100): " input_ip
input_ip="${input_ip#http://}"
input_ip="${input_ip#https://}"
input_ip="${input_ip%%/*}"
echo "http://${input_ip}:8080/api" > "$APP_DIR/server_url.txt"

# --- Setup systemd service ---
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=PatchPilot Client
After=network.target

[Service]
User=patchpilot
Group=patchpilot
ExecStart=${CLIENT_BINARY}
WorkingDirectory=${APP_DIR}
Restart=always
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "$SERVICE_NAME"

echo "✅ Installation complete!"
