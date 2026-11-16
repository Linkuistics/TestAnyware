"""Retain actual setuid-ps KILL controls through the real helper engine.

Each trial runs the installed setuid /bin/ps through inspection.capture_helper.
The ps argv prints more than the 256 KiB cap, so the engine itself sends KILL
to its exact unreaped child. An observation-only wrapper around os.kill records
liveness, identity and the kill result for that child, then calls the real
os.kill. It never changes the signal, the target or the engine's control flow.
"""

import argparse
import hashlib
import json
import os
import select
import signal
import stat
import subprocess
import sys
import time
from collections.abc import Callable
from contextlib import closing
from dataclasses import asdict
from pathlib import Path
from typing import cast

HERE = Path(__file__).resolve().parent
RUNTIME = HERE.parent.parent
sys.path.insert(0, str(RUNTIME))

import inspection  # noqa: E402

PS = "/bin/ps"
# Measured at 345-415 KB on this host, well above the 256 KiB capture cap.
ARGV = (PS, "-axww", "-o", "pid=,ppid=,pgid=,uid=,ruid=,stat=,lstart=,command=")
CAP = 256 * 1024
SUBJECTS = (RUNTIME / "inspection.py", Path(__file__).resolve())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def exit_state(pid: int) -> dict[str, object]:
    """Observe exit state without reaping, so the PID stays pinned.

    Python 3.12 on macOS has no os.waitid, so this uses the same kqueue
    NOTE_EXIT check as controller._exited. A zombie also reports NOTE_EXIT.
    """
    started = time.monotonic()
    # https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kqueue.2.html
    with closing(select.kqueue()) as queue:
        event = select.kevent(
            pid, filter=select.KQ_FILTER_PROC, flags=select.KQ_EV_ADD, fflags=select.KQ_NOTE_EXIT
        )
        try:
            events = queue.control([event], 1, 0)
        except OSError as error:
            return {"monotonic": started, "state": "error", "errno": error.errno}
    for result in events:
        if result.flags & select.KQ_EV_ERROR:
            return {"monotonic": started, "state": "error", "errno": result.data}
        if result.fflags & select.KQ_NOTE_EXIT:
            return {"monotonic": started, "state": "exit_observed", "status": result.data}
    return {"monotonic": started, "state": "live_unreaped"}


def identity(pid: int) -> dict[str, object]:
    """Read the kernel's credentials for one PID with a separate ps query."""
    started = time.monotonic()
    query = (PS, "-o", "pid=,ppid=,pgid=,uid=,ruid=,svuid=,stat=,comm=", "-p", str(pid))
    try:
        result = subprocess.run(query, capture_output=True, timeout=2, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"started_monotonic": started, "error": str(error)}
    fields = result.stdout.decode(errors="replace").split()
    record: dict[str, object] = {
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
        "argv": list(query),
        "returncode": result.returncode,
        "stdout": result.stdout.decode(errors="replace"),
    }
    if len(fields) >= 8:
        names = ("pid", "ppid", "pgid", "uid", "ruid", "svuid")
        record.update({name: int(value) for name, value in zip(names, fields[:6], strict=True)})
        record.update(stat=fields[6], comm=" ".join(fields[7:]))
    return record


