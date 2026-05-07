#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="pilot-core.service"
BASE_DIR="/opt/commandpilot"
SOURCE_DIR="${BASE_DIR}/src/CommandPilot"
BUILD_DIR="${SOURCE_DIR}/pilot-core"
RUNTIME_DIR="${BASE_DIR}/pilot-core"
REPO_URL="https://github.com/gitarman94/CommandPilot.git"
SERVER_URL="http://127.0.0.1:8080"

DEFAULT_ADMIN_USER="admin"
DEFAULT_ADMIN_PASS="admin"

VERBOSE=0
VERYVERBOSE=0

for arg in "$@"; do
    case "$arg" in
        --verbose|-v)
            VERBOSE=1
            ;;
        --veryverbose|-vv|--very-verbose)
            VERBOSE=1
            VERYVERBOSE=1
            ;;
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

pass() {
    echo "[PASS] $1"
}

fail() {
    echo "[FAIL] $1"
    exit 1
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

run apt-get install -y \
    git \
    curl \
    wget \
    sqlite3 \
    build-essential \
    ca-certificates || fail "dependency install failed"

pass "Dependencies installed"

stage "Go"

if ! command -v go >/dev/null 2>&1; then
    ARCH=$(uname -m)

    case "$ARCH" in
        x86_64)
            GO_ARCH="amd64"
            ;;
        aarch64|arm64)
            GO_ARCH="arm64"
            ;;
        *)
            fail "unsupported architecture: ${ARCH}"
            ;;
    esac

    run wget \
        "https://go.dev/dl/go1.25.0.linux-${GO_ARCH}.tar.gz" \
        -O /tmp/go.tar.gz || fail "Go download failed"

    rm -rf /usr/local/go

    run tar -C /usr/local -xzf /tmp/go.tar.gz || fail "Go extract failed"

    echo 'export PATH=$PATH:/usr/local/go/bin' >/etc/profile.d/golang.sh
fi

export PATH=$PATH:/usr/local/go/bin

go version >/dev/null 2>&1 || fail "Go install failed"

pass "Go ready"

stage "User"

if ! id -u commandpilot >/dev/null 2>&1; then
    useradd \
        --system \
        --no-create-home \
        --shell /usr/sbin/nologin \
        commandpilot || fail "failed creating commandpilot user"
fi

pass "User ready"

stage "Directories"

mkdir -p "$BASE_DIR"
mkdir -p "$(dirname "$SOURCE_DIR")"
mkdir -p "$RUNTIME_DIR"

pass "Directories ready"

stage "Source"

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

rm -f "${BUILD_DIR}/pilot-core"

run go build -o pilot-core . || fail "go build failed"

[[ -f "${BUILD_DIR}/pilot-core" ]] || fail "binary missing"

chmod +x "${BUILD_DIR}/pilot-core"

pass "Build succeeded"

stage "Runtime"

run systemctl stop "${SERVICE_NAME}" || true

rm -f "${RUNTIME_DIR}/pilot-core"

install -m 755 \
    "${BUILD_DIR}/pilot-core" \
    "${RUNTIME_DIR}/pilot-core" || fail "runtime binary install failed"

rm -rf "${RUNTIME_DIR}/templates"
rm -rf "${RUNTIME_DIR}/static"

cp -r "${SOURCE_DIR}/templates" "${RUNTIME_DIR}/templates" || fail "templates copy failed"

cp -r "${SOURCE_DIR}/static" "${RUNTIME_DIR}/static" || fail "static copy failed"

mkdir -p "${RUNTIME_DIR}/backups"

if [[ ! -f "${RUNTIME_DIR}/commandpilot.db" ]]; then
    touch "${RUNTIME_DIR}/commandpilot.db"
fi

chown -R commandpilot:commandpilot "$RUNTIME_DIR"

[[ -f "${RUNTIME_DIR}/pilot-core" ]] || fail "runtime binary missing"
[[ -x "${RUNTIME_DIR}/pilot-core" ]] || fail "runtime binary not executable"

pass "Runtime deployed"

stage "Database"

sqlite3 "${RUNTIME_DIR}/commandpilot.db" <<EOF
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'admin',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
EOF

