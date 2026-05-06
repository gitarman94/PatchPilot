#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="pilot-core.service"
INSTALL_DIR="/opt/commandpilot"
CORE_DIR="${INSTALL_DIR}/pilot-core"
REPO_URL="https://github.com/gitarman94/CommandPilot.git"
SERVER_URL="http://127.0.0.1:8080"

fail(){ echo "[FAIL] $1"; exit 1; }
pass(){ echo "[PASS] $1"; }
stage(){ echo; echo "== $1 =="; }

stage "Dependencies"
apt-get update -y >/dev/null 2>&1 || fail "apt update failed"
apt-get install -y git curl wget sqlite3 build-essential >/dev/null 2>&1 || fail "dependency install failed"
pass "Dependencies installed"

stage "Go"
if ! command -v go >/dev/null 2>&1; then
    ARCH=$(uname -m)
    [[ "$ARCH" == "x86_64" ]] && GO_ARCH="amd64" || GO_ARCH="arm64"
    wget -q https://go.dev/dl/go1.25.0.linux-${GO_ARCH}.tar.gz -O /tmp/go.tar.gz || fail "Go download failed"
    rm -rf /usr/local/go
    tar -C /usr/local -xzf /tmp/go.tar.gz || fail "Go extract failed"
    echo 'export PATH=$PATH:/usr/local/go/bin' >/etc/profile.d/golang.sh
fi
export PATH=$PATH:/usr/local/go/bin
go version >/dev/null 2>&1 || fail "Go install failed"
pass "Go ready"

stage "Source"
rm -rf "$INSTALL_DIR"
git clone "$REPO_URL" "$INSTALL_DIR" >/dev/null 2>&1 || fail "git clone failed"
[[ -d "$CORE_DIR" ]] || fail "pilot-core missing"
pass "Repository cloned"

stage "Build"
cd "$CORE_DIR"
go mod tidy >/dev/null 2>&1 || fail "go mod tidy failed"
go build -o pilot-core . || fail "go build failed"
[[ -f "${CORE_DIR}/pilot-core" ]] || fail "binary missing"
chmod +x "${CORE_DIR}/pilot-core"
pass "Build succeeded"

stage "Assets"
mkdir -p "${INSTALL_DIR}/templates" "${INSTALL_DIR}/static"
cp -r "${CORE_DIR}/templates/"* "${INSTALL_DIR}/templates/" 2>/dev/null || true
cp -r "${CORE_DIR}/static/"* "${INSTALL_DIR}/static/" 2>/dev/null || true
[[ -f "${INSTALL_DIR}/templates/login.html" ]] || fail "templates missing"
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

systemctl daemon-reload || fail "daemon-reload failed"
systemctl enable ${SERVICE_NAME} >/dev/null 2>&1 || fail "enable failed"
systemctl restart ${SERVICE_NAME} || fail "service start failed"
sleep 5
pass "Service started"

stage "Validation"
systemctl is-active --quiet ${SERVICE_NAME} || { journalctl -u ${SERVICE_NAME} -n 50 --no-pager; fail "service inactive"; }
pass "Service active"

ss -tulpn | grep -q ":8080" || { journalctl -u ${SERVICE_NAME} -n 50 --no-pager; fail "port 8080 closed"; }
pass "Port listening"

HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/")
[[ "$HTTP_CODE" == "200" ]] || fail "HTTP failed (${HTTP_CODE})"
pass "HTTP responding"

[[ -f "${INSTALL_DIR}/commandpilot.db" ]] || fail "database missing"
pass "Database exists"

sqlite3 "${INSTALL_DIR}/commandpilot.db" "SELECT 1;" >/dev/null 2>&1 || fail "sqlite failed"
pass "SQLite operational"

TABLES=$(sqlite3 "${INSTALL_DIR}/commandpilot.db" ".tables")
for table in devices actions history users roles settings; do
    echo "$TABLES" | grep -q "$table" || fail "missing table: $table"
done
pass "Schema valid"

IP_ADDR=$(hostname -I | awk '{print $1}')
echo
echo "CommandPilot deployed"
echo "URL: http://${IP_ADDR}:8080"
echo "Service: ${SERVICE_NAME}"