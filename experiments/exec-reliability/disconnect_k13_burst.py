"""One k13 live mode against one clone, on one monotonic clock.

    disconnect_k13_burst.py CLI VM MODE DOWNLOADS

MODE is `control`, `concurrent` or `term`. The script starts the original exec:
the issue #14 fork/wait payload with a 20 s guest timeout.

- `control`: the original runs to its end. The follow-up sequence runs after.
- `concurrent`: the follow-up sequence starts 2 s after the original starts.
  The original is not signalled.
- `term`: 2 s after the start, the script sends TERM to the CLI PID only.
  After 2 s more, it sends KILL if the CLI still runs. The follow-up
  sequence starts at once after the CLI ends or the KILL wait ends.

The follow-up sequence is: inspect, health, ordinary exec, download of the
immutable partial file, inspect again. Each request runs once, with no retry,
inside its own bound. A request that exceeds its bound gets TERM, then KILL, on
its process group, and stays a failed request. Two more inspections run on
their own timer threads, at `MID_AT` and at `LATE_AT` seconds after the start.
A slow follow-up therefore cannot move them. A waiter thread stamps the exit of
the original CLI when it happens.

The script prints one JSON document. It judges nothing.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any

GUEST_TIMEOUT = 20
ACT_AFTER = 2.0
TERM_GRACE = 2.0
KILL_WAIT = 2.0
ORIGINAL_BOUND = 40.0  # the guest timeout plus the 20 s host margin
FOLLOW_BOUND = 30.0  # the ordinary observation bound
TRANSFER_BOUND = 150.0  # the transfer ceiling
INSPECT_TIMEOUT = 10  # the guest timeout of one inspection
MID_AT = 18.0  # 2 s before the guest deadline
LATE_AT = 30.0  # after the deadline, the runner terminate wait and scheduling
STOP_WAIT = 2.0
MODES = ("control", "concurrent", "term")
ORDINARY = "printf ordinary-ok"
PARTIAL = b"partial\n"


def guest_dir(mode: str) -> str:
    return f"/tmp/taw-k13/{mode}"


def payload(mode: str) -> str:
    """The issue #14 fork/wait payload. Only its directory differs from k4."""
    d = guest_dir(mode)
    return "\n".join(
        (
            f"mkdir -p {d}",
            "sleep 120 & child=$!",
            f"printf '%s\\n' \"$child\" > {d}/child.pid",
            f"printf 'partial\\n' > {d}/partial",
            "wait",
        )
    )


def inspection(mode: str) -> str:
    """Print one line per role: `ROLE pid=PID STATE ppid=PPID command=COMMAND`.

    STATE is `present`, `absent` (no such process) or `unknown` (any other
    error). A role with no PID file prints `ROLE pid=none`. The payload writes
    no shell PID. The first inspection that sees the child alive under a parent
    other than PID 1 records that parent as the shell. The record is named by
    the child PID, so a record from an earlier payload never names this shell.
    """
    d = guest_dir(mode)
    return "\n".join(
        (
            f"cpid=$(cat {d}/child.pid 2>/dev/null)",
            f'if [ -n "$cpid" ] && [ ! -s {d}/shell.of."$cpid" ]; then',
            "  ppid=$(ps -o ppid= -p \"$cpid\" | tr -d ' ')",
            "  case \"$ppid\" in ''|1) ;; *)",
            f'    printf \'%s\\n\' "$ppid" > {d}/shell.of."$cpid" ;;',
            "  esac",
            "fi",
            "for role in shell child; do",
            '  if [ "$role" = shell ]; then',
            f'    pid=$(cat {d}/shell.of."$cpid" 2>/dev/null)',
            "  else",
            "    pid=$cpid",
            "  fi",
            '  if [ -z "$pid" ]; then printf \'%s pid=none\\n\' "$role"; continue; fi',
            '  if err=$(kill -0 "$pid" 2>&1); then state=present',
            '  else case "$err" in',
            "    *[Nn]o\\ such\\ process*) state=absent ;;",
            "    *) state=unknown ;;",
            "  esac",
            "  fi",
            "  ppid=$(ps -p \"$pid\" -o ppid= | tr -d ' ')",
            "  cmd=$(ps -p \"$pid\" -o command= | sed 's/ *$//')",
            "  fmt='%s pid=%s %s ppid=%s command=%s\\n'",
            '  printf "$fmt" "$role" "$pid" "$state" "$ppid" "$cmd"',
            "done",
        )
    )


