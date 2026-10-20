#!/usr/bin/env bash
#
# The herdr acceptance run: from a herdr session, an agent starts, drives and stops a macOS
# guest through the TestAnyware app, with no variable set. About two minutes, on a macOS host.
#
#   herdr-acceptance-live.sh <app-bundle>
#
# The same script checks the development bundle (`task app:herdr-acceptance`) and the release
# bundle of a candidate (release-publish-k37).
#
# A person does one thing first: allow the Local Network alert for the app, once. The alert
# appears at the app's first connection to a guest. This script never waits for it. A denial
# fails the run and names the remedy, and the person runs it again.
#
# The agent runs the rest, from a herdr session:
#   1. Start the app from the bundle with `open`, as macOS starts a login item. The CLI never
#      starts an app. This script does, and it quits only an app that it started. It refuses to
#      run when another app already answers, because that answer would not prove this bundle.
#   2. With TESTANYWARE_APP_SOCKET unset, run `vm start --platform macos`, then one agent
#      command (agent snapshot), one screen command (screen capture), one input command
#      (input move) and one file command (file upload, and download to compare), then `vm stop`.
#   3. Show that each command went through the app: the same command with the variable naming a
#      socket with no app fails with APP_UNREACHABLE, so the CLI has no direct path.
#
# The script stops its guest on every exit, failure included.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), XDG_STATE_HOME,
#      MAX_START_SECS (default 240; the bound on `vm start`),
#      MAX_CLEANUP_SECS (default 30; how long the app may take to remove a stopped guest).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "herdr-acceptance: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
vms_dir="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms"
max_cleanup=${MAX_CLEANUP_SECS:-30}

fail() {
  echo "herdr-acceptance: FAIL: $*" >&2
  exit 1
}

ok() {
  echo "herdr-acceptance: ok   $*"
}

bundle=${1:-}
[[ -n "$bundle" ]] || fail "usage: herdr-acceptance-live.sh <app-bundle>"
[[ -d "$bundle/Contents/MacOS" ]] || fail "$bundle is not an app bundle."
bundle=$(cd "$bundle" && pwd)
bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" 2>/dev/null) ||
  fail "no CFBundleIdentifier in $bundle/Contents/Info.plist."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
for tool in jq tart curl cmp; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
bundle_running() { pgrep -f "$bundle/Contents/MacOS/" >/dev/null; }
tart_names() { tart list --format json | jq -r '.[].Name'; }

id=herdr-acceptance-$$
app_started=0
work=$(mktemp -d)

