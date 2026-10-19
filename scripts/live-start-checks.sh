#!/usr/bin/env bash
#
# Live `vm start` checks, run on the macOS host against real tart VMs
# (Linkuistics/TestAnyware#3). These drive the built CLI binary; they are
# not unit tests and they allocate VMs, so run them deliberately.
#
#   live-start-checks.sh slot-exhaustion
#       With both macOS guest slots taken, a third `vm start` must fail within
#       seconds with VM_LIMIT_REACHED (exit 6), name the holders, and leave no
#       clone and no sidecars.
#   live-start-checks.sh keep-on-failure
#       An alive VM whose agent never becomes healthy (forced by pointing
#       HTTP_PROXY at a closed port, which reqwest honours) must fail with
#       VM_BOOT_TIMEOUT (exit 7, stage "agent"): torn down by default, kept
#       and reclaimable by `vm stop` under --keep-on-failure. Takes ~2 min per
#       case (the agent wait budget).
#   live-start-checks.sh qemu-death
#       The QEMU arm (a Windows golden): `kill -9` of QEMU during the agent
#       wait must end the start at once with QEMU_FAILED (exit 1), by default
#       and under --keep-on-failure alike (a dead QEMU has nothing to keep),
#       leaving no clone, session dir, sidecars, QEMU or swtpm. About a minute;
#       the guest need not become healthy. Needs qemu, swtpm and the Windows
#       golden; no tart.
#   live-start-checks.sh qemu-start
#       qemu-death, then one Windows `vm start` that must reach a Healthy agent
#       (minutes: the Windows boot).
#   live-start-checks.sh local-network
#       One macOS `vm start` from this shell. A shell that macOS Local Network
#       privacy allows reaches a Healthy agent, and the start is stopped again.
#       A shell it denies must fail within MAX_DENIAL_SECS with VM_BOOT_TIMEOUT
#       (exit 7), stage "agent", cause "local_network_denied", and leave no VM
#       (ADR-0019). Either way it says which kind of shell this is. Needs one
#       free macOS guest slot.
#   live-start-checks.sh list-custom-id
#       A macOS `vm start --id koine-verify-<n>` (an id without the testanyware-
#       prefix) must appear in `vm list --json`, enriched from its sidecars
#       (platform macos, a vnc endpoint, a pid), and must be gone from the listing
#       once `vm stop` reclaims it. A start that fails at the agent stage keeps its
#       VM (--keep-on-failure), so the check holds in a shell that Local Network
#       privacy denies too. Needs one free macOS guest slot.
#   live-start-checks.sh reliability [N]
#       N (default 10) consecutive macOS `vm start`s must each reach a Healthy
#       agent. Stops at the first failure and leaves that VM kept for
#       inspection.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware),
#      MAX_REFUSAL_SECS (default 30; the slot-exhaustion latency bound),
#      MAX_DENIAL_SECS (default 60; the local-network latency bound),
#      KILL_AFTER_SECS (default 15; how long a qemu-start waits before killing
#      QEMU, so the start is in its agent wait), MAX_DEATH_SECS (default 10;
#      how fast a killed QEMU must end the start).
# Precondition: slot-exhaustion and keep-on-failure need no macOS VM running (the
# limit is Apple's: two per Mac). reliability and local-network start one VM at a
# time and need one free slot.

set -euo pipefail
IFS=$'\n\t'

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REPO_ROOT
readonly BIN="${TESTANYWARE_BIN:-$REPO_ROOT/cli-rs/target/debug/testanyware}"
readonly MAX_REFUSAL_SECS="${MAX_REFUSAL_SECS:-30}"
# Apple's cap on concurrently running macOS guests per Mac.
readonly MACOS_SLOTS=2
readonly MAX_DENIAL_SECS="${MAX_DENIAL_SECS:-60}"
readonly KILL_AFTER_SECS="${KILL_AFTER_SECS:-15}"
readonly MAX_DEATH_SECS="${MAX_DEATH_SECS:-10}"
readonly VMS_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms"
readonly CLONES_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/testanyware/clones"
# Where QEMU's AF_UNIX sockets live (paths.rs `session_dir`).
readonly SESSIONS_DIR="${TMPDIR:-/tmp}"
# A port nothing listens on; used as a dead HTTP proxy to starve the agent probes.
readonly DEAD_PROXY="http://127.0.0.1:9"

started_ids=()

die() {
  echo "live-start-checks: $*" >&2
  exit 1
}

