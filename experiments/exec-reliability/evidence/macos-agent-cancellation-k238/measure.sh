#!/bin/sh
# One recorded measurement of leaf macos-agent-cancellation-k238.
# Usage: measure.sh OUT COMMAND...
# OUT must not exist. The script records the source identity, the digests of the
# subjects before and after, the command, its output, its exit and its monotonic
# duration. The subjects are every file of agents/macos outside .build and the
# shipped macOS agent. The script exits with the exit of COMMAND, or 3 when a
# subject changed during the run.
set -u
OUT=$1
shift
test ! -e "$OUT" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
mkdir -p "$OUT"
SHIPPED=/opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent
subjects() {
  find agents/macos -type f -not -path '*/.build/*' -not -path '*/.swiftpm/*' | LC_ALL=C sort |
    while read -r file; do shasum -a 256 "$file"; done
  shasum -a 256 "$SHIPPED"
}
subjects > "$OUT/subjects-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT/source-commit"
# The diff leaves out this evidence directory.
jj diff --git '~experiments/exec-reliability/evidence/macos-agent-cancellation-k238' > "$OUT/source-dirty.diff"
printf '%s\n' "$@" > "$OUT/argv"
swift --version > "$OUT/swift-version" 2>&1
start=$(perl -MTime::HiRes=clock_gettime,CLOCK_MONOTONIC -e 'printf "%.3f", clock_gettime(CLOCK_MONOTONIC)')
"$@" > "$OUT/output.log" 2>&1
status=$?
end=$(perl -MTime::HiRes=clock_gettime,CLOCK_MONOTONIC -e 'printf "%.3f", clock_gettime(CLOCK_MONOTONIC)')
echo "$status" > "$OUT/exit"
echo "$end - $start" | bc > "$OUT/seconds"
subjects > "$OUT/subjects-after.sha256"
if ! cmp -s "$OUT/subjects-before.sha256" "$OUT/subjects-after.sha256"; then
  echo "a subject changed during the run" >&2
  exit 3
fi
exit "$status"
