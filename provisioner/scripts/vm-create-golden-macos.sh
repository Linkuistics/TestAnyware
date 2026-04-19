#!/bin/bash
# Create a golden macOS VM image with testanyware-agent as a LaunchAgent service.
# Deletes any existing golden image with the same name first.
#
# Usage:
#   scripts/vm-create-golden-macos.sh [options]
#
# Options:
#   --version VERSION   macOS version: tahoe, sequoia, sonoma (default: tahoe)
#   --name NAME         Golden image name (default: testanyware-golden-macos-VERSION)
#
# Prerequisites:
#   - tart installed (/opt/homebrew/bin/tart)
#   - SSH public key at ~/.ssh/id_ed25519.pub or ~/.ssh/id_rsa.pub
#
# What this creates:
#   A tart VM cloned from Cirrus Labs' vanilla macOS image with:
#   - testanyware-agent running as LaunchAgent on port 8648
#   - TCC accessibility permission granted (SIP disable/enable cycle)
#   - Host SSH public key in ~/.ssh/authorized_keys
#   - Xcode CLI tools, Homebrew
#   - Session restore disabled, clean desktop state
#
# Boot sequence (3 normal + 2 recovery = 5 boots):
#   1. Normal: SSH key, defaults, CLT, Homebrew, agent + plist → shutdown
#   2. Recovery: disable SIP → Normal: TCC grant → shutdown
#   3. Recovery: enable SIP → Normal: verify agent health → shutdown → clone
#
# The golden image is never run directly — clone from it for each test session.

set -euo pipefail

_VERSION="tahoe"
_NAME=""
_VANILLA_USER="admin"
_VANILLA_PASS="admin"
_SETUP_VM="testanyware-setup-$$"
_SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=30"
_TESTANYWARE_BIN=""  # Set by install_agent(), used by _recovery_boot_csrutil()

while [[ $# -gt 0 ]]; do
    case $1 in
        --version) _VERSION="$2"; shift 2 ;;
        --name)    _NAME="$2"; shift 2 ;;
        *)         echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$_NAME" ]]; then
    _NAME="testanyware-golden-macos-$_VERSION"
fi

_VANILLA="ghcr.io/cirruslabs/macos-$_VERSION-vanilla:latest"

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
_VNC_OUTPUT=$(mktemp)
tart run "$_SETUP_VM" --no-graphics --vnc-experimental > "$_VNC_OUTPUT" 2>&1 &
_TART_PID=$!

# Wait for VNC (just to confirm it's booting)
for i in $(seq 1 60); do
    if grep -q 'vnc://' "$_VNC_OUTPUT" 2>/dev/null; then
        break
    fi
    sleep 1
done
rm -f "$_VNC_OUTPUT"

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

# --- Disable session restore ---

echo "Configuring macOS defaults..."
vm_ssh "defaults write NSGlobalDomain NSQuitAlwaysKeepsWindows -bool false"
vm_ssh "defaults write com.apple.loginwindow TALLogoutSavesState -bool false"
vm_ssh "defaults write com.apple.loginwindow LoginwindowLaunchesRelaunchApps -bool false"
vm_ssh "defaults write com.apple.Terminal NSQuitAlwaysKeepsWindows -bool false"

# --- Set solid wallpaper ---
# A solid background makes visual processing of screenshots more reliable.
# We compile a tiny helper on the host (needs AppKit/NSWorkspace) and SCP
# it to the VM since the vanilla image has no dev tools yet.

echo "Setting wallpaper to solid gray..."
_HELPER_SRC="$(cd "$(dirname "$0")/.." && pwd)/helpers/set-wallpaper.swift"
_HELPER_BIN=$(mktemp)
if [[ -f "$_HELPER_SRC" ]] && swiftc -o "$_HELPER_BIN" "$_HELPER_SRC" 2>/dev/null; then
    # Create a 1x1 mid-gray (128,128,128) PNG and scale it with sips
    vm_ssh 'echo "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNoaGgAAAMEAYFL09IQAAAAAElFTkSuQmCC" | base64 -d > /tmp/solid.png && sips -z 1080 1920 /tmp/solid.png >/dev/null 2>&1 && mkdir -p ~/Pictures && mv /tmp/solid.png ~/Pictures/solid_gray.png'
    vm_scp "$_HELPER_BIN" "/tmp/set-wallpaper"
    vm_ssh "chmod +x /tmp/set-wallpaper && /tmp/set-wallpaper /Users/$_VANILLA_USER/Pictures/solid_gray.png && rm /tmp/set-wallpaper"
