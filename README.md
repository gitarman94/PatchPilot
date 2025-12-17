
# EARLY DEVELOPMENT - WILL HAVE MANY BUGS

# PatchPilot

PatchPilot is a cross-platform patch client designed to manage and report software updates on Windows and Linux systems. It includes a lightweight Rust-based client with self-updating capabilities and a Rust-based backend server.

---

## Features

* 🦀 **Rust-based client** for speed and reliability
* 🖥️ Works as a **Windows Service** and **Linux systemd service**
* 🔄 **Self-updating** client from GitHub
* 🔒 Secure, runs under a **non-root system user on Linux**
* 📡 Reports missing updates and system info to central server
* ⚙️ Configurable patch server address per client

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
│       │
│       ├── state.rs                   # AppState (system, pending devices, settings)
│       ├── settings.rs                # ServerSettings load/save
│       │
│       ├── models.rs                  # Diesel models (Device, Action, AuditLog, etc.)
│       ├── schema.rs                  # Diesel schema (devices, actions, audit_log)
│       │
│       ├── routes/                    # HTTP routes (API + pages)
│       │   ├── mod.rs                 # api_routes() + page_routes()
│       │   ├── devices.rs             # Device registration, heartbeat, listing
│       │   ├── actions.rs             # Action creation, completion
│       │   ├── settings.rs            # Server settings API
│       │   ├── pages.rs               # HTML page api handlers
│       │   └── history.rs             # Audit / history API
│       │
│       ├── tasks/                     # Background jobs
│       │   ├── mod.rs
│       │   ├── action_ttl.rs           # Expire old actions
│       │   └── pending_cleanup.rs      # Cleanup pending devices
│       │
│       ├── db/                        # Database plumbing
│       │   ├── mod.rs
│       │   ├── pool.rs                # DbPool + init_pool()
│       │   ├── init.rs                # initialize_db()
│       └── └── logger.rs              # Diesel / app logging
│
├── patchpilot_client/                 # Rust client (Windows & Linux)
│   ├── Cargo.toml
│   │
│   └── src/
│       ├── main.rs                    # Client entry point
│       ├── service.rs                 # Windows service / Unix daemon glue
│       │
│       ├── system_info.rs             # CPU, RAM, disk, OS, network
│       ├── device.rs                  # Register, adopt, heartbeat
│       │
│       ├── action.rs                  # CommandSpec, ServerCommand, CommandResult
│       ├── command.rs                 # Polling, retries, result posting
│       ├── remote_cmd.rs              # Shell / PowerShell execution
│       │
│       ├── self_update.rs             # Client self-update logic
│       └── patchpilot_updater.rs      # Apply updates + restart
│
├── templates/                         # Rocket HTML templates
│   ├── navbar.html                    # Sidebar navigation
│   ├── dashboard.html                 # Main dashboard
│   ├── device_detail.html             # Single device view
│   ├── settings.html                  # server and client policy Settings
│   ├── devices.html                   # Table of all devices and basic information about each
│   └── history.html                   # Audit / history page
│
└── static/                            # Static web assets
    ├── bootstrap.min.css
    ├── bootstrap.bundle.min.js
    ├── navbar.css
    └── favicon.ico

````

---

## 🚀 Server Setup (Linux)

### Prerequisites

* Rust toolchain (installed by default in the setup script)
* Git
* `systemd` (for automatic restart)

### Install/Update in One Command

```bash
# Remove sudo at the beginning of lines if you're running as root
sudo apt-get update
sudo apt-get install -y curl git
sudo bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/refs/heads/main/setup_or_update_server.sh)"
````

This will:

* Install necessary dependencies

* Download/Update the server

* Set up systemd service

* Start and enable it on boot

* If you need to force reinstall:

```bash
sudo bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/refs/heads/main/setup_or_update_server.sh)" -- --force
```

---

## 💻 Client Setup (Linux)

### Requirements

* Ubuntu/Debian
* sudo/root access
* Internet connection

### Install/Update in One Command

```bash
sudo apt-get update
sudo apt-get install -y curl git
sudo bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)"
```

* Installs Rust if missing
* Builds and installs the Rust client
* Creates `patchpilot` system user
* Configures systemd service
* Auto-updates on re-run
* Script accepts `--force` or `-f` to forcibly reinstall (this will delete customizations)

### Update (Linux)

To Update the Linux client:

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)" -- --update
```

---

### Uninstallation (Linux)

To uninstall the Linux client completely:

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)" -- --uninstall
```

---

## 🪟 Client Setup (Windows)

### Requirements

* Windows 10/11
* Admin privileges

### Install/Update in One PowerShell Command

```powershell
irm https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.ps1 | iex
```

* Installs Rust toolchain if missing
* Builds the client using `cargo`
* Registers Windows service
* Sets up config and auto-update

---

## 🔧 Configuration

All clients (Linux & Windows) store:

* Patch server URL → `server_url.txt`
* Client ID (auto-generated) → `client_id.txt`
* Optional config file → `config.json`

To change the server URL:

```bash
sudo nano /opt/patchpilot_client/server_url.txt
# Or for Windows:
notepad "C:\ProgramData\RustPatchClient\server_url.txt"
```

Restart the service/timer after edits.

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
* Communication via REST API to the Rust-based server

---

## 📜 License

License Overview
This project is licensed under a dual licensing model:

* Free for Personal Use: Individuals may use, modify, and distribute this software without cost for personal, non-commercial purposes.
  
* Paid License for Commercial Use:Organizations or individuals intending to use this software for commercial purposes must obtain a paid license.
Terms of Use

1. Grant of License
You are granted a non-exclusive, worldwide license to use this software under the following terms:
    1. Personal Use: Individuals may download, install, and utilize this software without any payment or registration for personal, non-commercial purposes.
    2. Commercial Use: Businesses and organizations must contact me to negotiate a paid license before using the software for any commercial purpose.
    
2. Definitions
    * Personal Use: Refers to use by an individual for non-commercial, personal purposes.
    * Commercial Use: Refers to use by businesses, organizations, or any activities conducted for profit.
  
3. Payment Terms
    * Organizations utilizing the software for commercial purposes must pay the licensing fee determined by me. Payment details will be provided upon inquiry.
  
4. Compliance
    * Users must comply with this license agreement. Violation of these terms may result in termination of the license.
    * I reserve the right to enforce this agreement through appropriate legal actions.
      
5. Disclaimer
This software is provided "as-is," without warranty of any kind. I shall not be liable for any damages arising from its use.

6. Modifications
I may modify the terms of this license at any time, with updates posted in this file. Continued use of the software constitutes acceptance of the new terms.

---

## 🙋 Contact

Questions or bugs? Open an issue on GitHub.

---

```
