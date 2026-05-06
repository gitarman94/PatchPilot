#!/usr/bin/env bash
set -euo pipefail
SERVICE_NAME="pilot-core.service"
RUNTIME_DIR="/opt/commandpilot"
SOURCE_DIR="/usr/local/src/CommandPilot"
BUILD_DIR="${SOURCE_DIR}/pilot-core"
REPO_URL="https://github.com/gitarman94/CommandPilot.git"
SERVER_URL="http://127.0.0.1:8080"
VERBOSE=0
VERYVERBOSE=0

for arg in "$@"; do
    case "$arg" in
        --verbose|-v) VERBOSE=1 ;;
        --veryverbose|-vv|--very-verbose) VERBOSE=1; VERYVERBOSE=1 ;;
    esac
done

[[ "$VERYVERBOSE" -eq 1 ]] && set -x

run() {
    if [[ "$VERBOSE" -eq 1 || "$VERYVERBOSE" -eq 1 ]]; then
        echo "[RUN] $*"
        "$@"
    else
        "$@" >/dev/null 2>&1
    fi
}

fail() {
    echo "[FAIL] $1"
    exit 1
}

pass() {
    echo "[PASS] $1"
}

stage() {
    echo
    echo "== $1 =="
}

cleanup() {
    rm -f /tmp/go.tar.gz
}

trap cleanup EXIT

stage "Dependencies"
run apt-get update -y || fail "apt update failed"
run apt-get install -y git curl wget sqlite3 build-essential ca-certificates || fail "dependency install failed"
pass "Dependencies installed"

stage "Go"

if ! command -v go >/dev/null 2>&1; then
    ARCH=$(uname -m)

    case "$ARCH" in
        x86_64) GO_ARCH="amd64" ;;
        aarch64|arm64) GO_ARCH="arm64" ;;
        *) fail "unsupported architecture: $ARCH" ;;
    esac

    run wget "https://go.dev/dl/go1.25.0.linux-${GO_ARCH}.tar.gz" -O /tmp/go.tar.gz || fail "Go download failed"
    rm -rf /usr/local/go
    run tar -C /usr/local -xzf /tmp/go.tar.gz || fail "Go extract failed"

    echo 'export PATH=$PATH:/usr/local/go/bin' >/etc/profile.d/golang.sh
fi

export PATH=$PATH:/usr/local/go/bin
go version >/dev/null 2>&1 || fail "Go install failed"
pass "Go ready"

stage "User"

if ! id -u commandpilot >/dev/null 2>&1; then
    useradd --system --no-create-home --shell /usr/sbin/nologin commandpilot || fail "failed creating commandpilot user"
fi

pass "User ready"

stage "Source"

mkdir -p "$(dirname "$SOURCE_DIR")"

if [[ -d "${SOURCE_DIR}/.git" ]]; then
    run git -C "$SOURCE_DIR" fetch --all --prune || fail "git fetch failed"
    run git -C "$SOURCE_DIR" reset --hard origin/main || fail "git reset failed"
else
    run git clone "$REPO_URL" "$SOURCE_DIR" || fail "git clone failed"
fi

[[ -d "$BUILD_DIR" ]] || fail "pilot-core source missing"
pass "Repository synced"

stage "Build"

cd "$BUILD_DIR"
run go mod tidy || fail "go mod tidy failed"
run go build -o pilot-core . || fail "go build failed"

[[ -f "${BUILD_DIR}/pilot-core" ]] || fail "binary missing"

chmod +x "${BUILD_DIR}/pilot-core"
pass "Build succeeded"

stage "Runtime"

mkdir -p "$RUNTIME_DIR"

run systemctl stop "${SERVICE_NAME}" || true

cp "${BUILD_DIR}/pilot-core" "${RUNTIME_DIR}/pilot-core"

rm -rf "${RUNTIME_DIR}/templates"
rm -rf "${RUNTIME_DIR}/static"

cp -r "${SOURCE_DIR}/templates" "${RUNTIME_DIR}/templates"
cp -r "${SOURCE_DIR}/static" "${RUNTIME_DIR}/static"

mkdir -p "${RUNTIME_DIR}/backups"

if [[ ! -f "${RUNTIME_DIR}/commandpilot.db" ]]; then
    touch "${RUNTIME_DIR}/commandpilot.db"
fi

chown -R commandpilot:commandpilot "$RUNTIME_DIR"
pass "Runtime deployed"

stage "Assets"

for f in login.html navbar.html dashboard.html devices.html device_detail.html actions.html history.html settings.html users_groups.html roles.html audit.html; do
    [[ -f "${RUNTIME_DIR}/templates/${f}" ]] || fail "missing template: ${f}"
done

[[ -f "${RUNTIME_DIR}/static/styles.css" ]] || fail "styles.css missing"
pass "Assets ready"

stage "Service"

cat >/etc/systemd/system/${SERVICE_NAME} <<EOF
[Unit]
Description=CommandPilot Server
After=network.target

[Service]
Type=simple
User=commandpilot
Group=commandpilot
WorkingDirectory=${RUNTIME_DIR}
ExecStart=${RUNTIME_DIR}/pilot-core
Restart=always
RestartSec=3
Environment=PATH=/usr/local/go/bin:/usr/bin:/bin

[Install]
WantedBy=multi-user.target
EOF

run systemctl daemon-reload || fail "daemon-reload failed"
run systemctl enable ${SERVICE_NAME} || fail "enable failed"
run systemctl restart ${SERVICE_NAME} || fail "service start failed"

sleep 5
pass "Service started"

stage "Validation"

systemctl is-active --quiet ${SERVICE_NAME} || fail "service inactive"
pass "Service active"

ss -tulpn | grep -q ":8080" || fail "port 8080 closed"
pass "Port listening"

HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVER_URL}/")

[[ "$HTTP_CODE" == "200" || "$HTTP_CODE" == "401" ]] || fail "HTTP failed (${HTTP_CODE})"

pass "HTTP responding"

[[ -f "${RUNTIME_DIR}/commandpilot.db" ]] || fail "database missing"
pass "Database exists"

sqlite3 "${RUNTIME_DIR}/commandpilot.db" "SELECT 1;" >/dev/null 2>&1 || fail "sqlite failed"
pass "SQLite operational"

IP_ADDR=$(hostname -I | awk '{print $1}')

echo
echo "CommandPilot deployed successfully"
echo "URL: http://${IP_ADDR}:8080"
echo "Service: ${SERVICE_NAME}"