pass() {
  echo "  ✓ $*"
}

# Stop every VM this run started and still owns; best-effort. `vm stop` needs
# the VM's meta, so a clone that outlived its sidecars (the very failure these
# checks exist to catch) is removed through tart directly.
cleanup() {
  local id
  for id in ${started_ids[@]+"${started_ids[@]}"}; do
    "$BIN" vm stop "$id" >/dev/null 2>&1 || true
    if [[ -d "$CLONES_DIR/$id" ]]; then
      echo "live-start-checks: $id's QEMU clone survived 'vm stop'; removing it by hand" >&2
      pkill -f "qemu-system.*$id\.qcow2" 2>/dev/null || true
      pkill -f "swtpm.*clones/$id/" 2>/dev/null || true
      rm -rf "${CLONES_DIR:?}/$id" "${SESSIONS_DIR%/}/testanyware-$id"
    fi
    if tart_names 2>/dev/null | grep -qx "$id"; then
      echo "live-start-checks: $id survived 'vm stop'; removing it with tart" >&2
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
    fi
  done
}
trap cleanup EXIT

# The tools a check needs, as arguments; jq and the CLI binary always.
require_tools() {
  local tool
  for tool in jq "$@"; do
    command -v "$tool" >/dev/null || die "$tool not on PATH"
  done
  [[ -x "$BIN" ]] || die "no CLI binary at $BIN (run: task cli:build)"
}

# tart VM names, one per line, sorted.
tart_names() {
  tart list --format json | jq -r '.[].Name' | sort
}

running_macos_vms() {
  tart list --format json | jq -r '.[] | select(.State == "running") | .Name'
}

require_no_running_vms() {
  local running
  running="$(running_macos_vms)"
  [[ -z "$running" ]] || die "VMs are already running (stop them first): ${running//$'\n'/ }"
}

# One macOS guest slot must be free. A check that starts one VM at a time needs
# no more, so another session's VM does not stop it. If that session fills the
# other slot mid-run, the start fails honestly with VM_LIMIT_REACHED.
require_free_slot() {
  local running
  running="$(running_macos_vms)"
  (($(grep -c . <<<"$running") < MACOS_SLOTS)) ||
    die "both macOS guest slots are taken: ${running//$'\n'/ }"
}

# Occupy one macOS guest slot and set FILLED_ID to the holder's id (a global,
# not stdout: a command substitution would run this in a subshell and lose the
# `started_ids` registration the exit trap relies on). The slot is held by a
# running VM whether or not its agent is healthy, so a start that failed at the
# agent stage but kept its VM (--keep-on-failure, exit 7) fills one just as well
# as a fully successful start; any other outcome is a real failure.
fill_slot() {
  local out rc=0 id
  out="$("$BIN" vm start --platform macos --keep-on-failure --json)" || rc=$?
  if ((rc == 0)); then
    id="$(jq -r '.id' <<<"$out")"
  elif ((rc == 7)) && [[ "$(jq -r '.details.kept' <<<"$out")" == true ]]; then
    id="$(jq -r '.details.vm_id' <<<"$out")"
    echo "  (slot filled by a kept VM whose agent never became healthy: $id)" >&2
  else
    die "could not fill a slot: exit $rc: $out"
  fi
  started_ids+=("$id")
  FILLED_ID="$id"
}

# Run a vm start expected to fail. Sets OUT (stdout JSON), RC (exit code) and
# ELAPSED (whole seconds). Extra args are passed to `vm start`; the caller sets
# any environment with `env`.
run_failing_start() {
  local began=$SECONDS
  RC=0
  OUT="$("$@")" || RC=$?
  ELAPSED=$((SECONDS - began))
}

assert_eq() {
  local what="$1" want="$2" got="$3"
  [[ "$got" == "$want" ]] || die "$what: wanted '$want', got '$got' (output: ${OUT:-})"
  pass "$what = $want"
}

assert_vm_gone() {
  local id="$1"
  tart_names | grep -qx "$id" && die "clone $id still exists"
  [[ ! -e "$VMS_DIR/$id.json" && ! -e "$VMS_DIR/$id.meta.json" ]] ||
    die "sidecars for $id still exist in $VMS_DIR"
  pass "no clone and no sidecars for $id"
}