# Stop the guest of this run on every exit. The app reclaims it first. When the app cannot, tart
# removes it, so a failed run leaves no guest that holds one of the two macOS slots.
cleanup() {
  local rc=$?
  trap - ERR
  if [[ -f "$vms_dir/$id.json" ]] && status >/dev/null 2>&1; then
    "$bin" vm stop "$id" >/dev/null 2>&1 || true
  fi
  if tart_names 2>/dev/null | grep -qx "$id"; then
    echo "herdr-acceptance: $id survived the app's stop. Removing it with tart." >&2
    tart stop "$id" >/dev/null 2>&1 || true
    tart delete "$id" >/dev/null 2>&1 || true
    rm -f "$vms_dir/$id".*
  fi
  if ((app_started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "$bundle/Contents/MacOS/" 2>/dev/null || true
  fi
  rm -rf "$work"
  exit "$rc"
}
trap cleanup EXIT

if [[ -n "${HERDR_ENV:-}" ]]; then
  echo "herdr-acceptance: info herdr session $HERDR_PANE_ID"
else
  echo "herdr-acceptance: info not a herdr session. The run proves the routing, not the herdr case."
fi

running=$(tart list --format json | jq -r '[.[] | select(.State == "running")] | length')
((running < 2)) || fail "both macOS guest slots are taken. Stop a guest, and run this again."

# 1
if body=$(status 2>/dev/null); then
  bundle_running || fail "an app that is not $bundle answers on the socket. Quit it, and run this again."
  ok "$bundle already answers, build $(jq -r .build <<<"$body")"
else
  open -n -g "$bundle"
  app_started=1
  body=
  for _ in $(seq 1 50); do
    if body=$(status 2>/dev/null); then break; fi
    sleep 0.2
  done
  [[ -n "$body" ]] || fail "$bundle did not answer within 10 s."
  ok "started $bundle, build $(jq -r .build <<<"$body")"
fi

# 2: a command must succeed. A Local Network denial names the person's step.
grant_step="a person must allow the Local Network alert for the app, once, and then run this again. This check never waits for it."
run() {
  local name=$1
  shift
  local out
  # Progress goes to stderr, so a JSON result on stdout stays parseable.
  if ! out=$("$bin" "$@" 2>"$work/stderr"); then
    out="$out$(<"$work/stderr")"
    echo "$out" >&2
    if grep -qi 'local.network' <<<"$out"; then
      fail "$name: $grant_step"
    fi
    fail "$name failed."
  fi
  printf '%s' "$out"
}

begin=$SECONDS
out=$(run "vm start" vm start --platform macos --id "$id" --json)
[[ "$(jq -r .id <<<"$out")" == "$id" ]] || fail "vm start: the id is not $id: $out"
ok "vm start: $id reached a healthy agent in $((SECONDS - begin)) s"
((SECONDS - begin <= ${MAX_START_SECS:-240})) || fail "vm start took more than ${MAX_START_SECS:-240} s."

out=$(run "agent snapshot" agent snapshot --vm "$id" --json)
[[ "$(jq -r .ok <<<"$out")" == true ]] || fail "agent snapshot: ok is not true: $out"
ok "agent snapshot: $(jq '.windows | length' <<<"$out") windows"

png=$work/capture.png
run "screen capture" screen capture --vm "$id" -o "$png" >/dev/null
[[ "$(head -c 8 "$png" | xxd -p)" == 89504e470d0a1a0a ]] || fail "screen capture wrote no PNG."
ok "screen capture: $(sips -g pixelWidth -g pixelHeight "$png" | awk '/pixel/ {printf "%s ", $2}')pixels"

out=$(run "input move" input move 10 10 --vm "$id" --json)
[[ "$(jq -r .ok <<<"$out")" == true ]] || fail "input move: ok is not true: $out"
ok "input move"

head -c 200000 /dev/urandom >"$work/up.bin"
remote=/tmp/herdr-acceptance-$$.bin
run "file upload" file upload "$work/up.bin" "$remote" --vm "$id" >/dev/null
run "file download" file download "$remote" "$work/down.bin" --vm "$id" >/dev/null
cmp -s "$work/up.bin" "$work/down.bin" || fail "the downloaded file differs from the uploaded one."
run "exec" exec "rm -f $remote" --vm "$id" >/dev/null
ok "file transfer: 200000 bytes up and back, unchanged"

# 3: the control. A shell under herdr that can dial the guest would hide a direct path, so the
# script says which case this is, and the named dead socket removes the doubt for each command.
agent_host=$(jq -r '.agent.host' "$vms_dir/$id.json")
agent_port=$(jq -r '.agent.port' "$vms_dir/$id.json")
if nc -z -G 3 "$agent_host" "$agent_port" >/dev/null 2>&1; then
  echo "herdr-acceptance: info this shell can dial $agent_host:$agent_port, so a success above does not prove that the app carried it. The controls below do."
else
  ok "this shell cannot dial $agent_host:$agent_port, so the app carried each command above"
fi

nowhere=$work/none/testanyware.sock
control() {
  local name=$1
  shift
  local out
  if out=$(TESTANYWARE_APP_SOCKET=$nowhere "$bin" "$@" --json 2>&1); then
    fail "control: with no app on the socket, $name still succeeded: $out"
  fi
  [[ "$(jq -r .code <<<"$out" 2>/dev/null)" == APP_UNREACHABLE ]] ||
    fail "control: with no app on the socket, $name did not fail with APP_UNREACHABLE: $out"
  ok "control: $name fails with APP_UNREACHABLE when no app answers"
}
control "vm start" vm start --platform macos --id "$id-control"
control "agent snapshot" agent snapshot --vm "$id"
control "screen capture" screen capture --vm "$id" -o "$work/control.png"
control "input move" input move 10 10 --vm "$id"
control "file upload" file upload "$work/up.bin" "$remote" --vm "$id"
control "vm stop" vm stop "$id"
tart_names | grep -qx "$id-control" && fail "control: the refused vm start still created a guest."

# 4: the app answers after the guest and its sidecars are gone.
run "vm stop" vm stop "$id" --json >/dev/null
waited=0
while tart_names | grep -qx "$id" || [[ -e "$vms_dir/$id.json" || -e "$vms_dir/$id.meta.json" ]]; do
  ((++waited <= max_cleanup)) || break
  sleep 1
done
tart_names | grep -qx "$id" && fail "vm stop: the guest $id still exists."
[[ ! -e "$vms_dir/$id.json" && ! -e "$vms_dir/$id.meta.json" ]] || fail "vm stop: the sidecars of $id remain in $vms_dir."
"$bin" vm list --json --all | jq -e --arg id "$id" '[.items[] | select(.name == $id)] | length == 0' >/dev/null ||
  fail "vm stop: vm list still shows $id."
ok "vm stop: no guest, no sidecars, and vm list does not show $id"
echo "herdr-acceptance: PASS"
