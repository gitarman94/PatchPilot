#!/usr/bin/env bash
set -euo pipefail

echo "======================================"
echo "   PatchPilot Server Diagnostic Test  "
echo "======================================"

SERVICE_NAME="patchpilot_server.service"
APP_DIR="/opt/patchpilot_server"
DB_PATH="${APP_DIR}/patchpilot.db"
SERVER_URL="http://localhost:8080"
TEST_DEVICE_ID="diagnostic-test-device-001"
TMP_DIR="/tmp/patchpilot_test"
PATCHPILOT_HOME="/home/patchpilot"
mkdir -p "$TMP_DIR"

echo "🔍 Validating patchpilot environment..."

if id -u patchpilot >/dev/null 2>&1; then
    echo "✅ User 'patchpilot' exists"
else
    echo "❌ User 'patchpilot' does not exist"
fi

if [[ -d "$PATCHPILOT_HOME" ]]; then
    echo "✅ Home directory exists: $PATCHPILOT_HOME"
else
    echo "❌ Home directory missing: $PATCHPILOT_HOME"
fi

su -s /bin/bash patchpilot -c "
if [ -w \"$PATCHPILOT_HOME\" ]; then
    echo '✅ Home directory is writable'
else
    echo '❌ Home directory is not writable'
fi
"

for dir in ".cargo" ".rustup"; do
    su -s /bin/bash patchpilot -c "
    if [ -d \"$PATCHPILOT_HOME/$dir\" ]; then
        echo \"✅ $dir exists in home\"
    else
        echo \"⚠️ $dir missing in home\"
    fi
    "
done

su -s /bin/bash patchpilot -c "
if cd \"$APP_DIR\" 2>/dev/null; then
    echo '✅ App directory accessible'
else
    echo '❌ Cannot access app directory'
fi
"

if [[ -f "$DB_PATH" ]]; then
    echo "✅ DB file exists: $DB_PATH"
else
    echo "❌ DB file missing: $DB_PATH"
fi

su -s /bin/bash patchpilot -c "
if [ -w \"$DB_PATH\" ]; then
    echo '✅ DB file writable'
else
    echo '❌ DB file not writable'
fi
"

su -s /bin/bash patchpilot -c "
if sqlite3 \"$DB_PATH\" 'SELECT 1;' >/dev/null 2>&1; then
    echo '✅ SQLite test query successful'
else
    echo '❌ SQLite test query failed'
fi
"

echo "🔍 Checking systemd service: ${SERVICE_NAME}"
if systemctl is-active --quiet "${SERVICE_NAME}"; then
    echo "✔️  Service is active."
else
    if systemctl list-units --full -all | grep -q "${SERVICE_NAME}"; then
        status=$(systemctl is-active "${SERVICE_NAME}")
        echo "❌  Service exists but is not active: $status"
    else
        echo "❌  Service not found: ${SERVICE_NAME}"
    fi
    journalctl -u "${SERVICE_NAME}" -n 30 --no-pager || true
fi

echo "🔍 Checking if port 8080 is listening..."
if ss -tulpn | grep -q ":8080"; then
    echo "✔️  Port 8080 is open."
else
    echo "❌  Port 8080 is not open."
fi

echo "🔍 Checking /api/devices endpoint..."
resp_code=$(curl -s -o "${TMP_DIR}/devices.json" -w "%{http_code}" "${SERVER_URL}/api/devices")
if [[ "$resp_code" == "200" ]]; then
    echo "✔️  /api/devices endpoint reachable."
else
    echo "❌  Failed to reach /api/devices — HTTP $resp_code"
    cat "${TMP_DIR}/devices.json" || true
fi

echo "🔍 Testing device registration endpoint..."
read -r -d '' DEVICE_PAYLOAD <<EOF
{
  "device_type": "server_test",
  "device_model": "RustCheck",
  "system_info": {
    "os_name": "Linux",
    "architecture": "x86_64",
    "cpu": 2.4,
    "ram_total": 8192,
    "ram_used": 4096,
    "ram_free": 4096,
    "disk_total": 512000,
    "disk_free": 256000,
    "disk_health": "good",
    "network_throughput": 1000
    }
}
EOF

post_code=$(curl -s -o "${TMP_DIR}/register.json" -w "%{http_code}" \
    -X POST "${SERVER_URL}/api/device/${TEST_DEVICE_ID}" \
    -H "Content-Type: application/json" \
    -d "$DEVICE_PAYLOAD")

if [[ "$post_code" == "200" ]]; then
    echo "✔️  Device registration succeeded."
else
    echo "❌  Device registration failed with HTTP code $post_code"
    cat "${TMP_DIR}/register.json" || true
fi

if jq -e ".[] | select(.device_name == \"${TEST_DEVICE_ID}\")" "${TMP_DIR}/devices.json" >/dev/null; then
    echo "✔️  Device ${TEST_DEVICE_ID} appears in device list."
else
    echo "❌  Device ${TEST_DEVICE_ID} not found in device list."
    cat "${TMP_DIR}/devices.json"
fi

echo "🔍 Checking SQLite database integrity..."
if [[ -f "$DB_PATH" ]]; then
    echo "✔️  Database file exists."
else
    echo "❌  Database file missing."
fi

tables=$(sqlite3 "$DB_PATH" ".tables")
if echo "$tables" | grep -q "devices"; then
    echo "✔️  'devices' table exists."
else
    echo "❌  'devices' table missing!"
    echo "$tables"
fi

db_entry=$(sqlite3 "$DB_PATH" "SELECT device_name, os_name, cpu FROM devices WHERE device_name='${TEST_DEVICE_ID}';")
if [[ -n "$db_entry" ]]; then
    echo "✔️  Found device in DB: $db_entry"
else
    echo "❌  Device not found in DB."
fi

echo "🔍 Checking recent server logs for warnings or errors..."
recent_logs=$(journalctl -u "${SERVICE_NAME}" -n 50 --no-pager)
if echo "$recent_logs" | grep -Eiq "error|panic|failed"; then
    echo "❌  Errors found in recent logs:"
    echo "$recent_logs" | grep -Ei "error|panic|failed"
else
    echo "✔️  No critical errors in recent logs."
fi

echo "🔍 Capturing CPU and memory usage for patchpilot_server..."
ps -C patchpilot_server -o pid,%cpu,%mem,cmd || echo "⚠️  Process not found."

echo "🔍 Running SQLite integrity check..."
if sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q "ok"; then
    echo "✔️  SQLite integrity check passed."
else
    echo "❌  SQLite integrity check failed!"
    sqlite3 "$DB_PATH" "PRAGMA integrity_check;"
fi

rm -rf "$TMP_DIR"

echo "======================================"
echo "✅ All diagnostics completed."
echo "======================================"
