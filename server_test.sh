#!/bin/bash

echo "==============================="
echo " PatchPilot Server Test Script"
echo "==============================="

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
echo "Health check response: $health_status"
cat health_response.json

# Check HTTP status code
http_code=$(echo "$health_status" | tail -n1)
if [ "$http_code" -eq 200 ]; then
    # Parse the response to check if "status": "ok" is present
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

# 3. Checking Flask application logs for any issues (including template errors)
echo "🔍  Checking Flask application logs for errors..."
journalctl -u patchpilot_server.service -n 50 --no-pager | tail -n 20

# 4. Check for Jinja2 template syntax errors (e.g., the 'not' error mentioned)
echo "🔍  Checking for Jinja2 template syntax errors..."
jinja_errors=$(journalctl -u patchpilot_server.service -n 100 --no-pager | grep -i "jinja2.exceptions.TemplateSyntaxError")
if [ -n "$jinja_errors" ]; then
    echo "❌  Found Jinja2 template errors:"
    echo "$jinja_errors"
    exit 1
else
    echo "✔️  No Jinja2 template errors found."
fi

# 5. Checking if Flask application is running
echo "🔍  Checking if Flask process is running..."
flask_pid=$(pgrep -f 'flask run')
if [ -z "$flask_pid" ]; then
    echo "❌  Flask application is not running."
    exit 1
else
    echo "✔️  Flask application is running with PID: $flask_pid."
fi

# 6. Checking Gunicorn workers
echo "🔍  Checking Gunicorn workers..."
gunicorn_workers=$(ps aux | grep gunicorn | grep -v grep)
if [ -n "$gunicorn_workers" ]; then
    echo "✔️  Gunicorn workers are running:"
    echo "$gunicorn_workers"
else
    echo "❌  Gunicorn workers are not running."
    exit 1
fi

# 7. Verifying routes in Flask (ensuring /health route exists)
echo "🔍  Verifying Flask routes..."
flask_routes=$(python3 -c "
from server import app
with app.app_context():
    for rule in app.url_map.iter_rules():
        print(rule)
")
echo "$flask_routes" | grep "/health" > /dev/null
if [ $? -eq 0 ]; then
    echo "✔️  /health route exists."
else
    echo "❌  /health route not found."
    exit 1
fi

# 8. Checking system resource usage (CPU, Memory)
echo "🔍  Checking system resource usage..."
top -b -n 1 | head -n 20

# 9. Checking for missing critical Python packages
echo "🔍  Checking for missing Python packages..."
missing_packages=$(pip freeze | grep -Ev "flask|flask_sqlalchemy|flask_cors|gunicorn" || echo "Missing packages detected!")
if [ -z "$missing_packages" ]; then
    echo "✔️  All required Python packages are installed."
else
    echo "❌  Missing critical Python packages: $missing_packages"
    exit 1
fi

# 10. Checking Gunicorn logs for worker-related issues
echo "🔍  Checking Gunicorn logs for worker issues..."
gunicorn_logs=$(journalctl -u patchpilot_server.service -n 100 --no-pager | grep "worker")
if [ -n "$gunicorn_logs" ]; then
    echo "✔️  Found Gunicorn worker logs:"
    echo "$gunicorn_logs"
else
    echo "❌  No Gunicorn worker logs found."
    exit 1
fi

# End of script
echo "=============================="
echo "All tests completed."
