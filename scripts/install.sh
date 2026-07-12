#!/bin/bash
set -e

# Verify script is run as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)."
  exit 1
fi

# Automatically find and append user's cargo directory to PATH if cargo isn't found
if ! command -v cargo &> /dev/null; then
  if [ -n "$SUDO_USER" ]; then
    USER_HOME=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    if [ -x "$USER_HOME/.cargo/bin/cargo" ]; then
      export PATH="$PATH:$USER_HOME/.cargo/bin"
    fi
  fi
fi

echo "Building hw-control project..."
cargo build --release

echo "Installing binaries..."
cp target/release/hw-control-daemon /usr/local/bin/
cp target/release/hw-control-gui /usr/local/bin/

echo "Installing systemd service..."
cp scripts/systemd/hw-control.service /etc/systemd/system/
systemctl daemon-reload

echo "Creating default configuration at /etc/hw-control.toml if not present..."
if [ ! -f /etc/hw-control.toml ]; then
  cat <<EOF > /etc/hw-control.toml
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
EOF
  echo "Created default /etc/hw-control.toml"
else
  echo "/etc/hw-control.toml already exists, skipping."
fi

echo "Installing desktop launcher..."
cat <<EOF > /usr/share/applications/hw-control.desktop
[Desktop Entry]
Type=Application
Name=Hardware Controller
Comment=Manage GPU Switching and Custom Fan Curves
Exec=/usr/local/bin/hw-control-gui
Icon=chip
Terminal=false
Categories=System;Settings;HardwareSettings;
EOF
chmod 644 /usr/share/applications/hw-control.desktop

echo "Starting and enabling hw-control-daemon service..."
systemctl enable --now hw-control.service

echo "Installation complete!"
echo "Daemon is running. GUI application is installed in system menu and /usr/local/bin/hw-control-gui"
