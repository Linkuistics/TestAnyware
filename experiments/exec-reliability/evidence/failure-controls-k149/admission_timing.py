"""Separate cutoff admission from syscall execution in worker_capture._reap.

The historical timing failures timestamped waitpid inside a test wrapper. The
runtime admits each owed probe from its own earlier clock read. A thread that
loses the processor between that read and the syscall executes the probe after
the cutoff, although the runtime admitted it before the cutoff.

This tool owns one real paused child. It drives the real _reap of one input set
and injects a scheduling delay after the admission read, before the syscall.
It records the admission time from the _reap frame and the execution time from
the waitpid wrapper. The child is then killed and reaped by its exact PID.
"""

import argparse
import hashlib
import json
import os
import signal
import sys
import time
from pathlib import Path
from types import FrameType
from typing import Any

LATE = 0.0005  # The injected delay crosses the cutoff by at least 500 us.


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(worker_capture: Any, case: str) -> dict[str, Any]:
    pid = os.fork()
    if pid == 0:
        try:
            while True:
                signal.pause()
        finally:
            os._exit(70)
    real_monotonic, real_waitpid = time.monotonic, os.waitpid
    probes: list[dict[str, Any]] = []
    delay: dict[str, Any] = {"armed": False, "applied": None}

    def monotonic() -> float:
        now = real_monotonic()
        caller: FrameType = sys._getframe(1)  # noqa: SLF001  # pyright: ignore[reportPrivateUsage]
        if delay["armed"] and caller.f_code.co_name == "_reap":
            delay["armed"] = False
            # Model preemption after the clock read: return the old reading late.
            while real_monotonic() < owner.shutdown_deadline_monotonic + LATE:
                pass
            delay["applied"] = {"read": now, "returned": real_monotonic()}
        return now

    def waitpid(target: int, flags: int) -> tuple[int, int]:
        executed = real_monotonic()
        caller = sys._getframe(1)  # noqa: SLF001  # pyright: ignore[reportPrivateUsage]
        probes.append(
            {
                "caller": caller.f_code.co_name,
                "admitted": caller.f_locals.get("now"),
                "executed": executed,
                "attempt": getattr(owner, "reap_attempts", None),
            }
        )
        return real_waitpid(target, flags)

    owner = worker_capture._Owner(pid, real_monotonic() + 0.05)  # noqa: SLF001
    time.monotonic, os.waitpid = monotonic, waitpid
    try:
        worker_capture._reap(owner)  # noqa: SLF001  (first owed probe, before cutoff)
        if case == "late-execution":
            while real_monotonic() < owner.shutdown_deadline_monotonic - 0.002:
                time.sleep(0.0005)
            delay["armed"] = True
            worker_capture._reap(owner)  # noqa: SLF001
        while real_monotonic() < owner.shutdown_deadline_monotonic + 0.001:
            time.sleep(0.0005)
        worker_capture._reap(owner)  # noqa: SLF001  (after cutoff, not owed)
    finally:
        time.monotonic, os.waitpid = real_monotonic, real_waitpid
        os.kill(pid, signal.SIGKILL)
        reaped, status = os.waitpid(pid, 0)
    try:
        os.waitpid(pid, os.WNOHANG)
        released: object = "unexpected_owner"
    except ChildProcessError as error:
        released = error.errno
    end = owner.shutdown_deadline_monotonic
    execution_late = [item for item in probes if float(item["executed"]) >= end]
    admission_late = [
        item for item in probes if item["admitted"] is None or float(item["admitted"]) >= end
    ]
    return {
        "case": case,
        "deadline_monotonic": end,
        "delay": delay["applied"],
        "probes": probes,
        "execution_late": len(execution_late),
        "admission_late": len(admission_late),
        "historical_assertion_passes": not execution_late,
        "admission_assertion_passes": not admission_late,
        "cleanup": {
            "reaped_pid": reaped,
            "killed_by_sigkill": os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGKILL,
            "released_errno": released,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--repeat", type=int, default=5)
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    out.mkdir(parents=True)
    sys.path.insert(0, str(inputs))
    import worker_capture  # noqa: PLC0415  (imported from the frozen inputs)

    subject = inputs / "worker_capture.py"
    before = sha256(subject)
    cases = [
        run(worker_capture, case)
        for _ in range(args.repeat)
        for case in ("late-execution", "no-injection")
    ]
    after = sha256(subject)
    result = {
        "control": "reap-admission-versus-execution",
        "inputs": str(inputs),
        "module": worker_capture.__file__,
        "python": sys.version,
        "subject_before": before,
        "subject_after": after,
        "subject_unchanged": before == after,
        "cases": cases,
    }
    (out / "admission-timing.json").write_text(json.dumps(result, indent=2) + "\n")
    for item in cases:
        print(
            item["case"],
            "probes",
            len(item["probes"]),
            "execution_late",
            item["execution_late"],
            "admission_late",
            item["admission_late"],
            "cleanup",
            item["cleanup"],
        )


if __name__ == "__main__":
    main()
