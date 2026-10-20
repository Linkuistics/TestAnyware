#!/usr/bin/env bash
#
# Does the VNC keeper (ADR-0018) protect a tart guest on this macOS 27 host?
# Four checks on two clones, so that each clone answers as much as it can:
#
#   macOS clone A
#     1. `screen size`, 5 minutes with no other client, `screen size`: the
#        second command works and the clone still runs.
#     2. The negative control. Kill the keeper. `screen size` at once works,
#        so the keeper's close did not crash tart. After 70 s with no client,
#        `screen size` fails and tart has crashed: the keeper is what protects
#        the guest.
#   Linux clone B
#     3. `screen size`, 70 s, `screen size`: the second command works.
#   After `vm stop`
#     4. No `testanyware _vnc-keeper` process runs, and `vm list` shows no clone.
#
#   vnc-keeper-live.sh
#
# Env: TESTANYWARE (the CLI; default cli-rs/target/debug/testanyware, built by
# `task cli:build`); MACOS_BASE and LINUX_BASE (goldens to clone; default the
# platform's golden); IDLE (check 1 seconds, default 300); GAP (checks 2 and 3
# seconds, default 70). Always stops and deletes its clones.
#
# About 14 minutes. The macOS start waits about 4 minutes for an agent that
# this harness cannot reach, warns, and exits 0 with a running clone: the
# checks need VNC only.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "vnc-keeper-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cli=${TESTANYWARE:-$root/cli-rs/target/debug/testanyware}
idle=${IDLE:-300}
gap=${GAP:-70}

ids=()
cleanup() {
  local id
  for id in ${ids[@]+"${ids[@]}"}; do
    "$cli" vm stop "$id" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

say() { echo "vnc-keeper-live: $*" >&2; }

fail() {
  echo "vnc-keeper-live: FAIL: $1" >&2
  exit 1
}

tart_state() {
  tart list --format json | jq -r --arg n "$1" '.[] | select(.Name == $n) | .State' | head -n 1
}

# The keeper pids, one per line, none if there is no keeper. With an id, only
# that id's keeper. A process counts only when its program (the first word of
# its command line) is `testanyware`, or `testanyware-app` when the TestAnyware app started the
# VM (the app's bundle path may hold a space, so the match is on the text before the
# command): `pgrep -f _vnc-keeper` also matches any shell whose command line mentions the
# text, such as the shell that runs this script. `ps -o comm` is no help, because it cuts a
# long path short.
keeper_pids() {
  ps -axo pid=,args= |
    awk -v id="${1:-}" '$0 ~ ("testanyware(-app)? _vnc-keeper --id " id) { print $1 }'
}

# Start a clone with the CLI and set `clone_id`. It sets a variable and does not
# print, because a command substitution would lose the `ids` update that
# `cleanup` needs.
clone_id=
start_clone() {
  local platform=$1 base=${2:-} out
  local args=(vm start --platform "$platform" --json)
  [[ -n $base ]] && args+=(--base "$base")
  say "starting a $platform clone${base:+ of $base}..."
  out=$("$cli" "${args[@]}") || fail "vm start --platform $platform exited non-zero"
  clone_id=$(jq -r '.id // empty' <<<"$out")
  [[ -n $clone_id ]] || fail "vm start printed no VM id"
  ids+=("$clone_id")
}

screen_size() {
  "$cli" screen size --vm "$1" >/dev/null 2>&1
}

# ---- macOS clone A: checks 1 and 2 -----------------------------------------

start_clone macos "${MACOS_BASE:-}"
a=$clone_id
[[ $(tart_state "$a") == running ]] || fail "$a is not running after vm start"
keeper=$(keeper_pids "$a")
[[ -n $keeper && $keeper != *$'\n'* ]] || fail "expected one keeper for $a, found: ${keeper:-none}"
say "$a has keeper pid $keeper"

say "check 1: screen size, ${idle}s idle, screen size"
screen_size "$a" || fail "check 1: the first screen size failed"
sleep "$idle"
screen_size "$a" || fail "check 1: screen size after ${idle}s failed (state: $(tart_state "$a"))"
[[ $(tart_state "$a") == running ]] || fail "check 1: $a stopped (state: $(tart_state "$a"))"
say "check 1 passed"

say "check 2: kill the keeper, screen size at once, ${gap}s, screen size"
kill "$keeper"
for _ in 1 2 3 4 5; do
  kill -0 "$keeper" 2>/dev/null || break
  sleep 1
done
kill -0 "$keeper" 2>/dev/null && fail "check 2: the keeper ($keeper) did not stop"
screen_size "$a" || fail "check 2: screen size right after the keeper's close failed (the close crashed tart?)"
[[ $(tart_state "$a") == running ]] || fail "check 2: $a stopped right after the keeper's close"
sleep "$gap"
if screen_size "$a"; then
  fail "check 2: screen size worked after ${gap}s with no client and no keeper; the control shows nothing (is this host affected?)"
fi
sleep 3
[[ $(tart_state "$a") != running ]] || fail "check 2: screen size failed but $a still runs"
say "check 2 passed (tart state: $(tart_state "$a"))"

"$cli" vm stop "$a" >/dev/null 2>&1 || true
[[ -z $(keeper_pids "$a") ]] || fail "check 4: a keeper for $a outlived vm stop"

# ---- Linux clone B: check 3 ------------------------------------------------

start_clone linux "${LINUX_BASE:-}"
b=$clone_id
[[ $(tart_state "$b") == running ]] || fail "$b is not running after vm start"
[[ -n $(keeper_pids "$b") ]] || fail "$b has no keeper"

say "check 3: screen size, ${gap}s, screen size"
screen_size "$b" || fail "check 3: the first screen size failed"
sleep "$gap"
screen_size "$b" || fail "check 3: screen size after ${gap}s failed (state: $(tart_state "$b"))"
[[ $(tart_state "$b") == running ]] || fail "check 3: $b stopped"
say "check 3 passed"

"$cli" vm stop "$b" >/dev/null 2>&1 || fail "vm stop $b failed"

# ---- Check 4 ---------------------------------------------------------------

say "check 4: no keeper and no clone after vm stop"
[[ -z $(keeper_pids) ]] || fail "check 4: a keeper is still running: $(keeper_pids | tr '\n' ' ')"
clones=$("$cli" vm list --json | jq '[.items[] | select(.kind != "golden")] | length')
[[ $clones == 0 ]] || fail "check 4: vm list shows $clones clone(s)"
say "check 4 passed"

say "OK: all four checks passed"
