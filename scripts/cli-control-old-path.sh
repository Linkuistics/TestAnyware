#!/usr/bin/env bash
# The old-path control of leaf host-cli-control-k222 (spec
# docs/specs/guest-command-lifecycle.md, section "Test seams and required controls").
#
# It builds the Host CLI of an old revision from `git archive`, in a temporary directory,
# and runs five cases of tests/host-cli-control.rs against that binary:
#
#   - the CLI SIGTERM case of macos-disconnect-availability-k13, once for SIGINT and once
#     for SIGTERM. No cancellation is sent.
#   - the interrupt case of download-consistency-k7, once for SIGINT and once for SIGTERM.
#     The temporary file stays.
#   - the recorded old Linux reply of linux-download-consistency-k14. Bytes past the length
#     give success.
#
# Each signal has a test of its own. A test that loops over both signals stops at the
# first failure. Then the old binary never runs the second signal (review finding R5 of
# host-cli-control-k239).
#
# Each case must fail on the old binary with its known defect, and pass on the binary of
# this checkout. The script exits 0 only when both hold for each case.
#
# Usage: scripts/cli-control-old-path.sh REV
set -euo pipefail

rev=${1:?name the old revision, for example 251bf072e3}
root=$(cd "$(dirname "$0")/.." && pwd)
cli_dir=$root/cli-rs
# A target directory of its own, so the old build never replaces target/debug/testanyware.
# It persists, so a later run reuses the compiled dependencies.
old_target=$cli_dir/target/control-old-path

work=$(mktemp -d "${TMPDIR:-/tmp}/cli-control-old-path.XXXXXX")
trap 'rm -rf "$work"' EXIT

echo "old revision: $(git -C "$root" rev-parse "$rev")"
# The CLI embeds the docs, LLM_INSTRUCTIONS.md and the provisioner helpers, so the archive
# holds them too.
git -C "$root" archive "$rev" cli-rs docs provisioner LLM_INSTRUCTIONS.md | tar -x -C "$work"
cargo build --quiet --manifest-path "$work/cli-rs/Cargo.toml" --target-dir "$old_target" \
  -p testanyware-cli
old_cli=$old_target/debug/testanyware
echo "old Host CLI: $old_cli ($(shasum -a 256 "$old_cli" | cut -d' ' -f1))"

# Each case with the text that its failure on the old binary must show.
cases=(
  "an_exec_interrupted_by_sigint_sends_one_cancellation_and_dies_of_sigint|SIGINT: one cancellation for the exec id"
  "an_exec_interrupted_by_sigterm_sends_one_cancellation_and_dies_of_sigterm|SIGTERM: one cancellation for the exec id"
  "a_download_interrupted_by_sigint_removes_its_temp_file_and_dies_of_sigint|SIGINT: the temp file must be removed"
  "a_download_interrupted_by_sigterm_removes_its_temp_file_and_dies_of_sigterm|SIGTERM: the temp file must be removed"
  "a_download_with_bytes_past_its_length_fails_and_leaves_no_file|left: 0"
)

cd "$cli_dir"
cargo test --quiet -p testanyware-cli --test host-cli-control --no-run
status=0
for entry in "${cases[@]}"; do
  name=${entry%%|*}
  defect=${entry#*|}
  log=$work/$name.old.log
  if TESTANYWARE_CONTROL_CLI=$old_cli cargo test --quiet -p testanyware-cli \
      --test host-cli-control -- --exact "$name" >"$log" 2>&1; then
    echo "FAIL $name: passed on the old Host CLI"
    status=1
  elif grep -qF -- "$defect" "$log"; then
    echo "ok   $name: fails on the old Host CLI ($defect)"
  else
    echo "FAIL $name: failed on the old Host CLI without the known defect:"
    sed -n '/panicked/,+6p' "$log"
    status=1
  fi
  if cargo test --quiet -p testanyware-cli --test host-cli-control -- --exact "$name" \
      >"$work/$name.new.log" 2>&1; then
    echo "ok   $name: passes on this checkout"
  else
    echo "FAIL $name: fails on this checkout:"
    sed -n '/panicked/,+6p' "$work/$name.new.log"
    status=1
  fi
done
exit "$status"
