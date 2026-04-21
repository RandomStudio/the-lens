#!/usr/bin/env bash
# install.sh — AS5600 UDP sender setup for Raspberry Pi
# Installs dependencies, enables I²C, and registers a systemd service
# so main.py runs automatically on boot.
#
# Usage:
#   chmod +x install.sh
#   sudo ./install.sh

set -euo pipefail

# ── Colour helpers ────────────────────────────────────────────────────────────

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()    { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*"; exit 1; }

# ── Must run as root ──────────────────────────────────────────────────────────

[[ $EUID -eq 0 ]] || error "Run this script with sudo: sudo ./install.sh"

# ── Locate main.py next to this script ───────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MAIN_PY="$SCRIPT_DIR/main.py"

[[ -f "$MAIN_PY" ]] || error "main.py not found in $SCRIPT_DIR"

SERVICE_NAME="as5600-sender"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

# ── 1. System update & dependencies ──────────────────────────────────────────

info "Updating package lists..."
apt-get update -qq

info "Installing python3-smbus, i2c-tools, and python3-paho-mqtt..."
apt-get install -y -qq python3-smbus i2c-tools python3-paho-mqtt

# ── 2. Enable I²C ─────────────────────────────────────────────────────────────

info "Enabling I²C kernel module..."

# /boot/config.txt on older Pi OS, /boot/firmware/config.txt on Bookworm+
if [[ -f /boot/firmware/config.txt ]]; then
    CONFIG_TXT="/boot/firmware/config.txt"
else
    CONFIG_TXT="/boot/config.txt"
fi

if grep -q "^dtparam=i2c_arm=on" "$CONFIG_TXT"; then
    info "I²C already enabled in $CONFIG_TXT"
else
    echo "dtparam=i2c_arm=on" >> "$CONFIG_TXT"
    info "Added dtparam=i2c_arm=on to $CONFIG_TXT"
fi

# Load i2c-dev now (without needing a reboot for the rest of the install)
modprobe i2c-dev 2>/dev/null || true

# Persist i2c-dev across reboots
if ! grep -q "^i2c-dev" /etc/modules; then
    echo "i2c-dev" >> /etc/modules
    info "Registered i2c-dev in /etc/modules"
fi

# ── 3. Make main.py executable ────────────────────────────────────────────────

chmod +x "$MAIN_PY"
info "Made $MAIN_PY executable"

# ── 4. Write systemd service ──────────────────────────────────────────────────

info "Writing systemd service to $SERVICE_FILE..."

cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=AS5600 rotation sensor MQTT sender
[Service]
Type=simple
ExecStart=/usr/bin/python3 ${MAIN_PY}
WorkingDirectory=${SCRIPT_DIR}
Restart=always
RestartSec=2
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# ── 5. Enable & start the service ─────────────────────────────────────────────

info "Reloading systemd daemon..."
systemctl daemon-reload

info "Enabling ${SERVICE_NAME} to start on boot..."
systemctl enable "${SERVICE_NAME}"

info "Starting ${SERVICE_NAME} now..."
systemctl restart "${SERVICE_NAME}"

# ── 6. Summary ────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Installation complete!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "  Script location : $MAIN_PY"
echo "  Service name    : ${SERVICE_NAME}"
echo "  Config file     : ${CONFIG_TXT}"
echo ""
echo "  Useful commands:"
echo "    sudo systemctl status  ${SERVICE_NAME}   # check status"
echo "    sudo systemctl stop    ${SERVICE_NAME}   # stop"
echo "    sudo systemctl restart ${SERVICE_NAME}   # restart"
echo "    sudo journalctl -fu    ${SERVICE_NAME}   # live logs"
echo ""
warn "A reboot is required for I²C to be fully active if it was just enabled."
echo "  Run: sudo reboot"
echo ""