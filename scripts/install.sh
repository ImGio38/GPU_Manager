#!/bin/bash
set -e

# Verify script is run as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)."
  exit 1
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

echo "Starting and enabling hw-control-daemon service..."
systemctl enable --now hw-control.service

echo "Installation complete!"
echo "Daemon is running. GUI binary is available at /usr/local/bin/hw-control-gui"
