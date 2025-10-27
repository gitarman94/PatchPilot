#!/bin/bash

echo "==============================="
echo " PatchPilot Server Test Script"
echo "==============================="

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

# 3. Checking Flask application logs for any issues
echo "🔍  Checking Flask application logs..."
journalctl -u patchpilot_server.service -n 50 --no-pager | tail -n 20

# 4. Checking if Flask is running
echo "🔍  Checking if Flask process is running..."
flask_pid=$(pgrep -f 'flask run')
if [ -z "$flask_pid" ]; then
    echo "❌  Flask application is not running."
    exit 1
else
    echo "✔️  Flask application is running with PID: $flask_pid."
fi

# 5. Check system resource usage
echo "🔍  Checking system resource usage..."
top -b -n 1 | head -n 20

# 6. Check if any critical packages are missing
echo "🔍  Checking for missing Python packages..."
missing_packages=$(pip freeze | grep -Ev "flask|flask_sqlalchemy|flask_cors|gunicorn" || echo "Missing packages detected!")
if [ -z "$missing_packages" ]; then
    echo "✔️  All required Python packages are installed."
else
    echo "❌  Missing critical Python packages: $missing_packages"
    exit 1
fi

# End of script
echo "=============================="
echo "All tests completed."
