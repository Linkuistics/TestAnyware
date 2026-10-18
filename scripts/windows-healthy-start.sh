#!/usr/bin/env bash
#
# Does one `vm start --platform windows` reach a Healthy agent, unaided, with
# the TPM attached? Reads the agent itself: a start that never sees its agent
# warns and exits 0, so the start's exit status proves nothing.
#
#   windows-healthy-start.sh [BASE]
#
# BASE is a golden name (default: the platform's golden). Always stops and
# deletes the clone. A cold start takes about 2.5 minutes to the lock screen;
# if the agent never answers, the start's own wait is 10 to 14 minutes.
#
# Env: TESTANYWARE (the CLI; default cli-rs/target/debug/testanyware, built by
# `task cli:build`).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "windows-healthy-start: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cli=${TESTANYWARE:-$root/cli-rs/target/debug/testanyware}
base=${1:-}

args=(vm start --platform windows --json)
[[ -n $base ]] && args+=(--base "$base")

work=$(mktemp -d "${TMPDIR:-/tmp}/windows-healthy-start.XXXXXX")
id=
cleanup() {
  if [[ -n $id ]]; then
    "$cli" vm stop "$id" >/dev/null 2>&1 || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

fail() {
  echo "windows-healthy-start: FAIL: $1" >&2
  exit 1
}

echo "starting a Windows clone${base:+ of $base}..." >&2
"$cli" "${args[@]}" >"$work/start.json" || fail "vm start exited non-zero"
id=$(jq -r '.id // empty' "$work/start.json")
[[ -n $id ]] || fail "vm start printed no VM id"

# The agent endpoint is null when the boot wait timed out.
host=$(jq -r '.agent.host // empty' "$work/start.json")
port=$(jq -r '.agent.port // empty' "$work/start.json")
[[ -n $host && -n $port ]] || fail "the start reported no agent endpoint (the agent never answered)"

# Healthy means /health answers 2xx with a body; accessible may be either value.
curl -fsS --max-time 10 "http://$host:$port/health" >"$work/health.json" ||
  fail "GET http://$host:$port/health did not answer"

# The report says whether the TPM was attached; a Windows guest on this host must have one.
[[ $(jq -r '.tpm.attached' "$work/start.json") == true ]] ||
  fail "the start did not attach the TPM: $(jq -c '.tpm' "$work/start.json")"

echo "OK: $id agent healthy at $host:$port, TPM attached" >&2
jq -c '{id, agent, tpm}' "$work/start.json"
jq -c . "$work/health.json"
