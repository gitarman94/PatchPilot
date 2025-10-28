import os
import subprocess
import psutil
from flask import Flask, render_template, jsonify, request, redirect, url_for
from datetime import datetime, timedelta
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS
from flask_socketio import SocketIO, emit
from flask_login import LoginManager, UserMixin, login_user, login_required, logout_user, current_user

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
login_manager = LoginManager(app)

# Celery setup for background tasks
from celery import Celery
celery = Celery(app.name, broker='redis://localhost:6379/0')

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

# User model for RBAC
class User(db.Model, UserMixin):
    id = db.Column(db.Integer, primary_key=True)
    username = db.Column(db.String(100), unique=True, nullable=False)
    password_hash = db.Column(db.String(200), nullable=False)
    role = db.Column(db.String(50), nullable=False)  # Admin, Viewer, etc.

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

# Celery Task Example for patch installation
@celery.task
def install_patch(client_id):
    # Trigger patch installation for the client (simulated)
    client = Client.query.get(client_id)
    if client:
        # Example patch installation logic
        pass
    return 'Patch Installed'

# Real-time updates via WebSocket
@app.route('/api/clients')
def get_clients():
    """Return the list of all clients with updated info."""
    client_data = Client.query.all()
    for client in client_data:
        client.update_system_info()

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

# SocketIO for real-time updates
@socketio.on('connect')
def handle_connect():
    emit('alert', {'message': 'Client connected!'})

# Bulk actions (e.g., force patch, approve, etc.)
@app.route('/admin/force-patch', methods=['POST'])
def bulk_force_patch():
    client_ids = request.form.getlist('clientIds')
    # Trigger patch installation for clients
    for client_id in client_ids:
        install_patch.apply_async(args=[client_id])
    return jsonify({"status": "success", "message": "Patch installation triggered for clients."})

@app.route('/admin/approve-selected', methods=['POST'])
def bulk_approve():
    client_ids = request.form.getlist('clientIds')
    # Approve selected clients
    for client_id in client_ids:
        client = Client.query.get(client_id)
        client.approved = True
        db.session.commit()
    return jsonify({"status": "success", "message": "Selected clients approved."})

# Initialize the database if necessary
@app.before_first_request
def create_tables():
    """Creates the database tables if they don't exist yet."""
    db.create_all()

# User authentication routes
@login_manager.user_loader
def load_user(user_id):
    return User.query.get(int(user_id))

@app.route('/login', methods=['GET', 'POST'])
def login():
    if request.method == 'POST':
        username = request.form['username']
        password = request.form['password']
        user = User.query.filter_by(username=username).first()
        if user and user.password_hash == password:  # Add password verification
            login_user(user)
            return redirect(url_for('dashboard'))
    return render_template('login.html')

@app.route('/logout')
@login_required
def logout():
    logout_user()
    return redirect(url_for('login'))

@app.route('/')
@login_required
def dashboard():
    return render_template('dashboard.html')

if __name__ == '__main__':
    socketio.run(app, debug=True)
