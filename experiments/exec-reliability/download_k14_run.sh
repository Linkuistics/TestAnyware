#!/bin/sh
# One Linux download consistency run in a container (leaf linux-download-consistency-k14).
# Usage: download_k14_run.sh OUT
# The controller starts one container of the pinned k5 image. The container runs the Linux
# agent from this checkout twice: once with the k14 seam and once unchanged. No VM is used.
# The outer records read only: source identity, host identity, tool versions, the image
# identity and the containers with the k14 label before and after.
# The outer makes one run ID and passes it to the controller. The controller labels its
# container testanyware.k14.run=RUN_ID. containers-before.txt lists every k14 container,
# for the record only. containers-after.txt and cleanup see only this run's label value,
# so a second run on the same host cannot lose its container to this one.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that decide
# the outcome. subjects-before.sha256 and subjects-after.sha256 hold the digests of the
# checkout agent, the shipped Linux agent, the Host CLI and the bound tool. The first list
# of each pair is taken before the probe, the second after the check.
# GNU timeout bounds each step independently: probe 600 s, check 60 s, each inspection 30 s.
# timeout sends TERM to the process group of the step, then KILL 2 s later.
# The controller removes its container in its own finally path. On every exit, cleanup
# also removes each container that still has this run's label, within the 45 s bound.
# An undetected-wrong class is a finding. The check can pass with a reproduced failure.
# The script exits 0 only when the probe and the check exit 0, both digest pairs are equal
# and no container of this run survives. Else it exits 1.
set -u
OUT=$1
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=experiments/exec-reliability
SHIPPED=/opt/homebrew/opt/testanyware/share/testanyware/agents/linux
BOUND=/opt/homebrew/bin/timeout
LABEL=testanyware.k14.run
IMAGE=python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016
CLI_BIN=$(realpath "$(command -v testanyware)")
RUN_ID=$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')
case $RUN_ID in
  [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]) ;;
  *) echo "no run ID" >&2; exit 2 ;;
esac
TOOLS="$HERE/download_k14_run.sh $HERE/download_k14.py $HERE/download_k14_seam.py
$HERE/download_k14_check.py $HERE/download_k7.py $HERE/linux_k5.py $HERE/pyproject.toml
$HERE/uv.lock Taskfile.yml"
# bounded SECONDS COMMAND... runs one step under its own deadline.
bounded() {
  limit=$1
  shift
  "$BOUND" --kill-after=2 "$limit" "$@"
}
subjects() {
  for tree in agents/linux "$SHIPPED" "$HERE/linux_k5_stub"; do
    find "$tree" -type f -not -name '*.pyc' | LC_ALL=C sort |
      while read -r file; do shasum -a 256 "$file"; done
  done
  shasum -a 256 "$CLI_BIN" "$(realpath "$BOUND")"
}
# labelled [VALUE] lists the k14 containers, or only those whose label has VALUE.
labelled() {
  bounded 30 docker ps -a --filter "label=$LABEL${1:+=$1}" \
    --format '{{.ID}} {{.Names}} {{.Status}} {{.Labels}}'
}
cleanup() {
  trap - EXIT INT TERM HUP
  for id in $(bounded 30 docker ps -a -q --filter "label=$LABEL=$RUN_ID"); do
    bounded 45 docker rm -f "$id" >> "$OUT-outer/cleanup.txt" 2>&1
    echo "removed $id exit $?" >> "$OUT-outer/cleanup.txt"
  done
}
mkdir -p "$OUT-outer"
: > "$OUT-outer/cleanup.txt"
echo "$RUN_ID" > "$OUT-outer/run-id"
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
"$BOUND" --version > "$OUT-outer/timeout-version" 2>&1
bounded 30 docker version > "$OUT-outer/docker-version" 2>&1
bounded 30 docker image inspect "$IMAGE" > "$OUT-outer/image-inspect.json" 2>&1
labelled > "$OUT-outer/containers-before.txt"
bounded 600 uv run --project "$HERE" --locked python "$HERE/download_k14.py" "$OUT" "$RUN_ID" \
  > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
PROBE=$?
echo "$PROBE" > "$OUT-outer/probe.exit"
# A failed inspection leaves its exit in the file name, so it cannot read as absence.
labelled "$RUN_ID" > "$OUT-outer/containers-after.txt"
LISTED=$?
echo "$LISTED" > "$OUT-outer/containers-after.exit"
CHECK=1
if test "$PROBE" -eq 0; then
  bounded 60 uv run --project "$HERE" --locked python "$HERE/download_k14_check.py" "$OUT" \
    > "$OUT-outer/check.stdout" 2> "$OUT-outer/check.stderr"
  CHECK=$?
fi
echo "$CHECK" > "$OUT-outer/check.exit"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-after.sha256"
subjects > "$OUT-outer/subjects-after.sha256"
SURVIVORS=0
test "$LISTED" -eq 0 || SURVIVORS=1
test -s "$OUT-outer/containers-after.txt" && SURVIVORS=1
cleanup
if test "$PROBE" -eq 0 && test "$CHECK" -eq 0 && test "$SURVIVORS" -eq 0 &&
  cmp -s "$OUT-outer/tooling-before.sha256" "$OUT-outer/tooling-after.sha256" &&
  cmp -s "$OUT-outer/subjects-before.sha256" "$OUT-outer/subjects-after.sha256"; then
  exit 0
fi
exit 1