else
    echo "WARNING: Could not compile set-wallpaper helper — skipping wallpaper"
fi
rm -f "$_HELPER_BIN"

# --- Hide desktop widgets ---

echo "Hiding desktop widgets..."
vm_ssh "defaults write com.apple.WindowManager StandardHideWidgets -bool true"

# --- Install Xcode Command Line Tools ---

echo "Installing Xcode Command Line Tools (this takes a few minutes)..."
vm_ssh "touch /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress"
_CLT_LABEL=$(vm_ssh "softwareupdate -l 2>&1 | grep -B 1 'Command Line Tools' | grep '\\*' | head -1 | sed 's/^.*\\* Label: //'" || true)
if [[ -n "$_CLT_LABEL" ]]; then
    echo "  Found: $_CLT_LABEL"
    vm_ssh "softwareupdate --install '$_CLT_LABEL' --verbose 2>&1 | tail -1"
    vm_ssh "rm -f /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress"
    if vm_ssh "xcode-select -p" &>/dev/null; then
        echo "  Xcode CLI tools installed."
    else
        echo "  WARNING: Xcode CLI tools installation may have failed"
    fi
else
    echo "  WARNING: Could not find Xcode CLI tools in software update — skipping"
    vm_ssh "rm -f /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress"
fi

# --- Install Homebrew ---

echo "Installing Homebrew..."
vm_ssh 'NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
vm_ssh 'echo '\''eval "$(/opt/homebrew/bin/brew shellenv)"'\'' >> ~/.zprofile'
if vm_ssh "/opt/homebrew/bin/brew --version" &>/dev/null; then
    echo "  Homebrew installed."
else
    echo "  WARNING: Homebrew installation may have failed"
fi

# --- Close Terminal and clean desktop state ---

echo "Closing Terminal..."
vm_ssh "killall Terminal 2>/dev/null || true"
sleep 2
vm_ssh "rm -rf ~/Library/Saved\ Application\ State/*" 2>/dev/null || true

# --- Agent install and TCC/SIP functions ---

install_agent() {
    local _PROJECT_ROOT
    _PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

    echo "Building testanyware (host CLI)..."
    local _CLI_PKG="$_PROJECT_ROOT/cli/macos"
    local _HOST_BIN_PATH
    _HOST_BIN_PATH=$(swift build --package-path "$_CLI_PKG" -c release --show-bin-path 2>/dev/null)
    swift build --package-path "$_CLI_PKG" -c release
    _TESTANYWARE_BIN="$_HOST_BIN_PATH/testanyware"
    if [[ ! -f "$_TESTANYWARE_BIN" ]]; then
        echo "ERROR: testanyware binary not found at $_TESTANYWARE_BIN"
        exit 1
    fi

    echo "Building testanyware-agent (macOS agent)..."
    local _AGENT_PKG="$_PROJECT_ROOT/agents/macos"
    local _AGENT_BIN_PATH
    _AGENT_BIN_PATH=$(swift build --package-path "$_AGENT_PKG" -c release --show-bin-path 2>/dev/null)
    swift build --package-path "$_AGENT_PKG" -c release
    local _AGENT_BIN="$_AGENT_BIN_PATH/testanyware-agent"
    if [[ ! -f "$_AGENT_BIN" ]]; then
        echo "ERROR: testanyware-agent binary not found at $_AGENT_BIN"
        exit 1
    fi

    echo "Installing testanyware-agent to VM..."
    vm_scp "$_AGENT_BIN" "/tmp/testanyware-agent"
    vm_ssh "sudo mkdir -p /usr/local/bin"
    vm_ssh "sudo mv /tmp/testanyware-agent /usr/local/bin/testanyware-agent"
    vm_ssh "sudo chmod +x /usr/local/bin/testanyware-agent"

    echo "Verifying testanyware-agent install..."
    if vm_ssh "test -x /usr/local/bin/testanyware-agent"; then
        echo "  testanyware-agent binary installed."
    else
        echo "  ERROR: testanyware-agent binary not executable"
        exit 1
    fi

    echo "Installing launchd plist..."
    local _PLIST_SRC
    _PLIST_SRC="$(cd "$(dirname "$0")/.." && pwd)/helpers/com.linkuistics.testanyware.agent.plist"
    vm_scp "$_PLIST_SRC" "/tmp/com.linkuistics.testanyware.agent.plist"
    vm_ssh "mkdir -p ~/Library/LaunchAgents"
    vm_ssh "mv /tmp/com.linkuistics.testanyware.agent.plist ~/Library/LaunchAgents/"
    echo "  LaunchAgent plist installed."
}

