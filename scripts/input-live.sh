#!/usr/bin/env bash
#
# The live checks of the input commands on this Mac, with a real macOS 27 guest. About five
# minutes, on a macOS host.
#
#   input-live.sh <app-bundle>
#
# The same script checks the development bundle (`task input:live`) and the release bundle of a
# candidate. It answers GitHub issue 12: an `input` command exited 0 and printed its line, and
# the guest did nothing. Each check reads the effect in the guest, never the exit status of the
# command.
#
# A person does one thing first: allow the Local Network alert for the app, once. This script
# never waits for it. A denial fails the run and names the remedy, and the person runs it again.
#
# The fixture is a small AppKit window, scripts/fixtures/input-fixture.swift. The script builds it
# with swiftc, uploads it to the guest, and runs it there, so the golden image needs no change.
# The window holds a text field and a scroll view that is taller than the window.
#
# Step 0, the oracle. `agent set-value` acts through the accessibility surface, not through VNC.
#   It must change the field, and `agent snapshot` must read the change. A run that fails here has
#   a fault in the fixture or in the agent, and it says nothing of input.
# Check 1, type. `input click` on the field must focus it. `input type` must put the text in it.
# Check 2, scroll. A wheel scroll must move one element of the scroll view. A drag of the thumb
#   of the scroll bar must move it again.
#
# Each check prints PASS or FAIL, and each failed step is named. The run does not stop at the
# first failed step, so that one run shows every command that had no effect. It exits non-zero
# when any step failed.
#
# The script stops its guest on every exit. It starts the app from the bundle when no app
# answers, and it quits only an app that it started. It starts the guest with `vm start --wait`,
# so a macOS guest slot that is taken makes it wait (ADR-0022).
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware), XDG_STATE_HOME,
#      MAX_START_SECS (default 600; how long the start may wait and boot),
#      EFFECT_SECS (default 5; how long an input has to show its effect),
#      KEEP_GUEST (set to 1 to leave the guest running, for a look at what the run left).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "input-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=${TESTANYWARE_BIN:-$root/cli-rs/target/debug/testanyware}
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
max_start=${MAX_START_SECS:-600}
effect_secs=${EFFECT_SECS:-5}
window="Input Fixture"
field_label="Fixture field"
row_label="row-20"
typed="input-live-ok"

results=("not run" "not run")
names=("typed text reaches the focused field" "a wheel scroll and a thumb drag move a view")
failed_steps=()
check=0

fail() {
  echo "input-live: FAIL: $*" >&2
  if ((check > 0)); then
    results[check - 1]=FAIL
  fi
  exit 1
}

ok() {
  echo "input-live: ok   $*"
}

# A step that failed. The run goes on, so that one run shows every command that had no effect.
step_failed() {
  echo "input-live: FAIL step: $*" >&2
  failed_steps+=("check $check: $*")
  results[check - 1]=FAIL
}

begin_check() {
  check=$1
  results[check - 1]=PASS
  echo "input-live: check $check: ${names[check - 1]}"
}

end_check() {
  if [[ "${results[check - 1]}" == PASS ]]; then
    echo "input-live: PASS check $check: ${names[check - 1]}"
  else
    echo "input-live: FAIL check $check: ${names[check - 1]}"
  fi
}

bundle=${1:-}
[[ -n "$bundle" ]] || fail "usage: input-live.sh <app-bundle>"
[[ -d "$bundle/Contents/MacOS" ]] || fail "$bundle is not an app bundle."
bundle=$(cd "$bundle" && pwd)
bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" 2>/dev/null) ||
  fail "no CFBundleIdentifier in $bundle/Contents/Info.plist."
[[ -x "$bin" ]] || fail "no CLI at $bin. Run \`task cli:build\`."
for tool in jq tart curl swiftc; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
bundle_running() { pgrep -f "$bundle/Contents/MacOS/" >/dev/null; }
tart_names() { tart list --format json | jq -r '.[].Name'; }
tart_has() {
  local listed
  listed=$(tart_names)
  grep -qx -- "$1" <<<"$listed"
}

