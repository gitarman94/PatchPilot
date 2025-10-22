# PatchPilot

PatchPilot is a cross-platform patch client designed to manage and report software updates on Windows and Linux systems. It includes a lightweight Python server and a Rust-based client with self-updating capabilities.

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
├── server.py                      # Flask-like Python server
├── setup_or_update_server.sh     # Server install/update script (Linux)
├── setup_or_update_client.sh     # Client install/update script (Linux)
├── setup_or_update_client.ps1    # Client install/update script (Windows)
├── README.md
│
├── patchpilot_client_rust/       # Shared Rust client code (Windows & Linux)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── self_update.rs
│       ├── service.rs
│       ├── system_info.rs
│       ├── updater.rs
│       ├── commands.rs
│       └── platform/             # OS-specific support code (optional)
│
└── templates/                    # Server web UI (HTML)
    ├── dashboard.html
    └── client_detail.html
```

---

## 🚀 Server Setup (Linux)

### Prerequisites

* Python 3.8+
* Git
* `systemd` (for automatic restart)

### Install/Update in One Command

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_server.sh)"
```

This will:

* Install Python dependencies
* Download/Update the server
* Set up systemd service
* Start and enable it on boot

---

## 💻 Client Setup (Linux)

### Requirements

* Ubuntu/Debian
* sudo/root access
* Internet connection

### Install/Update in One Command

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/gitarman94/PatchPilot/main/setup_or_update_client.sh)"
```

* Installs Rust if missing
* Builds and installs the Rust client
* Creates `patchpilot` system user
* Configures systemd timers
* Auto-updates on re-run

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
# or for Windows:
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
* Communication via REST API to server.py

---

## 📜 License

This project is licensed under **GPL-3.0**. See the [LICENSE](LICENSE) file for full details.

---

## 🙋 Contact

Questions or bugs? Open an issue on GitHub.

---

Let me know if you want this saved as a `README.md` file or also want a LICENSE file generated.
