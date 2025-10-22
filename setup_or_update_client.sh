#!/bin/bash

set -e

INSTALL_DIR="/opt/patchpilot_client"
REPO_URL="https://github.com/gitarman94/PatchPilot.git"
CLIENT_SRC_DIR="/tmp/patchpilot_client_src/patchpilot_client_rust"

SUDO=""
if [ "$EUID" -ne 0 ]; then
  if command -v sudo >/dev/null 2>&1; then
    SUDO="sudo"
  else
    echo "This script must be run as root or with sudo"
    exit 1
  fi
fi

usage() {
  echo "Usage: $0 [--uninstall]"
  exit 1
}

uninstall() {
  echo "Uninstalling PatchPilot client..."
  $SUDO rm -rf "$INSTALL_DIR"
  $SUDO rm -rf /tmp/patchpilot_client_src
  echo "Uninstallation complete."
  exit 0
}

if [ "$1" == "--uninstall" ]; then
  uninstall
fi

if [ -d "$INSTALL_DIR" ]; then
  echo "Existing installation detected. Running update..."
else
  echo "No installation detected. Running full install..."
  echo "[*] Installing dependencies..."
  $SUDO apt-get update
  $SUDO apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  else
    echo "Rust already installed."
  fi

  echo "[*] Creating install directory..."
  $SUDO mkdir -p "$INSTALL_DIR"
fi

echo "[*] Cloning client source..."
$SUDO rm -rf /tmp/patchpilot_client_src
git clone --depth=1 "$REPO_URL" /tmp/patchpilot_client_src

echo "[*] Building Rust client binary..."

if [ -n "$SUDO_USER" ] && [ "$SUDO_USER" != "root" ]; then
  echo "[*] Building as user: $SUDO_USER"
  sudo -u "$SUDO_USER" bash -c "source \$HOME/.cargo/env && cd $CLIENT_SRC_DIR && cargo build --release"
else
  if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
  elif [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Error: Rust environment file not found."
    exit 1
  fi
  cd "$CLIENT_SRC_DIR"
  cargo build --release
fi

echo "[*] Installing binaries..."
$SUDO cp "$CLIENT_SRC_DIR/target/release/patchpilot_client" "$INSTALL_DIR/patchpilot_client"
$SUDO cp "$CLIENT_SRC_DIR/target/release/patchpilot_updater" "$INSTALL_DIR/patchpilot_updater"

# Set executable permissions
$SUDO chmod +x "$INSTALL_DIR/patchpilot_client" "$INSTALL_DIR/patchpilot_updater"

echo "[*] Setup complete."
