#!/usr/bin/env bash
#
# Does a tart guest survive idle time with no VNC client on this host (ADR-0018)? A host that
# runs the latest macOS 27 and tart needs no process that holds a VNC connection. Three checks
# on two guests that `vm start` starts through the app:
#
#   macOS guest A
#     1. Two `screen size` commands 5 s apart. The app resets the guest connection when its
#        client leaves, and on macOS 27.0.1 the next connection after a reset crashed tart.
#        The first size must be the size that `vm start` set (EXPECT_SIZE). The first session
#        after a tart boot can get a stale 1280x720, and the VNC size wait at the end of
#        `vm start` must hide it (ADR-0018).
#     2. 5 minutes with no VNC client, then `screen size`. The command works, and tart runs.
#   Linux guest B
#     3. `screen size`, 70 s with no VNC client, then `screen size`. The command works, and
#        tart runs. The first size must be EXPECT_SIZE, as in check 1.
#
# The script prints the progress output of each `vm start`, and says whether it names the VNC
# size wait. A start whose first session read the right size names no wait.
#
# While each guest runs, no `_vnc-keeper` process exists. The script first shows that its
# instrument finds a decoy process with that command line. After each `vm stop`, tart lists
# no guest of this run.
#
#   vnc-idle-live.sh <app-bundle>
#
# `vm start` runs in the app, so the run checks the app of the given bundle. It starts the app
# from the bundle when no app answers, and it quits only an app that it started. It refuses an
# app on the socket that is not that bundle.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware); MACOS_BASE and LINUX_BASE
# (the goldens to clone; default the golden of the platform); IDLE (check 2 seconds, default
# 300); GAP (check 3 seconds, default 70); MAX_START_SECS (default 600); EXPECT_SIZE (the size
# of each guest display, default 1920x1080). It stops its guests on every exit. About 10 minutes.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "vnc-idle-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
idle=${IDLE:-300}
gap=${GAP:-70}
max_start=${MAX_START_SECS:-600}
expect=${EXPECT_SIZE:-1920x1080}

say() { echo "vnc-idle-live: $*" >&2; }

fail() {
  echo "vnc-idle-live: FAIL: $*" >&2
  exit 1
}

bundle=${1:-}
[[ -n "$bundle" ]] || fail "usage: vnc-idle-live.sh <app-bundle>"
[[ -d "$bundle/Contents/MacOS" ]] || fail "$bundle is not an app bundle."
bundle=$(cd "$bundle" && pwd)
bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" 2>/dev/null) ||
  fail "no CFBundleIdentifier in $bundle/Contents/Info.plist."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
for tool in jq tart curl; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
bundle_running() { pgrep -f "$bundle/Contents/MacOS/" >/dev/null; }
tart_state() {
  tart list --format json | jq -r --arg n "$1" '.[] | select(.Name == $n) | .State' | head -n 1
}

