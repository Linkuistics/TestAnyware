#!/bin/bash
# Create a golden Linux VM image with testanyware-agent TCP service.
# Deletes any existing golden image with the same name first.
#
# Usage:
#   scripts/vm-create-golden-linux.sh [options]
#
# Options:
#   --version VERSION   Ubuntu version number: 24.04, 22.04 (default: 24.04)
#   --name NAME         Golden image name (default: testanyware-golden-linux-VERSION)
#
# Prerequisites:
#   - tart installed (/opt/homebrew/bin/tart)
#   - SSH public key at ~/.ssh/id_ed25519.pub or ~/.ssh/id_rsa.pub
#
# What this creates:
#   A tart VM cloned from Cirrus Labs' vanilla Ubuntu image with:
#   - Ubuntu Desktop (minimal) installed
#   - GDM autologin configured for the admin user
#   - Solid gray desktop background, screen lock and blanking disabled
#   - testanyware-agent Python TCP service on port 8648 (systemd user service)
#   - AT-SPI2 accessibility enabled for GUI automation
#   - xdotool for window management fallback
#   - SSH (openssh-server) disabled and masked — agent HTTP is the only ingress
#
# SSH is used during golden image CREATION only (host public key in
# authorized_keys for setup). The service is disabled and masked before the
# final shutdown, so clones boot with sshd off.
#
# The golden image is never run directly — clone from it for each test session.

set -euo pipefail

_VERSION="24.04"
_NAME=""
_VANILLA_USER="admin"
_VANILLA_PASS="admin"
_SETUP_VM="testanyware-setup-$$"
_ASKPASS_FILE=""
_SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=30"

while [[ $# -gt 0 ]]; do
    case $1 in
        --version) _VERSION="$2"; shift 2 ;;
        --name)    _NAME="$2"; shift 2 ;;
        *)         echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$_NAME" ]]; then
    _NAME="testanyware-golden-linux-$_VERSION"
fi

_VANILLA="ghcr.io/cirruslabs/ubuntu:$_VERSION"

# --- Preflight ---

if ! command -v tart &>/dev/null; then
    echo "ERROR: tart not found. Install from https://tart.run"
    exit 1
fi

# Find SSH public key
_SSH_KEY=""
for keyfile in ~/.ssh/id_ed25519.pub ~/.ssh/id_rsa.pub; do
    if [[ -f "$keyfile" ]]; then
        _SSH_KEY="$keyfile"
        break
    fi
done
if [[ -z "$_SSH_KEY" ]]; then
    echo "ERROR: No SSH public key found (~/.ssh/id_ed25519.pub or ~/.ssh/id_rsa.pub)"
    echo "Generate one with: ssh-keygen -t ed25519"
    exit 1
fi
echo "Using SSH key: $_SSH_KEY"

# --- Cleanup on exit ---

cleanup() {
    # Clean up setup VM and askpass helper
    tart stop "$_SETUP_VM" 2>/dev/null || true
    tart delete "$_SETUP_VM" 2>/dev/null || true
    rm -f "$_ASKPASS_FILE" 2>/dev/null || true
    if [[ -n "${_TART_PID:-}" ]] && kill -0 "$_TART_PID" 2>/dev/null; then
        kill "$_TART_PID" 2>/dev/null
        wait "$_TART_PID" 2>/dev/null
    fi
}
trap cleanup EXIT

# --- Delete existing golden if present ---

_VM_LIST=$(tart list --format json 2>/dev/null || echo "[]")
if echo "$_VM_LIST" | grep -q "\"$_NAME\""; then
    echo "Deleting existing golden image '$_NAME'..."
    tart stop "$_NAME" 2>/dev/null || true
    tart delete "$_NAME"
fi

# --- Pull and clone vanilla image ---

echo "Cloning $_VANILLA → $_SETUP_VM..."
echo "(This may pull the image on first run — can take several minutes)"
tart clone "$_VANILLA" "$_SETUP_VM"

# --- Boot the setup VM ---

