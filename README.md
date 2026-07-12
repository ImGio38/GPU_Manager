# Linux Hardware Control System

```diff
- ⚠️ PROJECT STATUS: STILL IN ACTIVE DEVELOPMENT & TEMPORARILY PAUSED
```

> [!WARNING]
> **THIS PROJECT IS UNDER DEVELOPMENT AND CURRENTLY PAUSED.**  
> Features are subject to change. Some system telemetry and fan controls may fall back to log mock patterns depending on your specific Linux distribution and hardware layout.

---

A production-grade, ultra-lightweight Linux hardware control system designed to manage global GPU switching (MUX control) and custom fan curve regulation.

It operates using a root-privileged background daemon communicating securely with a lightweight native desktop GUI application via Unix Domain Sockets (`/run/hw-control.sock`).

---

## Key Features
* **GPU MUX Toggle:** Switch dynamically between **Integrated**, **Hybrid**, and **Dedicated** GPU modes.
* **Interactive Fan Curves:** Drag-and-drop coordinate grid editor to configure custom fan cooling curves based on real-time CPU and GPU temperatures.
* **One-Click Uninstall:** A built-in uninstall button in the app that safely cleans up all files, disables systemd services, and restores system fans to standard BIOS auto-control.

---

## Installation
Run the installer script in your terminal:
```bash
sudo ./scripts/install.sh
```

## Running the App
Once installed, you can launch the app from your application launcher or by running:
```bash
hw-control-gui
```

## Uninstallation
To completely remove the application and all configurations from your system, simply click the **"Uninstall Application"** button at the bottom of the GUI.
