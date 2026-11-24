"""Explicit cancellation for the acceptance trial (leaf acceptance-plan-k223).

    acceptance_k223_cancel.py --cli CLI --vm VM --exec-id ID --guest-timeout S
        --host-margin S --health-after S --cancel-after S --cancel-wait S
        --observation S --command COMMAND

The controller runs one call at a time, so this helper is one controller call.
It uses only the standard library, because the plan runs it with `python -I`.
It does these steps, with times from the start of `file exec`:

1. Start `file exec --exec-id ID --timeout S --json COMMAND`.
2. At `--health-after`, run `agent health --json` while the command runs.
3. At `--cancel-after`, run `file cancel --exec-id ID --wait W --json`.
4. Wait for `file exec` until guest budget plus `--host-margin`.

Each health and cancel call has the observation bound. A child that passes its
bound gets SIGTERM, 2 s of grace, then SIGKILL and 2 s more. The children stay
in the helper's process group, so the controller's group signals reach them.

The helper prints one JSON object with each child's argv, exit status, raw
stdout and stderr in base64, and its monotonic times. It exits 0 when each
child ran and was reaped, and 1 otherwise. acceptance_k223_check.py judges
the content.
"""

import argparse
import base64
import json
import signal
import subprocess
import sys
import time
from typing import Any

GRACE = 2.0
KILL_WAIT = 2.0


def _encode(data: bytes | None) -> str | None:
    return None if data is None else base64.b64encode(data).decode("ascii")


def _finish(process: subprocess.Popen[bytes], deadline: float, origin: float) -> dict[str, Any]:
    """Wait for one child until `deadline`, then end it. Return its receipt."""
    receipt: dict[str, Any] = {"signals": []}
    out: bytes | None = None
    err: bytes | None = None
    try:
        out, err = process.communicate(timeout=max(0.0, deadline - time.monotonic()))
    except subprocess.TimeoutExpired:
        # Retrying communicate after TimeoutExpired loses no output.
        # https://docs.python.org/3.12/library/subprocess.html#subprocess.Popen.communicate
        receipt["deadline_passed"] = True
        for sig, wait in ((signal.SIGTERM, GRACE), (signal.SIGKILL, KILL_WAIT)):
            process.send_signal(sig)
            receipt["signals"].append({"signal": sig.name, "after": time.monotonic() - origin})
            try:
                out, err = process.communicate(timeout=wait)
                break
            except subprocess.TimeoutExpired:
                continue
    receipt.update(
        returncode=process.returncode,
        reaped=process.returncode is not None,
        ended_after=time.monotonic() - origin,
        stdout=_encode(out),
        stderr=_encode(err),
    )
    return receipt


def _run(argv: list[str], bound: float, origin: float) -> dict[str, Any]:
    """Run one short child to its end, within `bound` seconds."""
    started = time.monotonic()
    process = subprocess.Popen(
        argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    receipt = _finish(process, started + bound, origin)
    receipt.update(argv=argv, started_after=started - origin, elapsed=time.monotonic() - started)
    return receipt


def _sleep_until(moment: float) -> None:
    remaining = moment - time.monotonic()
    if remaining > 0:
        time.sleep(remaining)


def main() -> None:
    parser = argparse.ArgumentParser()
    for flag in ("--cli", "--vm", "--exec-id", "--command"):
        parser.add_argument(flag, required=True)
    for flag in ("--guest-timeout", "--cancel-wait"):
        parser.add_argument(flag, required=True, type=int)
    for flag in ("--host-margin", "--health-after", "--cancel-after", "--observation"):
        parser.add_argument(flag, required=True, type=float)
    args = parser.parse_args()
    exec_argv = [
        args.cli,
        "file",
        "exec",
        "--vm",
        args.vm,
        "--timeout",
        str(args.guest_timeout),
        "--exec-id",
        args.exec_id,
        "--json",
        args.command,
    ]
    health_argv = [args.cli, "agent", "health", "--vm", args.vm, "--json"]
    cancel_argv = [
        args.cli,
        "file",
        "cancel",
        "--vm",
        args.vm,
        "--exec-id",
        args.exec_id,
        "--wait",
        str(args.cancel_wait),
        "--json",
    ]
    record: dict[str, Any] = {"schema": "acceptance-k223-cancel/1", "exec_id": args.exec_id}
    origin = time.monotonic()
    process = subprocess.Popen(
        exec_argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    exec_deadline = origin + args.guest_timeout + args.host_margin
    try:
        _sleep_until(origin + args.health_after)
        record["health"] = _run(health_argv, args.observation, origin)
        _sleep_until(origin + args.cancel_after)
        record["exec_running_at_cancel"] = process.poll() is None
        record["cancel"] = _run(cancel_argv, args.observation, origin)
    finally:
        record["exec"] = _finish(process, exec_deadline, origin)
        record["exec"].update(argv=exec_argv, started_after=0.0)
    children = [record[key] for key in ("health", "cancel", "exec") if key in record]
    ok = len(children) == 3 and all(child["reaped"] is True for child in children)
    record["ok"] = ok
    sys.stdout.write(json.dumps(record, indent=2) + "\n")
    sys.stdout.flush()
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
