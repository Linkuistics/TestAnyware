#!/usr/bin/env bash
#
# The live checks of the admission queue on this Mac, with real macOS guests. Minutes, on a macOS
# host.
#
#   queue-live.sh <app-bundle>
#
# The same script checks the development bundle (`task queue:live`) and the release bundle of a
# candidate (release-publish-k37).
#
# A person does one thing first: allow the Local Network alert for the app, once. This script
# never waits for it. A denial fails the run and names the remedy, and the person runs it again.
#
# Check 1, the order. Three `vm start --wait` of a macOS guest run from three separate shells,
#   each a bash process of its own, in its own working directory. The first two are admitted. The
#   third waits at place 1, and `vm slots` names who asked. A stop of the first admits the third.
#   The join is the order of service.
# Check 2, an outside guest. A `tart run` that the app did not start holds one slot, and
#   `vm slots` names it as external. A start that the app admits waits for the second slot only.
#   A second start waits until the outside guest has ended.
# Check 3, a golden creation. Two guests hold both slots. `vm create-golden` of a macOS guest
#   waits and shows a place. Ctrl-C ends the wait and changes nothing. The check does not build a
#   golden: the golden creation never reaches its first change.
#
# Each check prints PASS or FAIL, and the script exits non-zero at the first FAIL.
#
# The script refuses to start when a guest that it did not start runs. It names the guest and
# stops. It never stops a guest that it did not start, and it stops its own on every exit.
# It starts the app from the bundle when no app answers, and it quits only an app that it started.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), XDG_STATE_HOME,
#      BASE (default testanyware-golden-macos-27; the image that the outside guest clones),
#      MAX_START_SECS (default 300; how long a start may take to reach a healthy agent),
#      MAX_ADMIT_SECS (default 60; how soon the queue admits a waiter once a slot is free),
#      STAY_SECS (default 5; how long a waiter must stay waiting while both slots are held),
#      MAX_CLEANUP_SECS (default 60; how long the app may take to free a stopped guest).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "queue-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
base=${BASE:-testanyware-golden-macos-27}
max_start=${MAX_START_SECS:-300}
max_admit=${MAX_ADMIT_SECS:-60}
stay=${STAY_SECS:-5}
max_cleanup=${MAX_CLEANUP_SECS:-60}

results=("not run" "not run" "not run")
names=("the order of the queue" "an outside guest counts" "a golden creation waits")
check=0

fail() {
  echo "queue-live: FAIL: $*" >&2
  if ((check > 0)); then
    results[check - 1]=FAIL
  fi
  exit 1
}

ok() {
  echo "queue-live: ok   $*"
}

pass() {
  results[check - 1]=PASS
  echo "queue-live: PASS check $check: ${names[check - 1]}"
}

bundle=${1:-}
[[ -n "$bundle" ]] || fail "usage: queue-live.sh <app-bundle>"
[[ -d "$bundle/Contents/MacOS" ]] || fail "$bundle is not an app bundle."
bundle=$(cd "$bundle" && pwd)
bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" 2>/dev/null) ||
  fail "no CFBundleIdentifier in $bundle/Contents/Info.plist."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
for tool in jq tart; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
bundle_running() { pgrep -f "$bundle/Contents/MacOS/" >/dev/null; }
tart_names() { tart list --format json | jq -r '.[].Name'; }
tart_running() { tart list --format json | jq -r '.[] | select(.State == "running") | .Name'; }
# The name list is read into a variable first, so that grep -q cannot close a pipe early.
tart_has() {
  local listed
  listed=$(tart_names)
  grep -qx -- "$1" <<<"$listed"
}
slots() { "$bin" vm slots --json; }

tag=queue-live-$$
id_a=$tag-a
id_b=$tag-b
id_c=$tag-c
id_d=$tag-d
id_e=$tag-e
id_ext=$tag-ext
golden=$tag-golden
work=$(mktemp -d)
# A start asks from its working directory, and the app reads it. The physical path is the one
# that the kernel reports.
work=$(cd "$work" && pwd -P)
app_started=0
pids=()
ours=("$id_a" "$id_b" "$id_c" "$id_d" "$id_e" "$id_ext")

