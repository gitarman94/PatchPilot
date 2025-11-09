
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
├───patchpilot_server                # Rust-based backend server
│   │   Cargo.toml                   # Rust configuration file, managing dependencies and project settings.
│   │
│   └───src                          # Source directory for Rust code.
│           main.rs                   # Entry point for the Rust server application.
│           models.rs                 # Defines data models and structures used by the server.
│           schema.rs                 # Defines the database schema for the server.
│
├───patchpilot_client                # Rust client code (shared across Windows & Linux) for handling communication and updates.
│   │   Cargo.toml                   # Rust configuration file, managing dependencies and project settings.
│   │
│   └───src                          # Source directory for Rust code.
│           commands.rs               # Rust file responsible for parsing and handling commands sent from the server (e.g., installing updates).
│           main.rs                   # Entry point for the Rust client application.
│           patchpilot_updater.rs     # Code for the update logic in the Rust client, managing patch installations and updates.
│           self_update.rs            # Logic for updating the Rust client itself (self-updating mechanism).
│           service.rs                # Provides the core service for the PatchPilot client, including running in the background and maintaining client health.
│           system_info.rs            # Collects system information (e.g., CPU, RAM, OS version) to send back to the server.
│
├───templates                         # HTML templates used by the Rust server for the web UI.
│       client_detail.html            # Template for showing detailed information about a specific client (e.g., status, updates, system info).
│       dashboard.html                # Main dashboard template that aggregates information about all clients and allows admin actions.

└───static                            # HTML resource location
│   │   favicon.ico                   # Decorative favorite icon used around the site
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
