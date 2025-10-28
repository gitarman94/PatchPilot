import os
import subprocess
import psutil
from flask import Flask, render_template, jsonify, request, redirect, url_for
from datetime import datetime

app = Flask(__name__)

# Mock database of clients (replace with your actual database model)
clients = []

# Function to get system information for a client
def get_system_info():
    """Fetch system information such as CPU, RAM, Disk Health, Network Throughput."""
    cpu_info = psutil.cpu_percent(interval=1)
    ram_info = psutil.virtual_memory()
    disk_info = psutil.disk_usage('/')
    disk_health = "Good" if disk_info.percent < 85 else "Warning"
    network_info = psutil.net_io_counters()
    ping_latency = get_ping_latency("8.8.8.8")  # Ping Google DNS for latency measurement
    
    return {
        'cpu': cpu_info,
        'ram_total': ram_info.total,
        'ram_used': ram_info.used,
        'ram_free': ram_info.free,
        'disk_total': disk_info.total,
        'disk_free': disk_info.free,
        'disk_health': disk_health,
        'network_throughput': network_info.bytes_sent + network_info.bytes_recv,
        'ping_latency': ping_latency,
    }

# Function to measure ping latency
def get_ping_latency(host="8.8.8.8"):
    """Get the ping latency in milliseconds."""
    try:
        response = subprocess.check_output(['ping', '-c', '1', host], stderr=subprocess.STDOUT, universal_newlines=True)
        time_index = response.find('time=')
        if time_index != -1:
            latency = response[time_index + 5:response.find(' ms', time_index)]
            return float(latency)
    except subprocess.CalledProcessError:
        return None
    return None

# Route to fetch client data
@app.route('/api/clients')
def get_clients():
    """Return the list of all clients with updated info."""
    # Example mock data for clients
    # Replace this with actual logic for fetching clients from a database
    client_data = [
        {
            'id': 1,
            'client_name': 'Client A',
            'os_name': 'Windows 10',
            'last_checkin': datetime.now() - timedelta(minutes=5),
            'updates_available': False,
            'approved': True,
        },
        {
            'id': 2,
            'client_name': 'Client B',
            'os_name': 'Linux',
            'last_checkin': datetime.now() - timedelta(minutes=30),
            'updates_available': True,
            'approved': False,
        },
    ]

    # Fetch system info for each client
    for client in client_data:
        system_info = get_system_info()
        client.update(system_info)  # Add system info to client data

    return jsonify({'clients': client_data})

# Route to get details of a single client
@app.route('/api/clients/<int:client_id>')
def get_client_detail(client_id):
    """Get detailed information of a client."""
    # Fetch client data (replace with actual database fetch)
    client = next((client for client in clients if client['id'] == client_id), None)
    if client:
        system_info = get_system_info()
        client.update(system_info)  # Add system info to client data
        return jsonify(client)
    else:
        return jsonify({"error": "Client not found"}), 404

# Route to display the dashboard
@app.route('/')
def dashboard():
    return render_template('dashboard.html')

# Route for bulk actions (e.g., force patch, approve, etc.)
@app.route('/admin/force-patch', methods=['POST'])
def bulk_force_patch():
    client_ids = request.form.getlist('clientIds')
    # Logic to trigger patch installation for selected clients
    # Example: Patch action on selected clients
    # Replace with your actual patch installation logic
    return jsonify({"status": "success", "message": "Patch installation triggered for clients."})

@app.route('/admin/force-checkin', methods=['POST'])
def bulk_force_checkin():
    client_ids = request.form.getlist('clientIds')
    # Logic to trigger force check-in for selected clients
    return jsonify({"status": "success", "message": "Check-in triggered for clients."})

@app.route('/admin/approve-selected', methods=['POST'])
def bulk_approve():
    client_ids = request.form.getlist('clientIds')
    # Logic to approve selected clients
    return jsonify({"status": "success", "message": "Selected clients approved."})

# More routes for other bulk actions...

if __name__ == '__main__':
    app.run(debug=True)
