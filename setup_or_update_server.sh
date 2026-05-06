#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="pilot-core.service"
INSTALL_DIR="/opt/commandpilot"
CORE_DIR="${INSTALL_DIR}/pilot-core"
REPO_URL="https://github.com/gitarman94/CommandPilot.git"
SERVER_URL="http://127.0.0.1:8080"

VERBOSE=0
VERYVERBOSE=0

for arg in "$@"; do
    case "$arg" in
        --verbose|-v) VERBOSE=1 ;;
        --veryverbose|-vv) VERBOSE=1; VERYVERBOSE=1 ;;
    esac
done

[[ "$VERYVERBOSE" -eq 1 ]] && set -x

run() {
    if [[ "$VERBOSE" -eq 1 ]]; then
        echo "[RUN] $*"
        "$@"
    else
        "$@" >/dev/null 2>&1
    fi
}

fail() {
    echo "[FAIL] $1"

    if [[ "$VERYVERBOSE" -eq 1 ]]; then
        echo "===== DEBUG INFO ====="
        echo "--- pwd ---"
        pwd || true

        echo "--- install dir ---"
        ls -lah "$INSTALL_DIR" || true

        echo "--- core dir ---"
        ls -lah "$CORE_DIR" || true

        echo "--- templates ---"
        find "$INSTALL_DIR" -name "*.html" || true

        echo "--- systemd unit ---"
        cat "/etc/systemd/system/${SERVICE_NAME}" 2>/dev/null || true

        echo "--- systemctl status ---"
        systemctl status "${SERVICE_NAME}" --no-pager -l || true

        echo "--- journalctl ---"
        journalctl -u "${SERVICE_NAME}" -n 100 --no-pager || true

        echo "--- listening ports ---"
        ss -tulpn || true

        echo "--- processes ---"
        ps aux | grep pilot-core || true
    fi

    exit 1
}

pass() {
    echo "[PASS] $1"
}

stage() {
    echo
    echo "== $1 =="
}

stage "Dependencies"

run apt-get update -y || fail "apt update failed"

run apt-get install -y \
git \
curl \
wget \
sqlite3 \
build-essential \
|| fail "dependency install failed"

pass "Dependencies installed"

stage "Go"

if ! command -v go >/dev/null 2>&1; then
    ARCH=$(uname -m)

    case "$ARCH" in
        x86_64) GO_ARCH="amd64" ;;
        aarch64|arm64) GO_ARCH="arm64" ;;
        *) fail "unsupported architecture: $ARCH" ;;
    esac

    run wget "https://go.dev/dl/go1.25.0.linux-${GO_ARCH}.tar.gz" \
    -O /tmp/go.tar.gz \
    || fail "Go download failed"

    rm -rf /usr/local/go

    run tar -C /usr/local -xzf /tmp/go.tar.gz \
    || fail "Go extract failed"

    echo 'export PATH=$PATH:/usr/local/go/bin' >/etc/profile.d/golang.sh
fi

export PATH=$PATH:/usr/local/go/bin

go version >/dev/null 2>&1 || fail "Go install failed"

pass "Go ready"

stage "Source"

rm -rf "$INSTALL_DIR"

run git clone "$REPO_URL" "$INSTALL_DIR" \
|| fail "git clone failed"

[[ -d "$CORE_DIR" ]] || fail "pilot-core missing"

pass "Repository cloned"

stage "Build"

cd "$CORE_DIR"

run go mod tidy || fail "go mod tidy failed"

run go build -o pilot-core . \
|| fail "go build failed"

[[ -f "${CORE_DIR}/pilot-core" ]] \
|| fail "binary missing"

chmod +x "${CORE_DIR}/pilot-core"

pass "Build succeeded"

stage "Assets"

mkdir -p "${INSTALL_DIR}/templates"
mkdir -p "${INSTALL_DIR}/static"

cp -r "${INSTALL_DIR}/templates/"* "${INSTALL_DIR}/templates/" 2>/dev/null || true
cp -r "${INSTALL_DIR}/static/"* "${INSTALL_DIR}/static/" 2>/dev/null || true

