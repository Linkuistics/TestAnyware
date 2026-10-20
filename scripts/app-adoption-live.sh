#!/usr/bin/env bash
#
# Do the VMs survive a quit of the TestAnyware app, and does the app adopt them when it opens
# again? About five minutes, on a macOS host, with one free macOS guest slot.
#
#   app-adoption-live.sh <app-bundle>
#
# The same script checks the development bundle (`task app:adoption-live`) and the release
# bundle of a candidate.
#
# A person does one thing first: allow the Local Network alert for the app, once. This script
# never waits for it. A denial fails the run and names the remedy, and the person runs it again.
#
# Phases, with no variable set:
#   1. Start the app from the bundle with `open`, as macOS starts a login item. The script
#      refuses to run when another app answers, because that answer would not prove this bundle.
#      `vm start --platform macos` then starts a guest through the app, and its meta must record
#      the identity of each process.
#   2. Quit the app through the Apple event that the Dock's Quit sends. The guest, its keeper and
#      its `tart run` process must run on, with the same pids and start times. With no app, an
#      agent command fails with APP_UNREACHABLE, so the CLI has no direct path.
#   3. Open the app again. It adopts the guest by identity (not legacy, agent and VNC
#      answering, not in `not_adopted`). `vm list` shows the guest, an agent command and a screen
#      capture reach it, and no process restarted.
#   4. The hand-made older meta. Quit the app, remove the identity fields from the meta, and
#      open the app. It adopts the guest as legacy and still serves its streams. `vm stop` then
#      stops the guest by name, and the guest, its sidecars and its `vm list` row are gone.
#   5. A guest that an older Host CLI started, when one is at hand (OLD_TESTANYWARE_BIN, or the
#      `testanyware` on PATH when it is not this build). With the app quit, it starts a guest by
#      its direct path. The app then adopts the guest as legacy, serves it, and `vm stop` stops it.
#      When no older CLI is at hand, or it cannot start a guest from this shell, the phase says
#      so and the hand-made meta of phase 4 stands for the legacy path.
#
# The script stops its guests on every exit, failure included, and quits only an app that it
# started.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), OLD_TESTANYWARE_BIN,
#      XDG_STATE_HOME,
#      MAX_START_SECS (default 240; the bound on `vm start`),
#      MAX_ADOPT_SECS (default 40; how long the adoption probes may take after an open),
#      MAX_CLEANUP_SECS (default 60; how long a stopped guest may take to disappear).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "app-adoption-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
vms_dir="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms"
max_start=${MAX_START_SECS:-240}
max_adopt=${MAX_ADOPT_SECS:-40}
max_cleanup=${MAX_CLEANUP_SECS:-60}

fail() {
  echo "app-adoption-live: FAIL: $*" >&2
  exit 1
}

ok() {
  echo "app-adoption-live: ok   $*"
}

info() {
  echo "app-adoption-live: info $*"
}

bundle=${1:-}
[[ -n "$bundle" ]] || fail "usage: app-adoption-live.sh <app-bundle>"
[[ -d "$bundle/Contents/MacOS" ]] || fail "$bundle is not an app bundle."
bundle=$(cd "$bundle" && pwd)
bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" 2>/dev/null) ||
  fail "no CFBundleIdentifier in $bundle/Contents/Info.plist."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
for tool in jq tart curl ps; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
# The app is the process with no arguments. Its VNC keepers run the same executable as
# `_vnc-keeper`, and they must outlive the app, so a pattern for the directory would match them.
app_pattern="^$bundle/Contents/MacOS/testanyware-app\$"
bundle_running() { pgrep -f "$app_pattern" >/dev/null; }
tart_names() { tart list --format json | jq -r '.[].Name'; }
tart_state() { tart list --format json | jq -r --arg n "$1" '.[] | select(.Name == $n) | .State'; }
alive() { kill -0 "$1" 2>/dev/null; }
started_at() { ps -o lstart= -p "$1" 2>/dev/null; }
meta_of() { printf '%s/%s.meta.json' "$vms_dir" "$1"; }

work=$(mktemp -d)
ids=()
app_opened=0

