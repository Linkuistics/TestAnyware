#!/bin/sh
# One macOS download consistency run on the host (leaf download-consistency-k7).
# Usage: download_k7_run.sh OUT
# The probe runs the shipped macOS agent and an agent built from this checkout, each on
# loopback, with the k7 shim. No VM, container or guest is used.
# The outer records read only: source identity, host identity, tool versions, the
# agent processes and the loopback listeners before and after.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that decide
# the outcome. The first is taken before the probe, the second after the check.
# An undetected-wrong class is a finding. The check can pass with a reproduced failure.
# The script exits 0 only when the build, the probe and the check exit 0, the two tool
# digest lists are equal and no agent process from the probe survives. Else it exits 1.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=experiments/exec-reliability
SHIPPED=/opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent
TOOLS="$HERE/download_k7_run.sh $HERE/download_k7.py $HERE/download_k7_shim.c
$HERE/download_k7_check.py $HERE/pyproject.toml $HERE/uv.lock Taskfile.yml"
mkdir -p "$OUT-outer"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
sysctl hw.ncpu hw.activecpu hw.memsize kern.osproductversion > "$OUT-outer/host-sysctl"
testanyware --version > "$OUT-outer/cli-version"
swift --version > "$OUT-outer/swift-version" 2>&1
clang --version > "$OUT-outer/clang-version" 2>&1
cp agents/macos/Package.resolved "$OUT-outer/Package.resolved"
codesign -dv --verbose=4 "$SHIPPED" > "$OUT-outer/shipped-codesign" 2>&1
pgrep -lf testanyware-agent > "$OUT-outer/agents-before.txt"
lsof -nP -iTCP -sTCP:LISTEN > "$OUT-outer/listen-before.txt"
# The build tree is large, so it lives outside the repository. Only its log, its exit
# and the digest of the agent binary are kept. The script removes the tree at the end.
BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/download-k7-build.XXXXXX")
echo "$BUILD_DIR" > "$OUT-outer/build-dir"
swift build -c release --package-path agents/macos --scratch-path "$BUILD_DIR" \
  > "$OUT-outer/build.log" 2>&1
BUILD=$?
echo "$BUILD" > "$OUT-outer/build.exit"
CHECKOUT="$BUILD_DIR/release/testanyware-agent"
PROBE=1
if test "$BUILD" -eq 0; then
  uv run --project "$HERE" --locked python "$HERE/download_k7.py" "$OUT" \
    "shipped=$SHIPPED" "checkout=$CHECKOUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
  PROBE=$?
fi
echo "$PROBE" > "$OUT-outer/probe.exit"
pgrep -lf testanyware-agent > "$OUT-outer/agents-after.txt"
lsof -nP -iTCP -sTCP:LISTEN > "$OUT-outer/listen-after.txt"
CHECK=1
if test "$PROBE" -eq 0; then
  uv run --project "$HERE" --locked python "$HERE/download_k7_check.py" "$OUT" \
    > "$OUT-outer/check.stdout" 2> "$OUT-outer/check.stderr"
  CHECK=$?
fi
echo "$CHECK" > "$OUT-outer/check.exit"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-after.sha256"
rm -rf "$BUILD_DIR"
test -e "$BUILD_DIR" && echo present > "$OUT-outer/build-dir-after" || echo removed > "$OUT-outer/build-dir-after"
SURVIVORS=0
# A survivor is an agent process listed after the probe and not before it.
if grep -vxF -f "$OUT-outer/agents-before.txt" "$OUT-outer/agents-after.txt" \
  > "$OUT-outer/survivors.txt"; then SURVIVORS=1; fi
if test "$BUILD" -eq 0 && test "$PROBE" -eq 0 && test "$CHECK" -eq 0 && test "$SURVIVORS" -eq 0 &&
  cmp -s "$OUT-outer/tooling-before.sha256" "$OUT-outer/tooling-after.sha256"; then
  exit 0
fi
exit 1
