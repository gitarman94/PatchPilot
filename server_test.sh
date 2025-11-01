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
mkdir -p "$TMP_DIR"

###############################################
# 1. Check systemd service
###############################################
echo "🔍 Checking systemd service: ${SERVICE_NAME}"
if systemctl list-units --full -all | grep -q "^${SERVICE_NAME}"; then
    status=$(systemctl is-active "${SERVICE_NAME}")
    if [[ "$status" == "active" ]]; then
        echo "✔️  Service is active."
    else
        echo "❌  Service exists but is not active: $status"
        journalctl -u "${SERVICE_NAME}" -n 30 --no-pager
        exit 1
    fi
else
    echo "❌  Service not found: ${SERVICE_NAME}"
    exit 1
fi

###############################################
# 2. Check open port
###############################################
echo "🔍 Checking if port 8080 is listening..."
if ss -tulpn | grep -q ":8080"; then
    echo "✔️  Port 8080 is open."
else
    echo "❌  Port 8080 is not open — server may have failed to bind."
    exit 1
fi

###############################################
# 3. Verify API routes
###############################################
echo "🔍 Checking /api/devices endpoint..."
resp_code=$(curl -s -o "${TMP_DIR}/devices.json" -w "%{http_code}" "${SERVER_URL}/api/devices")
if [[ "$resp_code" == "200" ]]; then
    echo "✔️  /api/devices endpoint reachable."
else
    echo "❌  Failed to reach /api/devices — HTTP $resp_code"
    cat "${TMP_DIR}/devices.json" || true
    exit 1
fi

###############################################
# 4. Register or update a test device
###############################################
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
    "network_throughput": 1000,
    "ping_latency": 10.5
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
    exit 1
fi

###############################################
# 5. Confirm device appears in GET /api/devices
###############################################
if jq -e ".[] | select(.device_name == \"${TEST_DEVICE_ID}\")" "${TMP_DIR}/devices.json" >/dev/null; then
    echo "✔️  Device ${TEST_DEVICE_ID} appears in device list."
else
    echo "❌  Device ${TEST_DEVICE_ID} not found in device list."
    echo "Response:"
    cat "${TMP_DIR}/devices.json"
    exit 1
fi

###############################################
# 6. Database validation
###############################################
echo "🔍 Checking SQLite database integrity..."
if [[ -f "$DB_PATH" ]]; then
    echo "✔️  Database file exists."
else
    echo "❌  Database file missing: $DB_PATH"
    exit 1
fi

echo "🔍 Validating database schema..."
tables=$(sqlite3 "$DB_PATH" ".tables")
if echo "$tables" | grep -q "devices"; then
    echo "✔️  'devices' table exists."
else
    echo "❌  'devices' table missing!"
    echo "$tables"
    exit 1
fi

echo "🔍 Checking that ${TEST_DEVICE_ID} exists in DB..."
db_entry=$(sqlite3 "$DB_PATH" "SELECT device_name, os_name, cpu FROM devices WHERE device_name='${TEST_DEVICE_ID}';")
if [[ -n "$db_entry" ]]; then
    echo "✔️  Found device in DB: $db_entry"
else
    echo "❌  Device not found in DB."
    exit 1
fi

###############################################
# 7. Log inspection
###############################################
echo "🔍 Checking recent server logs for warnings or errors..."
recent_logs=$(journalctl -u "${SERVICE_NAME}" -n 50 --no-pager)
if echo "$recent_logs" | grep -Eiq "error|panic|failed"; then
    echo "❌  Errors found in recent logs:"
    echo "$recent_logs" | grep -Ei "error|panic|failed"
    exit 1
else
    echo "✔️  No critical errors in recent logs."
fi

###############################################
# 8. Resource usage snapshot
###############################################
echo "🔍 Capturing CPU and memory usage for patchpilot_server..."
ps -C patchpilot_server -o pid,%cpu,%mem,cmd || echo "⚠️  Process not found (may be fine if using Rocket as PID 1)."

###############################################
# 9. Database integrity check (deep)
###############################################
echo "🔍 Running SQLite integrity check..."
if sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q "ok"; then
    echo "✔️  SQLite integrity check passed."
else
    echo "❌  SQLite integrity check failed!"
    sqlite3 "$DB_PATH" "PRAGMA integrity_check;"
    exit 1
fi

###############################################
# 10. Cleanup temporary test files
###############################################
rm -rf "$TMP_DIR"

echo "======================================"
echo "✅ All diagnostics completed successfully."
echo "======================================"