# Stop what this run started, on every exit, and print the result of each check. The app reclaims
# a guest first. When the app cannot, tart removes it. A guest that this run did not start is never
# touched, because the list holds only the ids that this run made.
cleanup() {
  local rc=$?
  trap - ERR
  local pid id
  for pid in ${pids[@]+"${pids[@]}"}; do
    if kill -0 "$pid" 2>/dev/null; then
      kill -TERM "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  for id in "${ours[@]}"; do
    if status >/dev/null 2>&1; then
      "$bin" vm stop "$id" >/dev/null 2>&1 || true
    fi
    if tart_has "$id" 2>/dev/null; then
      echo "queue-live: $id survived the app's stop. Removing it with tart." >&2
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
    fi
  done
  # A golden creation that was admitted by mistake leaves its golden and its setup VMs behind.
  if tart_has "$golden" 2>/dev/null; then
    echo "queue-live: $golden exists. Removing it with tart." >&2
    tart delete "$golden" >/dev/null 2>&1 || true
  fi
  if ((app_started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "$bundle/Contents/MacOS/" 2>/dev/null || true
  fi
  rm -rf "$work"
  local i
  for i in 0 1 2; do
    echo "queue-live: ${results[i]}  check $((i + 1)): ${names[i]}"
  done
  exit "$rc"
}
trap cleanup EXIT

gone() { ! kill -0 "$1" 2>/dev/null; }

# Poll until a command succeeds: until_true <seconds> <what> <command> [args]
until_true() {
  local limit=$1 what=$2
  shift 2
  local began=$SECONDS
  until "$@"; do
    ((SECONDS - began < limit)) || fail "$what did not happen within $limit s."
    sleep 1
  done
}

# 0. A guest that this run did not start may hold a slot. The run names it and stops.
foreign=$(tart_running)
if [[ -n "$foreign" ]]; then
  fail "a macOS guest that this run did not start is running: ${foreign//$'\n'/, }. The checks need both slots. This run never stops a guest that it did not start. Stop it yourself, and run this again."
fi

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

view=$(slots)
[[ "$(jq -r .slots <<<"$view")" == 2 ]] ||
  fail "the app serves $(jq -r .slots <<<"$view") macOS slots, and the checks need 2. Remove macos_slots from config.rhai."
[[ "$(jq '.holders | length' <<<"$view")" == 0 ]] ||
  fail "the app counts a holder at the start, and no guest of tart runs: $(jq -c '[.holders[].vms[]]' <<<"$view"). The state directory holds a sidecar without an identity for each of them, and the app counts such a sidecar for ever. Move the *.json and *.meta.json of each of them out of ${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms, and run this again."

# The queries of the view.
holds() { slots | jq -e --arg id "$1" 'any(.holders[]; .vms | index($id))' >/dev/null; }
waits() { slots | jq -e --arg id "$1" 'any(.queue[]; .vms | index($id))' >/dev/null; }
external_holds() { slots | jq -e --arg id "$1" 'any(.holders[]; .kind == "external" and (.vms | index($id)))' >/dev/null; }
holder_of() { slots | jq -c --arg id "$1" '[.holders[] | select(.vms | index($id))][0]'; }
waiter_of() { slots | jq -c --arg id "$1" '[.queue[] | select(.vms | index($id))][0]'; }
waiter_by() { slots | jq -c --argjson pid "$1" '[.queue[] | select(.requested_by.pid == $pid)][0]'; }
nobody_holds() { [[ "$(slots | jq '.holders | length')" == 0 ]]; }
nobody_waits() { [[ "$(slots | jq '.queue | length')" == 0 ]]; }

# Start a command in its own shell, in its own working directory, with job control on so that it
# has the default signals. A background job of a script ignores SIGINT otherwise, and the
# Ctrl-C of check 3 would do nothing. LAUNCHED is the pid, and it is the pid of the command,
# because the shell replaces itself with it.
launch() {
  local name=$1
  shift
  local dir=$work/$name
  mkdir -p "$dir"
  set -m
  bash -c 'cd "$1" && shift && exec "$@"' _ "$dir" "$bin" "$@" >"$dir/out" 2>"$dir/err" &
  LAUNCHED=$!
  set +m
  pids+=("$LAUNCHED")
}

grant_step="a person must allow the Local Network alert for the app, once, and then run this again. This check never waits for it."

# Wait for a launched start to end, and require that it succeeded with its id.
finish() {
  local name=$1 pid=$2 id=$3 rc=0
  until_true "$max_start" "the start of $id ending" gone "$pid"
  wait "$pid" || rc=$?
  local dir=$work/$name
  if ((rc != 0)); then
    cat "$dir/out" "$dir/err" >&2
    if grep -qi 'local.network' "$dir/out" "$dir/err"; then
      fail "the start of $id: $grant_step"
    fi
    fail "the start of $id exited with $rc."
  fi
  [[ "$(jq -r .id "$dir/out")" == "$id" ]] || fail "the start of $id printed another id: $(cat "$dir/out")"
}

# The wait line that a waiting command prints on stderr.
says_place() {
  grep -q "waiting for a macOS guest slot: request .*, place $2," "$work/$1/err"
}

# Stop a waiter as Ctrl-C does, and require that it ended by the signal.
interrupt() {
  local pid=$1 rc=0
  kill -INT "$pid"
  until_true 15 "the waiting command ending after SIGINT" gone "$pid"
  wait "$pid" || rc=$?
  ((rc == 130)) || fail "the waiting command did not end by SIGINT: exit $rc."
}

# ---- Check 1 ------------------------------------------------------------------------------

check=1
echo "queue-live: check 1: three starts with --wait, from three shells"
# The first two starts join one after the other, and each is in the view before the next joins.
launch a vm start --platform macos --id "$id_a" --wait=900 --json
pid_a=$LAUNCHED
until_true 30 "the view holding $id_a" holds "$id_a"
launch b vm start --platform macos --id "$id_b" --wait=900 --json
pid_b=$LAUNCHED
until_true 30 "the view holding $id_b" holds "$id_b"
launch c vm start --platform macos --id "$id_c" --wait=900 --json
pid_c=$LAUNCHED
until_true 30 "the view showing $id_c in the queue" waits "$id_c"

entry=$(waiter_of "$id_c")
[[ "$(jq -r .place <<<"$entry")" == 1 ]] || fail "the third start is not at place 1: $entry"
[[ "$(jq -r .kind <<<"$entry")" == vm ]] || fail "the third start is not a start: $entry"
[[ "$(jq -r .requested_by.pid <<<"$entry")" == "$pid_c" ]] || fail "the view names another asker than process $pid_c: $entry"
[[ "$(jq -r .requested_by.cwd <<<"$entry")" == "$work/c" ]] || fail "the view names another directory than $work/c: $entry"
until_true 10 "the third start printing its place" says_place c 1
ok "the third start waits at place 1; it was asked by process $pid_c in $work/c"

hold_a=$(holder_of "$id_a")
hold_b=$(holder_of "$id_b")
[[ "$(jq -r .requested_by.pid <<<"$hold_a")" == "$pid_a" && "$(jq -r .requested_by.pid <<<"$hold_b")" == "$pid_b" ]] ||
  fail "the holders do not name the askers $pid_a and $pid_b: $hold_a $hold_b"
(($(jq -r .since <<<"$hold_a") <= $(jq -r .since <<<"$hold_b"))) || fail "the second start holds a slot from before the first: $hold_a $hold_b"
ok "the first two starts hold the slots in the order of their join"

# While both slots stay held, the third stays in the queue. The two guests boot and reach a
# healthy agent, which is the time when the slot must not pass to the waiter.
sleep "$stay"
waits "$id_c" || fail "the third start left the queue while both slots were held."
holds "$id_c" && fail "the third start holds a slot while the first two hold both."
finish a "$pid_a" "$id_a"
finish b "$pid_b" "$id_b"
waits "$id_c" || fail "the third start left the queue while both slots were held."
ok "$id_a and $id_b reached a healthy agent, and the third start still waits"

stopped=$(date +%s)
"$bin" vm stop "$id_a" >/dev/null
until_true "$max_admit" "the third start taking the slot of $id_a" holds "$id_c"
hold_c=$(holder_of "$id_c")
(($(jq -r .since <<<"$hold_c") >= stopped)) || fail "the slot of the third start is older than the stop of the first: $hold_c"
holds "$id_b" || fail "the stop of $id_a freed the slot of $id_b."
nobody_waits || fail "the queue still holds a waiter after the third start was admitted."
finish c "$pid_c" "$id_c"
ok "the stop of $id_a admitted $id_c within $(($(date +%s) - stopped)) s, and it reached a healthy agent"

"$bin" vm stop "$id_c" >/dev/null
"$bin" vm stop "$id_b" >/dev/null
until_true "$max_cleanup" "the view showing no holder" nobody_holds
for id in "$id_a" "$id_b" "$id_c"; do
  tart_has "$id" && fail "the guest $id still exists after its stop."
done
pass

# ---- Check 2 ------------------------------------------------------------------------------

check=2
echo "queue-live: check 2: a guest outside the app holds a slot"
tart_has "$base" || fail "no image $base for the outside guest. Name another with BASE=<image>."
tart clone "$base" "$id_ext" >/dev/null
set -m
tart run --no-graphics "$id_ext" >"$work/ext.out" 2>&1 &
pid_ext=$!
set +m
pids+=("$pid_ext")
until_true 60 "the view naming $id_ext as external" external_holds "$id_ext"
ok "vm slots names $id_ext as external, and the app did not start it"

begin=$SECONDS
launch d vm start --platform macos --id "$id_d" --wait=900 --json
pid_d=$LAUNCHED
until_true "$max_admit" "the start of $id_d being admitted" holds "$id_d"
ok "the start of $id_d was admitted $((SECONDS - begin)) s after it asked: it needed the second slot only"

launch e vm start --platform macos --id "$id_e" --wait=900 --json
pid_e=$LAUNCHED
until_true 30 "the view showing $id_e in the queue" waits "$id_e"
[[ "$(jq -r .place <<<"$(waiter_of "$id_e")")" == 1 ]] || fail "the start of $id_e is not at place 1: $(waiter_of "$id_e")"
until_true 10 "the start of $id_e printing its place" says_place e 1
sleep "$stay"
waits "$id_e" || fail "the start of $id_e left the queue while the outside guest held a slot."
grep -q "$id_ext" "$work/e/err" || fail "the wait of $id_e does not name the outside guest: $(cat "$work/e/err")"
ok "the start of $id_e waits behind $id_ext and $id_d, and its line names $id_ext"

# The outside guest ends. The app sees it at its next observation and admits the waiter.
ended=$SECONDS
tart stop "$id_ext" >/dev/null 2>&1 || true
until_true 30 "the outside guest ending" gone "$pid_ext"
tart delete "$id_ext" >/dev/null 2>&1 || true
until_true "$max_admit" "the start of $id_e taking the slot of $id_ext" holds "$id_e"
ok "the end of $id_ext admitted $id_e after $((SECONDS - ended)) s"
finish d "$pid_d" "$id_d"
finish e "$pid_e" "$id_e"
pass

# ---- Check 3 ------------------------------------------------------------------------------

check=3
echo "queue-live: check 3: a golden creation waits for its slot"
# $id_d and $id_e hold both slots.
holds "$id_d" || fail "$id_d does not hold a slot."
holds "$id_e" || fail "$id_e does not hold a slot."
before=$(tart_names | sort)
# The default version, macOS 27, and a name that exists nowhere. The command passes its fast
# checks, and then it waits. It never reaches its first clone, so it builds no base VM.
launch golden vm create-golden --platform macos --version 27 --name "$golden" --json
pid_g=$LAUNCHED
waiting_golden() {
  kill -0 "$pid_g" 2>/dev/null || return 0
  [[ "$(waiter_by "$pid_g")" != null ]]
}
until_true 60 "the golden creation joining the queue" waiting_golden
if ! kill -0 "$pid_g" 2>/dev/null; then
  cat "$work/golden/out" "$work/golden/err" >&2
  fail "the golden creation ended before it waited. Its fast checks need an SSH key and the macOS agent binary."
fi
entry=$(waiter_by "$pid_g")
[[ "$(jq -r .kind <<<"$entry")" == golden ]] || fail "the waiter is not a golden creation: $entry"
[[ "$(jq -r .place <<<"$entry")" == 1 ]] || fail "the golden creation is not at place 1: $entry"
[[ "$(jq -r .requested_by.cwd <<<"$entry")" == "$work/golden" ]] || fail "the view names another directory than $work/golden: $entry"
until_true 10 "the golden creation printing its place" says_place golden 1
sleep "$stay"
kill -0 "$pid_g" 2>/dev/null || fail "the golden creation ended while both slots were held: $(cat "$work/golden/err")"
grep -q 'admitted this golden creation' "$work/golden/err" && fail "the golden creation was admitted while both slots were held."
ok "the golden creation waits at place 1 ($(jq -r '.vms | join(", ")' <<<"$entry"))"

interrupt "$pid_g"
until_true 15 "the queue showing no waiter" nobody_waits
holds "$id_d" || fail "the end of the wait freed the slot of $id_d."
holds "$id_e" || fail "the end of the wait freed the slot of $id_e."
[[ "$(slots | jq '.holders | length')" == 2 ]] || fail "the view holds another number of holders than 2 after the Ctrl-C: $(slots | jq -c .holders)"
grep -q 'admitted this golden creation\|Cloning' "$work/golden/err" && fail "the golden creation went on after its Ctrl-C: $(cat "$work/golden/err")"
after=$(tart_names | sort)
[[ "$before" == "$after" ]] || fail "the guests of tart changed between the join and the Ctrl-C. Before: $(tr '\n' ' ' <<<"$before") After: $(tr '\n' ' ' <<<"$after")"
tart_has "$golden" && fail "the golden $golden exists."
ok "Ctrl-C ended the wait, the two holders still hold, and tart holds the same guests"
pass

"$bin" vm stop "$id_d" >/dev/null
"$bin" vm stop "$id_e" >/dev/null
until_true "$max_cleanup" "the view showing no holder" nobody_holds
echo "queue-live: PASS"