# Shared helper: gracefully stop the VM and wait for the tart process to exit.
_stop_vm_graceful() {
    echo -n "Shutting down VM..."
    # Use System Events for clean shutdown — sudo shutdown -h now kills loginwindow
    # before it saves session state, causing apps to relaunch on next boot.
    vm_ssh "osascript -e 'tell application \"System Events\" to shut down'" 2>/dev/null || true
    for i in $(seq 1 60); do
        if ! kill -0 "$_TART_PID" 2>/dev/null; then
            echo " done."
            return 0
        fi
        echo -n "."
        sleep 2
    done
    echo " forcing stop."
    tart stop "$_SETUP_VM" 2>/dev/null || true
    wait "$_TART_PID" 2>/dev/null || true
}

# Shared helper: ensure the VM is running and SSH-reachable.
# If the tart process has exited (e.g. after a logout-triggered reboot),
# restart it and wait for SSH.
_ensure_vm_running() {
    if kill -0 "$_TART_PID" 2>/dev/null; then
        # VM is still running — verify SSH
        if ssh $_SSH_OPTS -o ConnectTimeout=5 "$_VANILLA_USER@$_IP" "true" 2>/dev/null; then
            return 0
        fi
    fi
    echo "VM process exited — restarting..."
    tart stop "$_SETUP_VM" 2>/dev/null || true
    wait "$_TART_PID" 2>/dev/null || true
    tart run "$_SETUP_VM" --no-graphics &
    _TART_PID=$!
    _wait_for_ssh_ready
}

# Shared helper: wait for SSH to become available after a reboot.
_wait_for_ssh_ready() {
    echo -n "Waiting for SSH..."
    for i in $(seq 1 60); do
        _IP=$(tart ip "$_SETUP_VM" 2>/dev/null | tr -d '[:space:]' || true)
        if [[ -n "$_IP" ]]; then
            if ssh $_SSH_OPTS "$_VANILLA_USER@$_IP" "true" 2>/dev/null; then
                echo " ready (IP: $_IP)"
                return 0
            fi
        fi
        echo -n "."
        sleep 3
    done
    echo ""
    echo "ERROR: SSH not reachable within 180s after reboot"
    exit 1
}

