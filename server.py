#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import os
import json
import time
from datetime import datetime, timedelta

from flask import Flask, request, jsonify, render_template, Response
from flask_sqlalchemy import SQLAlchemy
from flask_cors import CORS
from sqlalchemy import create_engine, inspect
from sqlalchemy.exc import OperationalError

app = Flask(__name__)
CORS(app)

SERVER_DIR = "/opt/patchpilot_server"
UPDATE_CACHE_DIR = os.path.join(SERVER_DIR, "updates")
os.makedirs(UPDATE_CACHE_DIR, exist_ok=True)

SQLITE_DB_PATH = os.path.join(SERVER_DIR, "patchpilot.db")
app.config["SQLALCHEMY_DATABASE_URI"] = f"sqlite:///{SQLITE_DB_PATH}"
app.config["SQLALCHEMY_TRACK_MODIFICATIONS"] = False
db = SQLAlchemy(app)

def load_admin_token() -> str:
    token = os.getenv("ADMIN_TOKEN")
    if token:
        return token
    token_file = os.path.join(SERVER_DIR, "admin_token.txt")
    if os.path.exists(token_file):
        with open(token_file) as f:
            return f.read().strip()
    return ""

ADMIN_TOKEN = load_admin_token()

class Client(db.Model):
    __tablename__ = "client"

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

    os_name = db.Column(db.String(50))
    os_version = db.Column(db.String(50))
    cpu = db.Column(db.String(100))
    ram = db.Column(db.String(50))
    disk_total = db.Column(db.String(50))
    disk_free = db.Column(db.String(50))
    uptime_val = db.Column("uptime", db.String(50))
    serial_number = db.Column(db.String(50), unique=True, nullable=True)

    def is_online(self) -> bool:
        if not self.last_checkin:
            return False
        return datetime.utcnow() - self.last_checkin <= timedelta(minutes=3)

    def uptime(self):
        return self.uptime_val or "N/A"

class ClientUpdate(db.Model):
    __tablename__ = "client_update"

    id = db.Column(db.Integer, primary_key=True)
    client_id = db.Column(db.String(36), db.ForeignKey("client.id"), nullable=False, index=True)
    kb_or_package = db.Column(db.String(200), nullable=False)
    title = db.Column(db.String(200), nullable=True)
    severity = db.Column(db.String(50), nullable=True)
    status = db.Column(db.String(50), nullable=False, default="pending")
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)

def init_db():
    try:
        engine = create_engine(app.config["SQLALCHEMY_DATABASE_URI"])
        engine.connect()
        inspector = inspect(engine)
        if not inspector.has_table("client"):
            db.create_all()
    except OperationalError as e:
        app.logger.error(f"DB init error: {e}")

with app.app_context():
    init_db()

def sse_generator():
    while True:
        data = {
            "ts": int(time.time()),
            "clients": [
                {"id": c.id, "online": c.is_online(), "updates": c.updates_available}
                for c in Client.query.all()
            ]
        }
        yield f"data: {json.dumps(data)}\n\n"
        time.sleep(5)

@app.route("/sse/clients")
def sse_clients():
    return Response(sse_generator(), mimetype="text/event-stream")

from sqlalchemy.exc import OperationalError

@app.route("/admin/force-reinstall/<client_id>", methods=["POST"])
def force_reinstall_client(client_id):
    try:
        client = Client.query.get(client_id)
    except OperationalError:
        # Table doesn't exist yet
        return "Database table not initialized", 500

    if not client:
        # No client with that ID yet
        return "Client not found", 404

    client.force_update = True
    db.session.commit()
    return "", 204

@app.route("/api/clients")
def api_clients():
    draw = int(request.args.get("draw", "1"))
    start = int(request.args.get("start", "0"))
    length = int(request.args.get("length", "10"))
    search = request.args.get("search[value]", "").lower()

    query = Client.query
    if search:
        like = f"%{search}%"
        query = query.filter(
            db.or_(
                Client.id.ilike(like),
                Client.client_name.ilike(like),
                Client.os_name.ilike(like),
                Client.cpu.ilike(like),
                Client.ram.ilike(like)
            )
        )
    total = query.count()
    rows = query.order_by(Client.id).offset(start).limit(length).all()

    now = datetime.utcnow()
    data = []
    for c in rows:
        data.append([
            c.id,
            c.client_name or "—",
            c.os_name or "—",
            c.cpu or "—",
            c.ram or "—",
            f"{c.disk_total or '—'}/{c.disk_free or '—'}",
            "✅" if c.updates_available else "⚠️",
            c.uptime(),
            "🟢" if c.is_online() else "🔴",
            "Yes" if c.updates_available else "No",
            c.id
        ])

    return jsonify(draw=draw,
                   recordsTotal=total,
                   recordsFiltered=total,
                   data=data)