[[ -f "${INSTALL_DIR}/templates/login.html" ]] \
|| fail "templates missing"

pass "Assets ready"

stage "Service"

cat >/etc/systemd/system/${SERVICE_NAME} <<EOF
[Unit]
Description=CommandPilot Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=${INSTALL_DIR}
ExecStart=${CORE_DIR}/pilot-core
Restart=always
RestartSec=3
Environment=PATH=/usr/local/go/bin:/usr/bin:/bin

[Install]
WantedBy=multi-user.target
EOF

run systemctl daemon-reload \
|| fail "daemon-reload failed"

run systemctl enable ${SERVICE_NAME} \
|| fail "enable failed"

run systemctl restart ${SERVICE_NAME} \
|| fail "service start failed"

sleep 5

pass "Service started"

stage "Validation"

systemctl is-active --quiet ${SERVICE_NAME} \
|| fail "service inactive"

pass "Service active"

ss -tulpn | grep -q ":8080" \
|| fail "port 8080 closed"

pass "Port listening"

HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/")

[[ "$HTTP_CODE" == "200" ]] \
|| fail "HTTP failed (${HTTP_CODE})"

pass "HTTP responding"

[[ -f "${INSTALL_DIR}/commandpilot.db" ]] \
|| fail "database missing"

pass "Database exists"

sqlite3 "${INSTALL_DIR}/commandpilot.db" "SELECT 1;" >/dev/null 2>&1 \
|| fail "sqlite failed"

pass "SQLite operational"

TABLES=$(sqlite3 "${INSTALL_DIR}/commandpilot.db" ".tables")

for table in devices actions history users roles settings; do
    echo "$TABLES" | grep -q "$table" \
    || fail "missing table: $table"
done

pass "Schema valid"

stage "Authenticated Validation"

ADMIN_EXISTS=$(sqlite3 "${INSTALL_DIR}/commandpilot.db" \
"SELECT COUNT(*) FROM users WHERE username='admin';")

if [[ "$ADMIN_EXISTS" == "0" ]]; then

HASH=$(cat <<'EOF' | go run /dev/stdin
package main

import (
    "fmt"
    "golang.org/x/crypto/bcrypt"
)

func main() {
    hash, err := bcrypt.GenerateFromPassword([]byte("admin"), bcrypt.DefaultCost)

    if err != nil {
        panic(err)
    }

    fmt.Print(string(hash))
}
EOF
)

sqlite3 "${INSTALL_DIR}/commandpilot.db" \
"INSERT INTO users (username,password_hash) VALUES ('admin','$HASH');" \
|| fail "failed creating admin user"

pass "Admin user created"

fi

COOKIE_JAR="/tmp/commandpilot.cookies"

LOGIN_CODE=$(curl -s \
-c "$COOKIE_JAR" \
-o /dev/null \
-w "%{http_code}" \
-X POST "${SERVER_URL}/auth/login" \
-d "username=admin&password=admin")

[[ "$LOGIN_CODE" == "302" || "$LOGIN_CODE" == "200" ]] \
|| fail "login failed"

pass "Login successful"

for endpoint in \
"/dashboard" \
"/devices_page" \
"/actions_page" \
"/history_page" \
"/users_groups_page" \
"/roles_page" \
"/settings_page" \
"/audit_page" \
"/api/devices" \
"/api/actions" \
"/api/history"
do
    CODE=$(curl -s \
    -b "$COOKIE_JAR" \
    -o /dev/null \
    -w "%{http_code}" \
    "${SERVER_URL}${endpoint}")

    [[ "$CODE" == "200" ]] \
    || fail "${endpoint} failed (${CODE})"

    pass "${endpoint} OK"
done

rm -f "$COOKIE_JAR"

IP_ADDR=$(hostname -I | awk '{print $1}')

echo
echo "CommandPilot deployed successfully"
echo "URL: http://${IP_ADDR}:8080"
echo "Service: ${SERVICE_NAME}"
echo
echo "Verbose:"
echo "bash setup_or_update_server.sh --verbose"
echo
echo "Very Verbose:"
echo "bash setup_or_update_server.sh --veryverbose"