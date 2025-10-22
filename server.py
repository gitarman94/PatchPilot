import os
import json
import base64
from datetime import datetime, timedelta
from flask import Flask, request, jsonify, render_template, abort, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS

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

    # New system info fields
    os = db.Column(db.String(50), nullable=True)
    os_version = db.Column(db.String(50), nullable=True)
    cpu = db.Column(db.String(100), nullable=True)
    ram = db.Column(db.String(50), nullable=True)
    disk = db.Column(db.String(100), nullable=True)

class ClientUpdate(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    client_id = db.Column(db.String(36), db.ForeignKey('client.id'), nullable=False)
    kb_or_package = db.Column(db.String(200), nullable=False)
    title = db.Column(db.String(200), nullable=True)
    severity = db.Column(db.String(50), nullable=True)
    status = db.Column(db.String(50), nullable=False, default='pending')  # pending/installing/installed/failed
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)

with app.app_context():
    db.create_all()

# == Helpers ==
def generate_token():
    return base64.urlsafe_b64encode(os.urandom(24)).decode()

def auth_client(client, token):
    if token is None:
        return False
    if token.startswith("Bearer "):
        token = token[7:]
    return token == client.token

def client_online(client):
    """Return True if last_checkin within 3 minutes (3 cycles of 1 minute)"""
    if not client.last_checkin:
        return False
    return datetime.utcnow() - client.last_checkin <= timedelta(minutes=3)

# == ROUTES ==

@app.route('/')
def index():
    clients = Client.query.all()
    return render_template('client.html', clients=clients, now=datetime.utcnow())

@app.route('/api/clients', methods=['POST'])
def add_client():
    data = request.json
    if not data or 'id' not in data:
        return jsonify({'error': 'Invalid data'}), 400

    client = Client.query.get(data['id'])
    if client:
        return jsonify({'error': 'Client already exists'}), 400

    token = generate_token()
    client = Client(
        id=data['id'],
        client_name=data.get('client_name', 'Unnamed Client'),
        ip_address=request.remote_addr,
        token=token,
        approved=False,
        allow_checkin=True,
        force_update=False,
        updates_available=False
    )
    db.session.add(client)
    db.session.commit()
    return jsonify({'token': token})

@app.route('/api/clients/<client_id>', methods=['POST'])
def client_update(client_id):
    data = request.json
    if not data:
        return jsonify({'error': 'Invalid data'}), 400

    client = Client.query.get(client_id)
    if not client:
        return jsonify({'error': 'Client not found'}), 404

    client_token = data.get('token') or request.headers.get('Authorization')
    if not auth_client(client, client_token):
        return jsonify({'error': 'Unauthorized'}), 401

    # Update system info and last_checkin
    client.client_name = data.get('client_name', client.client_name)
    client.ip_address = request.remote_addr
    client.last_checkin = datetime.utcnow()
    client.os = data.get('os', client.os)
    client.os_version = data.get('os_version', client.os_version)
    client.cpu = data.get('cpu', client.cpu)
    client.ram = data.get('ram', client.ram)
    client.disk = data.get('disk', client.disk)
    client.file_hashes = json.dumps(data.get('file_hashes', {}))

    # Handle updates from client
    reported = data.get('updates', None)
    if reported is not None:
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

    # Force check flag
    response = {'approved': client.approved, 'updates_available': client.updates_available}
    if client.force_update and client.allow_checkin:
        response['force_check'] = True
        client.force_update = False

    db.session.commit()
    return jsonify(response)

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
    return jsonify({'status': 'pong'})

@app.route('/approve/<client_id>', methods=['POST'])
def approve_client(client_id):
    client = Client.query.get(client_id)
    if not client:
        abort(404)
    client.approved = True
    db.session.commit()
    return ('', 204)

# Serve updates
@app.route('/updates/<path:filename>', methods=['GET'])
def serve_update_file(filename):
    return send_from_directory(UPDATE_CACHE_DIR, filename, as_attachment=True)

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