@app.route("/")
def index():
    clients = Client.query.all()
    return render_template("dashboard.html",
                           clients=clients,
                           now=datetime.utcnow(),
                           ADMIN_TOKEN=ADMIN_TOKEN)

@app.route("/clients/<client_id>")
def client_detail(client_id):
    client = Client.query.get_or_404(client_id)
    updates = ClientUpdate.query.filter_by(client_id=client_id).all()
    return render_template("client_detail.html",
                           client=client,
                           updates=updates,
                           ADMIN_TOKEN=ADMIN_TOKEN,
                           now=datetime.utcnow())

@app.route("/approve/<client_id>", methods=["POST"])
def approve_client(client_id):
    client = Client.query.get_or_404(client_id)
    client.approved = True
    db.session.commit()
    return ("", 204)

@app.route("/admin/force-update/<client_id>", methods=["POST"])
def force_update_client(client_id):
    client = Client.query.get_or_404(client_id)
    client.force_update = True
    db.session.commit()
    return ("", 204)

@app.route("/admin/allow-checkin/<client_id>", methods=["POST"])
def allow_checkin_client(client_id):
    client = Client.query.get_or_404(client_id)
    client.allow_checkin = True
    db.session.commit()
    return ("", 204)

@app.route("/api/clients/force_all", methods=["POST"])
def force_all_clients():
    payload = request.get_json(silent=True) or request.form
    if not auth_admin(payload):
        return jsonify({"error": "Unauthorized"}), 401
    for c in Client.query.all():
        c.force_update = True
    db.session.commit()
    return jsonify({"status": "all clients forced to update"})

@app.route("/api/clients/<client_id>/commands", methods=["POST"])
def send_command(client_id):
    client = Client.query.get_or_404(client_id)
    payload = request.get_json(silent=True) or request.form
    if not auth_admin(payload):
        return jsonify({"error": "Unauthorized"}), 401

    action = payload.get("action")
    updates = (payload.getlist("updates") if hasattr(payload, "getlist") else payload.get("updates", []))

    if action == "install_selected_updates" and updates:
        for upd in updates:
            cu = ClientUpdate.query.filter_by(client_id=client.id, kb_or_package=upd).first()
            if cu:
                cu.status = "installing"
    elif action == "install_all_updates":
        for cu in ClientUpdate.query.filter_by(client_id=client.id, status="pending").all():
            cu.status = "installing"
    else:
        return jsonify({"error": "Unknown action"}), 400

    db.session.commit()
    return jsonify({"status": "command queued"})

@app.route("/api/clients", methods=["POST"])
def add_client():
    data = request.get_json(silent=True)
    if not data or "id" not in data:
        return jsonify({"error": "Invalid data"}), 400

    client = Client.query.get(data["id"])

    if not client:
        client = Client.query.filter_by(serial_number=data.get("serial_number")).first()
        if client:
            client.token = generate_token()
            client.client_name = data.get("client_name", client.client_name)
            client.ip_address = request.remote_addr
            db.session.commit()
            return jsonify({"token": client.token})

    if client:
        return jsonify({"error": "Client already exists"}), 400

    token = generate_token()
    client = Client(
        id=data["id"],
        client_name=data.get("client_name", "Unnamed Client"),
        ip_address=request.remote_addr,
        token=token,
        serial_number=data.get("serial_number"),
    )
    db.session.add(client)
    db.session.commit()
    return jsonify({"token": token})

@app.route("/api/clients/<client_id>", methods=["POST"])
def client_update(client_id):
    client = Client.query.get_or_404(client_id)

    payload = request.get_json(silent=True)
    if not payload:
        return jsonify({"error": "Invalid data"}), 400

    token = payload.get("token") or request.headers.get("Authorization")
    if token != client.token:
        return jsonify({"error": "Unauthorized"}), 401

    client.client_name = payload.get("client_name", client.client_name)
    client.ip_address = request.remote_addr
    client.last_checkin = datetime.utcnow()
    client.os_name = payload.get("os_name", client.os_name)
    client.os_version = payload.get("os_version", client.os_version)
    client.cpu = payload.get("cpu", client.cpu)
    client.ram = payload.get("ram", client.ram)
    client.disk_total = payload.get("disk_total", client.disk_total)
    client.disk_free = payload.get("disk_free", client.disk_free)
    client.uptime_val = payload.get("uptime", client.uptime_val)

    db.session.commit()
    return jsonify({"status": "checked in successfully"})

@app.route("/api/health", methods=["GET"])
def health_check():
    return jsonify({"status": "ok"})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8080, debug=False, use_reloader=False)


