#!/bin/bash
# Create a golden Windows 11 ARM VM image with the testanyware-agent TCP service.
# Uses QEMU (not tart) since tart does not support Windows guests.
# Deletes any existing golden image with the same name first.
#
# Usage:
#   scripts/vm-create-golden-windows.sh [options]
#
# Options:
#   --version VERSION   Windows version (default: 11)
#   --name NAME         Golden image name (default: testanyware-golden-windows-VERSION)
#   --iso PATH          Path to a Windows 11 ARM64 evaluation ISO file
#
# The --iso option is required on first run (unless a cached install already
# exists at $XDG_DATA_HOME/testanyware/cache/). Download the ISO from:
#   https://www.microsoft.com/en-us/software-download/windows11arm64
# Download the ARM64 ISO from that page.
#
# The Windows installation is fully automated via autounattend.xml, which is
# served to Windows Setup on a virtual USB drive. The USB media also contains
# the testanyware-agent binary, VirtIO drivers, and setup scripts. SetupComplete.cmd
# installs the agent as a Task Scheduler logon task. No SSH is used at any point.
# VNC is available for monitoring progress. Typical install time: 20-40 minutes.
#
# Prerequisites:
#   - qemu-system-aarch64 installed (brew install qemu)
#   - qemu-img installed (comes with qemu)
#   - swtpm installed (brew install swtpm)
#   - .NET SDK (for building the Windows agent)
#
# What this creates:
#   A QEMU VM installed from a Microsoft evaluation ISO with:
#   - Local 'admin' account with autologin
#   - testanyware-agent TCP service on port 8648 (starts on logon via Task Scheduler)
#   - Chocolatey package manager installed
#   - Solid gray desktop background
#   - Desktop clutter disabled (widgets, notifications, Cortana, etc.)
#   - No SSH — the agent is the only communication channel
#
# Golden image files stored in $XDG_DATA_HOME/testanyware/golden/:
#   {name}.qcow2          — disk image
#   {name}-efivars.fd     — UEFI variables
#   {name}-tpm/           — TPM state directory
#
# The golden image is never run directly — use qemu-img create -b for COW clones.

set -euo pipefail
trap 'echo "SCRIPT ERROR at line $LINENO: $BASH_COMMAND" >&2' ERR

_VERSION="11"
_NAME=""
_SETUP_PREFIX="testanyware-setup-$$"
_AGENT_PORT=""   # set dynamically after QEMU boots
_VNC_PORT=""     # set dynamically after QEMU boots
_QEMU_PID=""
_SWTPM_PID=""
_GOLDEN_DONE=false
_ISO_PATH=""

_SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
_HELPERS_DIR="$(cd "$_SCRIPT_DIR/.." && pwd)/helpers"
_PROJECT_DIR="$(cd "$_SCRIPT_DIR/../.." && pwd)"

# shellcheck source=./_testanyware-paths.sh
source "$_SCRIPT_DIR/_testanyware-paths.sh"

_DATA_DIR="$(_testanyware_data_dir)"
_GOLDEN_DIR="$_DATA_DIR/golden"
_CACHE_DIR="$_DATA_DIR/cache"

while [[ $# -gt 0 ]]; do
    case $1 in
        --version) _VERSION="$2"; shift 2 ;;
        --name)    _NAME="$2"; shift 2 ;;
        --iso)     _ISO_PATH="$2"; shift 2 ;;
        *)         echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$_NAME" ]]; then
    _NAME="testanyware-golden-windows-$_VERSION"
fi

_SETUP_QCOW2="$_CACHE_DIR/${_SETUP_PREFIX}.qcow2"
_SETUP_EFIVARS="$_CACHE_DIR/${_SETUP_PREFIX}-efivars.fd"
_SETUP_TPM_DIR="$_CACHE_DIR/${_SETUP_PREFIX}-tpm"

# --- Preflight ---

echo "Creating golden Windows $_VERSION image: $_NAME"
echo ""

for cmd in qemu-system-aarch64 qemu-img swtpm dotnet; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd not found."
        case $cmd in
            dotnet) echo "Install with: brew install dotnet" ;;
            swtpm) echo "Install with: brew install swtpm" ;;
            *) echo "Install with: brew install qemu" ;;
        esac
        exit 1
    fi
done

mkdir -p "$_GOLDEN_DIR" "$_CACHE_DIR"

