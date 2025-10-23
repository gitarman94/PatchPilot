import os
import json
import base64
from datetime import datetime, timedelta
from flask import Flask, request, jsonify, render_template, abort, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS

# Initialize Flask application and enable CORS
app = Flask(__name__)
CORS(app)

# == CONFIG == 
SERVER_DIR = "/opt/patchpilot_server"
UPDATE_CACHE_DIR = os.path.join(SERVER_DIR, "updates")
if not os.path.isdir(UPDATE_CACHE_DIR):
    os.makedirs(UPDATE_CACHE_DIR, exist_ok=True)

app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///patchpilot.db'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
db = SQLAlchemy(app)

# == MODELS == 
class Client(db.Model):
    id = db.Column(db.String(36), primary_key=True)
    client_name = db.Column(db.String(100))
    ip_address = db.Column(db.String(45))
    approved = db.Column(db.Boolean, default=False)
    allow_checkin = db.Column(db.Boolean, default=True)
    force_update = db.Column(db.Boolean, default=False)
    last_checkin = db.Column(db.DateTime)
    token = db.Column(db.String(50), unique=True, nullable=False)
    file_hashes = db.Column(db.Text, nullable=True)
    updates_available = db.Column(db.Boolean, default=False)

    # Telemetry fields
    os_name = db.Column(db.String(50))
    os_version = db.Column(db.String(50))
    cpu = db.Column(db.String(100))
    ram = db.Column(db.String(50))
    disk_total = db.Column(db.String(50))
    disk_free = db.Column(db.String(50))
    uptime_val = db.Column("uptime", db.String(50))
    serial_number = db.Column(db.String(50), unique=True, nullable=True)  # New field for serial number

    def is_online(self):
        if not self.last_checkin:
            return False
        return datetime.utcnow() - self.last_checkin <= timedelta(minutes=3)

    def uptime(self):
        return self.uptime_val or "N/A"


class ClientUpdate(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    client_id = db.Column(db.String(36), db.ForeignKey('client.id'), nullable=False)
    kb_or_package = db.Column(db.String(200), nullable=False)
    title = db.Column(db.String(200), nullable=True)
    severity = db.Column(db.String(50), nullable=True)
    status = db.Column(db.String(50), nullable=False, default='pending')  # pending/installing/installed/failed
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)

# == HELPER FUNCTIONS ==

def generate_token():
    return base64.urlsafe_b64encode(os.urandom(24)).decode()

def auth_client(client, token):
    if token is None:
        return False
    if token.startswith("Bearer "):
        token = token[7:]
    return token == client.token

# == ROUTES ==

@app.route('/api/health', methods=['GET'])
def health_check():
    return jsonify({'status': 'ok'})

# --- DASHBOARD ---
@app.route('/')
def index():
    clients = Client.query.all()
    return render_template('client.html', clients=clients, now=datetime.utcnow())

# --- CLIENT DETAIL ---
@app.route('/clients/<client_id>')
def client_detail(client_id):
    client = Client.query.get(client_id)
    if not client:
        abort(404)
    updates = ClientUpdate.query.filter_by(client_id=client_id).all()
    ADMIN_TOKEN = os.getenv('ADMIN_TOKEN', 'dummy_admin_token')
    return render_template('client_detail.html', client=client, updates=updates, ADMIN_TOKEN=ADMIN_TOKEN)

# --- APPROVE CLIENT ---
@app.route('/approve/<client_id>', methods=['POST'])
def approve_client(client_id):
    client = Client.query.get(client_id)
    if not client:
        abort(404)
    client.approved = True
    db.session.commit()
    return ('', 204)

# --- FORCE UPDATE ---
@app.route('/admin/force-update/<client_id>', methods=['POST'])
def force_update_client(client_id):
    client = Client.query.get(client_id)
    if not client:
        abort(404)
    client.force_update = True
    db.session.commit()
    return ('', 204)

# --- ALLOW CHECKIN ---
@app.route('/admin/allow-checkin/<client_id>', methods=['POST'])
def allow_checkin_client(client_id):
    client = Client.query.get(client_id)
    if not client:
        abort(404)
    client.allow_checkin = True
    db.session.commit()
    return ('', 204)

# --- SERVE UPDATES ---
@app.route('/updates/<path:filename>', methods=['GET'])
def serve_update_file(filename):
    return send_from_directory(UPDATE_CACHE_DIR, filename, as_attachment=True)

