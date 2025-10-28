import os
import subprocess
import psutil
import logging
from flask import Flask, render_template, jsonify, request
from datetime import datetime
from flask_sqlalchemy import SQLAlchemy

# Initialize the Flask app
app = Flask(__name__)

# Set the DATABASE_URI environment variable if not already set
app.config['SQLALCHEMY_DATABASE_URI'] = os.getenv('DATABASE_URI', 'sqlite:///patchpilot.db')
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.secret_key = os.getenv('SECRET_KEY', 'defaultsecretkey')

# Initialize the database
db = SQLAlchemy(app)

# --- Logging Configuration ---
LOG_FILE = 'server.log'
logging.basicConfig(level=logging.DEBUG, format='%(asctime)s - %(levelname)s - %(message)s', handlers=[
    logging.FileHandler(LOG_FILE),
    logging.StreamHandler()
])

# Define the Client model
class Client(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    client_name = db.Column(db.String(100), nullable=False)
    hostname = db.Column(db.String(100), nullable=False, unique=True)
    os_name = db.Column(db.String(50), nullable=False)
    architecture = db.Column(db.String(50), nullable=False)
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

# Helper function for system info
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

# Heartbeat logic to check adoption
@app.route('/api/devices/heartbeat', methods=['POST'])
def heartbeat():
    data = request.get_json()
    client_id = data.get('client_id')
    system_info = data.get('system_info')
    client = Client.query.filter_by(hostname=client_id).first()

    if client:
        if client.os_name == system_info['os_name'] and client.architecture == system_info['architecture']:
            client.update_system_info()
            client.last_checkin = datetime.utcnow()
            db.session.commit()
            return jsonify({'adopted': True, 'message': 'Client approved and updated.'})
        else:
            return jsonify({'adopted': False, 'message': 'Client OS/architecture mismatch. Awaiting approval.'})
    
    new_client = Client(
        client_name=client_id,
        hostname=client_id,
        os_name=system_info['os_name'],
        architecture=system_info['architecture'],
        last_checkin=datetime.utcnow(),
        approved=False
    )
    db.session.add(new_client)
    db.session.commit()

    return jsonify({'adopted': False, 'message': 'New client. Awaiting approval.'})

# Initialize the database if necessary
with app.app_context():
    db.create_all()

# Route to get all client data for AJAX update
@app.route('/api/clients', methods=['GET'])
def get_clients():
    """Return client data in JSON format for AJAX updates."""
    clients = Client.query.all()
    clients_data = []
    
    for client in clients:
        client_info = {
            'client_name': client.client_name,
            'hostname': client.hostname,
            'os_name': client.os_name,
            'architecture': client.architecture,
            'last_checkin': client.last_checkin.strftime('%Y-%m-%d %H:%M:%S'),
            'cpu': client.cpu,
            'ram_total': client.ram_total,
            'ram_used': client.ram_used,
            'ram_free': client.ram_free,
            'disk_total': client.disk_total,
            'disk_free': client.disk_free,
            'disk_health': client.disk_health,
            'network_throughput': client.network_throughput,
            'ping_latency': client.ping_latency
        }
        clients_data.append(client_info)
    
    return jsonify(clients_data)

# Root route - Dashboard
@app.route('/')
def dashboard():
    """Render the dashboard template without authentication."""
    clients = Client.query.all()
    return render_template('dashboard.html', clients=clients, now=datetime.utcnow())

if __name__ == '__main__':
    app.logger.info("Starting the PatchPilot server...")
    app.run(debug=True)
