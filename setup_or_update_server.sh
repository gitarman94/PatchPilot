#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="pilot-core.service"

BASE_DIR="/opt/commandpilot"
SOURCE_DIR="${BASE_DIR}/src/CommandPilot"
BUILD_DIR="${SOURCE_DIR}/pilot-core"
RUNTIME_DIR="${BASE_DIR}/pilot-core"

DB_PATH="${RUNTIME_DIR}/commandpilot.db"

REPO_URL="https://github.com/gitarman94/CommandPilot.git"
SERVER_URL="http://127.0.0.1:8080"

DEFAULT_ADMIN_USER="admin"
DEFAULT_ADMIN_PASS="admin"

MODE=""
FORCE_TEMPLATES=0
FORCE_CONFIG=0

VERBOSE=0
VERYVERBOSE=0

for arg in "$@"; do
    case "$arg" in

        --install)
            MODE="install"
            ;;

        --upgrade)
            MODE="upgrade"
            ;;

        --force-templates)
            FORCE_TEMPLATES=1
            ;;

        --force-config)
            FORCE_CONFIG=1
            ;;

        --verbose|-v)
            VERBOSE=1
            ;;

        --veryverbose|-vv|--very-verbose)
            VERBOSE=1
            VERYVERBOSE=1
            ;;

        *)
            echo "[FAIL] unknown argument: $arg"
            exit 1
            ;;
    esac
done

[[ "$VERYVERBOSE" -eq 1 ]] && set -x

if [[ -z "$MODE" ]]; then
    echo
    echo "Usage:"
    echo "  $0 --install [options]"
    echo "  $0 --upgrade [options]"
    echo
    echo "Options:"
    echo "  --verbose"
    echo "  --veryverbose"
    echo "  --force-templates"
    echo "  --force-config"
    echo
    exit 1
fi

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

stage "Mode"

echo "Deployment mode: ${MODE}"

if [[ "$MODE" == "install" ]]; then
    echo "Fresh install mode enabled"
else
    echo "Upgrade mode enabled"
fi

stage "Dependencies"

run apt-get update -y || fail "apt update failed"

run apt-get install -y \
    git \
    curl \
    wget \
    sqlite3 \
    rsync \
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

    run tar -C /usr/local -xzf /tmp/go.tar.gz \
        || fail "Go extract failed"

    echo 'export PATH=$PATH:/usr/local/go/bin' \
        >/etc/profile.d/golang.sh
fi

export PATH=$PATH:/usr/local/go/bin

go version >/dev/null 2>&1 \
    || fail "Go install failed"

pass "Go ready"

stage "User"

if ! id -u commandpilot >/dev/null 2>&1; then
    useradd \
        --system \
        --no-create-home \
        --shell /usr/sbin/nologin \
        commandpilot \
        || fail "failed creating commandpilot user"
fi

pass "User ready"

stage "Directories"

mkdir -p "$BASE_DIR"
mkdir -p "$(dirname "$SOURCE_DIR")"
mkdir -p "$RUNTIME_DIR"

pass "Directories ready"

stage "Source"

if [[ -d "${SOURCE_DIR}/.git" ]]; then

    run git -C "$SOURCE_DIR" fetch --all --prune \
        || fail "git fetch failed"

    run git -C "$SOURCE_DIR" reset --hard origin/main \
        || fail "git reset failed"

else

    run git clone "$REPO_URL" "$SOURCE_DIR" \
        || fail "git clone failed"

fi

[[ -d "$BUILD_DIR" ]] \
    || fail "pilot-core source missing"

pass "Repository synced"

stage "Build"

cd "$BUILD_DIR"

run go mod tidy || fail "go mod tidy failed"

rm -f "${BUILD_DIR}/pilot-core"

run go build -o pilot-core . \
    || fail "go build failed"

[[ -f "${BUILD_DIR}/pilot-core" ]] \
    || fail "binary missing"

chmod +x "${BUILD_DIR}/pilot-core"

pass "Build succeeded"

stage "Runtime"

run systemctl stop "${SERVICE_NAME}" || true

install -m 755 \
    "${BUILD_DIR}/pilot-core" \
    "${RUNTIME_DIR}/pilot-core" \
    || fail "runtime binary install failed"

mkdir -p "${RUNTIME_DIR}/backups"

