#!/bin/sh
# One live completion probe on one macOS clone (leaf macos-completion-live-k186).
# Usage: completion_k186_run.sh OUT
# The outer records read only: inventories, app pid, source identity and host identity.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
mkdir -p "$OUT-outer"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
testanyware --version > "$OUT-outer/cli-version"
codesign -dv --verbose=4 /Applications/TestAnyware.app > "$OUT-outer/app-codesign" 2>&1
shasum -a 256 /opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent > "$OUT-outer/bundle-agent.sha256"
sysctl hw.ncpu kern.wq_max_threads kern.wq_max_constrained_threads > "$OUT-outer/host-sysctl"
tart list --format json > "$OUT-outer/tart-before.json"
testanyware vm list --json --all > "$OUT-outer/vm-before.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-before" || echo none > "$OUT-outer/app-pid-before"
uv run --project experiments/exec-reliability --locked python experiments/exec-reliability/completion_k186.py "$OUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
echo $? > "$OUT-outer/probe.exit"
tart list --format json > "$OUT-outer/tart-after.json"
testanyware vm list --json --all > "$OUT-outer/vm-after.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-after" || echo none > "$OUT-outer/app-pid-after"
RUN=$(sed -n 's/^verdict .* run //p' "$OUT-outer/probe.stdout")
if test -n "$RUN"; then
  uv run --project experiments/exec-reliability --locked python -I experiments/exec-reliability/completion_k186_check.py "$RUN" > "$OUT-outer/check.json"
  echo $? > "$OUT-outer/check.exit"
fi
cat "$OUT-outer/probe.stdout" "$OUT-outer/probe.exit"