def now_ns() -> int:
    return time.monotonic_ns()


def exec_argv(cli: str, vm: str, guest_timeout: int, command: str) -> list[str]:
    return [cli, "file", "exec", "--vm", vm, "--timeout", str(guest_timeout), "--json", command]


def stop_group(child: subprocess.Popen[bytes]) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    for sig, wait in ((signal.SIGTERM, STOP_WAIT), (signal.SIGKILL, STOP_WAIT)):
        try:
            os.killpg(child.pid, sig)
            result = "sent"
        except ProcessLookupError:
            result = "gone"
        signals.append({"signal": sig.name, "ns": now_ns(), "result": result})
        try:
            child.wait(timeout=wait)
            break
        except subprocess.TimeoutExpired:
            continue
    return signals


def request(name: str, argv: list[str], bound: float) -> dict[str, Any]:
    record: dict[str, Any] = {"name": name, "argv": argv, "start_ns": now_ns()}
    child = subprocess.Popen(  # noqa: S603 - the CLI under test
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = child.communicate(timeout=bound)
        record["bound_exceeded"] = False
    except subprocess.TimeoutExpired:
        record["bound_exceeded"] = True
        record["signals"] = stop_group(child)
        stdout, stderr = child.communicate()
    record.update(
        end_ns=now_ns(),
        returncode=child.returncode,
        stdout_hex=stdout.hex(),
        stderr_hex=stderr.hex(),
    )
    return record


def follow_sequence(cli: str, vm: str, mode: str, downloads: Path) -> list[dict[str, Any]]:
    local = downloads / f"{mode}-partial.bin"
    steps = [
        request("inspect", exec_argv(cli, vm, INSPECT_TIMEOUT, inspection(mode)), FOLLOW_BOUND),
        request("health", [cli, "agent", "health", "--vm", vm, "--json"], FOLLOW_BOUND),
        request("ordinary", exec_argv(cli, vm, INSPECT_TIMEOUT, ORDINARY), FOLLOW_BOUND),
        request(
            "download",
            [
                cli,
                "file",
                "download",
                "--vm",
                vm,
                f"{guest_dir(mode)}/partial",
                str(local),
                "--json",
            ],
            TRANSFER_BOUND,
        ),
        request(
            "inspect-after", exec_argv(cli, vm, INSPECT_TIMEOUT, inspection(mode)), FOLLOW_BOUND
        ),
    ]
    download = steps[3]
    download["local_hex"] = local.read_bytes().hex() if local.is_file() else None
    return steps


def sleep_until(deadline_ns: int) -> None:
    remaining = (deadline_ns - now_ns()) / 1e9
    if remaining > 0:
        time.sleep(remaining)


class Waiter:
    """Stamp the exit of one process on the monotonic clock as it happens."""

    def __init__(self, child: subprocess.Popen[bytes]) -> None:
        self.child = child
        self.exit_ns: int | None = None
        self.done = threading.Event()
        threading.Thread(target=self._wait, daemon=True).start()

    def _wait(self) -> None:
        self.child.wait()
        self.exit_ns = now_ns()
        self.done.set()

    def wait_until(self, until_ns: int) -> bool:
        return self.done.wait(timeout=max(0.0, (until_ns - now_ns()) / 1e9))


def signal_cli(original: subprocess.Popen[bytes], waiter: Waiter) -> list[dict[str, Any]]:
    """Send TERM to the CLI PID only. Send KILL after the grace if it still runs."""
    signals: list[dict[str, Any]] = []
    for sig, wait in ((signal.SIGTERM, TERM_GRACE), (signal.SIGKILL, KILL_WAIT)):
        if waiter.done.is_set():
            break
        try:
            os.kill(original.pid, sig)
            outcome = "sent"
        except ProcessLookupError:
            outcome = "gone"
        signals.append({"signal": sig.name, "ns": now_ns(), "result": outcome})
        waiter.wait_until(now_ns() + int(wait * 1e9))
    return signals


def inspect(cli: str, vm: str, mode: str, name: str) -> dict[str, Any]:
    return request(name, exec_argv(cli, vm, INSPECT_TIMEOUT, inspection(mode)), FOLLOW_BOUND)


class Timed:
    """One inspection that starts at a fixed time on its own thread."""

    def __init__(self, cli: str, vm: str, mode: str, name: str, at_ns: int) -> None:
        self.record: dict[str, Any] | None = None
        self._args = (cli, vm, mode, name)
        self._at_ns = at_ns
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def _run(self) -> None:
        sleep_until(self._at_ns)
        self.record = inspect(*self._args)

    def join(self) -> dict[str, Any]:
        # `request` bounds the inspection and stops its process group, so the join ends.
        self._thread.join()
        assert self.record is not None
        return self.record


def run(cli: str, vm: str, mode: str, downloads: Path) -> dict[str, Any]:
    out_path = downloads / f"{mode}-original.stdout"
    err_path = downloads / f"{mode}-original.stderr"
    argv = exec_argv(cli, vm, GUEST_TIMEOUT, payload(mode))
    result: dict[str, Any] = {"mode": mode, "argv": argv, "bounds": bounds()}
    with out_path.open("wb") as out, err_path.open("wb") as err:
        start = now_ns()
        original = subprocess.Popen(  # noqa: S603 - the CLI under test
            argv, stdin=subprocess.DEVNULL, stdout=out, stderr=err, start_new_session=True
        )
    waiter = Waiter(original)
    result["start_ns"] = start
    record: dict[str, Any] = {"pid": original.pid, "signals": []}
    limit_ns = start + int(ORIGINAL_BOUND * 1e9)
    timed: list[Timed] = []
    if mode == "control":
        waiter.wait_until(limit_ns)
    else:
        mid_ns = start + int(MID_AT * 1e9)
        timed.append(Timed(cli, vm, mode, "inspect-mid", mid_ns))
        timed.append(Timed(cli, vm, mode, "inspect-late", start + int(LATE_AT * 1e9)))
        sleep_until(start + int(ACT_AFTER * 1e9))
        if mode == "term":
            record["signals"] = signal_cli(original, waiter)
    result["act_ns"] = now_ns()
    result["follow"] = follow_sequence(cli, vm, mode, downloads)
    if mode == "control":
        # The control inspections run after the follow-up sequence, so they never overlap it.
        result["mid"] = inspect(cli, vm, mode, "inspect-mid")
        result["late"] = inspect(cli, vm, mode, "inspect-late")
    else:
        result["mid"], result["late"] = (item.join() for item in timed)
    if not waiter.wait_until(limit_ns):
        record["stop"] = stop_group(original)
        waiter.wait_until(now_ns() + int(STOP_WAIT * 1e9))
    record.update(
        bound_exceeded="stop" in record,
        exit_ns=waiter.exit_ns,
        returncode=original.returncode,
        stdout_hex=out_path.read_bytes().hex(),
        stderr_hex=err_path.read_bytes().hex(),
    )
    result["original"] = record
    result["end_ns"] = now_ns()
    return result


def bounds() -> dict[str, float]:
    return {
        "guest_timeout": GUEST_TIMEOUT,
        "act_after": ACT_AFTER,
        "term_grace": TERM_GRACE,
        "kill_wait": KILL_WAIT,
        "original_bound": ORIGINAL_BOUND,
        "follow_bound": FOLLOW_BOUND,
        "transfer_bound": TRANSFER_BOUND,
        "inspect_timeout": INSPECT_TIMEOUT,
        "mid_at": MID_AT,
        "late_at": LATE_AT,
    }


def main() -> None:
    if len(sys.argv) != 5 or sys.argv[3] not in MODES:
        raise SystemExit("usage: disconnect_k13_burst.py CLI VM control|concurrent|term DOWNLOADS")
    cli, vm, mode, downloads = sys.argv[1], sys.argv[2], sys.argv[3], Path(sys.argv[4])
    print(json.dumps(run(cli, vm, mode, downloads), sort_keys=True), flush=True)


if __name__ == "__main__":
    main()
