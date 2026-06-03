#!/bin/bash
# Thin wrapper around `testanyware vm start`. Retained so existing callers
# (docs, cron jobs, integration tests) keep working across the Rust port.
# See `testanyware vm start --help` for options.
set -euo pipefail

command -v testanyware >/dev/null 2>&1 || {
    echo "testanyware not found on PATH — build it first (cargo build --release --manifest-path cli-rs/Cargo.toml)" >&2
    exit 1
}

exec testanyware vm start "$@"
