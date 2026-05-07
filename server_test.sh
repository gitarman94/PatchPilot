#!/usr/bin/env bash
set -euo pipefail

echo "======================================"
echo "   CommandPilot Diagnostic Test      "
echo "======================================"

SERVICE_NAME="pilot-core.service"
APP_DIR="/opt/commandpilot/pilot-core"
DB_PATH="${APP_DIR}/commandpilot.db"
SERVER_URL="http://127.0.0.1:8080"
TMP_DIR="/tmp/commandpilot_test"
COOKIE_JAR="${TMP_DIR}/cookies.txt"

USERNAME="admin"
PASSWORD="admin"

mkdir -p "$TMP_DIR"

pass() {
    echo "[PASS] $1"
}

fail() {
    echo "[FAIL] $1"
}

echo "Checking systemd service..."
if systemctl is-active --quiet "${SERVICE_NAME}"; then
    pass "Service is active"
else
    fail "Service not active"
    journalctl -u "${SERVICE_NAME}" -n 50 --no-pager || true
fi

echo "Checking process..."
if ps -C pilot-core -o pid,%cpu,%mem,cmd >/dev/null 2>&1; then
    pass "Process running"
else
    fail "Process missing"
fi

echo "Checking port 8080..."
if ss -tulpn | grep -q ":8080"; then
    pass "Port 8080 listening"
else
    fail "Port 8080 not open"
fi

echo "Checking base HTTP..."
code=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/")

if [[ "$code" == "200" ]]; then
    pass "Login page reachable"
else
    fail "Base HTTP failed (${code})"
fi

echo "Checking auth redirect..."
code=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/dashboard")

if [[ "$code" == "302" ]]; then
    pass "Auth redirect working"
else
    fail "Unexpected dashboard response (${code})"
fi

echo "Testing login..."

curl -s -L \
    -c "$COOKIE_JAR" \
    -X POST "${SERVER_URL}/auth/login" \
    -d "username=${USERNAME}&password=${PASSWORD}" \
    -o /dev/null

if [[ ! -s "$COOKIE_JAR" ]]; then
    fail "Cookie jar empty"
else
    pass "Cookies received"
fi

code=$(curl -s \
    -b "$COOKIE_JAR" \
    -o /dev/null \
    -w "%{http_code}" \
    "${SERVER_URL}/dashboard")

if [[ "$code" == "200" ]]; then
    pass "Login works"
else
    fail "Login failed (${code})"
fi

echo "Testing authenticated pages..."

for ep in \
    /dashboard \
    /devices_page \
    /actions_page \
    /history_page \
    /users_groups_page \
    /roles_page \
    /settings_page
do
    code=$(curl -s \
        -b "$COOKIE_JAR" \
        -o /dev/null \
        -w "%{http_code}" \
        "${SERVER_URL}${ep}")

    if [[ "$code" == "200" ]]; then
        pass "${ep} OK"
    else
        fail "${ep} failed (${code})"
    fi
done

echo "Testing APIs..."

for ep in \
    /api/devices \
    /api/actions \
    /api/history
do
    code=$(curl -s \
        -b "$COOKIE_JAR" \
        -o /dev/null \
        -w "%{http_code}" \
        "${SERVER_URL}${ep}")

    if [[ "$code" == "200" ]]; then
        pass "${ep} OK"
    else
        fail "${ep} failed (${code})"
    fi
done

echo "Checking database..."

if [[ -f "$DB_PATH" ]]; then
    pass "Database exists"
else
    fail "Database missing"
fi

if sqlite3 "$DB_PATH" "SELECT 1;" >/dev/null 2>&1; then
    pass "SQLite operational"
else
    fail "SQLite failure"
fi

echo "Checking integrity..."

if sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q ok; then
    pass "Integrity OK"
else
    fail "Integrity failure"
fi

echo "Checking users..."

USER_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM users;" 2>/dev/null || echo 0)

if [[ "$USER_COUNT" -gt 0 ]]; then
    pass "Users present (${USER_COUNT})"
else
    fail "No users found"
fi

echo "Checking admin user..."

if sqlite3 "$DB_PATH" \
    "SELECT username FROM users WHERE username='admin';" \
    | grep -q admin; then
    pass "Admin user exists"
else
    fail "Admin user missing"
fi

rm -rf "$TMP_DIR"

echo "======================================"
echo "Diagnostics complete"
echo "======================================"