Got it! Here’s the updated README.md with the **GPL-3.0 License** reference instead of MIT:

---

````markdown
# PatchPilot

PatchPilot is a cross-platform patch client designed to streamline automated patching and updating of software clients. It supports Windows and Linux clients with systemd integration, self-updating capabilities, and efficient service management.

---

## Features

- Self-updating Rust-based client for reliability and performance  
- Runs as a systemd service on Linux with timer-based scheduling  
- Dedicated non-root user for secure execution  
- Supports Windows as a native service  
- Easy installation and update via shell scripts (Linux) and executable (Windows)  
- Configurable patch server URL and client ID management  

---

## Repository Structure

- `linux-client/` — Rust source code for the Linux patch client  
- `windows-client/` — Rust source code and service wrapper for Windows  
- `installer/` — Installation scripts for Linux client  
- `README.md` — This file  

---

## Installation (Linux Client)

### Prerequisites

- Ubuntu/Debian-based Linux distribution  
- `sudo` privileges  
- Internet connection  

### Installation Steps

1. Clone or download the repository:

   ```bash
   git clone https://github.com/gitarman94/PatchPilot.git
   cd PatchPilot/installer
````

2. Run the install/update script as root or with sudo:

   ```bash
   sudo ./install.sh [patch-server-url]
   ```

   Replace `[patch-server-url]` with your patch server address (e.g., `192.168.1.100:8080`). If omitted, the script will prompt for it.

3. The installer will:

   * Install required packages including Rust toolchain
   * Create a dedicated `patchpilot` system user and group
   * Build and install the patchpilot client binaries to `/opt/patchpilot_client`
   * Set proper permissions for secure execution
   * Install and enable systemd service and timer units running under `patchpilot` user

4. Check the status of the patchpilot service and timers:

   ```bash
   systemctl status patchpilot_client.timer
   systemctl status patchpilot_ping.timer
   ```

---

## Configuration

* Patch server URL is stored in `/opt/patchpilot_client/server_url.txt`
* Unique client ID is generated on first install and stored in `/opt/patchpilot_client/client_id.txt`
* Configuration file (`config.json`) resides in `/opt/patchpilot_client`

Modify these files if needed, and restart the service timers to apply changes:

```bash
sudo systemctl restart patchpilot_client.timer patchpilot_ping.timer
```

---

## Updating the Client

The installer script checks for updates automatically. To manually update the client, run:

```bash
sudo ./install.sh [patch-server-url]
```

If the client binaries or service files are outdated, the script will update them and restart the services.

---

## Windows Client

Refer to the `windows-client/` directory for Windows client source and service setup instructions.

---

## Troubleshooting

* Ensure the `patchpilot` user has ownership of `/opt/patchpilot_client`:

  ```bash
  sudo chown -R patchpilot:patchpilot /opt/patchpilot_client
  ```

* Check logs via journalctl:

  ```bash
  sudo journalctl -u patchpilot_client.service
  sudo journalctl -u patchpilot_ping.service
  ```

* Confirm Rust is installed correctly on the system:

  ```bash
  rustc --version
  cargo --version
  ```

---

## Contributing

Contributions are welcome! Please open issues or pull requests on GitHub.

---

## License

This project is licensed under the GNU General Public License v3.0 (GPL-3.0). See the [LICENSE](LICENSE) file for details.

---

## Contact

Created and maintained by [gitarman94](https://github.com/gitarman94).
For questions or support, please open an issue on GitHub.

```

---

Would you like me to generate a proper `LICENSE` file content for GPL-3.0 as well?
```