id=input-live-$$
work=$(mktemp -d)
app_started=0
guest_up=0

# Stop the guest of this run on every exit, and print the result of each check. The app reclaims
# the guest first. When the app cannot, tart removes it, so a failed run leaves no guest that
# holds a macOS slot.
cleanup() {
  local rc=$?
  trap - ERR
  if ((${KEEP_GUEST:-0})) && ((guest_up)); then
    echo "input-live: KEEP_GUEST is set. The guest $id keeps running. Stop it with: testanyware vm stop $id" >&2
  else
    if status >/dev/null 2>&1; then
      "$bin" vm stop "$id" >/dev/null 2>&1 || true
    fi
    if tart_has "$id" 2>/dev/null; then
      echo "input-live: $id survived the app's stop. Removing it with tart." >&2
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
    fi
  fi
  if ((app_started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "$bundle/Contents/MacOS/" 2>/dev/null || true
  fi
  rm -rf "$work"
  local i
  for i in 0 1; do
    echo "input-live: ${results[i]}  check $((i + 1)): ${names[i]}"
  done
  local step
  for step in ${failed_steps[@]+"${failed_steps[@]}"}; do
    echo "input-live: failed step, $step"
  done
  exit "$rc"
}
trap cleanup EXIT

grant_step="a person must allow the Local Network alert for the app, once, and then run this again. This check never waits for it."

# Run a command, and fail the run when it fails. Output goes to stdout, and the error to a file.
run() {
  local name=$1
  shift
  local out
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

# The state of the fixture window, as the agent reads it.
snapshot() { "$bin" agent snapshot --vm "$id" --window "$window" --mode full --json 2>/dev/null; }
field_value() { snapshot | jq -r --arg l "$field_label" '[.. | objects | select(.label? == $l)][0].value // ""'; }
field_focused() { snapshot | jq -r --arg l "$field_label" '[.. | objects | select(.label? == $l)][0].focused // false'; }
row_y() { snapshot | jq -r --arg l "$row_label" '[.. | objects | select(.label? == $l)][0].positionY // "none"'; }
# The centre of an element, as "x y". The first argument is a jq filter on one element.
centre_of() {
  snapshot | jq -r "[.. | objects | select($1)][0] | \"\\((.positionX + .sizeWidth / 2) | floor) \\((.positionY + .sizeHeight / 2) | floor)\""
}

# Poll until a command succeeds, or the time for an effect has gone. Returns 1 at the end of the
# time, and never fails the run.
shows() {
  local began=$SECONDS
  until "$@"; do
    ((SECONDS - began < effect_secs)) || return 1
    sleep 0.5
  done
}

# 1. The app.
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

# 2. The fixture, built on the host.
swiftc -O "$root/scripts/fixtures/input-fixture.swift" -o "$work/input-fixture" 2>"$work/swiftc.err" ||
  {
    cat "$work/swiftc.err" >&2
    fail "swiftc could not build the fixture."
  }
ok "built the fixture"

# 3. The guest. `--wait` queues for a macOS slot when both are taken.
begin=$SECONDS
out=$(run "vm start" vm start --platform macos --id "$id" --wait="$max_start" --json)
guest_up=1
[[ "$(jq -r .id <<<"$out")" == "$id" ]] || fail "vm start printed another id: $out"
ok "vm start: $id reached a healthy agent in $((SECONDS - begin)) s"
run "file upload" file upload "$work/input-fixture" /tmp/input-fixture --vm "$id" >/dev/null
# The agent runs in the GUI session, so the window opens on the guest's screen.
run "fixture start" exec "chmod +x /tmp/input-fixture; (nohup /tmp/input-fixture >/tmp/input-fixture.log 2>&1 &)" --vm "$id" >/dev/null
window_open() { [[ "$(row_y)" != none ]]; }
shows window_open || fail "the fixture window did not open within $effect_secs s."
ok "the fixture window is open"

# ---- Step 0, the oracle --------------------------------------------------------------------

echo "input-live: step 0: the oracle, an agent action that does not use VNC"
[[ "$(field_value)" == "" ]] || fail "oracle: the field holds a value before any action: $(field_value)"
[[ "$(field_focused)" == false ]] || fail "oracle: the field is focused before any action."
run "agent set-value" agent set-value --vm "$id" --window "$window" --role textfield --label "$field_label" --value oracle >/dev/null
[[ "$(field_value)" == oracle ]] || fail "oracle: agent set-value did not change the field, or agent snapshot does not read it: $(field_value)"
run "agent set-value" agent set-value --vm "$id" --window "$window" --role textfield --label "$field_label" --value "" >/dev/null
[[ "$(field_value)" == "" ]] || fail "oracle: the field did not clear."
ok "the agent changed the field, agent snapshot read the change, and the field is empty again"

# ---- Check 1 -------------------------------------------------------------------------------

begin_check 1
IFS=" " read -r fx fy <<<"$(centre_of ".label? == \"$field_label\"")"
[[ -n "${fx:-}" && -n "${fy:-}" ]] || fail "the snapshot has no position for the field."

# The field is not focused before the click, so the click is what focuses it.
[[ "$(field_focused)" == false ]] || fail "the field is focused before the click, so the click would prove nothing."
ok "the field is not focused before the click"

run "input click" input click "$fx" "$fy" --vm "$id" >/dev/null
field_is_focused() { [[ "$(field_focused)" == true ]]; }
if shows field_is_focused; then
  ok "input click focused the field"
else
  step_failed "input click $fx $fy did not focus the field"
fi

run "input type" input type "$typed" --vm "$id" >/dev/null
field_has_text() { [[ "$(field_value)" == "$typed" ]]; }
if shows field_has_text; then
  ok "input type put \"$typed\" in the field"
else
  step_failed "input type \"$typed\" left the field at \"$(field_value)\""
fi
end_check

# ---- Check 2 -------------------------------------------------------------------------------

begin_check 2
IFS=" " read -r sx sy <<<"$(centre_of ".role? == \"scroll-area\"")"
[[ -n "${sx:-}" && -n "${sy:-}" ]] || fail "the snapshot has no position for the scroll view."

y0=$(row_y)
[[ "$y0" != none ]] || fail "the snapshot has no position for $row_label."
ok "$row_label is at y=$y0 before the scroll"

# The wheel. A step that moves nothing is a failed step, whatever the direction.
run "input scroll" input scroll "$sx" "$sy" --dy=-5 --vm "$id" >/dev/null
row_moved_from() { [[ "$(row_y)" != "$1" ]]; }
if shows row_moved_from "$y0"; then
  y1=$(row_y)
  ok "input scroll moved $row_label from y=$y0 to y=$y1"
else
  y1=$y0
  step_failed "input scroll $sx $sy --dy=-5 left $row_label at y=$y0"
fi

# The thumb. It is the value indicator of the scroll bar, and it moved with the wheel. The drag
# starts at its centre, and it goes down by 60 points.
IFS=" " read -r tx ty <<<"$(centre_of ".platformRole? == \"AXValueIndicator\"")"
[[ -n "${tx:-}" && -n "${ty:-}" ]] || fail "the snapshot has no thumb for the scroll bar."
run "input drag" input drag "$tx" "$ty" "$tx" "$((ty + 60))" --vm "$id" >/dev/null
if shows row_moved_from "$y1"; then
  ok "input drag of the thumb moved $row_label from y=$y1 to y=$(row_y)"
else
  step_failed "input drag $tx $ty $tx $((ty + 60)) left $row_label at y=$y1"
fi
end_check

if ((${#failed_steps[@]} > 0)); then
  echo "input-live: FAIL"
  exit 1
fi
echo "input-live: PASS"
