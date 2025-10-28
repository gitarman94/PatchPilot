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

# --- Auto-detect the server ---
# Attempts to auto-discover the PatchPilot server on the local network.
# Returns discovered URL in $DISCOVERED_SERVER (e.g. http://192.168.1.100:8080/api) or empty if none found.
detect_server() {
  DISCOVERED_SERVER=""

  # Helper: verify a candidate by checking an expected endpoint for either JSON or a known string
  verify_candidate() {
    local ip="$1"
    local url="http://${ip}:8080/api"   # install uses :8080/api
    # Check quickly if server responds and if response looks like our API (contains "clients" or "adopted" fields).
    local resp
    resp=$(curl -s --max-time 2 "${url}/clients" || true)
    if [[ -n "$resp" ]]; then
      # crude JSON check: does it contain "clients" or "adopted"
      if echo "$resp" | grep -q '"clients"' || echo "$resp" | grep -q '"adopted"' ; then
        DISCOVERED_SERVER="$url"
        return 0
      fi
    fi
    return 1
  }

  # 1) Try mDNS via avahi-browse (if available). Look for HTTP services or a service name you might advertise.
  if command -v avahi-browse >/dev/null 2>&1; then
    echo "[*] Trying mDNS (avahi-browse) for HTTP services..."
    # find http services on the local network
    # -r for resolve, -t for terminating after browse
    while read -r line; do
      # avahi-browse -rt _http._tcp will output resolved lines containing host and port
      # Parse an IP if present
      candidate_ip=$(echo "$line" | grep -oP '\d+\.\d+\.\d+\.\d+' | head -n1 || true)
      if [[ -n "$candidate_ip" ]]; then
        echo "[*] mDNS candidate: $candidate_ip"
        if verify_candidate "$candidate_ip"; then
          echo "[+] Found server via mDNS: $DISCOVERED_SERVER"
          return 0
        fi
      fi
    done < <(avahi-browse -rt _http._tcp 2>/dev/null || true)
  fi

  # 2) Try nmap (if available) for a quick port 8080 scan of local /24
  if [[ -z "$DISCOVERED_SERVER" && $(command -v nmap >/dev/null 2>&1; echo $?) -eq 0 ]]; then
    echo "[*] Trying nmap scan of local /24 for port 8080..."
    # find local IPv4 address
    local my_ip
    my_ip=$(ip -4 addr show scope global | awk '/inet / {print $2}' | head -n1 | cut -d'/' -f1 || true)
    if [[ -n "$my_ip" ]]; then
      base="${my_ip%.*}.0/24"
      # Only scan port 8080 to find open hosts; output grepable and parse.
      nmap -Pn -p 8080 --open "$base" -oG - 2>/dev/null | awk '/8080\/open/ {print $2}' | while read -r host; do
        echo "[*] nmap open candidate: $host"
        if verify_candidate "$host"; then
          echo "[+] Found server via nmap: $DISCOVERED_SERVER"
          return 0
        fi
      done
    fi
  fi

  # 3) Fallback: fast /24 HTTP probe using curl with concurrency limit
  if [[ -z "$DISCOVERED_SERVER" ]]; then
    echo "[*] Falling back to fast /24 HTTP probe (this may take a few seconds)..."

    # Get first non-loopback IPv4 and assume /24 (safe default). If we can't find local IP, skip.
    local local_ip
    local_ip=$(ip -4 addr show scope global | awk '/inet / {print $2}' | head -n1 | cut -d'/' -f1 || true)
    if [[ -z "$local_ip" ]]; then
      echo "[!] Could not determine local IP for subnet scan."
      return 1
    fi

    local base="${local_ip%.*}"
    local concurrency=60   # how many parallel probes
    local -a pids=()
    for i in $(seq 1 254); do
      ip="${base}.${i}"
      {
        # tiny probe: check /api/clients for expected JSON
        if curl -s --max-time 2 "http://${ip}:8080/api/clients" | grep -q '"clients"' 2>/dev/null; then
          echo "[+] Found server at ${ip}"
          DISCOVERED_SERVER="http://${ip}:8080/api"
          # write discovered IP to temporary file to let other jobs stop quickly
          echo "$DISCOVERED_SERVER" > /tmp/.patchpilot_discovered 2>/dev/null || true
        fi
      } &
      pids+=($!)
      # throttle concurrency
      if (( ${#pids[@]} >= concurrency )); then
        wait -n 2>/dev/null || wait
        # cleanup completed pids array
        newpids=()
        for pid in "${pids[@]}"; do
          if kill -0 "$pid" 2>/dev/null; then
            newpids+=("$pid")
          fi
        done
        pids=("${newpids[@]}")
      fi
      # quick exit if another background job already found server
      if [[ -f /tmp/.patchpilot_discovered ]]; then
        DISCOVERED_SERVER=$(cat /tmp/.patchpilot_discovered 2>/dev/null || true)
        break
      fi
    done

    # wait for remaining jobs to finish (or bail if discovered)
    if [[ -z "$DISCOVERED_SERVER" ]]; then
      wait
      if [[ -f /tmp/.patchpilot_discovered ]]; then
        DISCOVERED_SERVER=$(cat /tmp/.patchpilot_discovered 2>/dev/null || true)
      fi
    fi

    # cleanup
    rm -f /tmp/.patchpilot_discovered 2>/dev/null || true

    if [[ -n "$DISCOVERED_SERVER" ]]; then
      echo "[+] Found server via HTTP probe: $DISCOVERED_SERVER"
      return 0
    fi
  fi

  # Nothing found
  return 1
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
  # Remove cron jobs related to patchpilot_client (if any)
  crontab -l | grep -v 'patchpilot_client' | crontab - || true
  rm -rf "$INSTALL_DIR"
  echo "Uninstalled."
}

update() {
  echo "Updating PatchPilot client..."

  # Check if the installation directory exists
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "Error: Installation not found at $INSTALL_DIR"
    echo "Attempting to install PatchPilot client..."
    install  # Trigger install if the directory doesn't exist
    return
  fi

  # Install dependencies and Rust toolchain as necessary
  echo "[*] Installing dependencies..."
  apt-get update -y
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain if missing..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  # Load Rust environment for root
  if [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found at /root/.cargo/env"
  fi

  echo "[*] Cloning client source..."
  rm -rf "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"

  cd "$SRC_DIR/patchpilot_client_rust"

  export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
  export OPENSSL_INCLUDE_DIR=/usr/include
  export OPENSSL_DIR=/usr
  export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig"

  echo "[*] Building PatchPilot client..."
  cargo build --release

  # Install the client binary
  echo "[*] Installing client to $CLIENT_PATH..."
  mkdir -p "$INSTALL_DIR"
  cp target/release/patchpilot_client "$CLIENT_PATH"

  # Try auto-detecting the server
  echo "[*] Attempting to auto-discover the PatchPilot server on the local network..."
  if detect_server; then
    final_url="$DISCOVERED_SERVER"
    echo "[+] Auto-discovered server: $final_url"
  else
    # only prompt if discovery failed
    read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip
    input_ip="${input_ip#http://}"
    input_ip="${input_ip#https://}"
    input_ip="${input_ip%%/*}"
    final_url="http://${input_ip}:8080/api"
  fi

  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  # Setup service
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

install() {
  echo "Installing PatchPilot client..."

  # Install dependencies and Rust toolchain as necessary
  echo "[*] Installing dependencies..."
  apt-get update -y
  apt-get install -y curl git build-essential pkg-config libssl-dev

  echo "[*] Installing Rust toolchain if missing..."
  if ! command -v rustc >/dev/null 2>&1; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
  fi

  # Load Rust environment for root
  if [ -f "/root/.cargo/env" ]; then
    source "/root/.cargo/env"
  else
    echo "Warning: Rust environment file not found at /root/.cargo/env"
  fi

  echo "[*] Cloning client source..."
  rm -rf "$SRC_DIR"
  git clone "$RUST_REPO" "$SRC_DIR"

  cd "$SRC_DIR/patchpilot_client_rust"

  export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
  export OPENSSL_INCLUDE_DIR=/usr/include
  export OPENSSL_DIR=/usr
  export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig"

  echo "[*] Building PatchPilot client..."
  cargo build --release

  # Install the client binary
  echo "[*] Installing client to $CLIENT_PATH..."
  mkdir -p "$INSTALL_DIR"
  cp target/release/patchpilot_client "$CLIENT_PATH"

  # Try auto-detecting the server
  echo "[*] Attempting to auto-discover the PatchPilot server on the local network..."
  if detect_server; then
    final_url="$DISCOVERED_SERVER"
    echo "[+] Auto-discovered server: $final_url"
  else
    # only prompt if discovery failed
    read -rp "Enter the patch server IP (e.g., 192.168.1.100): " input_ip
    input_ip="${input_ip#http://}"
    input_ip="${input_ip#https://}"
    input_ip="${input_ip%%/*}"
    final_url="http://${input_ip}:8080/api"
  fi

  echo "Saving server URL: $final_url"
  echo "$final_url" > "$SERVER_URL_FILE"

  # Setup service
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
