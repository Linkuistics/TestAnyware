#!/bin/sh
# One Linux disconnect availability run (leaf linux-disconnect-availability-k6).
# Usage: linux_k6_run.sh container|live OUT
# `container` runs the seven HTTP and client controls in one fresh container of the
# pinned k5 image. `live` runs the k13 burst on one fresh unchanged Linux clone.
# The outer records read only: inventories, source identity, host identity and agent bundle.
# tooling-before.sha256 and tooling-after.sha256 hold the digests of the tools that decide
# the outcome. The first is taken before the probe, the second after the check.
# The probe verdict is True only when every call ends as planned. A blocked channel or a
# surviving child is a finding, so verdict True can still go with a reproduced failure.
# The script exits 0 only when the probe exits 0 with verdict True, the check exits 0
# and the two tool digest lists are equal. Otherwise it exits 1.
set -u
MODE=$1
OUT=$2
case "$MODE" in container|live) ;; *) echo "usage: linux_k6_run.sh container|live OUT" >&2; exit 64 ;; esac
test ! -e "$OUT" && test ! -e "$OUT-outer" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
HERE=experiments/exec-reliability
BUNDLE=/opt/homebrew/opt/testanyware/share/testanyware/agents/linux/testanyware_agent
TOOLS="$HERE/linux_k6_run.sh $HERE/linux_k6.py $HERE/linux_k6_seam.py $HERE/linux_k6_check.py
$HERE/disconnect_k13_burst.py $HERE/disconnect_k13_check.py $HERE/disconnect_k13_seam.py
$HERE/linux_k204.py $HERE/linux_k5.py $HERE/linux_k5_seam.py $HERE/outcomes_k12_seam.py
$HERE/routes_k19.py $HERE/controller.py $HERE/inspection.py $HERE/retention.py
$HERE/storage_client.py $HERE/turns.py $HERE/worker_capture.py $HERE/writer.py
$HERE/linux_k5_stub/gi/__init__.py $HERE/linux_k5_stub/gi/repository/__init__.py
$HERE/linux_k5_stub/pyatspi/__init__.py agents/linux/testanyware_agent/*.py
$HERE/pyproject.toml $HERE/uv.lock scripts/vm_inventory.py Taskfile.yml"
mkdir -p "$OUT-outer"
# shellcheck disable=SC2086 # TOOLS is a fixed list of paths without spaces.
shasum -a 256 $TOOLS > "$OUT-outer/tooling-before.sha256"
jj log -r @- --no-graph -T 'commit_id ++ "\n"' > "$OUT-outer/source-commit"
jj diff --git > "$OUT-outer/source-dirty.diff"
sysctl hw.ncpu hw.activecpu hw.memsize > "$OUT-outer/host-sysctl"
shasum -a 256 agents/linux/testanyware_agent/*.py > "$OUT-outer/checkout-agent.sha256"
if test "$MODE" = live; then
  testanyware --version > "$OUT-outer/cli-version"
  tart --version > "$OUT-outer/tart-version"
  shasum -a 256 "$BUNDLE"/*.py > "$OUT-outer/bundle-agent.sha256"
  tart list --format json > "$OUT-outer/tart-before.json"
  testanyware vm list --json --all > "$OUT-outer/vm-before.json"
else
  docker version --format '{{json .}}' > "$OUT-outer/docker-version.json"
  docker ps -a --format '{{json .}}' > "$OUT-outer/containers-before.jsonl"
fi
uv run --project "$HERE" --locked python "$HERE/linux_k6.py" "$MODE" "$OUT" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
echo $? > "$OUT-outer/probe.exit"
if test "$MODE" = live; then
  tart list --format json > "$OUT-outer/tart-after.json"
  testanyware vm list --json --all > "$OUT-outer/vm-after.json"
else
  docker ps -a --format '{{json .}}' > "$OUT-outer/containers-after.jsonl"
fi
if test -f "$OUT/summary.json"; then
  uv run --project "$HERE" --locked python "$HERE/linux_k6_check.py" "$MODE" "$OUT" > "$OUT-outer/check.stdout"
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