# Shared helper: boot into recovery, run a csrutil command via VNC automation,
# then reboot normally.
#
# Sequence (matches TestAnyware/VMCommands.swift recoveryRun()):
#   1. Boot with --recovery --vnc-experimental
#   2. Navigate startup picker: Right→Right→Enter to reach recovery desktop
#   3. Open Terminal via press-hold-drag on Utilities menu bar item
#   4. Run csrutil command, confirm with y + username + password
#   5. Halt via 'halt' command in Terminal
#   6. Restart normally, wait for SSH
#
# Requires: $_TESTANYWARE_BIN set by install_agent()
_recovery_boot_csrutil() {
    local _CSRUTIL_CMD="$1"
    local _LABEL="$2"

    echo "=== SIP: ${_LABEL} via Recovery Mode ==="
    _stop_vm_graceful

    echo "Booting into Recovery Mode with VNC..."
    local _VNC_OUTPUT
    _VNC_OUTPUT=$(mktemp /tmp/testanyware-vnc-XXXXXX)
    tart run "$_SETUP_VM" --recovery --no-graphics --vnc-experimental > "$_VNC_OUTPUT" 2>&1 &
    _TART_PID=$!

    # Wait for VNC endpoint in tart output
    local _VNC_URL=""
    echo -n "Waiting for VNC..."
    for i in $(seq 1 60); do
        _VNC_URL=$(grep -o 'vnc://[^ ]*' "$_VNC_OUTPUT" 2>/dev/null || true)
        if [[ -n "$_VNC_URL" ]]; then
            echo " available ($_VNC_URL)"
            break
        fi
        echo -n "."
        sleep 1
    done
    rm -f "$_VNC_OUTPUT"

    if [[ -z "$_VNC_URL" ]]; then
        echo ""
        echo "ERROR: VNC not available for recovery boot"
        exit 1
    fi

    # Extract host:port and optional password from vnc://[:password@]host:port
    local _VNC_STRIPPED
    _VNC_STRIPPED=$(echo "$_VNC_URL" | sed 's|vnc://||')
    local _VNC_HOST_PORT _VNC_PASSWORD=""
    if [[ "$_VNC_STRIPPED" == *@* ]]; then
        _VNC_PASSWORD=$(echo "$_VNC_STRIPPED" | sed 's|@.*||; s|^:||')
        _VNC_HOST_PORT=$(echo "$_VNC_STRIPPED" | sed 's|.*@||')
    else
        _VNC_HOST_PORT="$_VNC_STRIPPED"
    fi

    # Split host and port for the connection spec
    local _VNC_HOST _VNC_PORT
    _VNC_HOST=$(echo "$_VNC_HOST_PORT" | cut -d: -f1)
    _VNC_PORT=$(echo "$_VNC_HOST_PORT" | cut -d: -f2)

    # Write connection spec for testanyware
    local _PW_JSON="null"
    if [[ -n "$_VNC_PASSWORD" ]]; then
        _PW_JSON="\"$_VNC_PASSWORD\""
    fi
    local _CONNECT_SPEC
    _CONNECT_SPEC=$(mktemp /tmp/testanyware-recovery-XXXXXX)
    cat > "$_CONNECT_SPEC" <<SPECEOF
{"vnc":{"host":"${_VNC_HOST}","port":${_VNC_PORT},"password":${_PW_JSON}}}
SPECEOF
    # --connect must come after the subcommand (swift-argument-parser requirement)
    local _GV_CONN="--connect $_CONNECT_SPEC"

    # --- Step 1: Wait for VNC framebuffer to be available ---
    echo -n "Waiting for Recovery VNC framebuffer..."
    for i in $(seq 1 90); do
        if "$_TESTANYWARE_BIN" screen-size $_GV_CONN &>/dev/null; then
            echo " ready."
            break
        fi
        echo -n "."
        sleep 2
    done

    # --- Step 2: Navigate startup picker ---
    # Cirrus Labs vanilla images boot recovery to a startup disk picker
    # (showing "Macintosh HD" and "Options"). Wait for "Options" via OCR,
    # then navigate with Right→Right→Enter.
    echo "Waiting for startup picker (OCR: 'Options')..."
    "$_TESTANYWARE_BIN" find-text $_GV_CONN "Options" --timeout 120 >/dev/null 2>&1 || true
    sleep 1

    echo "Navigating startup picker (Right→Right→Enter → Options)..."
    "$_TESTANYWARE_BIN" input key $_GV_CONN right
    sleep 0.3
    "$_TESTANYWARE_BIN" input key $_GV_CONN right
    sleep 0.3
    "$_TESTANYWARE_BIN" input key $_GV_CONN return
    sleep 0.3

    # --- Step 3: Wait for recovery desktop ---
    # Wait for "Utilities" to appear in the menu bar (confirms recovery desktop loaded).
    echo "Waiting for recovery desktop (OCR: 'Utilities')..."
    "$_TESTANYWARE_BIN" find-text $_GV_CONN "Utilities" --timeout 120 >/dev/null 2>&1 || true
    sleep 1

    # --- Step 4: Open Terminal via Utilities menu ---
    # The recovery desktop has a modal app-picker that blocks Cmd+T.
    # Open the Utilities menu by clicking, then type "t" to jump to Terminal
    # (macOS menus support type-to-select).
    # Menu item positions determined by pixel analysis of recovery screenshots:
    #   Apple(x≈27) | macOS Recovery(x≈84) | [item](x≈146) | Utilities(x≈181)
    echo "Opening Terminal via Utilities menu (OCR + drag)..."
    # The recovery desktop has a modal dialog that intercepts clicks.
    # Use drag (mouse-down → hold → move → mouse-up) to bypass it,
    # with OCR to find exact coordinates. Matches TestAnyware approach.

    # Find "Utilities" in the menu bar.
    local _UTIL_JSON
    _UTIL_JSON=$("$_TESTANYWARE_BIN" find-text $_GV_CONN "Utilities" --timeout 10 2>/dev/null || echo "[]")
    local _UX=250 _UY=14
    if [[ "$_UTIL_JSON" != "[]" ]]; then
        _UX=$(echo "$_UTIL_JSON" | python3 -c "import sys,json; m=json.load(sys.stdin)[0]; print(int(m['x']+m['width']/2))")
        _UY=$(echo "$_UTIL_JSON" | python3 -c "import sys,json; m=json.load(sys.stdin)[0]; print(int(m['y']+m['height']/2))")
        echo "  Found 'Utilities' at ($_UX, $_UY)"
    else
        echo "  WARNING: OCR did not find 'Utilities' — using fallback ($_UX, $_UY)"
    fi

    # Mouse-down on Utilities to open the dropdown (hold button).
    "$_TESTANYWARE_BIN" input mouse-down $_GV_CONN "$_UX" "$_UY"
    sleep 2  # Dropdown renders while button is held

    # While dropdown is open, use OCR to find "Terminal".
    local _TERM_JSON
    _TERM_JSON=$("$_TESTANYWARE_BIN" find-text $_GV_CONN "Terminal" --timeout 5 2>/dev/null || echo "[]")
    local _TX=300 _TY=95
    if [[ "$_TERM_JSON" != "[]" ]]; then
        _TX=$(echo "$_TERM_JSON" | python3 -c "import sys,json; m=json.load(sys.stdin)[0]; print(int(m['x']+m['width']/2))")
        _TY=$(echo "$_TERM_JSON" | python3 -c "import sys,json; m=json.load(sys.stdin)[0]; print(int(m['y']+m['height']/2))")
        echo "  Found 'Terminal' at ($_TX, $_TY)"
    else
        echo "  WARNING: OCR did not find 'Terminal' — using fallback ($_TX, $_TY)"
    fi

    # Drag to Terminal and release to select it.
    "$_TESTANYWARE_BIN" input move $_GV_CONN "$_TX" "$_TY"
    sleep 0.3
    "$_TESTANYWARE_BIN" input mouse-up $_GV_CONN "$_TX" "$_TY"
    sleep 5

    # --- Step 5: Run csrutil command ---
    # csrutil (both disable and enable on Tahoe) prompts for y/n, username, password.
    # We use generous fixed sleeps between prompts.
    echo "Running '${_CSRUTIL_CMD}'..."
    "$_TESTANYWARE_BIN" input type $_GV_CONN "$_CSRUTIL_CMD"
    sleep 0.5
    "$_TESTANYWARE_BIN" input key $_GV_CONN return
    sleep 15  # Wait for csrutil to show prompt

    echo "  Confirming with 'y'..."
    "$_TESTANYWARE_BIN" input type $_GV_CONN "y"
    sleep 0.2
    "$_TESTANYWARE_BIN" input key $_GV_CONN return
    sleep 10  # Wait for username prompt

    echo "  Entering username..."
    "$_TESTANYWARE_BIN" input type $_GV_CONN "$_VANILLA_USER"
    sleep 0.2
    "$_TESTANYWARE_BIN" input key $_GV_CONN return
    sleep 10  # Wait for password prompt

    echo "  Entering password..."
    "$_TESTANYWARE_BIN" input type $_GV_CONN "$_VANILLA_PASS"
    sleep 0.2
    "$_TESTANYWARE_BIN" input key $_GV_CONN return
    sleep 15  # Wait for csrutil to complete

    # --- Step 6: Halt VM from recovery Terminal ---
    echo "Halting recovery VM..."
    "$_TESTANYWARE_BIN" input type $_GV_CONN "halt"
    "$_TESTANYWARE_BIN" input key $_GV_CONN return

    rm -f "$_CONNECT_SPEC"

    # Wait for tart process to exit after halt (with force-stop fallback)
    echo -n "Waiting for VM to halt..."
    for i in $(seq 1 60); do
        if ! kill -0 "$_TART_PID" 2>/dev/null; then
            echo " done."
            break
        fi
        echo -n "."
        sleep 2
    done
    if kill -0 "$_TART_PID" 2>/dev/null; then
        echo " halt did not shut down VM — forcing stop."
        tart stop "$_SETUP_VM" 2>/dev/null || true
    fi
    wait "$_TART_PID" 2>/dev/null || true

    # --- Step 7: Restart normally and wait for SSH ---
    # Use --vnc-experimental to ensure WindowServer starts (needed for TCC/accessibility).
    echo "Rebooting normally after ${_LABEL}..."
    tart run "$_SETUP_VM" --no-graphics --vnc-experimental &
    _TART_PID=$!
    _wait_for_ssh_ready
}

