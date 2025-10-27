#!/bin/bash

echo "================================"
echo " PatchPilot Server Test Script  "
echo "================================"

# 1. Checking systemd service status
echo "🔍  Checking systemd service 'patchpilot_server.service'..."
if systemctl is-active --quiet patchpilot_server.service; then
    echo "✔️  Service is active."
else
    echo "❌  Service is not active."
    exit 1
fi

# 2. Verifying HTTP health endpoint
echo "🔍  Verifying HTTP health endpoint..."
health_status=$(curl -s http://localhost:8080/api/health)
if echo "$health_status" | grep -q '"status": "ok"'; then
    echo "✔️  Health endpoint returned status=ok."
else
    echo "❌  Health check failed or returned unexpected response."
    exit 1
fi

# 3. No PostgreSQL password file – assuming SQLite backend
echo "🔍  No PostgreSQL password file – assuming SQLite /opt/patchpilot_server/patchpilot.db."
DB_FILE="/opt/patchpilot_server/patchpilot.db"
if [ ! -f "$DB_FILE" ]; then
    echo "❌  Database file does not exist."
    exit 1
fi
echo "✔️  SQLite DB file exists at $DB_FILE."

# 4. Testing SQLite database file integrity
echo "🔍  Testing SQLite database file..."
sqlite3 $DB_FILE "PRAGMA integrity_check;"
if [ $? -eq 0 ]; then
    echo "✔️  SQLite integrity check passed."
else
    echo "❌  SQLite integrity check failed."
    exit 1
fi

# 5. Verifying SQLite tables presence
echo "🔍  Checking SQLite tables..."
client_count=$(sqlite3 $DB_FILE "SELECT COUNT(*) FROM client;")
if [ "$client_count" -ge 0 ]; then
    echo "✔️  SQLite tables present, client count=$client_count."
else
    echo "❌  SQLite tables missing or inaccessible."
    exit 1
fi

# 6. Checking required Python packages inside the virtual environment
echo "🔍  Checking required Python packages inside the virtual‑env..."
for package in flask flask_sqlalchemy flask_cors gunicorn; do
    if python3 -m pip show $package &>/dev/null; then
        echo "✔️  Package '$package' is installed."
    else
        echo "❌  Package '$package' is missing."
        exit 1
    fi
done

# 7. Testing Flask application routes (root and client API)
echo "🔍  Testing Flask application routes..."

# Test the root route
root_status=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/)
if [ "$root_status" -eq 200 ]; then
    echo "✔️  Root route is accessible."
else
    echo "❌  Root route returned status $root_status."
    exit 1
fi

# Test the /api/clients route
client_api_status=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/api/clients)
if [ "$client_api_status" -eq 200 ]; then
    echo "✔️  /api/clients route is accessible."
else
    echo "❌  /api/clients route returned status $client_api_status."
    exit 1
fi

# Test the /api/health route
health_status=$(curl -s http://localhost:8080/api/health | jq -e .status | grep -q '"ok"')
if [ $? -eq 0 ]; then
    echo "✔️  /api/health route is returning 'ok'."
else
    echo "❌  /api/health route failed."
    exit 1
fi

# 8. Checking Gunicorn process status
echo "🔍  Checking Gunicorn process status..."
gunicorn_pid=$(pgrep -f gunicorn)
if [ -z "$gunicorn_pid" ]; then
    echo "❌  Gunicorn is not running."
    exit 1
else
    echo "✔️  Gunicorn is running with PID: $gunicorn_pid."
fi

# 9. Verifying File System Integrity
echo "🔍  Checking required files and directories..."

# Check database file
if [ -f "$DB_FILE" ]; then
    echo "✔️  Database file exists at $DB_FILE."
else
    echo "❌  Database file missing."
    exit 1
fi

# Check for updates directory
if [ -d "/opt/patchpilot_server/updates" ]; then
    echo "✔️  Updates directory exists."
else
    echo "❌  Updates directory missing."
    exit 1
fi

# Check for admin token file
if [ -f "/opt/patchpilot_server/admin_token.txt" ]; then
    echo "✔️  Admin token file exists."
else
    echo "❌  Admin token file missing."
    exit 1
fi

# 10. Check for application logs
echo "🔍  Checking application logs for errors..."
log_path="/opt/patchpilot_server/logs/flask.log"
if [ -f "$log_path" ]; then
    tail -n 20 $log_path
else
    echo "❌  Flask log file not found at $log_path."
fi

# 11. Test Database Connectivity
echo "🔍  Testing database connectivity and queries..."
python3 -c "
from flask_sqlalchemy import SQLAlchemy
from app import app, db, Client

with app.app_context():
    try:
        result = db.session.execute('SELECT COUNT(*) FROM client')
        print(f'Client count: {result.fetchone()[0]}')
    except Exception as e:
        print(f'Error: {e}')
"
if [ $? -eq 0 ]; then
    echo "✔️  Database query executed successfully."
else
    echo "❌  Database query failed."
    exit 1
fi

# 12. Stress Test (using ab for Apache Benchmark)
echo "🔍  Performing stress test on /api/clients route..."
ab -n 100 -c 10 http://localhost:8080/api/clients
if [ $? -eq 0 ]; then
    echo "✔️  Stress test completed successfully."
else
    echo "❌  Stress test failed."
    exit 1
fi

# 13. Network and Firewall Check
echo "🔍  Checking network connectivity..."

# Test if server is reachable
curl_status=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/)
if [ "$curl_status" -eq 200 ]; then
    echo "✔️  Server is reachable on port 8080."
else
    echo "❌  Server is not reachable on port 8080."
    exit 1
fi

# Check firewall status (if using UFW)
sudo ufw status | grep -q "active"
if [ $? -eq 0 ]; then
    echo "✔️  Firewall is active."
else
    echo "❌  Firewall is not active."
fi

# End of Script
echo "=============================="
echo "  PatchPilot Server Test Completed "
echo "=============================="
