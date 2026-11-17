#!/bin/sh
# One macOS download consistency run on the host (leaf download-consistency-k7).
# Usage: download_k7_run.sh OUT
# The probe runs the shipped macOS agent and an agent built from this checkout, each on
# loopback, with the k7 shim. No VM, container or guest is used.
# The outer records read only: source identity, host identity, tool versions, the
# agent processes and the loopback listeners before and after.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that decide
# the outcome. subjects-before.sha256 and subjects-after.sha256 hold the digests of the
# agent sources, the shipped agent, the Host CLI and the bound tool. The first list of
# each pair is taken before the build, the second after the check.
# GNU timeout bounds each step independently (integration k210):
#   build 600 s, probe 300 s, check 60 s, each inspection 30 s.
# timeout sends TERM to the process group of the step, then KILL 2 s later.
# The agents run in their own sessions, so the controller lists each agent in
# OUT/agents.pids. On every exit, cleanup ends a listed agent only when its command line
# still names its binary. Then cleanup removes the build tree.
# An undetected-wrong class is a finding. The check can pass with a reproduced failure.
# The script exits 0 only when the build, the probe and the check exit 0, both digest
# pairs are equal and no agent process from the probe survives. Else it exits 1.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=experiments/exec-reliability
SHIPPED=/opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent
BOUND=/opt/homebrew/bin/timeout
CLI_BIN=$(realpath "$(command -v testanyware)")
TOOLS="$HERE/download_k7_run.sh $HERE/download_k7.py $HERE/download_k7_shim.c
$HERE/download_k7_check.py $HERE/pyproject.toml $HERE/uv.lock Taskfile.yml"
BUILD_DIR=
# bounded SECONDS COMMAND... runs one step under its own deadline.
bounded() {
  limit=$1
  shift
  "$BOUND" --kill-after=2 "$limit" "$@"
}
subjects() {
  find agents/macos -type f -not -path '*/.build/*' -not -path '*/.swiftpm/*' | LC_ALL=C sort |
    while read -r file; do shasum -a 256 "$file"; done
  shasum -a 256 "$SHIPPED" "$CLI_BIN" "$(realpath "$BOUND")"
}
cleanup() {
  trap - EXIT INT TERM HUP
  if test -f "$OUT/agents.pids"; then
    while read -r pid binary; do
      command=$(bounded 30 ps -p "$pid" -o command= 2>/dev/null) || continue
      case "$command" in
        "$binary "*)
          kill -TERM "$pid" 2>/dev/null
          bounded 2 sh -c "while kill -0 $pid 2>/dev/null; do sleep 0.1; done"
          kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null
          echo "ended $pid $binary" ;;
      esac
    done < "$OUT/agents.pids" >> "$OUT-outer/cleanup.txt"
  fi
  if test -n "$BUILD_DIR"; then
    rm -rf "$BUILD_DIR"
    test -e "$BUILD_DIR" && echo present > "$OUT-outer/build-dir-after" ||
      echo removed > "$OUT-outer/build-dir-after"
  fi
}
mkdir -p "$OUT-outer"
: > "$OUT-outer/cleanup.txt"
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-before.sha256"
subjects > "$OUT-outer/subjects-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
sysctl hw.ncpu hw.activecpu hw.memsize kern.osproductversion > "$OUT-outer/host-sysctl"
testanyware --version > "$OUT-outer/cli-version"
swift --version > "$OUT-outer/swift-version" 2>&1
clang --version > "$OUT-outer/clang-version" 2>&1
"$BOUND" --version > "$OUT-outer/timeout-version" 2>&1
cp agents/macos/Package.resolved "$OUT-outer/Package.resolved"
codesign -dv --verbose=4 "$SHIPPED" > "$OUT-outer/shipped-codesign" 2>&1
bounded 30 pgrep -lf testanyware-agent > "$OUT-outer/agents-before.txt"
bounded 30 lsof -nP -iTCP -sTCP:LISTEN > "$OUT-outer/listen-before.txt"
# The build tree is large, so it lives outside the repository. Only its log, its exit
# and the digest of the agent binary are kept. Cleanup removes the tree.
BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/download-k7-build.XXXXXX")
echo "$BUILD_DIR" > "$OUT-outer/build-dir"
bounded 600 swift build -c release --package-path agents/macos --scratch-path "$BUILD_DIR" \
  > "$OUT-outer/build.log" 2>&1
BUILD=$?
echo "$BUILD" > "$OUT-outer/build.exit"
CHECKOUT="$BUILD_DIR/release/testanyware-agent"
PROBE=1
if test "$BUILD" -eq 0; then
  bounded 300 uv run --project "$HERE" --locked python "$HERE/download_k7.py" "$OUT" \
    "shipped=$SHIPPED" "checkout=$CHECKOUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
  PROBE=$?
fi
echo "$PROBE" > "$OUT-outer/probe.exit"
# A failed inspection leaves its exit in the file name, so it cannot read as absence.
bounded 30 pgrep -lf testanyware-agent > "$OUT-outer/agents-after.txt"
PGREP=$?
echo "$PGREP" > "$OUT-outer/agents-after.exit"
bounded 30 lsof -nP -iTCP -sTCP:LISTEN > "$OUT-outer/listen-after.txt"
CHECK=1
if test "$PROBE" -eq 0; then
  bounded 60 uv run --project "$HERE" --locked python "$HERE/download_k7_check.py" "$OUT" \
    > "$OUT-outer/check.stdout" 2> "$OUT-outer/check.stderr"
  CHECK=$?
fi
echo "$CHECK" > "$OUT-outer/check.exit"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-after.sha256"
subjects > "$OUT-outer/subjects-after.sha256"
SURVIVORS=0
# pgrep exits 0 when it lists a process and 1 when it lists none. Any other exit is unknown.
test "$PGREP" -le 1 || SURVIVORS=1
# A survivor is an agent process listed after the probe and not before it.
if grep -vxF -f "$OUT-outer/agents-before.txt" "$OUT-outer/agents-after.txt" \
  > "$OUT-outer/survivors.txt"; then SURVIVORS=1; fi
cleanup
if test "$BUILD" -eq 0 && test "$PROBE" -eq 0 && test "$CHECK" -eq 0 && test "$SURVIVORS" -eq 0 &&
  cmp -s "$OUT-outer/tooling-before.sha256" "$OUT-outer/tooling-after.sha256" &&
  cmp -s "$OUT-outer/subjects-before.sha256" "$OUT-outer/subjects-after.sha256"; then
  exit 0
fi
exit 1