ADMIN_EXISTS=$(sqlite3 "${RUNTIME_DIR}/commandpilot.db" \
    "SELECT COUNT(*) FROM users WHERE username='${DEFAULT_ADMIN_USER}';")

if [[ "$ADMIN_EXISTS" == "0" ]]; then
    sqlite3 "${RUNTIME_DIR}/commandpilot.db" <<EOF
INSERT INTO users (
    username,
    password,
    role
) VALUES (
    '${DEFAULT_ADMIN_USER}',
    '${DEFAULT_ADMIN_PASS}',
    'admin'
);
EOF

    pass "Default admin user created"
else
    pass "Admin user already exists"
fi

sqlite3 "${RUNTIME_DIR}/commandpilot.db" "SELECT 1;" >/dev/null 2>&1 \
    || fail "database validation failed"

chown commandpilot:commandpilot "${RUNTIME_DIR}/commandpilot.db"

pass "Database ready"

stage "Assets"

for f in \
    login.html \
    navbar.html \
    dashboard.html \
    devices.html \
    device_detail.html \
    actions.html \
    history.html \
    settings.html \
    users_groups.html \
    roles.html \
    audit.html
do
    [[ -f "${RUNTIME_DIR}/templates/${f}" ]] \
        || fail "missing template: ${f}"
done

[[ -f "${RUNTIME_DIR}/static/styles.css" ]] \
    || fail "styles.css missing"

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

run systemctl enable "${SERVICE_NAME}" || fail "enable failed"

run systemctl restart "${SERVICE_NAME}" || fail "service start failed"

sleep 5

if ! systemctl is-active --quiet "${SERVICE_NAME}"; then
    journalctl -u "${SERVICE_NAME}" -n 100 --no-pager
    fail "service inactive"
fi

pass "Service started"

stage "Validation"

ss -tulpn | grep -q ":8080" || fail "port 8080 closed"

pass "Port listening"

HTTP_CODE=$(curl -s \
    -o /dev/null \
    -w "%{http_code}" \
    "${SERVER_URL}/")

[[ "$HTTP_CODE" == "200" || "$HTTP_CODE" == "401" ]] \
    || fail "HTTP failed (${HTTP_CODE})"

pass "HTTP responding"

[[ -f "${RUNTIME_DIR}/commandpilot.db" ]] \
    || fail "database missing"

pass "Database exists"

sqlite3 "${RUNTIME_DIR}/commandpilot.db" "SELECT 1;" >/dev/null 2>&1 \
    || fail "sqlite failed"

pass "SQLite operational"

USER_COUNT=$(sqlite3 "${RUNTIME_DIR}/commandpilot.db" \
    "SELECT COUNT(*) FROM users;" 2>/dev/null || echo 0)

[[ "$USER_COUNT" -gt 0 ]] \
    || fail "no users present"

pass "Users present (${USER_COUNT})"

TMP_DIR="/tmp/commandpilot_validation"
COOKIE_JAR="${TMP_DIR}/cookies.txt"

mkdir -p "$TMP_DIR"

curl -s -L \
    -c "$COOKIE_JAR" \
    -X POST "${SERVER_URL}/auth/login" \
    -d "username=${DEFAULT_ADMIN_USER}&password=${DEFAULT_ADMIN_PASS}" \
    -o /dev/null

LOGIN_CODE=$(curl -s \
    -b "$COOKIE_JAR" \
    -o /dev/null \
    -w "%{http_code}" \
    "${SERVER_URL}/dashboard")

rm -rf "$TMP_DIR"

if [[ "$LOGIN_CODE" == "200" ]]; then
    pass "Admin login verified"
else
    fail "admin login failed (${LOGIN_CODE})"
fi

IP_ADDR=$(hostname -I | awk '{print $1}')

echo
echo "======================================"
echo " CommandPilot deployed successfully"
echo "======================================"
echo "URL: http://${IP_ADDR}:8080"
echo "Service: ${SERVICE_NAME}"
echo "Runtime: ${RUNTIME_DIR}"
echo
echo "Default credentials:"
echo "Username: ${DEFAULT_ADMIN_USER}"
echo "Password: ${DEFAULT_ADMIN_PASS}"