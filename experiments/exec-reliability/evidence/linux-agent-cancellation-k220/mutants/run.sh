#!/bin/sh
# Usage: run.sh NAME PYTHON_MUTATION_SCRIPT PYTEST_SELECTION...
# Copies the agent into a scratch directory, applies one mutation, and runs the
# selected tests in the pinned container against the copy.
set -u
NAME=$1; MUT=$2; shift 2
SCR=$(dirname "$0")
SRC=/Users/antony/Development/AbstractingComputers/TestAnyware.gh-issue-14/agents/linux
DST=$SCR/$NAME
rm -rf "$DST"; mkdir -p "$DST"
cp -R "$SRC/testanyware_agent" "$SRC/tests" "$SRC/pyproject.toml" "$DST/"
(cd "$DST" && python3 "$MUT") || { echo "mutation did not apply"; exit 2; }
deps="$(cd "$SRC" && uv run --locked python -c 'import sysconfig; print(sysconfig.get_path("purelib"))')"
docker run --rm --init --network none -u 1000:1000 -v "$DST:/agent:ro" -v "$deps:/deps:ro" \
  -e PYTHONPATH=/deps -e PYTHONDONTWRITEBYTECODE=1 -e TESTANYWARE_LINUX_CONTROLS=required \
  -w /agent python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016 \
  python3 -m pytest -q -p no:cacheprovider "$@" > "$DST.pytest.txt" 2>&1
echo "exit $?" >> "$DST.pytest.txt"
tail -n 12 "$DST.pytest.txt" | grep -E "passed|failed|error|exit "
