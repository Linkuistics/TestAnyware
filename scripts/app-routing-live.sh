#!/usr/bin/env bash
#
# Does an agent under herdr drive a guest through the TestAnyware app, with no variable set?
# Seconds, on a macOS host, with one guest that a person started.
#
#   app-routing-live.sh <vm-id>
#
# The person does two things first, in Terminal, which has Local Network access:
#   1. Start the guest:  testanyware vm start --platform macos --id <vm-id>
#   2. Allow the Local Network alert for the app, once. The alert appears at the app's first
#      connection to a guest. This script never waits for it. A denial fails the run and names
#      the remedy, and the person runs it again.
#
# The agent runs the rest, from a herdr session:
#   1. Use the app that answers on the socket, or start the development bundle with `open`, as
#      macOS starts a login item. The CLI never starts an app. This script does, and it quits
#      only an app that it started.
#   2. With TESTANYWARE_APP_SOCKET unset, run `agent snapshot`, `screen capture`, `input move`
#      and a file upload and download. Each goes to the default socket.
#   3. Check the control: with the variable naming a socket with no app, the same command
#      fails with APP_UNREACHABLE, so the CLI has no direct path to a guest that answers.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), XDG_STATE_HOME.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "app-routing-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
readonly bundle_id=dev.antony.TestAnyware.dev

fail() {
  echo "app-routing-live: FAIL: $*" >&2
  exit 1
}

vm=${1:-}
[[ -n "$vm" ]] || fail "usage: app-routing-live.sh <vm-id>. A person starts the guest in Terminal first."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
spec="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/$vm.json"
[[ -f "$spec" ]] || fail "no spec at $spec. A person starts the guest in Terminal: testanyware vm start --platform macos --id $vm"

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
  rm -rf "${work:-}"
}
trap cleanup EXIT
work=$(mktemp -d)

if [[ -n "${HERDR_ENV:-}" ]]; then
  echo "app-routing-live: info herdr session $HERDR_PANE_ID"
else
  echo "app-routing-live: info not a herdr session. The run proves the routing, not the herdr case."
fi

# 1
if body=$(status 2>/dev/null); then
  echo "app-routing-live: ok   an app answers on the socket, build $(jq -r .build <<<"$body")"
else
  bundle=$("$root/scripts/build-app.sh" dev | tail -n 1)
  open -n -g "$bundle"
  started=1
  body=
  for _ in $(seq 1 50); do
    if body=$(status 2>/dev/null); then break; fi
    sleep 0.2
  done
  [[ -n "$body" ]] || fail "the development bundle did not answer within 10 s."
  echo "app-routing-live: ok   started $bundle, build $(jq -r .build <<<"$body")"
fi

# The command must succeed, and a denial names the person's step.
run() {
  local name=$1
  shift
  local out
  if ! out=$("$bin" "$@" 2>&1); then
    if grep -qi 'local.network' <<<"$out"; then
      echo "$out" >&2
      fail "$name: a person must allow the Local Network alert for the app, and then run this again. This check never waits for it."
    fi
    echo "$out" >&2
    fail "$name failed."
  fi
  printf '%s' "$out"
}

# 2
out=$(run "agent snapshot" agent snapshot --vm "$vm" --json)
[[ "$(jq -r .ok <<<"$out")" == true ]] || fail "agent snapshot: ok is not true: $out"
echo "app-routing-live: ok   agent snapshot: $(jq '.windows | length' <<<"$out") windows"

png=$work/capture.png
run "screen capture" screen capture --vm "$vm" -o "$png" >/dev/null
[[ "$(head -c 8 "$png" | xxd -p)" == 89504e470d0a1a0a ]] || fail "screen capture wrote no PNG."
echo "app-routing-live: ok   screen capture: $(sips -g pixelWidth -g pixelHeight "$png" | awk '/pixel/ {printf "%s ", $2}')pixels"

out=$(run "input move" input move 10 10 --vm "$vm" --json)
[[ "$(jq -r .ok <<<"$out")" == true ]] || fail "input move: ok is not true: $out"
echo "app-routing-live: ok   input move"

head -c 200000 /dev/urandom >"$work/up.bin"
remote=/tmp/app-routing-live-$$.bin
run "file upload" file upload "$work/up.bin" "$remote" --vm "$vm" >/dev/null
run "file download" file download "$remote" "$work/down.bin" --vm "$vm" >/dev/null
cmp -s "$work/up.bin" "$work/down.bin" || fail "the downloaded file differs from the uploaded one."
run "exec" exec "rm -f $remote" --vm "$vm" >/dev/null
echo "app-routing-live: ok   file transfer: 200000 bytes up and back, unchanged"

# 3
agent_host=$(jq -r '.agent.host' "$spec")
agent_port=$(jq -r '.agent.port' "$spec")
if nc -z -G 3 "$agent_host" "$agent_port" >/dev/null 2>&1; then
  echo "app-routing-live: info control: this shell can dial $agent_host:$agent_port, so a success above does not prove that the app carried it. The next step does."
else
  echo "app-routing-live: ok   control: this shell cannot dial $agent_host:$agent_port, so the app carried each command above"
fi
nowhere=$work/none/testanyware.sock
if out=$(TESTANYWARE_APP_SOCKET=$nowhere "$bin" agent snapshot --vm "$vm" --json 2>&1); then
  fail "with no app on the named socket, agent snapshot still succeeded: $out"
fi
[[ "$(jq -r .code <<<"$out")" == APP_UNREACHABLE ]] || fail "the control did not fail with APP_UNREACHABLE: $out"
echo "app-routing-live: ok   control: with no app on the socket, the CLI fails with APP_UNREACHABLE and has no direct path"
echo "app-routing-live: PASS"