# --- Helper functions ---

agent_health() {
    curl -sf -m 5 "http://localhost:$_AGENT_PORT/health" 2>/dev/null
}

agent_exec() {
    local cmd="$1"
    local timeout="${2:-30}"
    curl -sf -m "$((timeout + 5))" -X POST "http://localhost:$_AGENT_PORT/exec" \
        -H "Content-Type: application/json" \
        -d "{\"command\":$(printf '%s' "$cmd" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'),\"timeout\":$timeout}" 2>/dev/null
}

agent_shutdown() {
    curl -sf -m 10 -X POST "http://localhost:$_AGENT_PORT/shutdown" \
        -H "Content-Type: application/json" -d '{}' 2>/dev/null || true
}

_discover_qemu_ports() {
    # Discover dynamically assigned agent port via QEMU monitor
    # info usernet output: TCP[HOST_FORWARD]  FD  *  HOST_PORT  GUEST_ADDR  GUEST_PORT ...
    # Retry — usernet table may not populate instantly after QEMU starts
    for _try in 1 2 3 4 5; do
        _AGENT_PORT=$( (echo "info usernet"; sleep 0.5) | nc -U "$_MONITOR_SOCK" 2>/dev/null \
            | grep "HOST_FORWARD" | awk '{print $4}') || true
        [[ -n "$_AGENT_PORT" ]] && break
        sleep 1
    done
    if [[ -z "$_AGENT_PORT" ]]; then
        echo "ERROR: Could not discover agent port from QEMU monitor"
        exit 1
    fi

    # Discover dynamically assigned VNC port via QEMU monitor
    # info vnc output: Server: 127.0.0.1:PORT (ipv4)
    _VNC_PORT=$( (echo "info vnc"; sleep 0.5) | nc -U "$_MONITOR_SOCK" 2>/dev/null \
        | grep -o '127\.0\.0\.1:[0-9]*' | head -1 | cut -d: -f2) || true
    if [[ -z "$_VNC_PORT" ]]; then
        echo "WARNING: Could not discover VNC port — defaulting to 5900"
        _VNC_PORT=5900
    fi
}

# --- Cleanup on exit ---

