import os
import subprocess
import psutil
from flask import Flask, render_template, jsonify, request, redirect, url_for
from datetime import datetime, timedelta
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS

# Initialize the Flask app
app = Flask(__name__)
CORS(app)

# Set the DATABASE_URI environment variable if not already set
app.config['SQLALCHEMY_DATABASE_URI'] = os.getenv('DATABASE_URI', 'sqlite:///patchpilot.db')
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False

# Debug: print the database URI being used (for troubleshooting)
print(f"Using database URI: {app.config['SQLALCHEMY_DATABASE_URI']}")

# Initialize the database
db = SQLAlchemy(app)

# Define the Client model
class Client(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    client_name = db.Column(db.String(100), nullable=False)
    os_name = db.Column(db.String(50), nullable=False)
    last_checkin = db.Column(db.DateTime, nullable=False)
    updates_available = db.Column(db.Boolean, nullable=False, default=False)
    approved = db.Column(db.Boolean, nullable=False, default=False)
    
    # System info fields
    cpu = db.Column(db.Float, nullable=True)
    ram_total = db.Column(db.BigInteger, nullable=True)
    ram_used = db.Column(db.BigInteger, nullable=True)
    ram_free = db.Column(db.BigInteger, nullable=True)
    disk_total = db.Column(db.BigInteger, nullable=True)
    disk_free = db.Column(db.BigInteger, nullable=True)
    disk_health = db.Column(db.String(50), nullable=True)
    network_throughput = db.Column(db.BigInteger, nullable=True)
    ping_latency = db.Column(db.Float, nullable=True)

    def update_system_info(self):
        """Fetch and update system information for the client."""
        system_info = get_system_info()
        self.cpu = system_info['cpu']
        self.ram_total = system_info['ram_total']
        self.ram_used = system_info['ram_used']
        self.ram_free = system_info['ram_free']
        self.disk_total = system_info['disk_total']
        self.disk_free = system_info['disk_free']
        self.disk_health = system_info['disk_health']
        self.network_throughput = system_info['network_throughput']
        self.ping_latency = system_info['ping_latency']

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
    # Fetch clients from database
    client_data = Client.query.all()
    
    # Update system info for each client
    for client in client_data:
        client.update_system_info()  # Add system info to client data

    # Convert to a list of dictionaries for JSON response
    clients_dict = [
        {
            'id': client.id,
            'client_name': client.client_name,
            'os_name': client.os_name,
            'last_checkin': client.last_checkin,
            'updates_available': client.updates_available,
            'approved': client.approved,
            'cpu': client.cpu,
            'ram_total': client.ram_total,
            'ram_used': client.ram_used,
            'ram_free': client.ram_free,
            'disk_total': client.disk_total,
            'disk_free': client.disk_free,
            'disk_health': client.disk_health,
            'network_throughput': client.network_throughput,
            'ping_latency': client.ping_latency,
        }
        for client in client_data
    ]

    return jsonify({'clients': clients_dict})

# Route to get details of a single client
@app.route('/api/clients/<int:client_id>')
def get_client_detail(client_id):
    """Get detailed information of a client."""
    # Fetch client data (replace with actual database fetch)
    client = Client.query.get(client_id)
    if client:
        client.update_system_info()  # Add system info to client data
        return jsonify({
            'id': client.id,
            'client_name': client.client_name,
            'os_name': client.os_name,
            'last_checkin': client.last_checkin,
            'updates_available': client.updates_available,
            'approved': client.approved,
            'cpu': client.cpu,
            'ram_total': client.ram_total,
            'ram_used': client.ram_used,
            'ram_free': client.ram_free,
            'disk_total': client.disk_total,
            'disk_free': client.disk_free,
            'disk_health': client.disk_health,
            'network_throughput': client.network_throughput,
            'ping_latency': client.ping_latency,
        })
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

# Initialize the database if necessary
@app.before_first_request
def create_tables():
    """Creates the database tables if they don't exist yet."""
    db.create_all()

if __name__ == '__main__':
    # Ensure the database tables are created before running
    app.run(debug=True)
