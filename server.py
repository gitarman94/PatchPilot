import os
import subprocess
import psutil
import logging
from flask import Flask, render_template, jsonify, request, redirect, url_for
from datetime import datetime, timedelta
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS
from flask_socketio import SocketIO, emit

# Initialize the Flask app
app = Flask(__name__)
CORS(app)

# Set the DATABASE_URI environment variable if not already set
app.config['SQLALCHEMY_DATABASE_URI'] = os.getenv('DATABASE_URI', 'sqlite:///patchpilot.db')
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.secret_key = os.getenv('SECRET_KEY', 'defaultsecretkey')

# Initialize the database and other Flask extensions
db = SQLAlchemy(app)
socketio = SocketIO(app)

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
    hostname = db.Column(db.String(100), nullable=False, unique=True)  # Use hostname as unique identifier
    os_name = db.Column(db.String(50), nullable=False)
    architecture = db.Column(db.String(50), nullable=False)  # Track the architecture (e.g., x86_64)
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

    action_logs = db.relationship('ActionLog', back_populates='client')

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

# ActionLog Model for audit trail
class ActionLog(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    client_id = db.Column(db.Integer, db.ForeignKey('client.id'), nullable=False)
    action = db.Column(db.String(100), nullable=False)
    status = db.Column(db.String(20), nullable=False)  # Success or Failure
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)
    admin_user = db.Column(db.String(100), nullable=False)
    client = db.relationship('Client', back_populates='action_logs')

# Helper to log admin actions
def log_action(client_id, action, status, admin_user):
    log = ActionLog(client_id=client_id, action=action, status=status, admin_user=admin_user)
    db.session.add(log)
    db.session.commit()

# Function to get system information
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
    """Handle device heartbeat (client check-in)"""
    data = request.get_json()
    client_id = data.get('client_id')  # We are using the hostname as client_id
    system_info = data.get('system_info')  # Get the system info

    # Log received data for debugging
    app.logger.info(f"Received heartbeat from client: {client_id}, System Info: {system_info}")

    client = Client.query.filter_by(hostname=client_id).first()

    if client:
        # If the client exists, check if the OS and architecture match
        if client.os_name == system_info['os_name'] and client.architecture == system_info['architecture']:
            # Merge old client info with new info
            client.update_system_info()
            client.last_checkin = datetime.utcnow()
            db.session.commit()
            app.logger.info(f"Client {client_id} approved and updated.")
            return jsonify({'adopted': True, 'message': 'Client approved and updated.'})
        else:
            # If OS/architecture mismatch, put it into adoption mode
            app.logger.warning(f"Client {client_id} OS/architecture mismatch.")
            return jsonify({'adopted': False, 'message': 'Client OS/architecture mismatch. Awaiting approval.'})
    
    # If the client doesn't exist, enter adoption mode
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

    app.logger.info(f"New client {client_id} added. Awaiting approval.")
    return jsonify({'adopted': False, 'message': 'New client. Awaiting approval.'})

# Initialize the database if necessary
@app.before_first_request
def create_tables():
    """Creates the database tables if they don't exist yet."""
    db.create_all()

# Remove authentication routes and login requirement
@app.route('/')
def dashboard():
    return render_template('dashboard.html')

if __name__ == '__main__':
    app.logger.info("Starting the PatchPilot server...")
    socketio.run(app, debug=True)