echo "Booting setup VM..."
# Use --vnc-experimental so GDM has a virtual display after reboot.
# Without it, GDM crash-loops trying to start a graphical session.
tart run "$_SETUP_VM" --no-graphics --vnc-experimental &
_TART_PID=$!

# --- Set up SSH_ASKPASS for password auth to vanilla image ---

_ASKPASS_FILE=$(mktemp)
cat > "$_ASKPASS_FILE" << EOF
#!/bin/bash
echo '$_VANILLA_PASS'
EOF
chmod 700 "$_ASKPASS_FILE"
export SSH_ASKPASS="$_ASKPASS_FILE"
export SSH_ASKPASS_REQUIRE="force"
export DISPLAY=:0

# --- Wait for SSH ---

echo -n "Waiting for SSH..."
_IP=""
_SSH_READY=false
for i in $(seq 1 60); do
    _IP=$(tart ip "$_SETUP_VM" 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$_IP" ]]; then
        if ssh $_SSH_OPTS "$_VANILLA_USER@$_IP" "true" 2>/dev/null; then
            _SSH_READY=true
            echo " ready (IP: $_IP)"
            break
        fi
    fi
    echo -n "."
    sleep 3
done

if ! $_SSH_READY; then
    echo ""
    echo "ERROR: SSH not reachable within 180s"
    exit 1
fi

# --- Helper functions ---

vm_ssh() {
    ssh $_SSH_OPTS "$_VANILLA_USER@$_IP" "$1"
}

vm_scp() {
    scp $_SSH_OPTS "$1" "$_VANILLA_USER@$_IP:$2"
}

# --- Install SSH key ---

echo "Installing SSH key..."
vm_ssh "mkdir -p ~/.ssh && chmod 700 ~/.ssh"
vm_scp "$_SSH_KEY" "/tmp/host_key.pub"
vm_ssh "cat /tmp/host_key.pub >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && rm /tmp/host_key.pub"

# Verify key-based auth works without password
unset SSH_ASKPASS SSH_ASKPASS_REQUIRE DISPLAY
if ssh $_SSH_OPTS "$_VANILLA_USER@$_IP" "echo ok" 2>/dev/null | grep -q "ok"; then
    echo "SSH key auth verified."
else
    echo "ERROR: SSH key auth failed — password auth still required"
    exit 1
fi

# --- Install Ubuntu Desktop ---

echo "Installing Ubuntu Desktop (this takes several minutes)..."
vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get update -q"

# Remove needrestart — it restarts services like systemd-networkd
# after apt installs, which can break SSH connectivity mid-script.
# We do a full reboot at the end which restarts everything.
vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get remove -y needrestart >/dev/null 2>&1 || true"

# Prevent services from auto-starting during install. Without this,
# packages like gdm3 and gnome-remote-desktop try to start daemons
# that hang waiting for hardware/display that doesn't exist yet.
# We use two mechanisms:
#   1. policy-rc.d returning 101 blocks invoke-rc.d calls
#   2. Diverting systemctl to /bin/true blocks direct systemctl calls
#      (some packages call systemctl directly, bypassing invoke-rc.d)
vm_ssh "echo -e '#!/bin/sh\nexit 101' | sudo tee /usr/sbin/policy-rc.d > /dev/null && sudo chmod +x /usr/sbin/policy-rc.d"
vm_ssh "sudo dpkg-divert --local --rename --add /usr/bin/systemctl && sudo ln -sf /bin/true /usr/bin/systemctl"

# Pin firefox to never install during this apt run — it's a snap
# package that requires snapd, which can't start with systemctl diverted.
# apt-mark hold doesn't work here because apt already resolved the
# dependency before the hold takes effect. An apt pin of -1 prevents
# apt from selecting the package entirely.
# We install firefox after restoring systemctl (see below).
vm_ssh "printf 'Package: firefox\nPin: release *\nPin-Priority: -1\n' | sudo tee /etc/apt/preferences.d/no-firefox > /dev/null"

vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' ubuntu-desktop-minimal"

# Remove the firefox pin
vm_ssh "sudo rm -f /etc/apt/preferences.d/no-firefox"

# Mask unattended-upgrades BEFORE restoring systemctl so it cannot start
# and grab the dpkg lock (race condition that blocks subsequent apt ops).
vm_ssh "sudo ln -sf /dev/null /etc/systemd/system/unattended-upgrades.service"

# Restore systemctl and policy-rc.d so services start normally on boot
vm_ssh "sudo rm -f /usr/bin/systemctl && sudo dpkg-divert --local --rename --remove /usr/bin/systemctl"
vm_ssh "sudo rm -f /usr/sbin/policy-rc.d"

# Reload systemd so it sees the mask, then stop+mask unattended-upgrades
# properly. The mask-before-restore helps, but systemd needs daemon-reload
# to read unit file changes from disk.
vm_ssh "sudo systemctl daemon-reload"
vm_ssh "sudo systemctl stop unattended-upgrades.service 2>/dev/null || true"
vm_ssh "sudo systemctl mask unattended-upgrades.service 2>/dev/null || true"
vm_ssh "sudo killall unattended-upgr 2>/dev/null || true"
# Wait for the dpkg lock to be released
vm_ssh "while sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 1; done"

echo "  Ubuntu Desktop installed."

# --- Switch networking from systemd-networkd to NetworkManager ---
# The base Cirrus Labs image uses systemd-networkd via netplan. Ubuntu Desktop
# installs NetworkManager. We must tell netplan to use NM as the renderer,
# otherwise netplan generates networkd config on boot and NM has nothing to manage.
echo "Configuring NetworkManager via netplan..."

# Remove existing netplan configs (typically 50-cloud-init.yaml or similar
# that specify renderer: networkd)
vm_ssh "sudo rm -f /etc/netplan/*.yaml"

# Create a single netplan config that delegates everything to NetworkManager
vm_ssh "sudo tee /etc/netplan/01-network-manager-all.yaml > /dev/null << 'NETPLAN'
network:
  version: 2
  renderer: NetworkManager
NETPLAN"
vm_ssh "sudo chmod 600 /etc/netplan/01-network-manager-all.yaml"

# Disable systemd-networkd so it doesn't conflict with NetworkManager
vm_ssh "sudo systemctl disable systemd-networkd.service systemd-networkd-wait-online.service 2>/dev/null || true"

# Ensure NetworkManager is enabled (may already be, but be explicit)
vm_ssh "sudo systemctl enable NetworkManager.service 2>/dev/null || true"

# Now install Firefox — snapd can run with systemctl restored
echo "Installing Firefox (snap)..."
vm_ssh "sudo apt-mark unhold firefox 2>/dev/null || true"
# Safety: wait for dpkg lock one more time right before apt install
vm_ssh "while sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 1; done"
vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y firefox"
echo "  Firefox installed."

# --- Configure GDM autologin ---

echo "Configuring autologin and forcing X11 session..."
# WaylandEnable=false forces GDM to use X11 (Xorg) instead of Wayland.
# Required because GTK4's AT-SPI implementation returns (0,0) for all
# coordinate types under both Wayland and X11, and the agent's xdotool-based
# coordinate fix only works under X11 (xdotool can't see Wayland-native windows).
vm_ssh "sudo tee /etc/gdm3/custom.conf > /dev/null << 'GDMCONF'
[daemon]
AutomaticLoginEnable=True
AutomaticLogin=admin
WaylandEnable=false
GDMCONF"

# Skip GNOME Initial Setup wizard on first GUI login
vm_ssh "mkdir -p ~/.config && echo 'yes' > ~/.config/gnome-initial-setup-done"

# --- Set solid wallpaper and disable desktop clutter ---

echo "Configuring desktop settings..."
vm_ssh "sudo tee /usr/share/glib-2.0/schemas/99-testanyware.gschema.override > /dev/null << 'SCHEMA'
[org.gnome.desktop.background]
picture-options='none'
primary-color='#808080'

[org.gnome.desktop.screensaver]
lock-enabled=false

[org.gnome.desktop.session]
idle-delay=uint32 0

[org.gnome.desktop.notifications]
show-banners=false
SCHEMA"
vm_ssh "sudo glib-compile-schemas /usr/share/glib-2.0/schemas/"

# Run all pending updates so the golden image is fully patched
echo "Running system updates (this may take a few minutes)..."
vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold'"
echo "  Updates complete."

# Disable Software Updater and upgrade notifications so they
# don't pop up during tests
vm_ssh "sudo apt-get remove -y update-notifier 2>/dev/null || true"
vm_ssh "sudo systemctl disable apt-daily.timer apt-daily-upgrade.timer 2>/dev/null || true"

# --- Configure silent boot (skip text-mode, go straight to GUI) ---

echo "Configuring silent boot..."
# Remove the cloud image GRUB override — it forces console=ttyAMA0 which
# shows text-mode boot and overrides quiet/splash from /etc/default/grub.
vm_ssh "sudo rm -f /etc/default/grub.d/50-cloudimg-settings.cfg"
vm_ssh "sudo sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash loglevel=0 vt.global_cursor_default=0\"/' /etc/default/grub"
vm_ssh "sudo sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub"
vm_ssh "grep -q '^GRUB_TIMEOUT_STYLE=' /etc/default/grub && sudo sed -i 's/^GRUB_TIMEOUT_STYLE=.*/GRUB_TIMEOUT_STYLE=hidden/' /etc/default/grub || echo 'GRUB_TIMEOUT_STYLE=hidden' | sudo tee -a /etc/default/grub > /dev/null"
vm_ssh "sudo update-grub"

# --- Install testanyware-agent ---

echo "Installing testanyware-agent..."

# Install xdotool for window management fallback, and python3-pyatspi
# for AT-SPI2 accessibility bindings (not included in ubuntu-desktop-minimal)
vm_ssh "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y xdotool python3-pyatspi"

# Copy agent Python package to the VM
_AGENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/agents/linux"
if [[ ! -d "$_AGENT_DIR/testanyware_agent" ]]; then
    echo "ERROR: Agent package not found at $_AGENT_DIR/testanyware_agent"
    exit 1
fi

vm_ssh "sudo mkdir -p /opt/testanyware"
# tar the agent package on the host, pipe via SSH to extract in VM
tar -cf - -C "$_AGENT_DIR" testanyware_agent | \
    ssh $_SSH_OPTS "$_VANILLA_USER@$_IP" "sudo tar -xf - -C /opt/testanyware"

# Create a launcher script
vm_ssh "sudo tee /opt/testanyware/run-agent.sh > /dev/null << 'LAUNCHER'
#!/bin/bash
cd /opt/testanyware
exec python3 -m testanyware_agent
LAUNCHER"
vm_ssh "sudo chmod +x /opt/testanyware/run-agent.sh"

# Create systemd user service (runs in desktop session for AT-SPI2 access)
vm_ssh "mkdir -p ~/.config/systemd/user"
vm_ssh "cat > ~/.config/systemd/user/testanyware-agent.service << 'UNIT'
[Unit]
Description=TestAnyware Agent TCP Service
After=graphical-session.target

[Service]
Type=simple
ExecStart=/opt/testanyware/run-agent.sh
Restart=always
RestartSec=5
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
UNIT"

# Enable the service via direct symlink (works without an active user session)
vm_ssh "mkdir -p ~/.config/systemd/user/default.target.wants"
vm_ssh "ln -sf ~/.config/systemd/user/testanyware-agent.service ~/.config/systemd/user/default.target.wants/testanyware-agent.service"

# Enable AT-SPI2 accessibility — already in the gsettings override above,
# but also set it explicitly via gsettings for the current user session
# in case the override doesn't take effect until next login.
echo "Enabling AT-SPI2 accessibility..."

# Add AT-SPI2 to the existing gsettings override
vm_ssh "sudo tee -a /usr/share/glib-2.0/schemas/99-testanyware.gschema.override > /dev/null << 'ATSPI'

[org.gnome.desktop.interface]
toolkit-accessibility=true
ATSPI"
vm_ssh "sudo glib-compile-schemas /usr/share/glib-2.0/schemas/"

# Open firewall port 8648 (ufw may or may not be active)
vm_ssh "sudo ufw allow 8648/tcp 2>/dev/null || true"

echo "  Agent installed."

# --- Reboot cycle to apply settings ---
# tart run exits when the guest shuts down or reboots, so we:
# shutdown → restart tart run → wait for GDM autologin + agent → shutdown → clone.

echo "Shutting down VM for reboot cycle..."
vm_ssh "sudo shutdown -h now" 2>/dev/null || true

echo -n "Waiting for shutdown..."
for i in $(seq 1 60); do
    if ! kill -0 "$_TART_PID" 2>/dev/null; then
        echo " done."
        break
    fi
    echo -n "."
    sleep 2
done
wait "$_TART_PID" 2>/dev/null || true

echo "Restarting VM to apply settings..."
tart run "$_SETUP_VM" --no-graphics --vnc-experimental &
_TART_PID=$!

echo -n "Waiting for SSH..."
_SSH_BACK=false
for i in $(seq 1 90); do
    _IP=$(tart ip "$_SETUP_VM" 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$_IP" ]]; then
        if ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
               -o LogLevel=ERROR -o ConnectTimeout=5 \
               "$_VANILLA_USER@$_IP" "true" &>/dev/null; then
            _SSH_BACK=true
            echo " back online (IP: $_IP)."
            break
        fi
    fi
    echo -n "."
    sleep 3
done

if ! $_SSH_BACK; then
    echo ""
    echo "ERROR: VM did not come back online after restart"
    exit 1
fi

# Give the desktop a moment to fully load after autologin
sleep 10

# --- Verify agent health ---

echo -n "Waiting for agent at $_IP:8648..."
_AGENT_READY=false
for i in $(seq 1 60); do
    if curl -sf --connect-timeout 2 "http://$_IP:8648/health" 2>/dev/null | grep -q '"accessible": *true'; then
        _AGENT_READY=true
        echo " ready."
        break
    fi
    echo -n "."
    sleep 2
done

if ! $_AGENT_READY; then
    echo ""
    echo "ERROR: Agent not reachable at $_IP:8648 after reboot"
    echo "Debug: checking systemd user service status..."
    vm_ssh "systemctl --user status testanyware-agent.service" || true
    vm_ssh "journalctl --user -u testanyware-agent.service --no-pager -n 20" || true
    exit 1
fi

echo "Agent health verified: $(curl -sf "http://$_IP:8648/health")"

# --- Disable SSH + shutdown ---
# SSH was used during golden creation only. Clones do not need it: all
# runtime communication goes through the testanyware agent on port 8648,
# matching the Windows golden which has no SSH at all.
#
# `disable` removes auto-start across reboots; `mask` symlinks the unit to
# /dev/null so it cannot be enabled again accidentally. We deliberately
# omit `--now` so the running sshd (and our active session) survives long
# enough to queue the shutdown. The next boot has sshd off, masked, and
# unreachable.

echo "Disabling SSH service and shutting down VM..."
vm_ssh "sudo systemctl disable ssh.service 2>/dev/null || sudo systemctl disable ssh 2>/dev/null; sudo systemctl mask ssh.service 2>/dev/null || sudo systemctl mask ssh 2>/dev/null; sudo shutdown -h now" 2>/dev/null || true

echo -n "Waiting for shutdown..."
for i in $(seq 1 60); do
    if ! kill -0 "$_TART_PID" 2>/dev/null; then
        echo " done."
        break
    fi
    echo -n "."
    sleep 2
done
wait "$_TART_PID" 2>/dev/null || true

# --- Clone to golden ---

echo "Creating golden image '$_NAME'..."
tart clone "$_SETUP_VM" "$_NAME"
tart delete "$_SETUP_VM"

# Prevent cleanup trap from deleting the golden
_SETUP_VM="__already_deleted__"

echo ""
echo "Golden image '$_NAME' created successfully."
echo ""
echo "Use it with:"
echo "  scripts/test-integration.sh --base $_NAME"
echo "  source scripts/vm-start.sh --base $_NAME"