recovery_boot_disable_sip() {
    _recovery_boot_csrutil "csrutil disable" "disabling SIP"
}

recovery_boot_enable_sip() {
    _recovery_boot_csrutil "csrutil enable" "re-enabling SIP"
}

# Write the testanyware-agent accessibility grant directly into the system-level
# TCC database.  SIP MUST be disabled before this sqlite3 write will succeed —
# call this function between recovery_boot_disable_sip and recovery_boot_enable_sip.
grant_accessibility_permission() {
    echo "Granting accessibility permission to testanyware-agent..."

    # Stop tccd before writing — it holds a lock on TCC.db and causes
    # "database is locked" errors.  launchd will restart it automatically.
    echo "  Stopping tccd to release database lock..."
    vm_ssh "sudo killall tccd" 2>/dev/null || true
    sleep 2

    # Generate the csreq blob from the binary's designated code signing requirement.
    # macOS Tahoe requires this field for TCC to accept the entry.
    echo "  Generating code signing requirement blob..."
    vm_ssh 'CSREQ_HEX=$(codesign -dr- /usr/local/bin/testanyware-agent 2>&1 \
        | sed -n "s/.*=> //p" \
        | csreq -r- -b /dev/stdout \
        | xxd -p \
        | tr -d "\n") && \
        sudo sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
        "INSERT OR REPLACE INTO access \
          (service, client, client_type, auth_value, auth_reason, auth_version, \
           csreq, indirect_object_identifier_type, indirect_object_identifier, flags, last_modified) \
        VALUES \
          ('"'"'kTCCServiceAccessibility'"'"', '"'"'/usr/local/bin/testanyware-agent'"'"', 1, 2, 0, 1, \
           X'"'"'${CSREQ_HEX}'"'"', 0, '"'"'UNUSED'"'"', 0, CAST(strftime('"'"'%s'"'"','"'"'now'"'"') AS INTEGER));"'

    local _RESULT
    _RESULT=$(vm_ssh "sudo sqlite3 \"/Library/Application Support/com.apple.TCC/TCC.db\" \
        \"SELECT client, length(csreq) FROM access WHERE service='kTCCServiceAccessibility' \
          AND client='/usr/local/bin/testanyware-agent';\"" 2>/dev/null || true)
    if echo "$_RESULT" | grep -q "testanyware-agent"; then
        echo "  Accessibility permission granted (csreq: $(echo "$_RESULT" | cut -d'|' -f2) bytes)."
    else
        echo "  ERROR: TCC insert verification failed — SIP may still be enabled or sqlite3 unavailable"
        exit 1
    fi

    # Restart tccd so it re-reads the database (it caches TCC decisions).
    echo "  Restarting tccd to flush TCC cache..."
    vm_ssh "sudo killall tccd" 2>/dev/null || true
    sleep 3
}

