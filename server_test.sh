#!/bin/bash

# Define the necessary paths and variables
SERVER_DIR="/opt/patchpilot_server"
VENV_DIR="${SERVER_DIR}/venv"
DB_USER="patchpilot_user"
DB_NAME="patchpilot_db"
PASSWORD_FILE="${SERVER_DIR}/postgresql_pwd.txt"
SERVICE_NAME="patchpilot_server.service"
SYSTEMD_DIR="/etc/systemd/system"
GITHUB_REPO="PatchPilot"

# Function to print success with a green checkmark
function success() {
    echo -e "\033[0;32m✔️  $1\033[0m"
}

# Function to print failure with a red cross
function failure() {
    echo -e "\033[0;31m❌  $1\033[0m"
}

# Function to print information in blue
function info() {
    echo -e "\033[0;34m🔍  $1\033[0m"
}

# Test system dependencies installation (step 1)
echo "=============================="
echo "     Testing System Dependencies"
echo "=============================="

info "Checking if Python 3 is installed..."
if command -v python3 >/dev/null 2>&1; then
    success "Python 3 is installed."
else
    failure "Python 3 is NOT installed."
fi

info "Checking if pip is installed..."
if command -v pip3 >/dev/null 2>&1; then
    success "pip is installed."
else
    failure "pip is NOT installed."
fi

info "Checking if curl is installed..."
if command -v curl >/dev/null 2>&1; then
    success "curl is installed."
else
    failure "curl is NOT installed."
fi

info "Checking if unzip is installed..."
if command -v unzip >/dev/null 2>&1; then
    success "unzip is installed."
else
    failure "unzip is NOT installed."
fi

info "Checking if PostgreSQL is installed..."
if command -v psql >/dev/null 2>&1; then
    success "PostgreSQL is installed."
else
    failure "PostgreSQL is NOT installed."
fi

info "Checking if systemd is installed..."
if command -v systemctl >/dev/null 2>&1; then
    success "systemd is installed."
else
    failure "systemd is NOT installed."
fi

# Test PostgreSQL setup (step 2)
echo "=============================="
echo "     Testing PostgreSQL Setup"
echo "=============================="

info "Checking if PostgreSQL password file exists..."
if [ ! -f "$PASSWORD_FILE" ]; then
    failure "PostgreSQL password file '$PASSWORD_FILE' not found! Please check your installation or setup steps."
else
    success "PostgreSQL password file found."
fi

info "Testing PostgreSQL connection..."
if [ -f "$PASSWORD_FILE" ]; then
    DB_PASSWORD=$(cat "$PASSWORD_FILE")
    PG_CMD="psql -U $DB_USER -d $DB_NAME -h localhost -p 5432 -c '\q'"
    echo "$DB_PASSWORD" | PGPASSWORD="$DB_PASSWORD" $PG_CMD > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        success "PostgreSQL connection successful!"
    else
        failure "Failed to connect to PostgreSQL. Check logs or database credentials."
    fi
fi

# Test virtual environment setup (step 3)
echo "=============================="
echo "     Testing Virtual Environment"
echo "=============================="

info "Checking if virtual environment directory exists..."
if [ ! -d "$VENV_DIR" ]; then
    failure "Virtual environment directory '$VENV_DIR' does not exist! Please ensure the environment was set up correctly."
else
    success "Virtual environment directory exists."
fi

info "Checking if pip is installed inside the virtual environment..."
if [ -f "${VENV_DIR}/bin/pip" ]; then
    success "pip is installed in the virtual environment."
else
    failure "pip is NOT installed in the virtual environment. You may need to recreate the virtual environment."
fi

info "Checking if Flask is installed in the virtual environment..."
if ${VENV_DIR}/bin/python -c "import flask" &>/dev/null; then
    success "Flask is installed."
else
    failure "Flask is NOT installed in the virtual environment. Try running 'pip install flask' inside the virtual environment."
fi

