#!/usr/bin/env bash
set -euo pipefail

echo "======================================"
echo "   CommandPilot Diagnostic Test      "
echo "======================================"

SERVICE_NAME="commandpilot.service"
APP_DIR="/opt/commandpilot"
DB_PATH="${APP_DIR}/commandpilot.db"
SERVER_URL="http://127.0.0.1:8080"
TMP_DIR="/tmp/commandpilot_test"
COOKIE_JAR="${TMP_DIR}/cookies.txt"

mkdir -p "$TMP_DIR"

pass() { echo "[PASS] $1"; }
fail() { echo "[FAIL] $1"; }

echo "Checking systemd service..."
if systemctl is-active --quiet "${SERVICE_NAME}"; then
    pass "Service is active"
else
    fail "Service not active"
    journalctl -u "${SERVICE_NAME}" -n 30 --no-pager || true
fi

echo "Checking port 8080..."
if ss -tulpn | grep -q ":8080"; then
    pass "Port 8080 is listening"
else
    fail "Port 8080 is not open"
fi

echo "Checking base HTTP..."
code=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/")
[[ "$code" == "200" ]] && pass "Login page reachable" || fail "Login page failed ($code)"

echo "Checking auth redirect..."
code=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/dashboard")
[[ "$code" == "302" ]] && pass "Auth redirect OK" || fail "Auth redirect failed ($code)"

USERNAME="admin"
PASSWORD="admin"

echo "Testing login..."
curl -s -c "$COOKIE_JAR" -X POST "${SERVER_URL}/auth/login" -d "username=${USERNAME}&password=${PASSWORD}" -o /dev/null

code=$(curl -s -b "$COOKIE_JAR" -o /dev/null -w "%{http_code}" "${SERVER_URL}/dashboard")
[[ "$code" == "200" ]] && pass "Login works" || fail "Login failed ($code)"

echo "Testing pages..."
for ep in /devices_page /actions_page /history_page /users_groups_page /roles_page /settings_page; do
    code=$(curl -s -b "$COOKIE_JAR" -o /dev/null -w "%{http_code}" "${SERVER_URL}${ep}")
    [[ "$code" == "200" ]] && pass "$ep OK" || fail "$ep failed ($code)"
done

echo "Testing APIs..."
for ep in /api/devices /api/actions /api/history; do
    code=$(curl -s -b "$COOKIE_JAR" -o /dev/null -w "%{http_code}" "${SERVER_URL}${ep}")
    [[ "$code" == "200" ]] && pass "$ep OK" || fail "$ep failed ($code)"
done

echo "Checking DB..."
[[ -f "$DB_PATH" ]] && pass "DB exists" || fail "DB missing"

sqlite3 "$DB_PATH" "SELECT 1;" >/dev/null 2>&1 && pass "SQLite OK" || fail "SQLite failed"

echo "Checking integrity..."
sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q ok && pass "Integrity OK" || fail "Integrity failed"

echo "Checking process..."
ps -C commandpilot -o pid,%cpu,%mem,cmd >/dev/null 2>&1 && pass "Process running" || fail "Process missing"

rm -rf "$TMP_DIR"

echo "======================================"
echo "Diagnostics complete"
echo "======================================"