# Stop every guest of this run on every exit. The app reclaims a guest first. When the app
# cannot, tart removes it, so a failed run leaves no guest that holds a macOS slot.
cleanup() {
  local rc=$?
  trap - ERR
  local id keeper
  for id in ${ids[@]+"${ids[@]}"}; do
    if [[ -f "$(meta_of "$id")" ]] && status >/dev/null 2>&1; then
      "$bin" vm stop "$id" >/dev/null 2>&1 || true
    fi
    if tart_names 2>/dev/null | grep -qx "$id"; then
      echo "app-adoption-live: $id survived the app's stop. Removing it with tart." >&2
      keeper=$(jq -r '.keeper_pid // empty' "$(meta_of "$id")" 2>/dev/null || true)
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
      # Only a process that names this guest as its keeper.
      if [[ -n "$keeper" ]] && ps -o command= -p "$keeper" 2>/dev/null | grep -q -- "_vnc-keeper --id $id"; then
        kill "$keeper" 2>/dev/null || true
      fi
      rm -f "$vms_dir/$id".json "$vms_dir/$id".meta.json
    fi
  done
  if ((app_opened)) && bundle_running; then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "$app_pattern" 2>/dev/null || true
  fi
  rm -rf "$work"
  exit "$rc"
}
trap cleanup EXIT

if [[ -n "${HERDR_ENV:-}" ]]; then
  info "herdr session $HERDR_PANE_ID"
else
  info "not a herdr session. The run proves adoption, not the herdr case."
fi

free_slot() {
  local running
  running=$(tart list --format json | jq -r '[.[] | select(.State == "running")] | length')
  ((running < 2)) || fail "both macOS guest slots are taken. Stop a guest, and run this again."
}

# open_app: start the bundle as macOS starts a login item, and wait for its socket.
open_app() {
  local body=
  open -n -g "$bundle"
  app_opened=1
  for _ in $(seq 1 50); do
    if body=$(status 2>/dev/null); then break; fi
    sleep 0.2
  done
  [[ -n "$body" ]] || fail "$bundle did not answer within 10 s."
}

# quit_app: the Apple event that the Dock's Quit sends. The process and the socket file go.
quit_app() {
  osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null
  for _ in $(seq 1 50); do
    bundle_running || break
    sleep 0.2
  done
  ! bundle_running || fail "the app did not quit."
  [[ ! -e "$socket" ]] || fail "the socket file stays after a quit."
}

# wait_adopted: the app serves its socket first and probes the survivors later. Wait for the end
# of the probes, and print the status body.
wait_adopted() {
  local body
  for _ in $(seq 1 $((max_adopt * 2))); do
    if body=$(status 2>/dev/null) && [[ "$(jq -r '.adoption.finished' <<<"$body")" == true ]]; then
      printf '%s' "$body"
      return 0
    fi
    sleep 0.5
  done
  fail "the adoption probes did not finish within $max_adopt s: ${body:-no status}"
}

# A command must succeed. A Local Network denial names the person's step.
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

# Record the processes of a guest, so a later phase can show that none restarted.
record_processes() {
  local id=$1 meta pid keeper
  meta=$(meta_of "$id")
  pid=$(jq -r .pid "$meta")
  keeper=$(jq -r '.keeper_pid // empty' "$meta")
  alive "$pid" || fail "$id: the tart process $pid does not run."
  printf '%s %s\n' "$pid" "$(started_at "$pid")" >"$work/$id.main"
  if [[ -n "$keeper" ]]; then
    alive "$keeper" || fail "$id: the keeper $keeper does not run."
    printf '%s %s\n' "$keeper" "$(started_at "$keeper")" >"$work/$id.keeper"
  fi
}

