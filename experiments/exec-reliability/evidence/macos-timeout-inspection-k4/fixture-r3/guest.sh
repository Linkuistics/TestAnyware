#!/bin/bash
# Disposable measurement fixture; runs only inside an owned VM.
set -euo pipefail
IFS=$'\n\t'
root=/tmp/vmfit-k15
case "$1" in
  inventory)
    printf 'AGENT_AND_BASELINE_DETAILS\n'
    ps -axo pid=,comm= | while IFS=' ' read -r process_id executable; do
      case "$executable" in
        *testanyware*|*TestAnyware*)
          printf 'agent process %s executable %s\n' "$process_id" "$executable"
          if [[ -e "/proc/$process_id/exe" ]]; then executable=$(readlink "/proc/$process_id/exe"); fi
          if [[ -f "$executable" ]]; then
            ls -l "$executable"
            openssl dgst -sha256 "$executable"
          fi
          ;;
      esac
    done
    mount
    openssl version
    bash --version | head -1
    uname -a
    id
    if [[ "$(uname -s)" == Darwin ]]; then
      sw_vers
      system_profiler SPHardwareDataType | head -20
      command -v brew || true
      if command -v brew >/dev/null; then brew list --versions; fi
    else
      cat /etc/os-release
      dpkg-query -W -f='${Package} ${Version}\n'
    fi
    for tool in bash curl sudo openssl yq vmfit-lesson; do command -v "$tool" || true; done
    if [[ -e /usr/local/bin/vmfit-lesson || -e /tmp/vmfit-k15/marker || -e /usr/local/bin/yq ]]; then
      printf 'BASELINE_DIRTY\n'
      exit 19
    fi
    printf 'BASELINE_ABSENT\n'
    ;;
  install)
    sudo -n install -d /usr/local/bin
    sudo -n install -m 755 "$root/lesson" /usr/local/bin/vmfit-lesson
    touch "$root/marker"
    ls -l /usr/local/bin/vmfit-lesson
    openssl dgst -sha256 /usr/local/bin/vmfit-lesson
    ;;
  intact)
    set +e
    /usr/local/bin/vmfit-lesson > "$root/stdout.bin" 2> "$root/stderr.bin"
    status=$?
    set -e
    printf '%s\n' "$status" > "$root/status.txt"
    openssl dgst -sha256 "$root/stdout.bin" "$root/stderr.bin" "$root/artifact.bin"
    exit "$status"
    ;;
  online)
    curl --version
    curl --fail --location --connect-timeout 15 --max-time 90 "$2" -o "$root/yq"
    observed=$(openssl dgst -sha256 "$root/yq")
    [[ "${observed##* }" == "$3" ]]
    sudo -n install -m 755 "$root/yq" /usr/local/bin/yq
    /usr/local/bin/yq --version
    /usr/local/bin/yq -n '.answer = 42'
    ;;
  nonzero) printf 'intentional failure\n' >&2; exit 23 ;;
  bad-install) sudo -n install -m 755 "$root/intentionally-absent" /usr/local/bin/vmfit-bad ;;
  timeout|cancel)
    label=$1
    sleep 120 &
    printf '%s\n' "$!" > "$root/$label.pid"
    printf 'partial %s\n' "$label" > "$root/$label.partial"
    wait
    ;;
  observe)
    for label in timeout cancel; do
      if [[ -f "$root/$label.pid" ]]; then
        pid=$(cat "$root/$label.pid")
        printf '%s pid=%s\n' "$label" "$pid"
        ps -p "$pid" -o pid=,ppid=,stat=,comm= || true
      fi
    done
    ;;
  cleanup)
    for label in timeout cancel; do
      if [[ -f "$root/$label.pid" ]]; then
        pid=$(cat "$root/$label.pid")
        process=$(ps -p "$pid" -o comm= || true)
        if [[ "$process" == *sleep* ]]; then kill "$pid" || true; fi
      fi
    done
    ;;
  *) exit 64 ;;
esac