# Keep QEMU check ids short: the monitor's AF_UNIX path is
# $TMPDIR/testanyware-<id>/monitor.sock and macOS allows under 104 bytes of it.
# Nothing of a QEMU VM may remain: no clone dir, session dir, sidecars, QEMU
# or swtpm. (`vm stop` is the sanctioned remover; this checks a failed start
# did the equivalent itself.)
assert_qemu_vm_gone() {
  local id="$1"
  [[ ! -e "$CLONES_DIR/$id" ]] || die "clone dir for $id still exists"
  [[ ! -e "${SESSIONS_DIR%/}/testanyware-$id" ]] || die "session dir for $id still exists"
  [[ ! -e "$VMS_DIR/$id.json" && ! -e "$VMS_DIR/$id.meta.json" ]] ||
    die "sidecars for $id still exist in $VMS_DIR"
  ! pgrep -f "qemu-system.*$id\.qcow2" >/dev/null || die "QEMU for $id is still running"
  ! pgrep -f "swtpm.*clones/$id/" >/dev/null || die "swtpm for $id is still running"
  pass "no clone, session dir, sidecars or processes for $id"
}

# Start a Windows VM in the background, SIGKILL its QEMU once the start is in
# its agent wait, and set OUT/RC/ELAPSED for the start's end. Extra args go to
# `vm start`. The VM's id is registered by the caller.
start_and_kill_qemu() {
  local id="$1" out_file cli qemu_pid waited=0
  shift
  out_file="$(mktemp)"
  "$BIN" vm start --platform windows --id "$id" --json "$@" >"$out_file" &
  cli=$!
  until qemu_pid="$(pgrep -f "qemu-system.*$id\.qcow2" | head -n1)" && [[ -n "$qemu_pid" ]]; do
    kill -0 "$cli" 2>/dev/null || die "the start ended before QEMU launched: $(cat "$out_file")"
    ((++waited < 120)) || die "QEMU for $id never launched"
    sleep 0.5
  done
  # Past the launch and the monitor discovery, into the agent wait. A Windows
  # guest cannot have a healthy agent this early.
  sleep "$KILL_AFTER_SECS"
  kill -0 "$cli" 2>/dev/null || die "the start ended before the kill: $(cat "$out_file")"
  local began=$SECONDS
  kill -9 "$qemu_pid"
  RC=0
  wait "$cli" || RC=$?
  ELAPSED=$((SECONDS - began))
  OUT="$(cat "$out_file")"
  rm -f "$out_file"
}

check_qemu_healthy_start() {
  local ok="qc$$-ok" port

  # Registered before the start that may create it.
  started_ids+=("$ok")

  echo "qemu-start: a Windows start reaches a Healthy agent (minutes)"
  RC=0
  OUT="$("$BIN" vm start --platform windows --id "$ok" --json)" || RC=$?
  assert_eq "exit code" 0 "$RC"
  port="$(jq -r '.agent.port // empty' <<<"$OUT")"
  [[ -n "$port" ]] || die "the start succeeded without an agent endpoint: $OUT"
  pass "the start names its agent at localhost:$port"
  curl -fsS --max-time 10 "http://localhost:$port/health" >/dev/null ||
    die "the agent named by the start does not answer /health"
  pass "that agent answers /health"
  "$BIN" vm stop "$ok" >/dev/null || die "vm stop could not reclaim $ok"
  assert_qemu_vm_gone "$ok"
}

check_qemu_death() {
  local killed="qc$$-kill" kept="qc$$-keep"

  # Registered before the starts that may create them.
  started_ids+=("$killed" "$kept")

  echo "qemu-death: killing QEMU during the agent wait ends the start at once"
  start_and_kill_qemu "$killed"
  assert_eq "exit code" 1 "$RC"
  assert_eq "error code" QEMU_FAILED "$(jq -r '.code' <<<"$OUT")"
  # The wait's own report, not the launch's or the monitor's: otherwise the
  # kill landed before the agent wait and this exercised another path.
  jq -e '.details.qemu_error | (contains("monitor") or contains("right after launch")) | not' \
    <<<"$OUT" >/dev/null || die "the kill landed before the agent wait (raise KILL_AFTER_SECS): $OUT"
  pass "QEMU died during the agent wait: $(jq -r '.details.qemu_error' <<<"$OUT")"
  ((ELAPSED <= MAX_DEATH_SECS)) || die "the start took ${ELAPSED}s to notice QEMU died (> ${MAX_DEATH_SECS}s)"
  pass "the start ended ${ELAPSED}s after the kill"
  assert_qemu_vm_gone "$killed"

  echo "qemu-death: --keep-on-failure cannot keep a VM whose QEMU is dead"
  start_and_kill_qemu "$kept" --keep-on-failure
  assert_eq "exit code" 1 "$RC"
  assert_eq "error code" QEMU_FAILED "$(jq -r '.code' <<<"$OUT")"
  assert_qemu_vm_gone "$kept"
}

