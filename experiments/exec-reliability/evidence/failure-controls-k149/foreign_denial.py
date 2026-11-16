"""Sender side of the real UID-denial control (k149 control 2).

A separately authorised owner (foreign_owner.py, run by a human with sudo)
publishes one fixture whose real, effective and saved UIDs differ from this
sender. This tool never acquires privilege. It verifies the fixture identity,
then drives the real controller termination from frozen inputs against it, in
group scope and in PID scope. The kernel denies each signal with EPERM. The
fixture never exits, so the final group table is missing, and group KILL EPERM
must be classified as permission_unknown. The tool then writes the done marker,
waits for the owner's cleanup receipt and audits that the PID has gone.

With --self-fixture, the tool runs the same steps against a child that this
sender owns. That mode tests the plumbing only. Its verdict must fail, because
the fixture UIDs equal the sender UIDs and the signals succeed.
"""

import argparse
import errno
import hashlib
import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Any

IDENTITY = "pid=,ppid=,pgid=,uid=,ruid=,svuid=,stat=,lstart=,comm="


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def identity(pid: int) -> dict[str, Any]:
    result = subprocess.run(
        ("/bin/ps", "-o", IDENTITY, "-p", str(pid)), capture_output=True, check=False
    )
    fields = result.stdout.decode().split()
    row: dict[str, Any] = {"returncode": result.returncode, "row": result.stdout.decode().strip()}
    if len(fields) >= 13:
        row.update(
            pid=int(fields[0]),
            ppid=int(fields[1]),
            pgid=int(fields[2]),
            uid=int(fields[3]),
            ruid=int(fields[4]),
            svuid=int(fields[5]),
            stat=fields[6],
            lstart=" ".join(fields[7:12]),
            comm=" ".join(fields[12:]),
        )
    return row


def probe(pid: int) -> int | None:
    try:
        os.kill(pid, 0)
    except OSError as error:
        return error.errno
    return None


def terminate(controller: Any, pid: int, pid_only: bool) -> dict[str, Any]:
    plan = controller.Plan(uuid.uuid4().hex, (), term_grace=0.15, kill_wait=0.3)
    receipt: dict[str, Any] = {"name": "foreign-pid" if pid_only else "foreign-group"}
    started = time.monotonic()
    controller._terminate(pid, pid_only, plan, receipt, controller._InspectionState())  # noqa: SLF001
    receipt["control_started_monotonic"] = started
    receipt["control_finished_monotonic"] = time.monotonic()
    return json.loads(
        json.dumps(
            receipt, default=lambda value: value.hex() if isinstance(value, bytes) else str(value)
        )
    )