# same_processes: each recorded process runs on, with the same pid and start time.
same_processes() {
  local id=$1 file line pid start
  for file in "$work/$id.main" "$work/$id.keeper"; do
    [[ -f "$file" ]] || continue
    # The line is "<pid> <start time>", and the start time holds spaces.
    line=$(<"$file")
    pid=${line%% *}
    start=${line#* }
    alive "$pid" || fail "$id: process $pid ($(basename "$file")) ended."
    [[ "$(started_at "$pid")" == "$start" ]] || fail "$id: process $pid ($(basename "$file")) restarted."
  done
}

adopted_row() { jq -c --arg id "$2" '.adoption.adopted[] | select(.id == $id)' <<<"$1"; }

# vm_listed: does `vm list` show the guest?
vm_listed() {
  "$bin" vm list --json --all 2>/dev/null |
    jq -e --arg id "$1" '[.items[] | select(.name == $id)] | length == 1' >/dev/null
}

# serves_streams: an agent command and a screen capture reach the guest through the app.
serves_streams() {
  local id=$1 out png
  out=$(run "agent snapshot" agent snapshot --vm "$id" --json)
  [[ "$(jq -r .ok <<<"$out")" == true ]] || fail "agent snapshot: ok is not true: $out"
  ok "agent snapshot: $(jq '.windows | length' <<<"$out") windows"
  png=$work/$id.png
  run "screen capture" screen capture --vm "$id" -o "$png" >/dev/null
  [[ "$(head -c 8 "$png" | xxd -p)" == 89504e470d0a1a0a ]] || fail "screen capture wrote no PNG."
  ok "screen capture: a PNG through the guest's VNC stream"
}

# stop_and_check: vm stop through the app leaves no guest, no sidecars and no list row.
stop_and_check() {
  local id=$1 waited=0
  run "vm stop" vm stop "$id" --json >/dev/null
  while tart_names | grep -qx "$id" || [[ -e "$vms_dir/$id.json" || -e "$(meta_of "$id")" ]]; do
    ((++waited <= max_cleanup)) || break
    sleep 1
  done
  ! tart_names | grep -qx "$id" || fail "vm stop: the guest $id still exists."
  [[ ! -e "$vms_dir/$id.json" && ! -e "$(meta_of "$id")" ]] || fail "vm stop: the sidecars of $id remain in $vms_dir."
  ! vm_listed "$id" || fail "vm stop: vm list still shows $id."
  ok "vm stop: no guest, no sidecars, and vm list does not show $id"
}

# adopts_as: the app finished its probes, and the guest is adopted (legacy or not) and answering.
adopts_as() {
  local id=$1 legacy=$2 body row
  body=$(wait_adopted)
  row=$(adopted_row "$body" "$id")
  [[ -n "$row" ]] || fail "$id is not in adoption.adopted: $(jq -c .adoption <<<"$body")"
  [[ "$(jq -r .legacy <<<"$row")" == "$legacy" ]] || fail "$id: legacy is not $legacy: $row"
  [[ "$(jq -r .agent <<<"$row")" == answering ]] || fail "$id: the agent does not answer the probe: $row"
  jq -e --arg id "$id" '[.adoption.not_adopted[] | select(.id == $id)] | length == 0' <<<"$body" >/dev/null ||
    fail "$id is also in adoption.not_adopted: $(jq -c .adoption.not_adopted <<<"$body")"
  ok "the app adopted $id: $row"
}

# ---- 1: start a guest through the app --------------------------------------------------------

free_slot
if status >/dev/null 2>&1; then
  fail "an app already answers on the socket. Quit it, and run this again: it must be this bundle."
fi
open_app
ok "started $bundle, build $(jq -r .build <<<"$(status)")"

id=adoption-live-$$
ids+=("$id")
begin=$SECONDS
out=$(run "vm start" vm start --platform macos --id "$id" --json)
[[ "$(jq -r .id <<<"$out")" == "$id" ]] || fail "vm start: the id is not $id: $out"
((SECONDS - begin <= max_start)) || fail "vm start took more than $max_start s."
ok "vm start: $id reached a healthy agent in $((SECONDS - begin)) s"

meta=$(meta_of "$id")
jq -e '.identity.start_time and .identity.boot_session and .keeper_identity.start_time' "$meta" >/dev/null ||
  fail "the meta records no identity: $(<"$meta")"
ok "the meta records the pid, start time and boot session of the guest and its keeper"
record_processes "$id"

# ---- 2: quit the app, and the guest runs on ----------------------------------------------------

quit_app
ok "the app quit: no process, no socket file"
[[ "$(tart_state "$id")" == running ]] || fail "after the quit, tart says $id is $(tart_state "$id")."
same_processes "$id"
[[ -f "$meta" ]] || fail "after the quit, the meta of $id is gone."
ok "the guest runs on: tart says running, and its tart process and keeper keep their pids and start times"

nowhere=$work/none/testanyware.sock
if out=$(TESTANYWARE_APP_SOCKET=$nowhere "$bin" agent snapshot --vm "$id" --json 2>&1); then
  fail "control: with no app on the socket, agent snapshot still succeeded: $out"
fi
[[ "$(jq -r .code <<<"$out" 2>/dev/null)" == APP_UNREACHABLE ]] ||
  fail "control: with no app, agent snapshot did not fail with APP_UNREACHABLE: $out"
ok "control: with no app, a command fails with APP_UNREACHABLE and the CLI has no direct path"

# ---- 3: open the app again, and it adopts the guest ------------------------------------------

open_app
adopts_as "$id" false
vm_listed "$id" || fail "vm list does not show $id after the app opened again."
ok "vm list shows $id"
serves_streams "$id"
same_processes "$id"
ok "no process of $id restarted: the app adopted the guest and started nothing"

# ---- 4: the hand-made older meta -------------------------------------------------------------

quit_app
cp "$meta" "$work/$id.meta.with-identity"
jq 'del(.identity, .keeper_identity, .swtpm_identity)' "$work/$id.meta.with-identity" >"$work/$id.meta.older"
cp "$work/$id.meta.older" "$meta"
jq -e 'has("identity") | not' "$meta" >/dev/null || fail "the hand-made meta still records an identity."
info "the meta of $id is now an older meta: $(jq -c . "$meta")"
open_app
adopts_as "$id" true
vm_listed "$id" || fail "vm list does not show the legacy guest $id."
serves_streams "$id"
same_processes "$id"
ok "the legacy guest got streams, and the app signalled none of its processes"
stop_and_check "$id"

# ---- 5: a guest that an older Host CLI started -------------------------------------------------

old=${OLD_TESTANYWARE_BIN:-$(command -v testanyware || true)}
if [[ -n "$old" ]] && [[ "$(cd "$(dirname "$old")" && pwd -P)/$(basename "$old")" != "$(cd "$(dirname "$bin")" && pwd -P)/$(basename "$bin")" ]]; then
  old_real=$(realpath "$old")
  info "an older Host CLI is at hand: $old_real, $("$old" --version 2>&1 | head -n 1)"
  quit_app
  free_slot
  old_id=adoption-live-old-$$
  ids+=("$old_id")
  # The older CLI has no app and starts the guest by its direct path. From a shell with no Local
  # Network access, it fails, and the hand-made meta of phase 4 stands for the legacy path.
  if ! out=$("$old" vm start --platform macos --id "$old_id" --json 2>"$work/old-stderr"); then
    out="$out$(<"$work/old-stderr")"
    info "SKIP the older CLI could not start a guest from this shell, so the hand-made meta of phase 4 covers the legacy path: $(tr '\n' ' ' <<<"$out" | cut -c 1-300)"
    ids=("$id")
  elif jq -e 'has("identity")' "$(meta_of "$old_id")" >/dev/null; then
    info "SKIP the CLI in hand records an identity, so it is not an older Host CLI. The hand-made meta of phase 4 covers the legacy path."
  else
    ok "the older CLI started $old_id, and its meta records no identity: $(jq -c . "$(meta_of "$old_id")")"
    record_processes "$old_id"
    open_app
    adopts_as "$old_id" true
    vm_listed "$old_id" || fail "vm list does not show the older guest $old_id."
    serves_streams "$old_id"
    same_processes "$old_id"
    stop_and_check "$old_id"
  fi
else
  info "no older Host CLI is at hand. The hand-made meta of phase 4 covers the legacy path."
fi

echo "app-adoption-live: PASS"