if [[ "$MODE" == "install" || "$FORCE_TEMPLATES" -eq 1 ]]; then

    rm -rf "${RUNTIME_DIR}/templates"
    rm -rf "${RUNTIME_DIR}/static"

    cp -r "${SOURCE_DIR}/templates" "${RUNTIME_DIR}/templates" \
        || fail "templates copy failed"

    cp -r "${SOURCE_DIR}/static" "${RUNTIME_DIR}/static" \
        || fail "static copy failed"

else

    mkdir -p "${RUNTIME_DIR}/templates"
    mkdir -p "${RUNTIME_DIR}/static"

    rsync -a \
        --ignore-existing \
        "${SOURCE_DIR}/templates/" \
        "${RUNTIME_DIR}/templates/" \
        || fail "templates sync failed"

    rsync -a \
        --ignore-existing \
        "${SOURCE_DIR}/static/" \
        "${RUNTIME_DIR}/static/" \
        || fail "static sync failed"

fi

if [[ ! -f "$DB_PATH" ]]; then
    touch "$DB_PATH"
fi

chown -R commandpilot:commandpilot "$RUNTIME_DIR"

[[ -f "${RUNTIME_DIR}/pilot-core" ]] \
    || fail "runtime binary missing"

[[ -x "${RUNTIME_DIR}/pilot-core" ]] \
    || fail "runtime binary not executable"

pass "Runtime deployed"

stage "Database"

sqlite3 "$DB_PATH" <<EOF
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE,
    password_hash TEXT,
    role_id INTEGER
);

CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS user_groups (
    user_id INTEGER,
    group_id INTEGER
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
EOF

CURRENT_VERSION=$(sqlite3 "$DB_PATH" \
    "SELECT COALESCE(MAX(version),0) FROM schema_migrations;")

if [[ "$CURRENT_VERSION" -lt 1 ]]; then

    HAS_PASSWORD_HASH=$(sqlite3 -noheader "$DB_PATH" \
        "SELECT COUNT(1) FROM pragma_table_info('users') WHERE name='password_hash';")

    if [[ "$HAS_PASSWORD_HASH" == "0" ]]; then

        sqlite3 "$DB_PATH" \
            "ALTER TABLE users ADD COLUMN password_hash TEXT;" \
            || true

    fi

    sqlite3 "$DB_PATH" \
        "INSERT OR IGNORE INTO schema_migrations(version) VALUES(1);" \
        || true

    pass "Migration v1 applied"

fi

ROLE_EXISTS=$(sqlite3 "$DB_PATH" \
    "SELECT COUNT(*) FROM roles WHERE name='admin';")

if [[ "$ROLE_EXISTS" == "0" ]]; then

    sqlite3 "$DB_PATH" \
        "INSERT INTO roles (name) VALUES ('admin');"

    pass "Admin role created"

fi

ADMIN_ROLE_ID=$(sqlite3 "$DB_PATH" \
    "SELECT id FROM roles WHERE name='admin' LIMIT 1;")

ADMIN_EXISTS=$(sqlite3 "$DB_PATH" \
    "SELECT COUNT(*) FROM users WHERE username='${DEFAULT_ADMIN_USER}';")

if [[ "$MODE" == "install" ]]; then

    TMP_HASH_GEN="${BUILD_DIR}/.tmp_admin_hash.go"

    cat > "$TMP_HASH_GEN" <<'EOF'
package main

import (
	"fmt"
	"os"

	"golang.org/x/crypto/bcrypt"
)

func main() {
	pass := os.Getenv("ADMIN_PASSWORD")

	if pass == "" {
		panic("ADMIN_PASSWORD is empty")
	}

	hash, err := bcrypt.GenerateFromPassword(
		[]byte(pass),
		bcrypt.DefaultCost,
	)

	if err != nil {
		panic(err)
	}

	if bcrypt.CompareHashAndPassword(
		hash,
		[]byte(pass),
	) != nil {
		panic("bcrypt self-check failed")
	}

	fmt.Print(string(hash))
}
EOF

    ADMIN_HASH=$(
        cd "$BUILD_DIR" && \
        ADMIN_PASSWORD="$DEFAULT_ADMIN_PASS" \
        go run "$TMP_HASH_GEN"
    )

    rm -f "$TMP_HASH_GEN"

    [[ -n "$ADMIN_HASH" ]] \
        || fail "failed generating bcrypt hash"

    if [[ "$ADMIN_EXISTS" == "0" ]]; then

        sqlite3 "$DB_PATH" <<EOF
INSERT INTO users (
    username,
    password_hash,
    role_id
) VALUES (
    '${DEFAULT_ADMIN_USER}',
    '${ADMIN_HASH}',
    ${ADMIN_ROLE_ID}
);
EOF

        pass "Default admin user created"

    else

        sqlite3 "$DB_PATH" <<EOF
UPDATE users
SET
    password_hash='${ADMIN_HASH}',
    role_id=${ADMIN_ROLE_ID}
WHERE username='${DEFAULT_ADMIN_USER}';
EOF

        pass "Default admin user reset"

    fi

else

    pass "Existing users preserved"

fi

sqlite3 "$DB_PATH" "SELECT 1;" >/dev/null 2>&1 \
    || fail "database validation failed"

chown commandpilot:commandpilot "$DB_PATH"

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

run systemctl daemon-reload \
    || fail "daemon-reload failed"

run systemctl enable "${SERVICE_NAME}" \
    || fail "enable failed"

run systemctl restart "${SERVICE_NAME}" \
    || fail "service start failed"

sleep 5

if ! systemctl is-active --quiet "${SERVICE_NAME}"; then

    journalctl -u "${SERVICE_NAME}" -n 100 --no-pager

    fail "service inactive"

fi

pass "Service started"

stage "Validation"

ss -tulpn | grep -q ":8080" \
    || fail "port 8080 closed"

pass "Port listening"

HTTP_CODE=$(curl -s \
    -o /dev/null \
    -w "%{http_code}" \
    "${SERVER_URL}/")

[[ "$HTTP_CODE" == "200" || "$HTTP_CODE" == "401" ]] \
    || fail "HTTP failed (${HTTP_CODE})"

pass "HTTP responding"

[[ -f "$DB_PATH" ]] \
    || fail "database missing"

pass "Database exists"

sqlite3 "$DB_PATH" "SELECT 1;" >/dev/null 2>&1 \
    || fail "sqlite failed"

pass "SQLite operational"

sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q ok \
    || fail "database integrity failed"

pass "Database integrity OK"

USER_COUNT=$(sqlite3 "$DB_PATH" \
    "SELECT COUNT(*) FROM users;" 2>/dev/null || echo 0)

echo "User count: ${USER_COUNT}"

if [[ "$MODE" == "install" ]]; then

    TMP_DIR="/tmp/commandpilot_validation"
    COOKIE_JAR="${TMP_DIR}/cookies.txt"

    mkdir -p "$TMP_DIR"

    curl -s -L \
        -c "$COOKIE_JAR" \
        -X POST "${SERVER_URL}/auth/login" \
        -d "username=${DEFAULT_ADMIN_USER}&password=${DEFAULT_ADMIN_PASS}" \
        -o /dev/null

    echo
    echo "=== COOKIE JAR ==="

    cat "$COOKIE_JAR" || true

    echo

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

fi

if [[ "$VERYVERBOSE" -eq 1 ]]; then

    echo
    echo "=== PROCESS INFO ==="

    ps aux | grep pilot-core || true

    echo
    echo "=== PORTS ==="

    ss -tulpn | grep 8080 || true

    echo
    echo "=== SERVICE STATUS ==="

    systemctl status "${SERVICE_NAME}" --no-pager || true

    echo
    echo "=== JOURNAL ==="

    journalctl -u "${SERVICE_NAME}" -n 50 --no-pager || true

    echo
    echo "=== TEMPLATE FILES ==="

    find "${RUNTIME_DIR}/templates" -type f || true

    echo
    echo "=== STATIC FILES ==="

    find "${RUNTIME_DIR}/static" -type f || true

fi

IP_ADDR=$(hostname -I | awk '{print $1}')

echo
echo "======================================"
echo " CommandPilot deployment complete"
echo "======================================"

echo "Mode: ${MODE}"
echo "URL: http://${IP_ADDR}:8080"
echo "Service: ${SERVICE_NAME}"
echo "Runtime: ${RUNTIME_DIR}"

if [[ "$MODE" == "install" ]]; then
    echo
    echo "Default credentials:"
    echo "Username: ${DEFAULT_ADMIN_USER}"
    echo "Password: ${DEFAULT_ADMIN_PASS}"
fi