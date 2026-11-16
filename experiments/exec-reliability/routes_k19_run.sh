#!/bin/sh
# One live route probe for one OS (leaf diagnostic-routes-k19). Usage: routes_k19_run.sh OUT OS
# The outer listings and app pid record golden preservation. They read only.
set -u
OUT=$1
OS=$2
test ! -e "$OUT" || { echo "refusing an existing OUT: $OUT" >&2; exit 2; }
mkdir -p "$OUT-outer"
tart list --format json > "$OUT-outer/tart-before.json"
testanyware vm list --json --all > "$OUT-outer/vm-before.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-before" || echo none > "$OUT-outer/app-pid-before"
uv run --project experiments/exec-reliability --locked python experiments/exec-reliability/routes_k19.py "$OUT" "$OS" > "$OUT-outer/probe.stdout" 2> "$OUT-outer/probe.stderr"
echo $? > "$OUT-outer/probe.exit"
tart list --format json > "$OUT-outer/tart-after.json"
testanyware vm list --json --all > "$OUT-outer/vm-after.json"
pgrep -x testanyware-app > "$OUT-outer/app-pid-after" || echo none > "$OUT-outer/app-pid-after"
cat "$OUT-outer/probe.stdout" "$OUT-outer/probe.exit"