# --- Install agent and launchd plist (still in boot 1) ---

install_agent

# --- SIP/TCC cycle ---
# defaults write changes take effect for new processes immediately — no separate
# reboot needed. The recovery-cycle's normal reboot will pick them up.

recovery_boot_disable_sip
echo "Verifying SIP is disabled..."
_SIP_STATUS=$(vm_ssh "csrutil status" 2>/dev/null || echo "unknown")
echo "  SIP status: $_SIP_STATUS"
if echo "$_SIP_STATUS" | grep -q "disabled"; then
    echo "  SIP successfully disabled."
else
    echo "  WARNING: SIP may not be disabled — csrutil output: $_SIP_STATUS"
    echo "  Check /tmp/testanyware-recovery-*.png screenshots for debugging."
fi

grant_accessibility_permission

# Verify the TCC entry directly in the database.
# Note: AXIsProcessTrusted() returns false when called via SSH because macOS
# Ventura+ checks the "responsible client" (sshd, not testanyware-agent).
# The TCC entry IS correct and will work when the agent is launched by launchd
# during actual testing (matching TestAnyware's approach).
echo "Verifying TCC database entry..."
_TCC_CHECK=$(vm_ssh "sudo sqlite3 '/Library/Application Support/com.apple.TCC/TCC.db' \
    'SELECT auth_value, length(csreq) FROM access \
     WHERE service=\"kTCCServiceAccessibility\" AND client=\"/usr/local/bin/testanyware-agent\";'" 2>/dev/null || true)