def wait_for(path: Path, seconds: float) -> bool:
    deadline = time.monotonic() + seconds
    while not path.exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    return path.exists()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--owner-dir", type=Path)
    parser.add_argument("--self-fixture", action="store_true")
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    out.mkdir(parents=True)
    sys.path.insert(0, str(inputs))
    import controller  # noqa: PLC0415  (imported from the frozen inputs)

    subjects = sorted(inputs.glob("*.py"))
    before = {path.name: sha256(path) for path in subjects}
    sender = {"uid": os.getuid(), "euid": os.geteuid()}
    child: subprocess.Popen[bytes] | None = None
    if args.self_fixture:
        child = subprocess.Popen(("/bin/sleep", "600"), start_new_session=True)
        time.sleep(0.2)
        owner: dict[str, Any] = {
            "fixture_pid": child.pid,
            "owner_pid": os.getpid(),
            "self_fixture": True,
        }
    else:
        if args.owner_dir is None:
            parser.error("--owner-dir is required without --self-fixture")
        owner = json.loads((args.owner_dir / "owner-receipt.json").read_text())
    pid = int(owner["fixture_pid"])

    identity_before = identity(pid)
    probe_before = probe(pid)
    group = terminate(controller, pid, False)
    pid_scope = terminate(controller, pid, True)
    identity_between = identity(pid)
    probe_after = probe(pid)

    cleanup: dict[str, Any] | None = None
    if child is not None:
        child.kill()
        cleanup = {"self_fixture_returncode": child.wait(timeout=5)}
    elif args.owner_dir is not None:
        (args.owner_dir / "sender-done").write_text(
            json.dumps({"finished_monotonic": time.monotonic()})
        )
        if wait_for(args.owner_dir / "owner-cleanup.json", 10):
            cleanup = json.loads((args.owner_dir / "owner-cleanup.json").read_text())
    identity_after = identity(pid)
    time.sleep(0.1)
    after = {path.name: sha256(path) for path in subjects}

    group_events: dict[str, dict[str, Any]] = {
        event["signal"]: event for event in group.get("signals", [])
    }
    pid_events: dict[str, dict[str, Any]] = {
        event["signal"]: event for event in pid_scope.get("signals", [])
    }
    observations: list[dict[str, Any]] = group.get("group_observations") or [{}]
    final_group = observations[-1]
    same_identity = all(
        identity_between.get(key) == identity_before.get(key) and key in identity_before
        for key in ("pid", "pgid", "uid", "ruid", "svuid", "stat", "lstart", "comm")
    )
    checks: dict[str, bool] = {
        "fixture_published_by_separate_owner": not owner.get("self_fixture")
        and owner.get("owner_euid") == 0
        and owner.get("owner_pid") != os.getpid(),
        "fixture_is_owner_child": identity_before.get("ppid") == owner.get("owner_pid"),
        "fixture_leads_own_group": identity_before.get("pgid") == pid,
        "fixture_uids_differ_from_sender": all(
            identity_before.get(key) not in (sender["uid"], sender["euid"], None)
            for key in ("uid", "ruid", "svuid")
        ),
        "kernel_probe_denied": probe_before == errno.EPERM and probe_after == errno.EPERM,
        "group_term_eperm": group_events.get("TERM", {}).get("errno") == errno.EPERM
        and group_events.get("TERM", {}).get("outcome") == "permission_unknown",
        "group_kill_eperm": group_events.get("KILL", {}).get("errno") == errno.EPERM,
        "missing_table": final_group.get("query_attempted") is False
        and final_group.get("state") == "unknown",
        "group_kill_permission_unknown": group_events.get("KILL", {}).get("outcome")
        == "permission_unknown",
        "group_termination_failed": group.get("termination_ok") is False,
        "pid_signals_eperm": bool(pid_events)
        and all(
            event.get("errno") == errno.EPERM and event.get("outcome") == "signal_error"
            for event in pid_events.values()
        ),
        "pid_termination_failed": pid_scope.get("termination_ok") is False,
        "fixture_survived_unchanged": same_identity,
        "owner_killed_and_reaped_exact_child": cleanup is not None
        and cleanup.get("waited_pid") == pid
        and cleanup.get("terminating_signal") == 9
        and cleanup.get("owner_has_children_after") is False,
        "fixture_absent_after_cleanup": identity_after.get("returncode") != 0
        and not identity_after.get("row"),
        "subjects_unchanged": before == after,
    }
    result: dict[str, Any] = {
        "control": "genuine-uid-denial-and-permission-unknown",
        "inputs": str(inputs),
        "module": controller.__file__,
        "python": sys.version,
        "sender": sender,
        "owner": owner,
        "identity_before": identity_before,
        "identity_between": identity_between,
        "identity_after": identity_after,
        "probe_errno_before": probe_before,
        "probe_errno_after": probe_after,
        "group_receipt": group,
        "pid_receipt": pid_scope,
        "owner_cleanup": cleanup,
        "subjects_before": before,
        "subjects_after": after,
        "checks": checks,
        "verdict": "pass" if all(checks.values()) else "fail",
    }
    (out / "foreign-denial.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"verdict": result["verdict"], "checks": checks}, indent=1))


if __name__ == "__main__":
    main()
