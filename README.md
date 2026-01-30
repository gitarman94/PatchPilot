# EARLY DEVELOPMENT - WILL HAVE MANY BUGS

# PatchPilot

PatchPilot is a **cross-platform patch management client** designed to monitor, report, and deploy software updates on Windows and Linux systems. It includes a lightweight Rust-based client with **self-updating capabilities** and a Rust-based backend server with authentication, role-based access, and audit logging.

---

## Features

* 🦀 **Rust-based client** for performance and reliability
* 🖥️ Runs as **Windows Service** or **Linux systemd service**
* 🔄 **Self-updating client** via GitHub releases
* 🔒 Secure: runs under a **non-root system user** on Linux
* 📡 Reports **missing updates, system info, and command results** to the central server
* ⚙️ Configurable patch server address per client
* 🛡 Role-based access control (RBAC) and authentication for server
* 📜 Audit logging and history tracking
* 🌐 Rocket-served web UI

---

## Project Structure

```

PatchPilot/
│
├── patchpilot_server/                 # Rust-based backend server
│   ├── Cargo.toml                     # Server dependencies & config
│   │
│   └── src/
│       ├── main.rs                    # Rocket entry point
│       ├── state.rs                   # AppState (system info, pending devices, settings)
│       ├── settings.rs                # ServerSettings load/save
│       ├── models.rs                  # Diesel models (Device, Action, AuditLog, User, Role, etc.)
│       ├── schema.rs                  # Diesel schema for database tables
│       ├── db.rs                      # Database pool & initialization
│       ├── action_ttl.rs              # Expire old actions
│       ├── pending_cleanup.rs         # Cleanup pending devices
│       │
│       ├── routes/                    # HTTP routes (API + pages)
│       │   ├── mod.rs                 # api_routes() + page_routes()
│       │   ├── devices.rs             # Device registration, heartbeat, listing
│       │   ├── actions.rs             # Action creation and completion
│       │   ├── settings.rs            # Server settings API
│       │   ├── history.rs             # Audit/history API
│       │   ├── auth.rs                # Authentication endpoints
│       │   ├── users_groups.rs        # User and group management API
│       │   └── roles.rs               # Role-based permissions API
│       │
│       └── logger.rs                  # Diesel / app logging
│
├── patchpilot_client/                 # Rust client (Windows & Linux)
│   ├── Cargo.toml
│   │
│   └── src/
│       ├── main.rs                    # Client entry point
│       ├── service.rs                 # Windows service / Unix daemon glue
│       ├── system_info.rs             # CPU, RAM, disk, OS, network
│       ├── device.rs                  # Register, adopt, heartbeat
│       ├── action.rs                  # CommandSpec, ServerCommand, CommandResult
│       ├── command.rs                 # Polling, retries, result posting
│       ├── self_update.rs             # Client self-update logic
│       └── patchpilot_updater.rs      # Apply updates + restart
│
├── templates/                         # Rocket Handlebars templates
│   ├── navbar.hbs                     # Sidebar navigation
│   ├── dashboard.hbs                  # Main dashboard
│   ├── device_detail.hbs              # Single device view
│   ├── settings.hbs                   # Server and client policy settings
│   ├── devices.hbs                    # Table of all devices
│   ├── history.hbs                    # Audit/history page
│   ├── actions.hbs                    # List and manage actions
│   └── audit.hbs                      # Detailed audit log view
│
└── static/                            # Static web assets
├── bootstrap.min.css
├── bootstrap.bundle.min.js
├── navbar.css
└── favicon.ico

```

---

## ⚠️ Template Naming (IMPORTANT)

The PatchPilot server uses **Rocket + `rocket_dyn_templates`** with the **Handlebars engine**.

**All templates must use the `.hbs` extension.**

`.html` templates will **not be discovered** by Rocket and will cause runtime errors such as:

```

Template 'dashboard' does not exist

```

Rename the following files **before committing**:

```

templates/navbar.html        → navbar.hbs
templates/dashboard.html     → dashboard.hbs
templates/device_detail.html → device_detail.hbs
templates/settings.html      → settings.hbs
templates/devices.html       → devices.hbs
templates/history.html       → history.hbs
templates/actions.html       → actions.hbs
templates/audit.html         → audit.hbs

````

No route changes are required — Rocket resolves templates by name, not extension.

---

## 🚀 Server Setup (Linux)

### Prerequisites

* Rust toolchain (installed automatically in the setup script)
* Git
* `systemd` for automatic restart

### Install/Update in One Command

```bash
apt-get update
apt-get install -y curl git
curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --debug --force
````

This will:

* Install dependencies
* Download or update the server
* Initialize the database
* Set up a systemd service
* Start and enable it on boot

**Force reinstall:**

```bash
curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/refs/heads/main/setup_or_update_server.sh | bash -s -- --debug --force
```

---

## 💻 Client Setup (Linux)

### Requirements

* Ubuntu/Debian
* sudo/root access
* Internet connection

### Install/Update in One Command

```bash
apt-get update
apt-get install -y curl git
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)"
```

* Installs Rust if missing
* Builds and installs the Rust client
* Creates `patchpilot` system user
* Configures systemd service
* Supports auto-updates

**Update client:**

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)" -- --update
```

**Uninstall client:**

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)" -- --uninstall
```

---

## 🪟 Client Setup (Windows)

### Requirements

* Windows 10/11
* Admin privileges

### Install/Update

```powershell
irm https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.ps1 | iex
```

* Installs Rust toolchain if missing
* Builds client with `cargo`
* Registers Windows service
* Sets up config and auto-update

---

## 🔧 Configuration

All clients (Linux & Windows) store:

* `server_url.txt` → Patch server URL
* `client_id.txt` → Client ID (auto-generated)
* Optional `config.json` → Custom client settings

Edit server URL:

```bash
sudo nano /opt/patchpilot_client/server_url.txt
# Windows:
notepad "C:\ProgramData\RustPatchClient\server_url.txt"
```

Restart the client service after edits.

---

## 📋 Check Status

**Linux:**

```bash
systemctl status patchpilot_client.timer
journalctl -u patchpilot_client.service
```

**Windows:**

```powershell
Get-Service RustPatchClientService
```

---

## 🛠 Developer Info

* Rust-based client shared across OSes
* Self-updates from GitHub Releases using version/tag logic
* Platform-specific system info collected via PowerShell or Rust crates
* Communication via REST API to Rust-based server
* Server includes authentication, roles, RBAC, and audit logging
* Web UI rendered by Rocket using Handlebars templates

---

## 📜 License

Dual licensing:

* **Free for Personal Use** – Free to use, modify, and distribute for non-commercial purposes
* **Commercial Use** – Paid license required for commercial use

See full license terms in the `LICENSE` file.

---

## 🙋 Contact

Questions or bugs? Open an issue on GitHub.

```
