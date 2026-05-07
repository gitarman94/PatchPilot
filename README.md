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
```

---

## 🚀 pilot-core

`pilot-core` is the central control plane responsible for:

- Device inventory management
- Device approval workflows
- Action dispatch and tracking
- Historical event logging
- Audit visibility
- Role-based access control (RBAC)
- Administrative web interface
- REST API integrations
- Session authentication
- Platform configuration management

---

## ✨ Features

- Go-based server
- Single static binary deployment
- SQLite embedded database
- Session-based authentication
- Secure bcrypt password hashing
- Server-side session tracking
- User self-service password changes
- Users, roles, and groups
- Device tracking and inventory
- Action lifecycle tracking
- Dashboard metrics and charts
- Audit and history visibility
- Native Go HTML templates
- Minimal runtime dependencies
- systemd service integration
- Install and upgrade automation
- Schema migration support

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

---

## 🧱 Server Requirements

- Debian or Ubuntu
- Root or sudo access
- Internet connectivity

---

## ⚡ Installation

### Initial Install

```bash
apt update -y && apt upgrade -y
apt install -y curl

curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --install
```

### Upgrade Existing Deployment

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/CommandPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --upgrade
```

---

## 🔍 Installer Modes

### Install Mode

`--install`

Performs a full initial deployment:

- installs dependencies
- installs Go
- clones repository
- builds binary
- deploys templates/static assets
- initializes database
- creates default admin account
- creates and enables systemd service
- validates runtime startup

### Upgrade Mode

`--upgrade`

Performs a non-destructive upgrade:

- preserves existing database
- preserves users and passwords
- preserves settings/configuration
- applies schema migrations safely
- rebuilds binaries
- restarts services
- validates runtime health

### Optional Flags

#### Verbose Output

```bash
--verbose
```

Displays:

- install stages
- executed commands
- validation steps
- service checks

#### Deep Debug Output

```bash
--veryverbose
```

Displays additional diagnostics:

- systemd logs
- process state
- listening ports
- filesystem validation
- template discovery
- runtime paths

#### Force Template Replacement

```bash
--force-templates
```

Replaces runtime templates during upgrade.

#### Force Configuration Replacement

```bash
--force-config
```

Reserved for future configuration management logic.

---

## 🔧 Installer Responsibilities

The installer performs staged validation and exits immediately on failure.

### Validation Stages

1. Dependency installation
2. Go installation validation
3. Repository synchronization
4. Binary compilation
5. Runtime deployment
6. Database migration validation
7. Asset validation
8. systemd service validation
9. HTTP validation
10. Authentication validation

---

## 🌐 Web Interface

Default URL:

```text
http://<server-ip>:8080
```

---

## 🔑 Default Credentials

Initial installs create a default administrative account:

```text
Username: admin
Password: admin
```

You should immediately change the password after first login.

---

## 🔄 Upgrade Behavior

Upgrade mode is designed to be non-destructive.

### Preserved During Upgrade

- database contents
- users
- password hashes
- sessions
- settings
- runtime state

### Updated During Upgrade

- Go binary
- templates
- static assets
- systemd service definitions
- schema migrations

---

## 📊 Core API Endpoints

```text
/api/devices
/api/actions
/api/history
```

Used by:

- dashboard widgets
- charts
- administrative tables
- UI refresh logic

---

## 🗄️ Database

Default database location:

```text
/opt/commandpilot/pilot-core/commandpilot.db
```

### Core Tables

- devices
- actions
- history
- users
- roles
- groups
- user_groups
- settings
- sessions
- schema_migrations

### Database Engine

- SQLite embedded database
- no external DBMS required
- single-file deployment model

---

## 🔐 Authentication & Security

### Current Security Model

- bcrypt password hashing
- server-side session tracking
- HttpOnly session cookies
- authenticated route middleware
- user password change support
- role-aware administration

### Password Storage

Passwords are never stored in plaintext.

User passwords are stored as bcrypt hashes inside:

```text
users.password_hash
```

### Session Handling

Authenticated sessions are tracked using server-side session tokens stored in:

```text
sessions
```

---

## 🧪 Development Notes

- Single binary deployment
- No Node.js required
- No Rust required
- No external database required
- Static assets served directly
- Templates rendered server-side
- API and UI tightly integrated
- SQLite-backed persistence
- Native Go HTTP server

---

## ⚠️ Current Limitations

- Early-stage project
- Limited schema migration system
- No HA clustering
- No TLS termination
- No MFA support
- Agent component incomplete
- RBAC still evolving
- Limited frontend validation
- No API token support yet

---

## 🔮 Planned Features

- `pilot-agent`
- `pilot-cli`
- TLS support
- API tokens
- MFA support
- Live WebSocket updates
- Multi-node support
- Centralized event streaming
- Remote command execution
- Policy management
- Agent enrollment workflows
- Event pipelines
- Plugin architecture

---

## 📜 License

TBD

---

## 🙋 Repository

GitHub:

```text
https://github.com/gitarman94/CommandPilot
```