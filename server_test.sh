#!/bin/bash

echo "==============================="
echo " PatchPilot Server Test Script"
echo "==============================="

# Activate virtualenv
source /opt/patchpilot_server/venv/bin/activate

# Install jq if not already installed
apt install jq -y

# 1. Checking systemd service status
echo "🔍  Checking systemd service 'patchpilot_server.service'..."
service_status=$(systemctl is-active patchpilot_server.service)
if [ "$service_status" == "active" ]; then
    echo "✔️  Service is active."
else
    echo "❌  Service is not active. Status: $service_status"
    exit 1
fi

# 2. Verifying HTTP health endpoint
echo "🔍  Verifying HTTP health endpoint..."
health_status=$(curl -s -w "%{http_code}" -o health_response.json http://localhost:8080/api/health)

# Log the raw response for debugging
echo "Health check HTTP code: $health_status"
cat health_response.json

# Extract HTTP code
http_code=$(tail -n1 <<< "$health_status")
if [ "$http_code" -eq 200 ]; then
    if jq -e '.status == "ok"' health_response.json > /dev/null; then
        echo "✔️  Health endpoint returned status=ok."
    else
        echo "❌  Health check returned unexpected content: $(cat health_response.json)"
        exit 1
    fi
else
    echo "❌  Health check failed with HTTP code $http_code. Response: $(cat health_response.json)"
    exit 1
fi

# 3. Checking Flask/Gunicorn logs for recent errors
echo "🔍  Checking Flask/Gunicorn logs for recent errors..."
journalctl -u patchpilot_server.service -n 50 --no-pager | tail -n 20

# 4. Checking for Jinja2 template syntax errors
echo "🔍  Checking for Jinja2 template syntax errors..."
jinja_errors=$(journalctl -u patchpilot_server.service -n 100 --no-pager | grep -i "jinja2.exceptions.TemplateSyntaxError")
if [ -n "$jinja_errors" ]; then
    echo "❌  Found Jinja2 template errors:"
    echo "$jinja_errors"
    exit 1
else
    echo "✔️  No Jinja2 template errors found."
fi

# 5. Checking Gunicorn workers
echo "🔍  Checking Gunicorn workers..."
gunicorn_workers=$(pgrep -af gunicorn)
if [ -n "$gunicorn_workers" ]; then
    echo "✔️  Gunicorn workers are running:"
    echo "$gunicorn_workers"
else
    echo "❌  Gunicorn workers are not running."
    exit 1
fi

# 6. Verifying Flask routes via HTTP (avoid import issues)
echo "🔍  Verifying /api/health route via HTTP..."
route_check=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/api/health)
if [ "$route_check" -eq 200 ]; then
    echo "✔️  /api/health route exists."
else
    echo "❌  /api/health route not found. HTTP code: $route_check"
    exit 1
fi

# 7. Verifying communication for /api/devices/heartbeat endpoint (to test client-server communication)
echo "🔍  Verifying /api/devices/heartbeat route..."
heartbeat_response=$(curl -s -w "%{http_code}" -o heartbeat_response.json http://localhost:8080/api/devices/heartbeat)

# Log the response
echo "Heartbeat response HTTP code: $heartbeat_response"
cat heartbeat_response.json

heartbeat_http_code=$(tail -n1 <<< "$heartbeat_response")
if [ "$heartbeat_http_code" -eq 200 ]; then
    if jq -e '.status == "success"' heartbeat_response.json > /dev/null; then
        echo "✔️  Heartbeat response success."
    else
        echo "❌  Heartbeat endpoint returned unexpected content: $(cat heartbeat_response.json)"
        exit 1
    fi
else
    echo "❌  Heartbeat failed with HTTP code $heartbeat_http_code. Response: $(cat heartbeat_response.json)"
    exit 1
fi

# 8. Checking client registration status (this tests if the client is visible in the web UI)
echo "🔍  Verifying client registration status in the web UI..."
# Fetch client list from API (assuming there's an endpoint like /api/devices)
client_check=$(curl -s http://localhost:8080/api/devices)

# Check if new client exists in the list (you can customize the client name or ID here)
new_client_id="example-client-id"  # Replace with actual client ID you're expecting
if echo "$client_check" | jq -e ".[] | select(.client_id == \"$new_client_id\")" > /dev/null; then
    echo "✔️  Client is registered and visible in the web UI."
else
    echo "❌  Client is NOT registered or not visible in the web UI."
    echo "Response: $client_check"
    exit 1
fi

# 9. Checking system resource usage (CPU, Memory)
echo "🔍  Checking system resource usage..."
top -b -n 1 | head -n 20

# 10. Checking for missing critical Python packages
echo "🔍  Checking for required Python packages..."
required_packages=("flask" "flask_sqlalchemy" "flask_cors" "gunicorn" "sqlalchemy")
missing_packages=""
for pkg in "${required_packages[@]}"; do
    pip show "$pkg" >/dev/null 2>&1 || missing_packages+="$pkg "
done

if [ -z "$missing_packages" ]; then
    echo "✔️  All required Python packages are installed."
else
    echo "❌  Missing critical Python packages: $missing_packages"
    exit 1
fi

# 11. Checking Gunicorn logs for worker-related issues
echo "🔍  Checking Gunicorn logs for worker issues..."
gunicorn_logs=$(journalctl -u patchpilot_server.service -n 100 --no-pager | grep -i "worker")
if [ -n "$gunicorn_logs" ]; then
    echo "✔️  Found Gunicorn worker logs:"
    echo "$gunicorn_logs"
else
    echo "⚠️  No Gunicorn worker logs found (may be okay if startup was clean)."
fi

# 12. Checking Database Communication
echo "🔍  Checking database communication (SQLite)..."
# Check if the database is accessible and the required table exists
sqlite3 /opt/patchpilot_server/patchpilot.db ".tables" | grep -q "client"
if [ $? -eq 0 ]; then
    echo "✔️  Database table 'client' exists."
else
    echo "❌  Database table 'client' not found."
    exit 1
fi

# 13. Checking database logs for errors related to communication
echo "🔍  Checking database logs for errors..."
db_errors=$(journalctl -u patchpilot_server.service -n 100 --no-pager | grep -i "sqlite3")
if [ -n "$db_errors" ]; then
    echo "❌  Found database errors:"
    echo "$db_errors"
    exit 1
else
    echo "✔️  No database errors found."
fi

# 14. Checking for missing or incorrect database entries
echo "🔍  Checking for missing or incorrect database entries for the new client..."
client_check_db=$(sqlite3 /opt/patchpilot_server/patchpilot.db "SELECT * FROM devices WHERE client_id='$new_client_id';")
if [ -n "$client_check_db" ]; then
    echo "✔️  Client entry found in the database:"
    echo "$client_check_db"
else
    echo "❌  Client entry not found in the database."
    exit 1
fi

# End of script
echo "=============================="
echo "All tests completed successfully."
