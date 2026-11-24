#!/bin/sh
# One k13 disconnect-availability run (leaf macos-disconnect-availability-k13).
# Usage: disconnect_k13_run.sh host|live OUT
# host: the deterministic HTTP and client controls on a loopback copy of the
#   checkout agent. The run builds the copy, so it needs no VM.
# live: the control, concurrent and term modes on one fresh unchanged macOS clone.
# The outer records are read only. tooling-before.sha256 is taken before the probe.
# tooling-after.sha256 is taken after the check. The script exits 0 only when the
# probe exits 0 with verdict True, the check exits 0 and the two digest lists are equal.
# A finding, such as a blocked channel, is a measured outcome. It does not change
# the exit status. An unknown outcome does.
set -u
MODE=${1:-}
OUT=${2:-}
case "$MODE" in host | live) ;; *) echo "usage: disconnect_k13_run.sh host|live OUT" >&2; exit 64 ;; esac
test -n "$OUT" || { echo "usage: disconnect_k13_run.sh host|live OUT" >&2; exit 64; }
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=tests/exec-reliability
TOOLS="$HERE/disconnect_k13_run.sh $HERE/disconnect_k13.py $HERE/disconnect_k13_burst.py
$HERE/disconnect_k13_host.py $HERE/disconnect_k13_seam.py $HERE/disconnect_k13_check.py
$HERE/outcomes_k12.py $HERE/outcomes_k12_seam.py $HERE/timeout_k4.py $HERE/routes_k19.py
$HERE/controller.py $HERE/pyproject.toml $HERE/uv.lock scripts/vm_inventory.py Taskfile.yml"
if test "$MODE" = host; then
  PROBE=$HERE/disconnect_k13_host.py
  TOOLS="$TOOLS cli-rs/target/debug/testanyware cli-rs/target/debug/testanyware-app"
else
  PROBE=$HERE/disconnect_k13.py
fi
mkdir -p "$OUT-outer"
echo "$MODE" > "$OUT-outer/mode"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
testanyware --version > "$OUT-outer/cli-version" 2>&1
codesign -dv --verbose=4 /Applications/TestAnyware.app > "$OUT-outer/app-codesign" 2>&1
shasum -a 256 /opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent > "$OUT-outer/bundle-agent.sha256"
sysctl hw.ncpu hw.activecpu kern.wq_max_threads kern.wq_max_constrained_threads > "$OUT-outer/host-sysctl"
tart list --format json > "$OUT-outer/tart-before.json"
testanyware vm list --json --all > "$OUT-outer/vm-before.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-before" || echo none > "$OUT-outer/app-pid-before"
uv run --project "$HERE" --locked python "$PROBE" "$OUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
echo $? > "$OUT-outer/probe.exit"
tart list --format json > "$OUT-outer/tart-after.json"
testanyware vm list --json --all > "$OUT-outer/vm-after.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-after" || echo none > "$OUT-outer/app-pid-after"
if test -f "$OUT/summary.json"; then
  uv run --project "$HERE" --locked python "$HERE/disconnect_k13_check.py" "$MODE" "$OUT" > "$OUT-outer/check.stdout"
  echo $? > "$OUT-outer/check.exit"
else
  echo "no summary in OUT" > "$OUT-outer/check.exit"
fi
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-after.sha256"
cat "$OUT-outer/probe.stdout" "$OUT-outer/probe.exit"
test -f "$OUT-outer/check.stdout" && cat "$OUT-outer/check.stdout"
STATUS=0
test "$(cat "$OUT-outer/probe.exit")" = 0 || { echo "probe exit $(cat "$OUT-outer/probe.exit")" >&2; STATUS=1; }
grep -q '^verdict True run ' "$OUT-outer/probe.stdout" || { echo "probe verdict is not True" >&2; STATUS=1; }
test "$(cat "$OUT-outer/check.exit")" = 0 || { echo "check exit $(cat "$OUT-outer/check.exit")" >&2; STATUS=1; }
cmp -s "$OUT-outer/tooling-before.sha256" "$OUT-outer/tooling-after.sha256" || { echo "tool digests changed during the run" >&2; STATUS=1; }
echo "$STATUS" > "$OUT-outer/run.exit"
exit "$STATUS"
