#!/bin/bash
# Thin wrapper around `testanyware vm delete`. Retained so existing callers
# (docs, cron jobs, integration tests) keep working after the Swift port.
# See `testanyware vm delete --help` for options.
set -euo pipefail

command -v testanyware >/dev/null 2>&1 || {
    echo "testanyware not found on PATH — build cli/ first (swift build --package-path cli/)" >&2
    exit 1
}

exec testanyware vm delete "$@"
