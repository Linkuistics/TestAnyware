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
#   live-start-checks.sh reliability [N]
#       N (default 10) consecutive macOS `vm start`s must each reach a Healthy
#       agent. Stops at the first failure and leaves that VM kept for
#       inspection.
#
# Env: TESTANYWARE_BIN (default cli-rs/target/debug/testanyware),
#      MAX_REFUSAL_SECS (default 30; the slot-exhaustion latency bound).
# Precondition: no macOS VM is running (the limit is Apple's: two per Mac).

set -euo pipefail
IFS=$'\n\t'

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REPO_ROOT
readonly BIN="${TESTANYWARE_BIN:-$REPO_ROOT/cli-rs/target/debug/testanyware}"
readonly MAX_REFUSAL_SECS="${MAX_REFUSAL_SECS:-30}"
readonly VMS_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms"
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
    if tart_names 2>/dev/null | grep -qx "$id"; then
      echo "live-start-checks: $id survived 'vm stop'; removing it with tart" >&2
      tart stop "$id" >/dev/null 2>&1 || true
      tart delete "$id" >/dev/null 2>&1 || true
    fi
  done
}
trap cleanup EXIT

require_tools() {
  local tool
  for tool in jq tart; do
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

check_reliability() {
  local runs="${1:-10}" run id began
  [[ "$runs" =~ ^[1-9][0-9]*$ ]] || die "run count must be a positive integer, got '$runs'"
  require_no_running_vms
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
  require_tools
  case "$check" in
    slot-exhaustion) check_slot_exhaustion ;;
    keep-on-failure) check_keep_on_failure ;;
    reliability) check_reliability "$@" ;;
    *) usage ;;
  esac
}

main "$@"
