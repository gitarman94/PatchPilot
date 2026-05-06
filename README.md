# ⚠️ EARLY DEVELOPMENT — EXPECT BREAKAGE

# CommandPilot

CommandPilot is a cross-platform systems management platform designed to monitor devices, execute actions, and provide centralized operational control over infrastructure.

It consists of a lightweight Go-based backend (`pilot-core`) with a web UI, with future components for agents and CLI tooling.

---

## 🧠 Architecture Overview

```text
CommandPilot
├── pilot-core     (Go server / control plane)
├── pilot-agent    (future endpoint agent)
├── pilot-ui       (web interface)
└── pilot-cli      (future CLI)
````

---

## 🚀 pilot-core

`pilot-core` is the central control plane responsible for:

* Device inventory management
* Device approval workflows
* Action dispatch and tracking
* Historical event logging
* Audit visibility
* Role-based access control (RBAC)
* Administrative web interface
* REST API integrations

---

## ✨ Features

* Go-based server
* Single static binary deployment
* SQLite embedded database
* Session authentication
* bcrypt password hashing
* Users, roles, and groups
* Device tracking and inventory
* Action lifecycle tracking
* Dashboard metrics and charts
* Audit and history visibility
* Native Go HTML templates
* Minimal runtime dependencies

---

## 📁 Project Structure

```text
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
│   └── utils.go
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
├── setup_or_update_server.sh
└── server_test.sh
```

## 🧱 Server Requirements

* Debian or Ubuntu
* Root or sudo access
* Internet connectivity

---

## ⚡ Install / Update

```bash
apt update -y && apt upgrade -y
apt install -y curl

curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash
```

---

## 🔍 Verbose Installer Modes

### Standard Verbose

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --verbose
```

Displays:

* install stages
* executed commands
* validation steps
* service checks

### Deep Debug Mode

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --veryverbose
```

Displays additional diagnostics:

* systemd status
* journal logs
* process state
* listening ports
* template discovery
* filesystem validation
* working directories

---

## 🔧 Installer Responsibilities

The installer performs staged validation and exits immediately on failure.

### Stages

1. Dependency installation
2. Go installation validation
3. Repository retrieval
4. Binary compilation
5. Static/template asset validation
6. systemd service creation
7. Service startup validation
8. HTTP validation
9. Database validation
10. Authenticated endpoint validation

---

## 🌐 Web Interface

Default URL:

```text
http://<server-ip>:8080
```

---

## 🔄 Updating

Re-run the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash
```

Update flow:

* pulls latest source
* rebuilds binary
* replaces service binary
* validates startup
* validates endpoints

---

## 📊 Core API Endpoints

```text
/api/devices
/api/actions
/api/history
```

Used by:

* dashboard widgets
* charts
* administrative tables
* UI refresh logic

---

## 🗄️ Database

Default database location:

```text
/opt/commandpilot/commandpilot.db
```

### Core Tables

* devices
* actions
* history
* users
* roles
* groups
* settings

---

## 🔐 Authentication

* bcrypt password hashing
* session cookie authentication
* middleware-protected routes
* role-aware administration

---

## 🧪 Development Notes

* Single binary deployment
* No Node.js required
* No Rust required
* No external database required
* Static assets served directly
* Templates rendered server-side
* API and UI tightly integrated

---

## ⚠️ Current Limitations

* Early-stage project
* No database migrations
* No HA clustering
* Basic session handling
* Agent component incomplete
* RBAC still evolving
* Limited frontend validation

---

## 🔮 Planned Features

* `pilot-agent`
* `pilot-cli`
* TLS support
* API tokens
* Live WebSocket updates
* Multi-node support
* Centralized event streaming
* Remote command execution
* Policy management

---

## 📜 License

TBD

---

## 🙋 Repository

GitHub:

```text
https://github.com/gitarman94/CommandPilot
```

```
```
