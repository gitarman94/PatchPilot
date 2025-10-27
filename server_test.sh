#!/bin/bash

# Define the necessary paths and variables
SERVER_DIR="/opt/patchpilot_server"
DB_USER="patchpilot_user"
DB_NAME="patchpilot_db"
PASSWORD_FILE="${SERVER_DIR}/postgresql_pwd.txt"

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

# Start testing

echo "=============================="
echo "     Running Server Test      "
echo "=============================="

info "Checking if PatchPilot service is running..."

# Check if PatchPilot server is running (by checking for its process)
if pgrep -f "server.py" > /dev/null; then
    success "PatchPilot server is running!"
else
    failure "PatchPilot server is NOT running!"
    echo "🔍 Let's investigate why:"
    
    info "Checking if 'server.py' exists..."
    if [ -f "${SERVER_DIR}/server.py" ]; then
        success "'server.py' found at ${SERVER_DIR}/server.py"
    else
        failure "'server.py' does not exist!"
        exit 1
    fi

    info "Attempting to start 'server.py' manually to capture errors..."
    python3 "${SERVER_DIR}/server.py" > server_startup.log 2>&1 & 
    sleep 5  # Give the server time to start
    tail -n 20 server_startup.log
    failure "Manual startup attempt failed. Check server logs above."
    exit 1
fi

info "Checking PostgreSQL credentials..."

# Check if the PostgreSQL password file exists
if [ ! -f "$PASSWORD_FILE" ]; then
    failure "PostgreSQL password file '$PASSWORD_FILE' not found!"
    exit 1
fi

# Retrieve the PostgreSQL password from the file
DB_PASSWORD=$(cat "$PASSWORD_FILE")

# Test the PostgreSQL connection
PG_CMD="psql -U $DB_USER -d $DB_NAME -h localhost -p 5432 -c '\q'"
echo "$DB_PASSWORD" | PGPASSWORD="$DB_PASSWORD" $PG_CMD > /dev/null 2>&1

if [ $? -eq 0 ]; then
    success "PostgreSQL connection successful!"
else
    failure "Failed to connect to PostgreSQL with user '$DB_USER'."
    PG_ERROR=$(echo "$DB_PASSWORD" | PGPASSWORD="$DB_PASSWORD" $PG_CMD 2>&1)
    failure "PostgreSQL connection failed with error: $PG_ERROR"
    exit 1
fi

info "Checking if Flask is running in debug mode..."

# Check if Flask is running in debug mode (check if app.run has debug=True)
if grep -q "app.run(host='0.0.0.0', port=8080, debug=True)" "$SERVER_DIR/server.py"; then
    success "Flask is running in debug mode!"
else
    failure "Flask is NOT running in debug mode."
    info "Consider adding 'debug=True' to 'app.run()' in 'server.py' for easier debugging."
fi

info "Checking Python package dependencies..."

# Check for required packages
REQUIRED_PACKAGES=("flask" "flask_sqlalchemy" "psycopg2" "flask_cors")
for package in "${REQUIRED_PACKAGES[@]}"; do
    if python3 -c "import $package" &> /dev/null; then
        success "$package is installed."
    else
        failure "$package is NOT installed."
        exit 1
    fi
done

info "Checking if database tables exist..."

# Test if tables exist in PostgreSQL (client and client_update)
python3 -c "
from server import db
from server import Client, ClientUpdate
try:
    # Check for the tables existence
    client_table_exists = db.session.execute('SELECT to_regclass(\'public.client\')').scalar()
    update_table_exists = db.session.execute('SELECT to_regclass(\'public.client_update\')').scalar()
    if not client_table_exists or not update_table_exists:
        raise Exception('Tables do not exist or cannot be found.')
    
    # Check if there is at least one record in the client table
    client_count = Client.query.count()
    if client_count == 0:
        raise Exception('No records found in the client table.')

    print('✔️ Database tables are correctly set up and contain records.')
except Exception as e:
    print(f'❌ Error with database setup: {e}')
" > /dev/null 2>&1

if [ $? -eq 0 ]; then
    success "Database tables are correctly set up and contain records."
else
    failure "Database tables are not set up properly or contain no records."
    exit 1
fi

info "Checking system performance..."

# Check system resource usage (CPU, Memory, Disk)
echo "🔍 System Resource Usage:"
top -n 1 | head -n 20

info "Troubleshooting complete!"

echo "=============================="
echo "Review the errors above and take necessary actions to resolve the issues."
echo "If the issue persists, consider checking the PostgreSQL server or Flask logs."
echo "=============================="