check_slot_exhaustion() {
  require_no_running_vms
  echo "slot-exhaustion: filling both macOS guest slots"
  local first second probe
  fill_slot
  first="$FILLED_ID"
  fill_slot
  second="$FILLED_ID"
  probe="testanyware-slotcheck-$$"
  # Registered before the start that may create it, so an unexpected success
  # or a failed assertion still cleans it up.
  started_ids+=("$probe")
  local before
  before="$(tart_names)"

  echo "slot-exhaustion: starting a third (must be refused)"
  run_failing_start "$BIN" vm start --platform macos --id "$probe" --json
  assert_eq "exit code" 6 "$RC"
  assert_eq "error code" VM_LIMIT_REACHED "$(jq -r '.code' <<<"$OUT")"
  jq -e --arg a "$first" --arg b "$second" '.details.holders | index($a) and index($b)' \
    <<<"$OUT" >/dev/null || die "holders do not name $first and $second: $OUT"
  pass "holders name both running VMs"
  ((ELAPSED <= MAX_REFUSAL_SECS)) || die "refusal took ${ELAPSED}s (> ${MAX_REFUSAL_SECS}s)"
  pass "refused in ${ELAPSED}s"
  assert_eq "clones after the refusal" "$before" "$(tart_names)"
  assert_vm_gone "$probe"
}

check_keep_on_failure() {
  require_no_running_vms
  local torn="testanyware-keepcheck-$$-torn" kept="testanyware-keepcheck-$$-kept"

  # Both ids are registered before the starts that may create them.
  started_ids+=("$torn" "$kept")

  echo "keep-on-failure: default failure tears the VM down (~2 min)"
  run_failing_start env "HTTP_PROXY=$DEAD_PROXY" "$BIN" vm start --platform macos --id "$torn" --json
  assert_eq "exit code" 7 "$RC"
  assert_eq "error code" VM_BOOT_TIMEOUT "$(jq -r '.code' <<<"$OUT")"
  assert_eq "failed stage" agent "$(jq -r '.details.stage' <<<"$OUT")"
  assert_eq "last probe error" connection_refused "$(jq -r '.details.last_error.kind' <<<"$OUT")"
  assert_eq "kept" false "$(jq -r '.details.kept' <<<"$OUT")"
  assert_vm_gone "$torn"

  echo "keep-on-failure: --keep-on-failure leaves a reclaimable VM (~2 min)"
  run_failing_start env "HTTP_PROXY=$DEAD_PROXY" "$BIN" vm start --platform macos --id "$kept" \
    --keep-on-failure --json
  assert_eq "exit code" 7 "$RC"
  assert_eq "kept" true "$(jq -r '.details.kept' <<<"$OUT")"
  assert_eq "kept VM state" running \
    "$(tart list --format json | jq -r --arg id "$kept" '.[] | select(.Name == $id) | .State')"
  [[ -e "$VMS_DIR/$kept.meta.json" && -e "$VMS_DIR/$kept.json" ]] ||
    die "kept VM is missing a sidecar in $VMS_DIR"
  pass "kept VM runs with both sidecars"
  "$BIN" vm stop "$kept" >/dev/null || die "vm stop could not reclaim $kept"
  assert_vm_gone "$kept"
}

check_local_network() {
  local id="testanyware-lncheck-$$"
  require_free_slot

  # Registered before the start that may create it.
  started_ids+=("$id")

  echo "local-network: one macOS start from this shell"
  run_failing_start "$BIN" vm start --platform macos --id "$id" --json
  if ((RC == 0)); then
    "$BIN" exec --vm "$id" true >/dev/null || die "$id started but the agent cannot exec"
    pass "this shell may reach guests: $id reached a Healthy agent in ${ELAPSED}s"
    "$BIN" vm stop "$id" >/dev/null || die "vm stop could not reclaim $id"
    assert_vm_gone "$id"
    return
  fi
  assert_eq "exit code" 7 "$RC"
  assert_eq "error code" VM_BOOT_TIMEOUT "$(jq -r '.code' <<<"$OUT")"
  assert_eq "failed stage" agent "$(jq -r '.details.stage' <<<"$OUT")"
  assert_eq "cause" local_network_denied "$(jq -r '.details.cause' <<<"$OUT")"
  pass "this shell is denied the guest subnet, and the error says so"
  ((ELAPSED <= MAX_DENIAL_SECS)) || die "the denial took ${ELAPSED}s to report (> ${MAX_DENIAL_SECS}s)"
  pass "reported in ${ELAPSED}s"
  assert_vm_gone "$id"
}

