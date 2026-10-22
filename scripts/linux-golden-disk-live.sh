#!/usr/bin/env bash
#
# Does a clone of a Linux golden have the free disk that golden creation grows
# it for? Starts one clone, reads `df` of `/`, and fails when the clone has
# less than MIN_AVAIL_GB available. Always stops and deletes the clone.
#
#   linux-golden-disk-live.sh <golden>
#
# Env: TESTANYWARE (the CLI; default cli-rs/target/debug/testanyware, built by
# `task cli:build`); MIN_AVAIL_GB (default 30). A clone of a golden that was not
# grown has about 8.8 GB available; a grown one about 36 GB.
#
# About 1 minute. Run it from a terminal that may reach guest IPs (macOS Local
# Network permission).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "linux-golden-disk-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cli=${TESTANYWARE:-$root/cli-rs/target/debug/testanyware}
golden=${1:-}
min_avail_gb=${MIN_AVAIL_GB:-30}

say() { echo "linux-golden-disk-live: $*" >&2; }

fail() {
  echo "linux-golden-disk-live: FAIL: $1" >&2
  exit 1
}

[[ -n $golden ]] || fail "usage: linux-golden-disk-live.sh <golden>"

clone_id=
cleanup() {
  if [[ -n $clone_id ]]; then
    "$cli" vm stop "$clone_id" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

say "starting a clone of $golden..."
started=$SECONDS
out=$("$cli" vm start --platform linux --base "$golden" --json) || fail "vm start exited non-zero"
clone_id=$(jq -r '.id // empty' <<<"$out")
[[ -n $clone_id ]] || fail "vm start printed no VM id"
say "$clone_id started in $((SECONDS - started)) s"

# Bytes, so that the threshold is exact: 1B-blocks header, then one number each.
df_out=$("$cli" exec --vm "$clone_id" 'df -B1 --output=size,avail /') || fail "exec df failed"
say "df of / in $clone_id: $(tail -n 1 <<<"$df_out")"
IFS=' ' read -r size avail < <(tail -n 1 <<<"$df_out")
[[ $size =~ ^[0-9]+$ && $avail =~ ^[0-9]+$ ]] || fail "could not read the sizes from: $df_out"

size_gb=$(awk -v b="$size" 'BEGIN { printf "%.1f", b / 1e9 }')
avail_gb=$(awk -v b="$avail" 'BEGIN { printf "%.1f", b / 1e9 }')
say "/ is $size_gb GB, $avail_gb GB available (minimum $min_avail_gb GB available)"
awk -v b="$avail" -v m="$min_avail_gb" 'BEGIN { exit !(b >= m * 1e9) }' ||
  fail "$avail_gb GB available on / is below $min_avail_gb GB (was $golden built before the disk growth?)"

"$cli" vm stop "$clone_id" >/dev/null 2>&1 || fail "vm stop $clone_id failed"
clone_id=
say "OK: $golden gives a clone $avail_gb GB available on /"
