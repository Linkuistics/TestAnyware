#!/usr/bin/env bash
#
# Can this host's QEMU realize a Windows guest's TPM device the way TestAnyware
# attaches it (ADR-0017)? Seconds: a paused, diskless `virt` guest under HVF
# with an swtpm-backed `tpm-tis-device`, no golden and no clone.
#
#   qemu-tpm-check.sh
#
# Runs the device as TestAnyware attaches it (PPI off when the binary offers
# the `ppi` property) and, for information, with QEMU's default PPI. Exit 0
# when TestAnyware's configuration stays up, 1 when it does not. Once the
# default line stays up too, QEMU's HVF listener is fixed and ADR-0017's
# `ppi=off` can be revisited.
#
# Env: QEMU (default qemu-system-aarch64), SWTPM (default swtpm).
# A macOS host only: it needs HVF.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "qemu-tpm-check: error on line $LINENO" >&2' ERR

qemu=${QEMU:-qemu-system-aarch64}
swtpm=${SWTPM:-swtpm}
work=$(mktemp -d "${TMPDIR:-/tmp}/qemu-tpm-check.XXXXXX")
trap 'rm -rf "$work"' EXIT

# Run a paused guest with the given -device for 3 s behind its own swtpm.
# Succeeds if QEMU is still up; otherwise prints QEMU's last line and fails.
run_stays_up() {
  local device=$1 dir qemu_pid swtpm_pid status=0
  dir=$(mktemp -d "$work/run.XXXXXX")
  mkdir "$dir/state"
  "$swtpm" socket --tpmstate dir="$dir/state" --ctrl type=unixio,path="$dir/sock" \
    --tpm2 --flags not-need-init >"$dir/swtpm.log" 2>&1 &
  swtpm_pid=$!
  for _ in {1..50}; do
    [[ -S $dir/sock ]] && break
    sleep 0.1
  done
  "$qemu" -machine virt,highmem=on,gic-version=3 -accel hvf -cpu host -m 512 -S \
    -display none -monitor none -serial none \
    -chardev socket,id=chrtpm,path="$dir/sock" \
    -tpmdev emulator,id=tpm0,chardev=chrtpm \
    -device "$device" >"$dir/qemu.log" 2>&1 &
  qemu_pid=$!
  sleep 3
  if kill -0 "$qemu_pid" 2>/dev/null; then
    kill "$qemu_pid"
  else
    status=1
  fi
  wait "$qemu_pid" 2>/dev/null || true
  kill "$swtpm_pid" 2>/dev/null || true
  wait "$swtpm_pid" 2>/dev/null || true
  ((status == 0)) || tail -n 1 "$dir/qemu.log"
  return "$status"
}

"$qemu" --version | sed -n 1p
# QEMU exits 0 even for an unknown device, so read the text, not the status.
help=$("$qemu" -machine none -device tpm-tis-device,help 2>&1 || true)
device=tpm-tis-device,tpmdev=tpm0
if ! grep -q '^tpm-tis-device options:' <<<"$help"; then
  echo "FAIL  this QEMU has no tpm-tis-device: TestAnyware omits the TPM (ADR-0017)"
  exit 1
fi
if grep -q '^ *ppi=' <<<"$help"; then
  ours=$device,ppi=off
else
  ours=$device
fi

result=0
if why=$(run_stays_up "$ours"); then
  echo "PASS  $ours stays up"
else
  echo "FAIL  $ours: $why"
  result=1
fi
if [[ $ours != "$device" ]]; then
  if why=$(run_stays_up "$device"); then
    echo "INFO  $device (PPI on) stays up too: ADR-0017's ppi=off can be revisited"
  else
    echo "INFO  $device (PPI on) aborts, as ADR-0017 expects: $why"
  fi
fi
exit "$result"
