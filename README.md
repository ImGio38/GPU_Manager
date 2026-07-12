# Linux Hardware Control System (hw-control)

A production-grade, ultra-lightweight Linux hardware control system designed to manage GPU switching (Integrated, Hybrid, Dedicated) and custom fan curve regulation. 

The system consists of:
1. **`hw-control-core`**: A shared library interfacing directly with Linux kernel pseudo-filesystems (`sysfs`, `hwmon`, and ACPI/WMI).
2. **`hw-control-daemon`**: A privileged root-level background service that listens on a Unix Domain Socket (UDS), performs fan polling/interpolation, and executes GPU modes.
3. **`hw-control-gui`**: A native, lightweight vector-rendered user interface written in Rust using `egui`/`eframe` that interacts with the daemon as a normal user.

---

## Directory Structure

```text
/
├── Cargo.toml                # Workspace definition
├── README.md                 # Architecture, installation, and usage documentation
├── LICENSE                   # MIT License
├── hw-control-core/          # Shared low-level kernel interface library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── sysfs_gpu.rs      # GPU power states, module management, and ACPI/WMI MUX controls
│       ├── sysfs_fan.rs      # hwmon auto-detection, manual PWM controls, and interpolation
│       └── config.rs         # TOML parser for /etc/hw-control.toml
├── hw-control-daemon/        # Root-privileged background service
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Entry point and privilege checks
│       ├── socket_server.rs  # UDS server (/run/hw-control.sock)
│       └── scheduler.rs      # Temperature monitoring and fan speed adjustment loop
├── hw-control-gui/           # eframe/egui native GUI application
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # GUI entry point
│       ├── app_ui.rs         # Dashboard and Fan curve grid editor
│       └── socket_client.rs  # UDS client wrapper
└── scripts/
    ├── install.sh            # Build and installation script
    └── systemd/
        └── hw-control.service # Systemd system service configuration
```

---

## Technical Specifications

### GPU Switching Modes
- **Integrated Mode**: Unbinds the dedicated GPU, unloads modules (`nvidia`, `nouveau`, `amdgpu` if dGPU), enables driver blacklisting, toggles WMI platform `dgpu_disable` to `1` (disable), and removes the PCI device from the bus to trigger hardware power-down.
- **Hybrid Mode**: Removes driver blacklisting, toggles WMI `dgpu_disable` to `0`, rescans the PCI bus, and sets power management to `auto` (allowing runtime D3cold / RTD3).
- **Dedicated Mode**: Removes driver blacklisting, toggles WMI `dgpu_disable` to `0` and `gpu_mux` to `1` (MUX dedicated routing), rescans the PCI bus, and sets power management to `on` (preventing dGPU suspend for high performance).

### Fan Curve Interpolation
- Scans all cooling devices under `/sys/class/hwmon`.
- Maps CPU/GPU temp sensors dynamically (e.g. `coretemp`, `k10temp`, `amdgpu`).
- Calculates fan duty cycles (0-255) using linear interpolation based on a custom temperature matrix defined in `/etc/hw-control.toml`.

### Secure IPC via Unix Domain Sockets
- The daemon binds to the socket at `/run/hw-control.sock`.
- The GUI runs with standard user privileges, sending commands (such as switching GPU modes or changing fan curves).
- Strict validation is applied to all incoming commands to prevent privilege escalation.

---

## Configuration (`/etc/hw-control.toml`)

Here is an example config file created automatically if none exists:

```toml
[gpu]
default_mode = "Hybrid"

[fan]
poll_interval_secs = 2

[[fan.curves]]
name = "cpu"
points = [
    { temp = 30.0, speed = 50 },
    { temp = 50.0, speed = 100 },
    { temp = 70.0, speed = 180 },
    { temp = 85.0, speed = 255 }
]

[[fan.curves]]
name = "gpu"
points = [
    { temp = 35.0, speed = 0 },
    { temp = 55.0, speed = 90 },
    { temp = 75.0, speed = 170 },
    { temp = 85.0, speed = 255 }
]
```

---

## Build and Run

### Requirements
- Rust toolchain (`cargo`, `rustc`)
- NVIDIA/AMD drivers installed (if using dedicated GPU features)
- Support for sysfs hwmon controls

### Local Build
To compile the entire workspace:
```bash
cargo build --release
```

---

## License

This project is licensed under the MIT License. See [LICENSE](file:///home/gio/Documents/GitHubGio/GPU_Manager/LICENSE) for details.
