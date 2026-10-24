#!/usr/bin/env bash
#
# Does the app close the guest connection of a VNC stream once its client has gone?
# Minutes, on a macOS host, with one tart guest that already runs.
#
#   app-stream-end-live.sh <vm-id> [commands]
#
# tart's VNC server keeps a connection open after the client's end and sends nothing more. An
# app that waits for the guest to end its half keeps two descriptors for each VNC command, and
# the app of release 2.4.0 failed after about 120 commands at the launchd limit of 256. This
# check runs many `screen size` commands, which only read, and compares two counts before and
# after:
#   - the descriptors that the app holds open, and
#   - the connections in CLOSE_WAIT that the tart process of the guest holds on its VNC port.
#
# The development bundle must be the app that answers on the socket, so a person quits a
# running app first and opens it again afterwards. This script starts the development bundle,
# and it quits only that one. The guest must run, and the app must hold the Local Network
# allowance (see `app-bundle-live.sh`).
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), XDG_STATE_HOME,
#      APP_BUNDLE (a bundle to run instead of a new development bundle).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "app-stream-end-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
readonly bundle_id=dev.antony.TestAnyware.dev

fail() {
  echo "app-stream-end-live: FAIL: $*" >&2
  exit 1
}

vm=${1:-}
commands=${2:-150}
[[ -n "$vm" ]] || fail "usage: app-stream-end-live.sh <vm-id> [commands]. The tart guest must run."
[[ "$commands" =~ ^[0-9]+$ ]] || fail "the number of commands is not a number: $commands"
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
spec="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/$vm.json"
[[ -f "$spec" ]] || fail "no spec at $spec."
vnc_port=$(jq -r '.vnc.port' "$spec")

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }

started=0
cleanup() {
  if ((started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "TestAnyware Dev.app/Contents/MacOS" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# The tart process that listens on the guest's VNC port.
tart_pid=$(lsof -nP -iTCP:"$vnc_port" -sTCP:LISTEN -t | head -n 1 || true)
[[ -n "$tart_pid" ]] || fail "no process listens on the VNC port $vnc_port of $vm. Does the guest run?"

# 1. The development bundle answers on the socket, and no other app does.
if status >/dev/null 2>&1; then
  fail "an app already answers on $socket. Quit it first (osascript -e 'tell application \"TestAnyware\" to quit'), and open it again after this check."
fi
bundle=${APP_BUNDLE:-$("$root/scripts/build-app.sh" dev | tail -n 1)}
open -n -g "$bundle"
started=1
body=
for _ in $(seq 1 50); do
  if body=$(status 2>/dev/null); then break; fi
  sleep 0.2
done
[[ -n "$body" ]] || fail "the bundle did not answer within 10 s."
echo "app-stream-end-live: ok   started $bundle, build $(jq -r .build <<<"$body")"
app_pid=$(pgrep -f "$bundle/Contents/MacOS/testanyware-app" | head -n 1 || true)
[[ -n "$app_pid" ]] || fail "no process of $bundle."

# Descriptors of the app, and connections that the tart process holds in CLOSE_WAIT.
# lsof exits 1 when it lists nothing, which is the good case here.
app_descriptors() { { lsof -n -P -p "$app_pid" || true; } | tail -n +2 | wc -l | tr -d ' '; }
tart_close_wait() {
  { lsof -n -P -a -p "$tart_pid" -iTCP:"$vnc_port" -sTCP:CLOSE_WAIT || true; } | tail -n +2 | wc -l | tr -d ' '
}

# 2. One command warms the app up, and the counts after it are the baseline. A denial of the
# Local Network fails here and names the person's step.
run() {
  local out
  if ! out=$("$bin" screen size --vm "$vm" 2>&1); then
    if grep -qi 'local.network' <<<"$out"; then
      echo "$out" >&2
      fail "a person must allow the Local Network alert for the app, and then run this again."
    fi
    echo "$out" >&2
    fail "screen size failed."
  fi
  printf '%s' "$out"
}
size=$(run)
sleep 2
app_before=$(app_descriptors)
wait_before=$(tart_close_wait)
echo "app-stream-end-live: info after 1 command: the app holds $app_before descriptors, tart holds $wait_before connections in CLOSE_WAIT"

# 3. Many commands. The app of release 2.4.0 fails after about 120 of them.
for _ in $(seq 1 "$commands"); do
  run >/dev/null
done
sleep 2
app_after=$(app_descriptors)
wait_after=$(tart_close_wait)
echo "app-stream-end-live: info after $((commands + 1)) commands: the app holds $app_after descriptors, tart holds $wait_after connections in CLOSE_WAIT"

# 4. The counts stay flat. A few descriptors may belong to a command that is still ending.
slack=4
((app_after <= app_before + slack)) || fail "the app holds $app_after descriptors after $((commands + 1)) commands, and held $app_before after one."
((wait_after <= wait_before + slack)) || fail "tart holds $wait_after connections in CLOSE_WAIT after $((commands + 1)) commands, and held $wait_before after one."
echo "app-stream-end-live: ok   the descriptors of the app and the connections in CLOSE_WAIT stayed flat over $((commands + 1)) commands"

# 5. The app still serves.
[[ "$(run)" == "$size" ]] || fail "the last command printed another size."
echo "app-stream-end-live: ok   the app still reaches the guest ($(tr -d '\n' <<<"$size"))"
echo "app-stream-end-live: PASS"