# The `vm list` row for $1, or empty when it is not listed.
list_row() {
  "$BIN" vm list --json --all | jq -c --arg id "$1" '.items[] | select(.kind == "running" and .name == $id)'
}

check_list_custom_id() {
  local id="koine-verify-lc$$" out rc=0 row
  require_free_slot

  # Registered before the start that may create it.
  started_ids+=("$id")

  echo "list-custom-id: a macOS start with the id $id"
  out="$("$BIN" vm start --platform macos --id "$id" --keep-on-failure --json)" || rc=$?
  if ((rc != 0)); then
    ((rc == 7)) || die "start failed with exit $rc, not the agent-stage exit 7: $out"
    echo "  (the agent never became healthy, so the VM is kept: still a VM TestAnyware started)" >&2
  fi

  row="$(list_row "$id")"
  [[ -n "$row" ]] || die "$id is running but 'vm list' does not show it"
  pass "'vm list' shows $id although it lacks the testanyware- prefix"
  assert_eq "platform" macos "$(jq -r '.platform' <<<"$row")"
  assert_eq "backend" tart "$(jq -r '.backend' <<<"$row")"
  [[ "$(jq -r '.vnc' <<<"$row")" != null ]] || die "$id's row has no vnc endpoint: $row"
  [[ "$(jq -r '.pid' <<<"$row")" != null ]] || die "$id's row has no pid: $row"
  pass "the row is enriched from the sidecars: $row"

  "$BIN" vm stop "$id" >/dev/null || die "vm stop could not reclaim $id"
  [[ -z "$(list_row "$id")" ]] || die "$id is still listed after 'vm stop'"
  pass "$id is gone from 'vm list' after 'vm stop'"
  assert_vm_gone "$id"
}

check_reliability() {
  local runs="${1:-10}" run id began
  [[ "$runs" =~ ^[1-9][0-9]*$ ]] || die "run count must be a positive integer, got '$runs'"
  require_free_slot
  for ((run = 1; run <= runs; run++)); do
    began=$SECONDS
    RC=0
    OUT="$("$BIN" vm start --platform macos --keep-on-failure --json)" || RC=$?
    if ((RC != 0)); then
      echo "  ✗ run $run/$runs failed (exit $RC) after $((SECONDS - began))s:" >&2
      jq -c '{code, message, details}' <<<"$OUT" >&2 || echo "$OUT" >&2
      id="$(jq -r '.details.vm_id // empty' <<<"$OUT")"
      [[ -z "$id" ]] || echo "  the VM is kept for inspection: testanyware vm stop $id" >&2
      exit 1
    fi
    id="$(jq -r '.id' <<<"$OUT")"
    started_ids+=("$id")
    "$BIN" exec --vm "$id" true >/dev/null || die "run $run: $id started but the agent cannot exec"
    pass "run $run/$runs: $id reached a Healthy agent in $((SECONDS - began))s"
    "$BIN" vm stop "$id" >/dev/null || die "run $run: could not stop $id"
  done
  echo "reliability: $runs/$runs consecutive starts reached a Healthy agent"
}

usage() {
  # The header comment block (everything before `set -euo`) is the usage text.
  sed -n '3,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//' >&2
  exit 2
}

main() {
  local check="${1:-}"
  [[ -n "$check" ]] || usage
  shift
  case "$check" in
    slot-exhaustion)
      require_tools tart
      check_slot_exhaustion
      ;;
    keep-on-failure)
      require_tools tart
      check_keep_on_failure
      ;;
    local-network)
      require_tools tart
      check_local_network
      ;;
    list-custom-id)
      require_tools tart
      check_list_custom_id
      ;;
    reliability)
      require_tools tart
      check_reliability "$@"
      ;;
    qemu-death)
      require_tools pgrep
      check_qemu_death
      ;;
    qemu-start)
      require_tools curl pgrep
      check_qemu_death
      check_qemu_healthy_start
      ;;
    *) usage ;;
  esac
}

main "$@"