cleanup() {
    if [[ -n "${_KEYPRESS_PID:-}" ]] && kill -0 "$_KEYPRESS_PID" 2>/dev/null; then
        kill "$_KEYPRESS_PID" 2>/dev/null || true
    fi
    if [[ -n "${_QEMU_PID:-}" ]] && kill -0 "$_QEMU_PID" 2>/dev/null; then
        echo "Cleaning up: stopping QEMU..."
        kill "$_QEMU_PID" 2>/dev/null || true
        wait "$_QEMU_PID" 2>/dev/null || true
    fi
    if [[ -n "${_SWTPM_PID:-}" ]] && kill -0 "$_SWTPM_PID" 2>/dev/null; then
        echo "Cleaning up: stopping swtpm..."
        kill "$_SWTPM_PID" 2>/dev/null || true
        wait "$_SWTPM_PID" 2>/dev/null || true
    fi
    rm -f "$_CACHE_DIR/${_SETUP_PREFIX}-monitor.sock" 2>/dev/null || true
    rm -f "$_CACHE_DIR/${_SETUP_PREFIX}-autounattend.img" 2>/dev/null || true
    rm -f "$_CACHE_DIR/${_SETUP_PREFIX}-qemu.log" 2>/dev/null || true
    if ! $_GOLDEN_DONE; then
        rm -f "$_SETUP_QCOW2" 2>/dev/null || true
        rm -f "$_SETUP_EFIVARS" 2>/dev/null || true
        rm -rf "$_SETUP_TPM_DIR" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# --- Delete existing golden if present ---

if [[ -f "$_GOLDEN_DIR/$_NAME.qcow2" ]]; then
    echo "Deleting existing golden image '$_NAME'..."
    rm -f "$_GOLDEN_DIR/$_NAME.qcow2"
    rm -f "$_GOLDEN_DIR/$_NAME-efivars.fd"
    rm -rf "$_GOLDEN_DIR/$_NAME-tpm"
fi

# --- Build Windows agent ---

echo "Building Windows agent (cross-compile for ARM64)..."
_AGENT_PROJECT="$_PROJECT_DIR/agents/windows"
_AGENT_EXE="$_AGENT_PROJECT/bin/Release/net9.0-windows/win-arm64/publish/testanyware-agent.exe"

dotnet publish "$_AGENT_PROJECT" -r win-arm64 --self-contained \
    -p:PublishSingleFile=true -c Release --nologo -v quiet
if [[ ! -f "$_AGENT_EXE" ]]; then
    echo "ERROR: Agent build failed — testanyware-agent.exe not found"
    exit 1
fi
echo "  Agent binary: $(du -h "$_AGENT_EXE" | cut -f1) self-contained"

# --- Locate ISO and prepare setup disk ---

_CACHED_ISO="$_CACHE_DIR/windows-${_VERSION}-arm64-eval.iso"

if [[ -n "$_ISO_PATH" ]]; then
    if [[ ! -f "$_ISO_PATH" ]]; then
        echo "ERROR: ISO file not found: $_ISO_PATH"
        exit 1
    fi
    echo "Copying ISO to cache..."
    cp "$_ISO_PATH" "$_CACHED_ISO"
elif [[ ! -f "$_CACHED_ISO" ]]; then
    echo "ERROR: No Windows ARM64 evaluation ISO available."
    echo ""
    echo "Download one from Microsoft and pass it with --iso:"
    echo "  1. Visit https://www.microsoft.com/en-us/software-download/windows11arm64"
    echo "  2. Download the ARM64 ISO"
    echo "  3. Run: $0 --iso /path/to/downloaded.iso"
    echo ""
    echo "The ISO is cached after first use, so subsequent runs won't need --iso."
    exit 1
fi

echo "Creating setup disk (64GB)..."
qemu-img create -f qcow2 "$_SETUP_QCOW2" 64G

# --- Create autounattend media ---
# FAT disk image with autounattend.xml, agent binary, setup scripts, and drivers.
# Mounted as a USB flash drive so Windows Setup finds autounattend.xml during its
# implicit answer file search on removable disk drives.

echo "Creating autounattend media..."
_AUTOUNATTEND_IMG="$_CACHE_DIR/${_SETUP_PREFIX}-autounattend.img"
_AUTOUNATTEND_TMP=$(mktemp -d)

# Copy setup files: autounattend.xml, SetupComplete.cmd, desktop-setup.ps1
cp "$_HELPERS_DIR/autounattend.xml" "$_AUTOUNATTEND_TMP/"
cp "$_HELPERS_DIR/SetupComplete.cmd" "$_AUTOUNATTEND_TMP/"
cp "$_HELPERS_DIR/desktop-setup.ps1" "$_AUTOUNATTEND_TMP/"

# Copy agent binary
cp "$_AGENT_EXE" "$_AUTOUNATTEND_TMP/testanyware-agent.exe"

# Create startup.nsh for UEFI Shell fallback
cat > "$_AUTOUNATTEND_TMP/startup.nsh" << 'NSHEOF'
FS0:\efi\boot\bootaa64.efi
FS1:\efi\boot\bootaa64.efi
FS2:\efi\boot\bootaa64.efi
FS3:\efi\boot\bootaa64.efi
NSHEOF

# Extract VirtIO ARM64 network driver from virtio-win ISO
_VIRTIO_ISO="$_CACHE_DIR/virtio-win.iso"
if [[ ! -f "$_VIRTIO_ISO" ]]; then
    echo "Downloading virtio-win drivers (~600MB, cached after first run)..."
    curl -L -o "$_VIRTIO_ISO" "https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/stable-virtio/virtio-win.iso"
fi
_VIRTIO_MNT=$(mktemp -d)
hdiutil attach "$_VIRTIO_ISO" -mountpoint "$_VIRTIO_MNT" -readonly -nobrowse -quiet
mkdir -p "$_AUTOUNATTEND_TMP/drivers/netkvm"
mkdir -p "$_AUTOUNATTEND_TMP/drivers/viogpu"
cp "$_VIRTIO_MNT/NetKVM/w11/ARM64/"* "$_AUTOUNATTEND_TMP/drivers/netkvm/"
cp "$_VIRTIO_MNT/viogpudo/w11/ARM64/"* "$_AUTOUNATTEND_TMP/drivers/viogpu/"
hdiutil detach "$_VIRTIO_MNT" -quiet
rmdir "$_VIRTIO_MNT" 2>/dev/null || true
echo "  NetKVM + VioGPU ARM64 drivers included."

# Build FAT32 disk image to fit the ~150MB untrimmed agent binary + drivers + scripts.
# FAT32 used instead of FAT16 because the agent binary exceeds FAT16's practical limits.
hdiutil create -size 200m -fs "MS-DOS FAT32" -volname UNATTEND \
    -srcfolder "$_AUTOUNATTEND_TMP" -ov "$_AUTOUNATTEND_IMG" -quiet
qemu-img convert -f dmg -O raw "$_AUTOUNATTEND_IMG.dmg" "$_AUTOUNATTEND_IMG"
rm -f "$_AUTOUNATTEND_IMG.dmg"
rm -rf "$_AUTOUNATTEND_TMP"
echo "  Media: $(du -h "$_AUTOUNATTEND_IMG" | cut -f1) (autounattend + agent + drivers)"

# --- Prepare UEFI and TPM ---

echo "Preparing UEFI firmware and TPM..."

_QEMU_PREFIX=$(dirname "$(dirname "$(command -v qemu-system-aarch64)")")
_UEFI_CODE="$_QEMU_PREFIX/share/qemu/edk2-aarch64-code.fd"
if [[ ! -f "$_UEFI_CODE" ]]; then
    echo "ERROR: UEFI firmware not found at $_UEFI_CODE"
    echo "Ensure qemu is installed via Homebrew: brew install qemu"
    exit 1
fi

# AArch64 QEMU doesn't ship a vars template — create a blank 64MB file.
truncate -s 64M "$_SETUP_EFIVARS"

mkdir -p "$_SETUP_TPM_DIR"
_TPM_SOCKET="$_SETUP_TPM_DIR/swtpm-sock"

swtpm socket \
    --tpmstate "dir=$_SETUP_TPM_DIR" \
    --ctrl "type=unixio,path=$_TPM_SOCKET" \
    --tpm2 \
    --log "level=0" &
_SWTPM_PID=$!

sleep 1
if ! kill -0 "$_SWTPM_PID" 2>/dev/null; then
    echo "ERROR: swtpm failed to start"
    exit 1
fi
echo "  swtpm running (PID: $_SWTPM_PID)"

# --- Boot with QEMU ---

_QEMU_LOG="$_CACHE_DIR/${_SETUP_PREFIX}-qemu.log"
_MONITOR_SOCK="$_CACHE_DIR/${_SETUP_PREFIX}-monitor.sock"
_VNC_PASS="admin"

echo "Booting Windows VM from ISO with QEMU..."
echo "  VNC and agent ports: dynamic (printed after boot)"
echo "  QEMU log: $_QEMU_LOG"

qemu-system-aarch64 \
    -machine virt,highmem=on,gic-version=3 \
    -accel hvf \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -drive "if=pflash,format=raw,file=$_UEFI_CODE,readonly=on" \
    -drive "if=pflash,format=raw,file=$_SETUP_EFIVARS" \
    -chardev "socket,id=chrtpm,path=$_TPM_SOCKET" \
    -tpmdev "emulator,id=tpm0,chardev=chrtpm" \
    -device "tpm-tis-device,tpmdev=tpm0" \
    -drive "file=$_SETUP_QCOW2,if=none,id=hd0,format=qcow2" \
    -device "nvme,drive=hd0,serial=boot,bootindex=0" \
    -device "ramfb" \
    -device "qemu-xhci" \
    -device "usb-kbd" \
    -device "usb-tablet" \
    -drive "file=$_CACHED_ISO,if=none,id=cd0,media=cdrom,readonly=on" \
    -device "usb-storage,drive=cd0,bootindex=1" \
    -drive "file=$_AUTOUNATTEND_IMG,if=none,id=unattend,format=raw" \
    -device "usb-storage,drive=unattend,removable=on" \
    -device "virtio-net-pci,netdev=net0" \
    -netdev "user,id=net0,hostfwd=tcp::0-:8648" \
    -vnc "localhost:0,to=99,password=on" \
    -monitor "unix:$_MONITOR_SOCK,server,nowait" \
    -serial "file:$_QEMU_LOG" \
    -d guest_errors \
    -display none 2>>"$_QEMU_LOG" &
_QEMU_PID=$!

sleep 2
if ! kill -0 "$_QEMU_PID" 2>/dev/null; then
    echo "ERROR: QEMU does not appear to have started"
    echo "Log output:"
    cat "$_QEMU_LOG" 2>/dev/null || true
    exit 1
fi
echo "  QEMU running (PID: $_QEMU_PID)"

# --- Set VNC password and send keypresses ---

set +e

for _vnc_try in 1 2 3; do
    (echo "set_password vnc $_VNC_PASS"; sleep 1) | nc -U "$_MONITOR_SOCK" >/dev/null 2>&1 && break
    sleep 1
done

_discover_qemu_ports
echo "  Agent port: $_AGENT_PORT"
echo "  VNC port: $_VNC_PORT"

echo ""
echo "--- QEMU device diagnostics ---"
_MON_OUT=$( (echo "info block"; sleep 1) | nc -U "$_MONITOR_SOCK" 2>/dev/null )
echo "Block devices:"
echo "$_MON_OUT" | grep -E "(cd0|unattend|hd0)" || echo "  (none detected)"
echo "--- end diagnostics ---"
echo ""

# Send periodic keypresses to dismiss "Press any key to boot from CD..." prompt.
(
    for i in $(seq 1 8); do
        sleep 1
        echo "sendkey ret"
    done
) | nc -U "$_MONITOR_SOCK" >/dev/null 2>&1 &
_KEYPRESS_PID=$!

set -e

# --- Automated installation ---
# Windows Setup automatically finds autounattend.xml on the USB drive and:
#   1. Bypasses TPM/SecureBoot/RAM checks
#   2. Loads VirtIO network driver (NetKVM)
#   3. Partitions the NVMe disk (EFI + MSR + NTFS)
#   4. Applies the Windows image from the ISO
#   5. SetupComplete.cmd copies agent + scripts, creates scheduled task
#   6. Creates admin/admin user with autologin
#   7. FirstLogonCommands installs Chocolatey
#   8. Task Scheduler starts the agent on logon
# No SSH is used at any point.

echo ""
echo "=========================================================="
echo "  Automated Windows installation via autounattend.xml"
echo "=========================================================="
echo ""
echo "  Windows Setup will:"
echo "    1. Partition disk and install Windows (~15-25 min)"
echo "    2. Configure admin user and autologin"
echo "    3. Install testanyware-agent and Chocolatey"
echo ""
echo "  Monitor progress via VNC:"
echo "    open vnc://localhost:$_VNC_PORT"
echo "    VNC password: $_VNC_PASS"
echo "=========================================================="
echo ""

# --- Wait for agent ---
# The agent starts via Task Scheduler when admin logs in after OOBE.
# Typical wait: 20-40 minutes for install + first logon.

echo "Waiting for agent on localhost:$_AGENT_PORT..."
echo "(Typical wait: 20-40 minutes for install + agent startup)"

_AGENT_READY=false
_LAST_LOG_SIZE=0
for i in $(seq 1 120); do
    if ! kill -0 "$_QEMU_PID" 2>/dev/null; then
        echo ""
        echo "ERROR: QEMU process died during installation"
        echo "Last QEMU log output:"
        tail -20 "$_QEMU_LOG" 2>/dev/null || true
        exit 1
    fi
    if agent_health | grep -q "accessible"; then
        _AGENT_READY=true
        echo ""
        echo "Agent ready."
        break
    fi

    _ELAPSED=$(( i * 30 ))
    _MINS=$(( _ELAPSED / 60 ))
    _SECS=$(( _ELAPSED % 60 ))
    printf "\r  [%02d:%02d] Waiting for agent..." "$_MINS" "$_SECS"

    if (( i % 4 == 0 )); then
        _CUR_LOG_SIZE=$(wc -c < "$_QEMU_LOG" 2>/dev/null || echo 0)
        if [[ "$_CUR_LOG_SIZE" -gt "$_LAST_LOG_SIZE" ]]; then
            echo ""
            echo "  --- QEMU log (new output) ---"
            tail -5 "$_QEMU_LOG" 2>/dev/null || true
            echo "  ---"
            _LAST_LOG_SIZE=$_CUR_LOG_SIZE
        fi
    fi
    sleep 30
done

if ! $_AGENT_READY; then
    echo ""
    echo "ERROR: Agent not reachable within 60 minutes"
    echo "Connect via VNC to diagnose: open vnc://localhost:$_VNC_PORT"
    echo ""
    echo "QEMU log tail:"
    tail -30 "$_QEMU_LOG" 2>/dev/null || true
    exit 1
fi

# --- Print SetupComplete.log for diagnostics ---

echo "SetupComplete.log:"
agent_exec "type C:\\Windows\\Setup\\Scripts\\SetupComplete.log" 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('stdout','(empty)'))" 2>/dev/null || echo "  (not available)"

