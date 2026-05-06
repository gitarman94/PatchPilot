# ⚠️ EARLY DEVELOPMENT - EXPECT BREAKAGE

# CommandPilot

CommandPilot is a cross-platform systems management platform designed to monitor devices, execute actions, and provide centralized operational control over infrastructure.

It consists of a lightweight Go-based backend (pilot-core) with a web UI, with future components for agents and CLI tooling.

---

## 🧠 Architecture Overview

CommandPilot (Product)
├── pilot-core     (Go-based server / control plane)
├── pilot-agent    (future endpoint agent)
├── pilot-ui       (web interface)
├── pilot-cli      (future CLI)

---

## 🚀 pilot-core (Server)

pilot-core is the central control plane providing:

- Device inventory and approval workflows
- Action dispatching and lifecycle tracking
- Audit logging and historical records
- Role-based access control (RBAC)
- Web UI dashboard with charts
- REST API for integrations

---

## ✨ Features

- Go-based server (single static binary, minimal dependencies)
- SQLite database (embedded, no external DB required)
- Authentication using bcrypt + session cookies
- Users, roles, and groups
- Device tracking (hostname, IP, OS, last seen)
- Action system with status + timestamps
- Dashboard powered by API-driven charts
- Full audit/history logging
- Native HTML templates (Go html/template)

---

## 📁 Project Structure

commandpilot/
├── pilot-core/
│   ├── main.go
│   ├── db.go
│   ├── models.go
│   ├── devices.go
│   ├── actions.go
│   ├── history.go
│   ├── auth.go
│   ├── users_groups.go
│   ├── roles.go
│   ├── settings.go
│
├── templates/
│   ├── navbar.html
│   ├── dashboard.html
│   ├── devices.html
│   ├── device_detail.html
│   ├── actions.html
│   ├── history.html
│   ├── settings.html
│   ├── users_groups.html
│   ├── roles.html
│   ├── login.html
│   └── audit.html
│
├── static/
│   ├── app.js
│   └── styles.css
│
└── setup_or_update_server.sh

---

## ⚠️ Templates (IMPORTANT)

pilot-core uses Go’s built-in html/template engine.

Rules:
- Templates must use .html
- Do NOT use .hbs or Handlebars syntax
- Use Go template syntax

Example:
{{range .Devices}}
  {{.Hostname}}
{{end}}

If templates are incorrect, you may see:
- Blank pages
- Missing data
- Rendering errors

---

## 🧱 Server Installation (Linux)

Requirements:
- Debian/Ubuntu-based system
- root or sudo access
- internet access

---

## ⚡ Install / Update (One Command)

apt update -y && apt upgrade -y
apt install -y curl
curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash

## Verbose mode

curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --verbose

---

## 🔧 What the Installer Does

- Installs Go (if missing)
- Installs SQLite and build dependencies
- Downloads CommandPilot source
- Builds pilot-core binary
- Creates system user (commandpilot)
- Configures systemd service (pilot-core.service)
- Starts and enables service

---

## 🌐 Access the UI

http://<server-ip>:8080

---

## 🔄 Updating

Re-run the installer:

curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash

This will:
- Pull latest code
- Rebuild the binary
- Restart the service

---

## 📊 Core API Endpoints

/api/devices   → Device list
/api/actions   → Actions list
/api/history   → Audit history

These endpoints power the dashboard charts and UI.

---

## 🗄️ Database

Default location:
/opt/commandpilot/commandpilot.db

Tables:
- devices
- actions
- history
- users
- roles
- groups
- settings

---

## 🔐 Authentication

- bcrypt password hashing
- session-based login
- protected routes via middleware

---

## 🧪 Development Notes

- Single binary deployment (no runtime dependencies)
- No Node, Python, or Rust required
- Templates + static assets served directly
- API and UI tightly coupled

---

## ⚠️ Current Limitations

- Early-stage (expect bugs)
- No migrations yet
- No clustering / high availability
- Basic session handling (not distributed)
- Agent not implemented yet

---

## 🔮 Planned Features

- pilot-agent (endpoint daemon)
- pilot-cli (management CLI)
- TLS support
- API authentication tokens
- WebSocket live updates
- Multi-node support

---

## 📜 License

TBD

---

## 🙋 Support

https://github.com/gitarman94/CommandPilot
