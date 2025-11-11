#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="patchpilot_client.service"
APP_DIR="/opt/patchpilot_client"
BINARY="${APP_DIR}/patchpilot_client"
CONFIG_FILE="${APP_DIR}/server_url.txt"
TMP_DIR="/tmp/patchpilot_client_diagnostics"
LOG_FILE="${TMP_DIR}/client_debug.log"

mkdir -p "$TMP_DIR"

echo "======================================"
echo "   PatchPilot Client Diagnostic Test  "
echo "======================================"

# Verify binary & config existence
echo "🔍 Checking binary and configuration files..."
if [[ -x "$BINARY" ]]; then
    echo "✔️  Binary found at $BINARY"
else
    echo "❌  Binary missing or not executable: $BINARY"
    exit 1
fi

if [[ -f "$CONFIG_FILE" ]]; then
    echo "✔️  Found server URL file: $CONFIG_FILE"
else
    echo "❌  Missing configuration file: $CONFIG_FILE"
    exit 1
fi

SERVER_URL=$(<"$CONFIG_FILE")
echo "ℹ️  Server URL: $SERVER_URL"

# Check systemd service presence
echo "🔍 Checking systemd service: ${SERVICE_NAME}"
if systemctl list-units --all --full | grep -q "$SERVICE_NAME"; then
    echo "✔️  Service is installed and recognized by systemd."
elif [[ -f "/etc/systemd/system/${SERVICE_NAME}" ]]; then
    echo "✔️  Service file exists on disk but not loaded. Reloading systemd..."
    systemctl daemon-reload
else
    echo "❌  Service not found at /etc/systemd/system/${SERVICE_NAME}"
    exit 1
fi

# Check service status & restart limit
echo "🔍 Checking current service status..."
if systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "✔️  Service is active and running."
else
    echo "⚠️  Service not running. Gathering failure info..."
    systemctl status "$SERVICE_NAME" --no-pager || true
    echo "🪵 Last 30 log entries:"
    journalctl -u "$SERVICE_NAME" -n 30 --no-pager || true

    echo "🔍 Checking if systemd hit a restart limit..."
    if journalctl -u "$SERVICE_NAME" | grep -q "start-limit-hit"; then
        echo "❌  Restart limit reached. The service is crashing or exiting too quickly."
    fi

    echo "🔍 Checking service exit codes:"
    journalctl -u "$SERVICE_NAME" | grep -E "code=" | tail -n 10 || true
fi

# Check permissions & ownership
echo "🔍 Checking permissions for app directory..."
ls -ld "$APP_DIR"
ls -l "$BINARY" "$CONFIG_FILE" || true

OWNER=$(stat -c "%U" "$BINARY")
if [[ "$OWNER" != "patchpilot" ]]; then
    echo "⚠️  Binary owned by $OWNER — expected 'patchpilot'"
fi

# Test direct execution manually (without sudo)
echo "🔍 Testing binary execution manually..."
set +e
su -s /bin/bash patchpilot -c "$BINARY >${TMP_DIR}/manual_out.txt 2>${TMP_DIR}/manual_err.txt"
status=$?
set -e

if [[ $status -eq 0 ]]; then
    echo "✔️  Binary executed successfully (exit code 0)."
else
    echo "❌  Binary exited with non-zero status: $status"
fi

if [[ -s "${TMP_DIR}/manual_err.txt" ]]; then
    echo "⚠️  STDERR output detected:"
    cat "${TMP_DIR}/manual_err.txt"
else
    echo "✔️  No STDERR output from binary."
fi

# Validate JSON output
if [[ -s "${TMP_DIR}/manual_out.txt" ]]; then
    echo "🔍 Checking JSON output structure..."
    head -n 5 "${TMP_DIR}/manual_out.txt"
    if grep -q '{' "${TMP_DIR}/manual_out.txt"; then
        echo "✔️  Output appears to be valid JSON."
    else
        echo "❌  Output missing JSON structure."
    fi
else
    echo "❌  No output produced from binary."
fi

# Connectivity check to server
echo "🔍 Testing basic connectivity to $SERVER_URL..."
HOST=$(echo "$SERVER_URL" | sed -E 's|https?://||' | cut -d/ -f1)
if ping -c 1 -W 2 "$HOST" &>/dev/null; then
    echo "✔️  Host reachable via ping."
else
    echo "❌  Cannot reach host $HOST"
fi

# Capture running process info (if any)
echo "🔍 Capturing patchpilot_client process info..."
ps -eo pid,user,%cpu,%mem,cmd | grep "[p]atchpilot_client" || echo "⚠️  Process not found."

# Detect crash loops or segfaults
echo "🔍 Scanning logs for segfaults or panics..."
journalctl -u "$SERVICE_NAME" | grep -Ei "panic|segfault|abort|core" | tail -n 5 || echo "✔️  No crashes detected."

# Summary
echo ""
echo "======================================"
echo "✅ Diagnostics complete."
echo "Logs & output in: ${TMP_DIR}"
echo "======================================"