info "Checking if psycopg2 is installed in the virtual environment..."
if ${VENV_DIR}/bin/python -c "import psycopg2" &>/dev/null; then
    success "psycopg2 is installed."
else
    failure "psycopg2 is NOT installed in the virtual environment. Try running 'pip install psycopg2' inside the virtual environment."
fi

# Test repository download and extraction (step 4)
echo "=============================="
echo "     Testing Repository Download"
echo "=============================="

info "Testing GitHub repository download..."
TMPDIR=$(mktemp -d)
cd "$TMPDIR"
ZIP_URL="https://github.com/${GITHUB_USER}/${GITHUB_REPO}/archive/refs/heads/main.zip"
curl -L "${ZIP_URL}" -o latest.zip
if [ $? -eq 0 ]; then
    success "Repository ZIP file downloaded."
else
    failure "Failed to download repository ZIP file from GitHub. Check your network connection or GitHub status."
    exit 1
fi

info "Testing if the downloaded file is a valid ZIP file..."
if unzip -t latest.zip &>/dev/null; then
    success "ZIP file is valid."
else
    failure "ZIP file is corrupted or incomplete. The download may have failed. Please check the download."
    exit 1
fi

info "Testing repository extraction..."
unzip -o latest.zip
EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
if [ -z "${EXTRACTED_DIR}" ]; then
    failure "Failed to locate extracted repo directory. Ensure the ZIP file is correctly structured."
    exit 1
else
    success "Repository extracted."
fi

info "Cleaning up temporary directory..."
rm -rf "$TMPDIR"

# Test permissions and executable checks (step 5)
echo "=============================="
echo "     Testing Permissions and Executables"
echo "=============================="

info "Checking if 'server.py' is executable..."
if [ -x "${SERVER_DIR}/server.py" ]; then
    success "'server.py' is executable."
else
    failure "'server.py' is NOT executable. You may need to run 'chmod +x server.py' to fix this."
fi

# Test systemd service creation (step 6)
echo "=============================="
echo "     Testing Systemd Service"
echo "=============================="

info "Checking if systemd service file exists..."
if [ -f "${SYSTEMD_DIR}/${SERVICE_NAME}" ]; then
    success "Systemd service file found."
else
    failure "Systemd service file NOT found! Check the service installation process."
    exit 1
fi

info "Checking if systemd service is enabled..."
if systemctl is-enabled "$SERVICE_NAME" >/dev/null 2>&1; then
    success "Systemd service is enabled."
else
    failure "Systemd service is NOT enabled. Try running 'systemctl enable $SERVICE_NAME'."
    exit 1
fi

info "Checking if systemd service is running..."
if systemctl is-active "$SERVICE_NAME" >/dev/null 2>&1; then
    success "Systemd service is running."
else
    failure "Systemd service is NOT running. Check logs with 'journalctl -u $SERVICE_NAME'."
fi

# Test Flask application startup (step 7)
echo "=============================="
echo "     Testing Flask Server"
echo "=============================="

info "Checking if Flask app is running..."
if pgrep -f "server.py" >/dev/null; then
    success "Flask app is running."
else
    failure "Flask app is NOT running. Check Flask logs for errors during startup."
    echo "🔍 Checking Flask startup logs..."
    tail -n 50 /var/log/syslog | grep "patchpilot_server"
fi

info "Checking if Flask port 8080 is open..."
if netstat -tuln | grep ":8080"; then
    success "Port 8080 is open."
else
    failure "Port 8080 is NOT open. Flask may not be running or blocked by firewall."
fi

# Final check of server status
echo "=============================="
echo "     Final Server Status Check"
echo "=============================="

info "Checking if PatchPilot service is running (pgrep)..."
if pgrep -f "server.py" >/dev/null; then
    success "PatchPilot service is running!"
else
    failure "PatchPilot service is NOT running! Check logs for details."
    journalctl -u "$SERVICE_NAME" --since "1 hour ago" | tail -n 20
fi