def children() -> list[str]:
    """List remaining direct children, excluding this audit's own ps query."""
    query = subprocess.Popen(
        (PS, "-A", "-o", "pid=,ppid=,stat=,comm="),
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    stdout, _ = query.communicate(timeout=5)
    if query.returncode != 0:
        raise RuntimeError(f"child audit failed: {query.returncode}")
    mine = str(os.getpid())
    rows = [line.split() for line in stdout.decode(errors="replace").splitlines()]
    return [
        " ".join(row)
        for row in rows
        if len(row) >= 2 and row[1] == mine and row[0] != str(query.pid)
    ]


def trial(index: int, *, suppress_kill: bool) -> dict[str, object]:
    created: list[inspection.HelperIdentity] = []
    kills: list[dict[str, object]] = []
    real_kill: Callable[[int, int], None] = os.kill

    def observe_kill(pid: int, sig: int) -> None:
        if not created or pid != created[0]["pid"] or sig != signal.SIGKILL:
            real_kill(pid, sig)
            return
        record: dict[str, object] = {"target": pid, "signal": signal.Signals(sig).name}
        record["before"] = exit_state(pid)
        record["identity"] = identity(pid)
        # The window ends at this second observation, immediately before KILL.
        record["before_confirm"] = exit_state(pid)
        record["kill_monotonic"] = time.monotonic()
        try:
            if suppress_kill:
                # Red control only: the engine's KILL never reaches the child.
                record["kill_result"] = "suppressed"
                return
            real_kill(pid, sig)
            record["kill_result"] = "sent"
        except OSError as error:
            record.update(kill_result="error", kill_errno=error.errno)
            raise
        finally:
            record["kill_returned_monotonic"] = time.monotonic()
            record["after"] = exit_state(pid)
            kills.append(record)

    os.kill = observe_kill
    try:
        receipt = inspection.capture_helper(
            ARGV, time.monotonic() + 5, publish_created=created.append
        )
    finally:
        os.kill = real_kill
    pid = receipt.pid
    audit: dict[str, object] = {}
    if pid is not None:
        try:
            os.waitpid(pid, os.WNOHANG)
            audit["waitpid"] = "unexpected_owner"
        except ChildProcessError as error:
            audit["waitpid"] = {"errno": error.errno}
        audit["pid_query"] = identity(pid)
    audit["children"] = children()
    capture = asdict(receipt)
    capture["stdout_bytes"] = len(receipt.stdout)
    capture["stdout"] = receipt.stdout[:512].decode(errors="replace")
    capture["stderr"] = receipt.stderr.decode(errors="replace")
    checks = verdict(receipt, created, kills, audit)
    return {
        "trial": index,
        "created": created,
        "kill_observations": kills,
        "capture": capture,
        "post_audit": audit,
        "checks": checks,
        "passed": all(checks.values()),
    }


def verdict(
    receipt: inspection.HelperCapture,
    created: list[inspection.HelperIdentity],
    kills: list[dict[str, object]],
    audit: dict[str, object],
) -> dict[str, bool]:
    kill = kills[0] if len(kills) == 1 else {}
    seen = cast(dict[str, object], kill.get("identity", {}))
    before = cast(dict[str, object], kill.get("before", {}))
    confirm = cast(dict[str, object], kill.get("before_confirm", {}))
    pid = receipt.pid
    return {
        "child_published_by_engine": len(created) == 1 and created[0]["pid"] == pid,
        "cap_triggered_kill": receipt.error is not None
        and "ps output limit exceeded" in receipt.error
        and len(receipt.stdout) == CAP,
        "one_engine_kill_observed": len(kills) == 1 and kill.get("target") == pid,
        "live_unreaped_before_kill": before.get("state") == "live_unreaped"
        and confirm.get("state") == "live_unreaped",
        "identified_setuid_ps": seen.get("pid") == pid
        and seen.get("ppid") == os.getpid()
        and seen.get("uid") == 0
        and seen.get("ruid") == os.getuid()
        and str(seen.get("comm", "")).endswith("ps"),
        "kill_sent": kill.get("kill_result") == "sent" and receipt.kill_errno is None,
        "killed_by_sigkill": receipt.reaped and receipt.returncode == -signal.SIGKILL,
        "reaped_before_deadline": receipt.reaped_monotonic is not None
        and receipt.reaped_monotonic <= receipt.deadline_monotonic,
        "no_residual": receipt.residual_remedy is None and not receipt.ownership_lost,
        "owner_released": audit.get("waitpid") == {"errno": 10},
        "no_children_left": audit.get("children") == [],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--mutant", choices=("suppress-kill",))
    args = parser.parse_args()
    out: Path = args.out
    out.mkdir(parents=True)
    ps_stat = os.stat(PS)
    before = {str(path): sha256(path) for path in SUBJECTS}
    suppress = args.mutant == "suppress-kill"
    trials = [trial(index, suppress_kill=suppress) for index in range(args.trials)]
    after = {str(path): sha256(path) for path in SUBJECTS}
    result = {
        "control": "actual-identified-setuid-ps-kill",
        "python": sys.version,
        "mutant": args.mutant,
        "sender": {"real_uid": os.getuid(), "effective_uid": os.geteuid()},
        "ps_executable": {
            "path": PS,
            "owner_uid": ps_stat.st_uid,
            "owner_gid": ps_stat.st_gid,
            "mode_octal": oct(ps_stat.st_mode),
            "setuid": bool(ps_stat.st_mode & stat.S_ISUID),
            "sha256": sha256(Path(PS)),
        },
        "argv": list(ARGV),
        "cap_bytes": CAP,
        "subjects_before": before,
        "subjects_after": after,
        "subjects_unchanged": before == after,
        "trials": trials,
        "passed": before == after and all(item["passed"] for item in trials),
    }
    (out / "setuid-ps-kill.json").write_text(json.dumps(result, indent=2, default=str) + "\n")
    print(json.dumps({"passed": result["passed"], "trials": len(trials)}))
    if not result["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