# --- CLIENT COMMANDS FROM DASHBOARD ---
@app.route('/api/clients/<client_id>/commands', methods=['POST'])
def send_command(client_id):
    client = Client.query.get(client_id)
    if not client:
        return jsonify({'error': 'Client not found'}), 404

    # Accept either JSON or form
    data = request.get_json() or request.form
    admin_token = data.get('admin_token') or ''
    if admin_token != os.getenv('ADMIN_TOKEN', 'dummy_admin_token'):
        return jsonify({'error': 'Unauthorized'}), 401

    action = data.get('action')
    updates = data.getlist('updates') if hasattr(data, 'getlist') else data.get('updates', [])

    if action == 'install_selected_updates' and updates:
        for kb in updates:
            cu = ClientUpdate.query.filter_by(client_id=client.id, kb_or_package=kb).first()
            if cu:
                cu.status = 'installing'
    elif action == 'install_all_updates':
        for cu in ClientUpdate.query.filter_by(client_id=client.id, status='pending').all():
            cu.status = 'installing'
    else:
        return jsonify({'error': 'Unknown action'}), 400

    db.session.commit()
    return jsonify({'status': 'command queued'})

# --- FORCE ALL CLIENTS ---
@app.route('/api/clients/force_all', methods=['POST'])
def force_all_clients():
    for client in Client.query.all():
        client.force_update = True
    db.session.commit()
    return jsonify({'status': 'all clients forced to check updates'})

# --- REGISTER CLIENT ---
@app.route('/api/clients', methods=['POST'])
def add_client():
    data = request.json
    if not data or 'id' not in data:
        return jsonify({'error': 'Invalid data'}), 400

    client = Client.query.get(data['id'])
    
    # If client is found by serial number, we update its information (e.g. reinstall case)
    if not client:
        client = Client.query.filter_by(serial_number=data.get('serial_number')).first()
        if client:
            # Handle case of reinstallation: generate new token, update information
            client.token = generate_token()
            client.client_name = data.get('client_name', client.client_name)
            client.ip_address = request.remote_addr
            db.session.commit()
            return jsonify({'token': client.token})

    if client:
        return jsonify({'error': 'Client already exists'}), 400

    # New client registration
    token = generate_token()
    serial_number = data.get('serial_number')  # Store serial number during registration
    client = Client(
        id=data['id'],
        client_name=data.get('client_name', 'Unnamed Client'),
        ip_address=request.remote_addr,
        token=token,
        serial_number=serial_number  # Save serial number
    )
    db.session.add(client)
    db.session.commit()
    return jsonify({'token': token})

# --- CLIENT UPDATE CHECKIN ---
@app.route('/api/clients/<client_id>', methods=['POST'])
def client_update(client_id):
    client = Client.query.get(client_id)
    if not client:
        return jsonify({'error': 'Client not found'}), 404

    data = request.json
    if not data:
        return jsonify({'error': 'Invalid data'}), 400

    client_token = data.get('token') or request.headers.get('Authorization')
    if not auth_client(client, client_token):
        return jsonify({'error': 'Unauthorized'}), 401

    # --- telemetry ---
    client.client_name = data.get('client_name', client.client_name)
    client.ip_address = request.remote_addr
    client.last_checkin = datetime.utcnow()
    client.os_name = data.get('os_name', client.os_name)
    client.os_version = data.get('os_version', client.os_version)
    client.cpu = data.get('cpu', client.cpu)
    client.ram = data.get('ram', client.ram)
    client.disk_total = data.get('disk_total', client.disk_total)
    client.disk_free = data.get('disk_free', client.disk_free)
    client.uptime_val = data.get('uptime', client.uptime_val)
    client.file_hashes = json.dumps(data.get('file_hashes', {}))

    # --- updates ---
    reported = data.get('updates', None)
    if reported is not None:
        # Clear existing updates
        ClientUpdate.query.filter_by(client_id=client.id).delete()
        client.updates_available = False
        for upd in reported:
            cu = ClientUpdate(
                client_id=client.id,
                kb_or_package=upd.get('kb_or_package'),
                title=upd.get('title'),
                severity=upd.get('severity'),
                status='pending'
            )
            db.session.add(cu)
            client.updates_available = True

    # --- force update response ---
    response = {'approved': client.approved, 'updates_available': client.updates_available, 'online': client.is_online()}
    if client.force_update and client.allow_checkin:
        response['force_check'] = True
        client.force_update = False

    db.session.commit()
    return jsonify(response)

# --- CLIENT PING ---
@app.route('/api/clients/<client_id>/ping', methods=['POST'])
def client_ping(client_id):
    client = Client.query.get(client_id)
    if not client:
        return jsonify({'error': 'Client not found'}), 404
    auth_header = request.headers.get('Authorization', '')
    if not auth_client(client, auth_header):
        return jsonify({'error': 'Unauthorized'}), 401
    client.last_checkin = datetime.utcnow()
    db.session.commit()
    return jsonify({'status': 'pong', 'online': client.is_online()})

if __name__ == '__main__':
    # Initialize database tables if they don't already exist
    with app.app_context():
        print("Initializing database tables...")
        db.create_all()  # Creates tables if they do not exist
        print("Database tables created.")
    
    # Start the Flask application with debug and reloader turned off
    app.run(host='0.0.0.0', port=8080, debug=False, use_reloader=False)


