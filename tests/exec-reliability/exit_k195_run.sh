#!/bin/sh
# One traced run of the k4 run 02 workload on one owned macOS clone
# (leaf macos-exit-observation-instrumented-k195).
# Usage: exit_k195_run.sh OUT
# Run from the repository root, after the owner's app handover: the development
# app must answer on the app socket and the installed app must not run.
# The script builds the traced agent from this checkout into a temporary scratch
# directory, and copies the binary to OUT-outer for the upload. It removes that
# copy at the end and keeps its digest.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that
# decide the outcome. The script exits 2 before any clone when the route is wrong.
# It exits 0 only when the probe exits 0 with verdict True, the check exits 0 and
# the two digest lists are equal. Otherwise it exits 1.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=tests/exec-reliability
ROOT=$(pwd)
CLI=$ROOT/cli-rs/target/debug/testanyware
TART=$(command -v tart)
DEV="$ROOT/cli-rs/target/app/dev/TestAnyware Dev.app"
SOCKET="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
TOOLS="$HERE/exit_k195_run.sh $HERE/exit_k195.py $HERE/exit_k195_check.py
$HERE/timeout_k4.py $HERE/timeout_k4_check.py $HERE/routes_k19.py $HERE/controller.py
$HERE/pyproject.toml $HERE/uv.lock scripts/vm_inventory.py Taskfile.yml
agents/macos/Package.swift agents/macos/Package.resolved
agents/macos/Sources/TestAnywareAgent/ExitTrace.swift
agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift"
mkdir -p "$OUT-outer"
O=$OUT-outer
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$O/tooling-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$O/source-commit"
jj diff --git > "$O/source-dirty.diff"
"$CLI" --version > "$O/cli-version"
shasum -a 256 "$CLI" > "$O/cli.sha256"
codesign -dv --verbose=4 "$DEV" > "$O/app-codesign" 2>&1
shasum -a 256 "$DEV/Contents/MacOS/testanyware-app" > "$O/app.sha256"
pgrep -fl testanyware-app > "$O/app-pids-before" || echo none > "$O/app-pids-before"
curl -sf --max-time 2 --unix-socket "$SOCKET" http://localhost/v1/status > "$O/app-status-before" 2>&1
echo $? > "$O/app-status-before.exit"
sysctl hw.ncpu hw.activecpu kern.wq_max_threads kern.wq_max_constrained_threads > "$O/host-sysctl"
"$TART" list --format json > "$O/tart-before.json"
"$CLI" vm list --json --all > "$O/vm-before.json" 2>&1
if pgrep -f /Applications/TestAnyware.app/ > /dev/null ||
  ! pgrep -f "TestAnyware Dev.app/Contents/MacOS/testanyware-app" > /dev/null ||
  test "$(cat "$O/app-status-before.exit")" != 0; then
  echo "route unavailable: the development app must answer and the installed app must not run" >&2
  echo 2 > "$O/run.exit"
  exit 2
fi
BUILD=$(mktemp -d "${TMPDIR:-/tmp}/k195-agent.XXXXXX")
(cd agents/macos && swift build -c release --product testanyware-agent \
  --scratch-path "$BUILD" -Xswiftc -DEXIT_TRACE) > "$O/agent-build.log" 2>&1
echo $? > "$O/agent-build.exit"
cp "$BUILD/release/testanyware-agent" "$O/testanyware-agent" 2> /dev/null
rm -rf "$BUILD"
if test "$(cat "$O/agent-build.exit")" != 0 || test ! -f "$O/testanyware-agent"; then
  echo "traced agent build failed" >&2
  echo 2 > "$O/run.exit"
  exit 2
fi
shasum -a 256 "$O/testanyware-agent" > "$O/agent.sha256"
nm -j "$O/testanyware-agent" | grep -c ExitTrace > "$O/agent-trace-symbols"
codesign -dr- "$O/testanyware-agent" > "$O/agent-requirement" 2>&1
uv run --project "$HERE" --locked python "$HERE/exit_k195.py" "$OUT" "$CLI" "$TART" \
  "$O/testanyware-agent" > "$O/probe.stdout" 2> "$O/probe.stderr"
echo $? > "$O/probe.exit"
"$TART" list --format json > "$O/tart-after.json"
"$CLI" vm list --json --all > "$O/vm-after.json" 2>&1
pgrep -fl testanyware-app > "$O/app-pids-after" || echo none > "$O/app-pids-after"
RUN=$(sed -n 's/^verdict .* run //p' "$O/probe.stdout")
if test -n "$RUN"; then
  uv run --project "$HERE" --locked python "$HERE/exit_k195_check.py" "$RUN" > "$O/check.json"
  echo $? > "$O/check.exit"
else
  echo "no run directory in probe output" > "$O/check.exit"
fi
rm -f "$O/testanyware-agent"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$O/tooling-after.sha256"
cat "$O/probe.stdout" "$O/probe.exit"
STATUS=0
test "$(cat "$O/probe.exit")" = 0 || { echo "probe exit $(cat "$O/probe.exit")" >&2; STATUS=1; }
grep -q '^verdict True run ' "$O/probe.stdout" || { echo "probe verdict is not True" >&2; STATUS=1; }
test "$(cat "$O/check.exit")" = 0 || { echo "check exit $(cat "$O/check.exit")" >&2; STATUS=1; }
cmp -s "$O/tooling-before.sha256" "$O/tooling-after.sha256" || { echo "tool digests changed during the run" >&2; STATUS=1; }
echo "$STATUS" > "$O/run.exit"
exit "$STATUS"
