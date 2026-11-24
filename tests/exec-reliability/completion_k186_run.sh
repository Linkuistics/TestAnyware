#!/bin/sh
# One live completion probe on one macOS clone (leaf macos-completion-live-k186).
# Usage: completion_k186_run.sh OUT
# The outer records read only: inventories, app pid, source identity and host identity.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that decide
# the outcome. The first is taken before the probe, the second after the receipt check.
# The script exits 0 only when the probe exits 0 with verdict True, the receipt check
# exits 0 and the two tool digest lists are equal. Otherwise it exits 1.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=tests/exec-reliability
TOOLS="$HERE/completion_k186_run.sh $HERE/completion_k186.py $HERE/completion_k186_check.py
$HERE/concurrent_k186.py $HERE/routes_k19.py $HERE/controller.py $HERE/pyproject.toml
$HERE/uv.lock scripts/vm_inventory.py Taskfile.yml"
mkdir -p "$OUT-outer"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
testanyware --version > "$OUT-outer/cli-version"
codesign -dv --verbose=4 /Applications/TestAnyware.app > "$OUT-outer/app-codesign" 2>&1
shasum -a 256 /opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent > "$OUT-outer/bundle-agent.sha256"
sysctl hw.ncpu hw.activecpu kern.wq_max_threads kern.wq_max_constrained_threads > "$OUT-outer/host-sysctl"
tart list --format json > "$OUT-outer/tart-before.json"
testanyware vm list --json --all > "$OUT-outer/vm-before.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-before" || echo none > "$OUT-outer/app-pid-before"
uv run --project tests/exec-reliability --locked python tests/exec-reliability/completion_k186.py "$OUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
echo $? > "$OUT-outer/probe.exit"
tart list --format json > "$OUT-outer/tart-after.json"
testanyware vm list --json --all > "$OUT-outer/vm-after.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-after" || echo none > "$OUT-outer/app-pid-after"
RUN=$(sed -n 's/^verdict .* run //p' "$OUT-outer/probe.stdout")
if test -n "$RUN"; then
  uv run --project tests/exec-reliability --locked python -I tests/exec-reliability/completion_k186_check.py "$RUN" > "$OUT-outer/check.json"
  echo $? > "$OUT-outer/check.exit"
else
  echo "no run directory in probe output" > "$OUT-outer/check.exit"
fi
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-after.sha256"
cat "$OUT-outer/probe.stdout" "$OUT-outer/probe.exit"
STATUS=0
test "$(cat "$OUT-outer/probe.exit")" = 0 || { echo "probe exit $(cat "$OUT-outer/probe.exit")" >&2; STATUS=1; }
grep -q '^verdict True run ' "$OUT-outer/probe.stdout" || { echo "probe verdict is not True" >&2; STATUS=1; }
test "$(cat "$OUT-outer/check.exit")" = 0 || { echo "receipt check exit $(cat "$OUT-outer/check.exit")" >&2; STATUS=1; }
cmp -s "$OUT-outer/tooling-before.sha256" "$OUT-outer/tooling-after.sha256" || { echo "tool digests changed during the run" >&2; STATUS=1; }
echo "$STATUS" > "$OUT-outer/run.exit"
exit "$STATUS"