# --- Wait for desktop setup to complete ---

echo -n "Waiting for desktop setup to complete..."
for i in $(seq 1 150); do
    if agent_exec "if exist C:\\Windows\\Setup\\Scripts\\desktop-setup-done.txt echo DONE" | grep -q "DONE"; then
        echo " done."
        break
    fi
    echo -n "."
    sleep 2
done

if ! agent_exec "if exist C:\\Windows\\Setup\\Scripts\\desktop-setup-done.txt echo DONE" | grep -q "DONE"; then
    echo ""
    echo "ERROR: Desktop setup script did not complete"
    exit 1
fi

# Let Windows fully settle — background tasks (search indexing, app readiness,
# component store cleanup) run for minutes after first login.
echo "Waiting 60s for Windows to settle..."
sleep 60

# --- Reboot: wallpaper and taskbar changes take full effect on login ---

echo -n "Rebooting to finalize..."
agent_exec "shutdown /r /t 0" >/dev/null 2>&1 || true

# Wait for agent to go away then come back
sleep 15
for i in $(seq 1 120); do
    if agent_health | grep -q "accessible"; then
        echo " back online."
        break
    fi
    echo -n "."
    sleep 5
done

if ! agent_health | grep -q "accessible"; then
    echo ""
    echo "ERROR: VM did not come back online after reboot"
    exit 1
