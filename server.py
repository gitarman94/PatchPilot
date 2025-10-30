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

# Ensure log file exists and has proper permissions
if not os.path.exists(os.path.dirname(LOG_FILE)):
    os.makedirs(os.path.dirname(LOG_FILE))

# Set up rotating log handler (5MB max file size, keep 5 backup files)
handler = RotatingFileHandler(LOG_FILE, maxBytes=5*1024*1024, backupCount=5)
handler.setLevel(logging.INFO)  # Set the level of logging to INFO

# Define log format: date/time - endpoint/product - log message
formatter = logging.Formatter('%(asctime)s - %(message)s')
handler.setFormatter(formatter)

# Add the handler to the app's logger
app.logger.addHandler(handler)

# Optionally, log to the console for errors only (stdout)
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.ERROR)  # Only log errors to the console
console_handler.setFormatter(formatter)
app.logger.addHandler(console_handler)

# Set the logging level for the app's logger (capture INFO level and above)
app.logger.setLevel(logging.INFO)

# Define the Device model
class Device(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    device_name = db.Column(db.String(100), nullable=False)
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
        """Fetch and update system information for the device."""
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
    ping_latency = get_ping_latency("1.1.1.1")  # Ping Cloudflare DNS for latency measurement
    
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
def get_ping_latency(host="1.1.1.1"):
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

@app.route('/api/devices/heartbeat', methods=['POST'])
def heartbeat():
    try:
        # Log the raw data received
        data = request.get_json()
        app.logger.debug(f"Received data: {data}")

        if not data:
            app.logger.error("No data received.")
            return jsonify({'adopted': False, 'message': 'No data received'}), 400

        device_id = data.get('device_id')
        system_info = data.get('system_info')

        if not device_id or not system_info:
            app.logger.error(f"Missing device_id or system_info in the request. Device ID: {device_id}, System Info: {system_info}")
            return jsonify({'adopted': False, 'message': 'Missing device_id or system_info'}), 400

        # Log the incoming heartbeat
        app.logger.info(f"Heartbeat received from device: {device_id}")

        # Query device
        device = Device.query.filter_by(hostname=device_id).first()
        if device:
            # Check if device's system information matches the heartbeat data
            if device.os_name == system_info['os_name'] and device.architecture == system_info['architecture']:
                device.update_system_info()  # Update system stats for the device
                device.last_checkin = datetime.utcnow()
                db.session.commit()
                app.logger.info(f"Device {device_id} approved and updated.")
                return jsonify({'adopted': True, 'message': 'Device approved and updated.'})
            else:
                app.logger.warning(f"Device {device_id} OS/architecture mismatch. Awaiting approval.")
                return jsonify({'adopted': False, 'message': 'Device OS/architecture mismatch. Awaiting approval.'})

        # If the device doesn't exist, register a new device
        new_device = Device(
            device_name=device_id,
            hostname=device_id,
            os_name=system_info['os_name'],
            architecture=system_info['architecture'],
            last_checkin=datetime.utcnow(),
            approved=False
        )
        db.session.add(new_device)
        db.session.commit()
        app.logger.info(f"New device {device_id} added. Awaiting approval.")
        return jsonify({'adopted': False, 'message': 'New device. Awaiting approval.'})

    except Exception as e:
        # Log unexpected errors with more context
        app.logger.error(f"Error processing heartbeat: {str(e)}. Data: {data}")
        return jsonify({'adopted': False, 'message': 'An error occurred while processing the request'}), 500

# Route to get all device data for AJAX update
@app.route('/api/devices', methods=['GET'])
def get_devices():
    """Return device data in JSON format for AJAX updates."""
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/devices - Device data requested.")

    devices = Device.query.all()
    devices_data = []
    
    for device in devices:
        device_info = {
            'device_name': device.device_name,
            'hostname': device.hostname,
            'os_name': device.os_name,
            'architecture': device.architecture,
            'last_checkin': device.last_checkin.strftime('%Y-%m-%d %H:%M:%S'),
            'cpu': device.cpu,
            'ram_total': device.ram_total,
            'ram_used': device.ram_used,
            'ram_free': device.ram_free,
            'disk_total': device.disk_total,
            'disk_free': device.disk_free,
            'disk_health': device.disk_health,
            'network_throughput': device.network_throughput,
            'ping_latency': device.ping_latency
        }
        devices_data.append(device_info)
    
    app.logger.info(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - /api/devices - Returned {len(devices_data)} devices.")
    return jsonify(devices_data)

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
    devices = Device.query.all()
    return render_template('dashboard.html', devices=devices, now=datetime.utcnow())

# General error handler
@app.errorhandler(Exception)
def handle_exception(e):
    # Log unexpected errors
    app.logger.error(f"{datetime.utcnow().strftime('%Y-%m-%d %H:%M:%S')} - Error - An unexpected error occurred: {str(e)}")
    return jsonify({'error': 'An unexpected error occurred'}), 500

# Initialize the database if necessary
with app.app_context():
    app.logger.info("Initializing database and creating tables...")
    try:
        db.create_all()  # This forces any pending migrations
    except Exception as e:
        app.logger.error(f"Database creation failed: {str(e)}")

if __name__ == '__main__':
    with app.app_context():
        print("Listing all routes:")
        for rule in app.url_map.iter_rules():
            print(f"  {rule} -> {rule.endpoint}")
    app.run(debug=False, host='0.0.0.0', port=8080)