echo "  TCC entry: $_TCC_CHECK"
if echo "$_TCC_CHECK" | grep -q "^2|"; then
    echo "  TCC accessibility grant verified (auth_value=2)."
else
    echo "  ERROR: TCC entry missing or denied"
    exit 1
fi

recovery_boot_enable_sip
echo "Verifying SIP is re-enabled..."
_SIP_STATUS=$(vm_ssh "csrutil status" 2>/dev/null || echo "unknown")
echo "  SIP status: $_SIP_STATUS"
if echo "$_SIP_STATUS" | grep -q "enabled"; then
    echo "  SIP successfully re-enabled."
else
    echo "  WARNING: SIP may not be re-enabled — csrutil output: $_SIP_STATUS"
fi

echo "Final agent verification..."

# Verify TCC entry survives SIP re-enable
_TCC_FINAL=$(vm_ssh "sudo sqlite3 '/Library/Application Support/com.apple.TCC/TCC.db' \
    'SELECT auth_value, length(csreq) FROM access \
     WHERE service=\"kTCCServiceAccessibility\" AND client=\"/usr/local/bin/testanyware-agent\";'" 2>/dev/null || true)
echo "  TCC entry: $_TCC_FINAL"
if ! echo "$_TCC_FINAL" | grep -q "^2|"; then
    echo "  ERROR: TCC accessibility entry missing after SIP re-enable"
    exit 1
fi

# Verify agent is running via launchd and responding on port 8648
echo "  Checking agent health on port 8648..."
_AGENT_HEALTHY=false
for i in $(seq 1 30); do
    if vm_ssh "curl -sf http://localhost:8648/health" &>/dev/null; then
        _AGENT_HEALTHY=true
        break
    fi
    sleep 2
done
if $_AGENT_HEALTHY; then
    echo "  Agent is running and healthy on port 8648."
else
    echo "  ERROR: Agent not responding on port 8648 — check launchd and /tmp/testanyware-agent.*.log"
    exit 1
fi
echo "  testanyware-agent installed, TCC verified, health OK."

# --- Clean desktop state before shutdown ---
# Close Terminal and clear saved application state so the golden image
# boots to a clean desktop. Terminal may have opened during the SIP/TCC
# recovery boot cycle or from SSH session activity.

echo "Cleaning desktop state..."
# The Cirrus Labs vanilla image boots with Terminal open by default.
# Kill it and clear saved state before a CLEAN shutdown.
vm_ssh "killall Terminal 2>/dev/null || true"
sleep 2
vm_ssh "rm -rf ~/Library/Saved\ Application\ State/*" 2>/dev/null || true

# --- Clean shutdown ---
# CRITICAL: Use System Events "shut down" (not `sudo shutdown -h now`).
# `sudo shutdown -h now` kills loginwindow before it can save session state,
# causing apps (including Terminal) to be relaunched on next boot.
# System Events triggers loginwindow's full shutdown sequence which properly
# records that no apps are open. (From TestAnyware/VMCommands.swift.)

echo "Shutting down VM (clean, via System Events)..."
vm_ssh "osascript -e 'tell application \"System Events\" to shut down'" 2>/dev/null || true

echo -n "Waiting for shutdown..."
for i in $(seq 1 60); do
    if ! kill -0 "$_TART_PID" 2>/dev/null; then
        echo " done."
        break
    fi
    echo -n "."
    sleep 2
done
if kill -0 "$_TART_PID" 2>/dev/null; then
    echo " forcing stop."
    tart stop "$_SETUP_VM" 2>/dev/null || true
    wait "$_TART_PID" 2>/dev/null || true
fi

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