fi

echo "Waiting 30s for final settle..."
sleep 30

# --- Clean desktop before shutdown ---
# Close any startup apps that opened windows so clones inherit a clean desktop.

echo "Cleaning desktop state..."
agent_exec "powershell -Command \"@('GetStarted','Video.UI','HelpPane','SearchHost','SearchApp','PhoneExperienceHost','msedge','Widgets') | ForEach-Object { Get-Process -Name \$_ -ErrorAction SilentlyContinue | Stop-Process -Force }\"" >/dev/null 2>&1 || true

# --- Shutdown ---

if ! kill -0 "$_QEMU_PID" 2>/dev/null; then
    echo "ERROR: QEMU process died unexpectedly"
    exit 1
fi

echo "Shutting down VM..."
agent_shutdown

echo -n "Waiting for shutdown..."
for i in $(seq 1 60); do
    if [[ -z "$_QEMU_PID" ]] || ! kill -0 "$_QEMU_PID" 2>/dev/null; then
        echo " done."
        break
    fi
    echo -n "."
    sleep 2
done
if [[ -n "$_QEMU_PID" ]] && kill -0 "$_QEMU_PID" 2>/dev/null; then
    echo " forcing stop."
    kill "$_QEMU_PID" 2>/dev/null || true
    wait "$_QEMU_PID" 2>/dev/null || true
fi

# Stop swtpm
if [[ -n "$_SWTPM_PID" ]] && kill -0 "$_SWTPM_PID" 2>/dev/null; then
    kill "$_SWTPM_PID" 2>/dev/null || true
    wait "$_SWTPM_PID" 2>/dev/null || true
fi
_SWTPM_PID=""

# --- Finalize golden ---

echo "Creating golden image '$_NAME'..."
mv "$_SETUP_QCOW2" "$_GOLDEN_DIR/$_NAME.qcow2"
mv "$_SETUP_EFIVARS" "$_GOLDEN_DIR/$_NAME-efivars.fd"
mv "$_SETUP_TPM_DIR" "$_GOLDEN_DIR/$_NAME-tpm"

_GOLDEN_DONE=true
_QEMU_PID=""

echo ""
echo "Golden image '$_NAME' created successfully."
echo "  Disk:    $_GOLDEN_DIR/$_NAME.qcow2"
echo "  UEFI:    $_GOLDEN_DIR/$_NAME-efivars.fd"
echo "  TPM:     $_GOLDEN_DIR/$_NAME-tpm/"
echo ""
echo "Use it with:"
echo "  scripts/test-integration.sh --base $_NAME"
echo "  source scripts/vm-start.sh --base $_NAME"