ids=()
app_started=0
cleanup() {
  local rc=$? id
  trap - ERR
  for id in ${ids[@]+"${ids[@]}"}; do
    if status >/dev/null 2>&1; then
      "$bin" vm stop "$id" >/dev/null 2>&1 || true
    fi
    if [[ -n $(tart_state "$id" 2>/dev/null) ]]; then
      say "$id survived the app's stop. Removing it with tart."
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
    fi
  done
  if ((app_started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "$bundle/Contents/MacOS/" 2>/dev/null || true
  fi
  exit "$rc"
}
trap cleanup EXIT

# The pids of each process whose program is `testanyware` or `testanyware-app` and whose first
# argument is `_vnc-keeper`, one per line. `pgrep -f _vnc-keeper` would also match a shell
# whose command line holds the text. The pattern does not match the command line of awk itself.
keeper_pids() {
  ps -axo pid=,args= | awk '$0 ~ "testanyware(-app)? _vnc-keeper" { print $1 }'
}

no_keeper() {
  local found
  found=$(keeper_pids)
  [[ -z $found ]] || fail "$1: a _vnc-keeper process exists: $(tr '\n' ' ' <<<"$found")"
}

# The control of the instrument. A decoy with the command line of a keeper must be found.
decoy_control() {
  local decoy found
  (exec -a "testanyware _vnc-keeper --id decoy" sleep 30) &
  decoy=$!
  sleep 0.5
  found=$(keeper_pids)
  kill "$decoy" 2>/dev/null || true
  wait "$decoy" 2>/dev/null || true
  [[ $found == "$decoy" ]] || fail "the instrument did not find the decoy $decoy (found: ${found:-none})"
  say "control: the instrument finds a decoy keeper"
}

# Start a guest, then print its progress output and whether that output names the VNC size wait.
start_guest() {
  local platform=$1 id=$2 base=${3:-} out progress line
  local args=(vm start --platform "$platform" --id "$id" --wait="$max_start" --json)
  [[ -n $base ]] && args+=(--base "$base")
  say "starting the $platform guest $id${base:+ from $base}..."
  ids+=("$id")
  progress=$(mktemp)
  if ! out=$("$bin" "${args[@]}" 2>"$progress"); then
    cat "$progress" >&2
    rm -f "$progress"
    fail "vm start --platform $platform exited non-zero: $out"
  fi
  while IFS= read -r line; do
    say "  start output: $line"
  done <"$progress"
  if grep -q 'VNC size wait' "$progress"; then
    say "the start output of $id names the VNC size wait"
  else
    say "the start output of $id names no VNC size wait"
  fi
  rm -f "$progress"
  [[ $(jq -r '.id // empty' <<<"$out") == "$id" ]] || fail "vm start printed another id: $out"
  [[ $(tart_state "$id") == running ]] || fail "$id is not running after vm start"
}

# Print the size that `screen size` reports, or fail with the state of tart.
screen_size() {
  local out
  out=$("$bin" screen size --vm "$1" 2>&1) || fail "$2: screen size failed: $out (tart state: $(tart_state "$1"))"
  printf '%s' "$out"
}

stop_guest() {
  "$bin" vm stop "$1" >/dev/null 2>&1 || fail "vm stop $1 failed"
  [[ -z $(tart_state "$1") ]] || fail "tart still lists $1 after vm stop"
}

# ---- The app ---------------------------------------------------------------------------------

if body=$(status 2>/dev/null); then
  bundle_running || fail "an app that is not $bundle answers on the socket. Quit it, and run this again."
  say "$bundle already answers, build $(jq -r .build <<<"$body")"
else
  open -n -g "$bundle"
  app_started=1
  body=
  for _ in $(seq 1 50); do
    if body=$(status 2>/dev/null); then break; fi
    sleep 0.2
  done
  [[ -n "$body" ]] || fail "$bundle did not answer within 10 s."
  say "started $bundle, build $(jq -r .build <<<"$body")"
fi

decoy_control

# ---- macOS guest A: checks 1 and 2 -----------------------------------------------------------

a=vnc-idle-macos-$$
start_guest macos "$a" "${MACOS_BASE:-}"
no_keeper "after the start of $a"

say "check 1: screen size, 5 s, screen size"
first=$(screen_size "$a" "check 1")
say "the first screen size after the boot: $first"
[[ $first == "$expect" ]] || fail "check 1: the first screen size after vm start is $first, not $expect"
sleep 5
second=$(screen_size "$a" "check 1, second command")
say "the second screen size, 5 s later: $second"
[[ $(tart_state "$a") == running ]] || fail "check 1: $a stopped (state: $(tart_state "$a"))"
say "check 1 passed"

say "check 2: ${idle} s with no VNC client, then screen size"
sleep "$idle"
screen_size "$a" "check 2" >/dev/null
[[ $(tart_state "$a") == running ]] || fail "check 2: $a stopped (state: $(tart_state "$a"))"
no_keeper "check 2"
say "check 2 passed"

stop_guest "$a"

# ---- Linux guest B: check 3 ------------------------------------------------------------------

b=vnc-idle-linux-$$
start_guest linux "$b" "${LINUX_BASE:-}"
no_keeper "after the start of $b"

say "check 3: screen size, ${gap} s with no VNC client, screen size"
first=$(screen_size "$b" "check 3, first command")
say "the first screen size after the boot: $first"
[[ $first == "$expect" ]] || fail "check 3: the first screen size after vm start is $first, not $expect"
sleep "$gap"
screen_size "$b" "check 3" >/dev/null
[[ $(tart_state "$b") == running ]] || fail "check 3: $b stopped (state: $(tart_state "$b"))"
no_keeper "check 3"
say "check 3 passed"

stop_guest "$b"

say "OK: all three checks passed, and no _vnc-keeper process existed"
