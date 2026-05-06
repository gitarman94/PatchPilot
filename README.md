# ⚠️ EARLY DEVELOPMENT - EXPECT BREAKAGE

# Kentro

**Kentro** is a cross-platform systems management platform designed to monitor devices, manage actions, and provide centralized control over infrastructure.

It consists of a lightweight backend server (**KentroCore**) with a web UI, and future components for agents and CLI tooling.

---

## 🧠 Architecture Overview

```
Kentro (Product)
├── KentroCore      (Go-based server / control plane)
├── KentroAgent     (future endpoint agent)
├── KentroUI        (web interface)
├── KentroCLI       (future CLI)
```

---

## 🚀 KentroCore (Server)

KentroCore is a **Go-based backend server** that provides:

* Device inventory and approval workflow
* Action dispatch and tracking
* Audit history and logging
* Role-based access control (RBAC)
* Web UI with dashboard + charts
* REST API for integrations

---

## ✨ Features

* ⚡ **Go-based server** (fast, simple deployment, single binary)
* 🗄️ **SQLite database** (no external DB required)
* 🔐 Authentication with bcrypt + session handling
* 👥 Users, roles, and groups
* 📡 Device tracking (hostname, IP, OS, last seen)
* 🧾 Action system with status + timestamps
* 📊 Dashboard with live API-fed charts
* 📜 Full audit/history logging
* 🌐 Native HTML templates (no template engine dependency)

---

## 📁 Project Structure

```
kentro/
│
├── kentrocore/               # Go backend
│   ├── main.go
│   ├── db.go
│   ├── models.go
│   ├── devices.go
│   ├── actions.go
│   ├── history.go
│   ├── auth.go
│   ├── users.go
│   ├── roles.go
│   ├── settings.go
│
├── templates/                # HTML templates (Go html/template)
│   ├── navbar.html
│   ├── dashboard.html
│   ├── devices.html
│   ├── device_detail.html
│   ├── actions.html
│   ├── history.html
│   ├── settings.html
│   ├── users_groups.html
│   ├── roles.html
│   └── login.html
│
├── static/
│   ├── app.js
│   └── styles.css
│
└── setup_or_update_server.sh
```

---

## ⚠️ Templates (IMPORTANT)

KentroCore uses Go’s built-in:

```
html/template
```

### Rules:

* Templates must be `.html`
* No `.hbs` or Handlebars syntax
* Use Go template syntax:

```
{{range .Devices}}
{{.Hostname}}
{{end}}
```

If templates are incorrect, you will see:

* blank pages
* missing data
* or rendering errors

---

## 🧱 Server Installation (Linux)

### Requirements

* Debian/Ubuntu-based system
* root/sudo access
* internet access

---

## ⚡ Install / Update (One Command)

```bash
apt-get update
apt-get install -y curl
curl -fsSL https://raw.githubusercontent.com/gitarman94/kentro/main/setup_or_update_server.sh | bash
```

---

### What the installer does

* Installs Go (if missing)
* Installs SQLite + build tools
* Downloads Kentro source from GitHub
* Builds `kentrocore` binary
* Creates system user (`kentro`)
* Sets up systemd service
* Starts server automatically

---

## 🌐 Access the UI

After install:

```
http://<server-ip>:8080
```

---

## 🔄 Updating

Re-run the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/kentro/main/setup_or_update_server.sh | bash
```

This will:

* pull latest code
* rebuild
* restart service

---

## 📊 Core API Endpoints

| Endpoint       | Description   |
| -------------- | ------------- |
| `/api/devices` | Device list   |
| `/api/actions` | Actions list  |
| `/api/history` | Audit history |

Used by dashboard charts and UI.

---

## 🗄️ Database

SQLite file:

```
/opt/kentro/kentro.db
```

Tables include:

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
* session-based login
* protected routes via middleware

---

## 🧪 Development Notes

* No external runtime dependencies (single binary)
* No Node, no Python, no Rust required
* Templates + static files served directly
* API + UI tightly coupled

---

## ⚠️ Current Limitations

* Early-stage (expect bugs)
* No migrations yet
* No clustering / HA
* Sessions are basic (no distributed store)
* Agent not implemented yet

---

## 🔮 Planned Features

* KentroAgent (endpoint daemon)
* CLI management tool
* TLS support
* API authentication tokens
* Real-time updates (WebSockets)
* Multi-node support

---

## 📜 License

TBD

---

## 🙋 Support

Open issues on GitHub:
https://github.com/gitarman94/kentro
