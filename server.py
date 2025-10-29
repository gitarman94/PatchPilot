import os
import subprocess
import psutil
import logging
from logging.handlers import RotatingFileHandler
from flask import Flask, render_template, jsonify, request
from datetime import datetime
from flask_sqlalchemy import SQLAlchemy

# Initialize the Flask app
app = Flask(__name__)

# Set the DATABASE_URI environment variable if not already set
app.config['SQLALCHEMY_DATABASE_URI'] = os.getenv('DATABASE_URI', 'sqlite:////opt/patchpilot_server/patchpilot.db')
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.secret_key = os.getenv('SECRET_KEY', 'defaultsecretkey')

# Initialize the database
db = SQLAlchemy(app)

# --- Logging Configuration ---
LOG_FILE = '/opt/patchpilot_server/server.log'

# Set up rotating log handler (5MB max file size, keep 5 backup files)
handler = RotatingFileHandler(LOG_FILE, maxBytes=5*1024*1024, backupCount=5)
handler.setLevel(logging.INFO)

# Define log format: date/time - endpoint/product - log message
formatter = logging.Formatter('%(asctime)s - %(message)s')
handler.setFormatter(formatter)

# Add the handler to the app's logger
app.logger.addHandler(handler)
app.logger.setLevel(logging.INFO)

# Optionally, log to the console as well (stdout)
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.ERROR)  # Only log errors to the console
console_handler.setFormatter(formatter)
app.logger.addHandler(console_handler)

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
@app.route('/api/client/heartbeat', methods=['POST'])
def heartbeat():
    data = request.get_json()
    client_id = data.get('client_id')
    system_info = data.get('system_info')
    
    # Log the incoming heartbeat
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/client/heartbeat - Heartbeat received from client: {client_id}")

    client = Client.query.filter_by(hostname=client_id).first()

    if client:
        if client.os_name == system_info['os_name'] and client.architecture == system_info['architecture']:
            client.update_system_info()
            client.last_checkin = datetime.utcnow()
            db.session.commit()
            app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/client/heartbeat - Client {client_id} approved and updated.")
            return jsonify({'adopted': True, 'message': 'Client approved and updated.'})
        else:
            app.logger.warning(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/client/heartbeat - Client {client_id} OS/architecture mismatch. Awaiting approval.")
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
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/client/heartbeat - New client {client_id} added. Awaiting approval.")
    return jsonify({'adopted': False, 'message': 'New client. Awaiting approval.'})

# Route to get all client data for AJAX update
@app.route('/api/clients', methods=['GET'])
def get_clients():
    """Return client data in JSON format for AJAX updates."""
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/client - Client data requested.")

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
    
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/clients - Returned {len(clients_data)} clients.")
    return jsonify(clients_data)

# Get health status of server
@app.route('/api/health', methods=['GET'])
def health():
    """Return a simple health check response."""
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/health - Health check requested.")
    return jsonify({'status': 'ok'})

# Root route - Dashboard
@app.route('/')
def dashboard():
    """Render the dashboard template without authentication."""
    clients = Client.query.all()
    return render_template('dashboard.html', clients=clients, now=datetime.utcnow())

# General error handler
@app.errorhandler(Exception)
def handle_exception(e):
    # Log unexpected errors
    app.logger.error(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - Error - An unexpected error occurred: {str(e)}")
    return jsonify({'error': 'An unexpected error occurred'}), 500

# Initialize the database if necessary
with app.app_context():
    app.logger.info("Initializing database and creating tables...")
    db.create_all()

if __name__ == '__main__':
    with app.app_context():
        print("Listing all routes:")
        for rule in app.url_map.iter_rules():
            print(f"{rule.endpoint}: {rule}")
    app.run(debug=True)





