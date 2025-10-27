#!/usr/bin/env bash
# --------------------------------------------------------------
# server_test.sh – basic functional test for the PatchPilot server
# --------------------------------------------------------------

set -euo pipefail

# -------------------------- Configuration -------------------------
SERVER_DIR="/opt/patchpilot_server"
VENV_DIR="${SERVER_DIR}/venv"
SERVICE_NAME="patchpilot_server.service"
ENV_FILE="${SERVER_DIR}/admin_token.env"
TOKEN_FILE="${SERVER_DIR}/admin_token.txt"

# SQLite defaults (used when no PostgreSQL password file is present)
SQLITE_DB="${SERVER_DIR}/patchpilot.db"

# PostgreSQL defaults (kept for backward compatibility)
PG_USER="patchpilot_user"
PG_DB="patchpilot_db"
PG_PASSWORD_FILE="${SERVER_DIR}/postgresql_pwd.txt"

# --------------------------- Helpers ----------------------------
function success() {
    echo -e "\033[0;32m✔️  $1\033[0m"
}
function failure() {
    echo -e "\033[0;31m❌  $1\033[0m"
}
function info() {
    echo -e "\033[0;34m🔍  $1\033[0m"
}
function warn() {
    echo -e "\033[0⚠️  $1\033[0m"
}

# --------------------------- Header ----------------------------
echo "=============================="
echo "      PatchPilot Server Test   "
echo "=============================="

# ------------------------ Service check ------------------------
info "Checking systemd service '${SERVICE_NAME}'..."

if systemctl is-active --quiet "${SERVICE_NAME}"; then
    success "Service is active."
else
    failure "Service is NOT running."
    warn "Attempting to start the service..."
    systemctl start "${SERVICE_NAME}" || {
        failure "Failed to start service via systemctl."
        exit 1
    }
    # re‑check
    if systemctl is-active --quiet "${SERVICE_NAME}"; then
        success "Service started successfully."
    else
        failure "Service still not running after start attempt."
        exit 1
    fi
fi

# -------------------------- Health check -----------------------
info "Verifying HTTP health endpoint..."

SERVER_IP=$(hostname -I | awk '{print $1}')
HEALTH_URL="http://${SERVER_IP}:8080/api/health"

if curl -s --max-time 5 "${HEALTH_URL}" | grep -q '"status":"ok"'; then
    success "Health endpoint responded with status=ok."
else
    failure "Health endpoint not reachable or returned unexpected result."
    warn "Fetching recent journal entries for diagnosis:"
    journalctl -u "${SERVICE_NAME}" -n 20 --no-pager
    exit 1
fi

# --------------------------- DB type ---------------------------
if [[ -f "${PG_PASSWORD_FILE}" ]]; then
    DB_BACKEND="postgresql"
    info "PostgreSQL credentials detected."
else
    DB_BACKEND="sqlite"
    info "No PostgreSQL password file – assuming SQLite (${SQLITE_DB})."
fi

# ------------------------ DB connectivity ----------------------
if [[ "${DB_BACKEND}" == "postgresql" ]]; then
    info "Testing PostgreSQL connection..."

    if [[ ! -f "${PG_PASSWORD_FILE}" ]]; then
        failure "Password file missing despite earlier detection."
        exit 1
    fi
    PG_PASSWORD=$(< "${PG_PASSWORD_FILE}")

    # Use PGPASSWORD env var for non‑interactive auth
    PGPASSWORD="${PG_PASSWORD}" psql -U "${PG_USER}" -d "${PG_DB}" -h localhost -p 5432 -c '\q' \
        >/dev/null 2>&1 && success "PostgreSQL connection succeeded." || {
        failure "Unable to connect to PostgreSQL."
        PGPASSWORD="${PG_PASSWORD}" psql -U "${PG_USER}" -d "${PG_DB}" -h localhost -p 5432 -c '\q' 2>&1 | tail -n 20
        exit 1
    }
else
    info "Testing SQLite database file..."

    if [[ -f "${SQLITE_DB}" ]]; then
        success "SQLite DB file exists (${SQLITE_DB})."
    else
        failure "SQLite DB file not found at ${SQLITE_DB}."
        exit 1
    fi

    # Quick sanity check – can we open a connection via the app's SQLAlchemy instance?
    "${VENV_DIR}/bin/python" - <<'PYEND'
import sys, os
sys.path.insert(0, os.getenv("SERVER_DIR", "/opt/patchpilot_server"))
from server import db, Client, ClientUpdate
try:
    # ensure tables exist
    client_exists = db.session.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='client'").scalar()
    update_exists = db.session.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='client_update'").scalar()
    if not client_exists or not update_exists:
        raise RuntimeError("Required tables are missing.")
    # optional: at least one client record
    count = db.session.execute("SELECT COUNT(*) FROM client").scalar()
    print(f"✔️  SQLite tables present, client count={count}")
except Exception as e:
    print(f"❌  SQLite sanity check failed: {e}")
    sys.exit(1)
PYEND
    # The Python block prints its own success/failure; just propagate exit status
    [[ $? -eq 0 ]] && success "SQLite sanity check passed." || failure "SQLite sanity check failed."
fi

# ------------------------ Python deps -------------------------
info "Checking required Python packages inside the virtual‑env..."

REQUIRED_PKGS=(flask flask_sqlalchemy flask_cors gunicorn)

MISSING_PKGS=()
for pkg in "${REQUIRED_PKGS[@]}"; do
    if "${VENV_DIR}/bin/python" -c "import ${pkg}" >/dev/null 2>&1; then
        success "Package '${pkg}' is installed."
    else
        failure "Package '${pkg}' is NOT installed."
        MISSING_PKGS+=("${pkg}")
    fi
done

if (( ${#MISSING_PKGS[@]} )); then
    warn "Attempting to install missing packages..."
    "${VENV_DIR}/bin/pip" install "${MISSING_PKGS[@]}" || {
        failure "Failed to install required packages."
        exit 1
    }
    success "Missing packages installed."
fi

# --------------------- Debug‑mode warning --------------------
info "Checking whether Flask is running in debug mode..."

# In gunicorn the debug flag is irrelevant, but we can still scan the source.
if grep -q "debug=True" "${SERVER_DIR}/server.py"; then
    warn "Flask is started with debug=True – remember to disable in production."
else
    success "Flask debug mode not forced in source."
fi

# --------------------- System resources ----------------------
info "Current system resource snapshot (top, first 20 lines):"
top -b -n 1 | head -n 20

# -------------------------- Finish ---------------------------
echo "=============================="
success "All checks completed successfully."
echo "If any warnings appeared, review them for possible improvements."
echo "=============================="
