"""Bounded diagnostic calls and exact owned-resource teardown (macOS hosts only).

This is an evidence instrument, not a guest execution or cancellation service.
Callers supply trusted, frozen argv and every file their fixtures read.
"""

from __future__ import annotations

import errno
import hashlib
import json
import math
import os
import re
import select
import signal
import subprocess
import sys
import time
from contextlib import closing
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Literal, TypedDict, cast

from inspection import fork_admission


@dataclass(frozen=True)
class Call:
    name: str
    argv: tuple[str, ...]
    budget: float
    expected_exit: int = 0
    expected_stdout: bytes | None = None
    interrupt_after: float | None = None
    expected_reason: Literal["exit", "interrupted", "deadline"] = "exit"


@dataclass(frozen=True)
class Resource:
    kind: str
    identity: str
    acquire: Call
    cleanup: Call
    dispose: Call
    inspect: Call


@dataclass(frozen=True)
class Plan:
    run_id: str
    calls: tuple[Call, ...]
    resources: tuple[Resource, ...] = ()
    subjects: tuple[Path, ...] = ()
    term_grace: float = 2
    kill_wait: float = 2


def write_json(path: Path, value: object) -> None:
    """Exclusive creation prevents replacement of an earlier receipt."""
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, default=_encode)
        stream.write("\n")


def _encode(value: object) -> str:
    if isinstance(value, bytes):
        return value.hex()
    if isinstance(value, Path):
        return str(value)
    raise TypeError(type(value).__name__)


def _hashes(subjects: tuple[Path, ...]) -> dict[str, str]:
    result: dict[str, str] = {}
    for subject in subjects:
        try:
            with subject.open("rb") as stream:
                result[str(subject)] = hashlib.file_digest(stream, "sha256").hexdigest()
        except OSError as error:
            result[str(subject)] = f"unavailable: {error}"
    return result


def _validate(plan: Plan) -> None:
    if sys.platform != "darwin":
        raise ValueError("this controller currently supports macOS hosts only")
    if not re.fullmatch(r"[0-9a-f]{32}", plan.run_id):
        raise ValueError("run_id must be uuid.uuid4().hex")
    if not all(math.isfinite(n) and 0 < n <= 2 for n in (plan.term_grace, plan.kill_wait)):
        raise ValueError("termination bounds must be positive and at most two seconds")
    calls = list(plan.calls)
    identities: set[str] = set()
    for resource in plan.resources:
        if plan.run_id not in resource.identity or resource.identity in identities:
            raise ValueError("resource identity must be unique and contain this run_id")
        identities.add(resource.identity)
        for call in (resource.acquire, resource.cleanup, resource.dispose, resource.inspect):
            if call.expected_reason != "exit":
                raise ValueError("resource actions must expect ordinary exit")
            if resource.identity not in call.argv:
                raise ValueError("resource action must name the exact identity as an argv token")
            calls.append(call)
        reserve = plan.term_grace + plan.kill_wait
        if resource.cleanup.budget + reserve > 30 or resource.dispose.budget + reserve > 45:
            raise ValueError("cleanup/disposal exceeds the commissioned ceiling")
        if resource.inspect.expected_stdout is None:
            raise ValueError("absence inspection needs an exact stdout oracle")
    names: set[str] = set()
    for call in calls:
        if call.name in {"plan", "result", "subjects-before"} or call.name.startswith("intent-"):
            raise ValueError("call name is reserved for controller receipts")
        if not re.fullmatch(r"[a-zA-Z0-9_-]+", call.name) or call.name in names:
            raise ValueError("call names must be unique safe filenames")
        names.add(call.name)
        if not call.argv or not Path(call.argv[0]).is_absolute():
            raise ValueError("use an explicit absolute executable path")
        if not math.isfinite(call.budget) or call.budget <= 0:
            raise ValueError("each call needs a positive finite budget")
        if call.interrupt_after is not None and not 0 < call.interrupt_after <= call.budget:
            raise ValueError("interruption must be within the call budget")
        if call.expected_reason not in {"exit", "interrupted", "deadline"}:
            raise ValueError("unsupported expected controller reason")
        if call.expected_reason == "interrupted" and call.interrupt_after is None:
            raise ValueError("expected interruption needs interrupt_after")
        if call.expected_reason == "deadline" and call.interrupt_after is not None:
            raise ValueError("expected deadline cannot declare interruption")


def _exited(pid: int) -> bool:
    # NOTE_EXIT observes exit without reaping, pinning the group leader identity
    # until every group signal is sent. A zombie also reports NOTE_EXIT.
    # https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kqueue.2.html
    with closing(select.kqueue()) as queue:
        event = select.kevent(
            pid, filter=select.KQ_FILTER_PROC, flags=select.KQ_EV_ADD, fflags=select.KQ_NOTE_EXIT
        )
        events = queue.control([event], 1, 0)
        for result in events:
            if result.flags & select.KQ_EV_ERROR:
                if result.data == errno.ESRCH:
                    return True
                raise OSError(result.data, os.strerror(result.data))
        return any(result.fflags & select.KQ_NOTE_EXIT for result in events)


def _wait(pid: int, deadline: float) -> bool:
    while not _exited(pid):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        time.sleep(min(0.005, remaining))
    return True


class GroupObservation(TypedDict):
    started_monotonic: float
    finished_monotonic: float
    state: Literal["live", "zombie_only", "unknown"]
    members: list[dict[str, str | int]]
    stdout: bytes
    stderr: bytes
    returncode: int | None
    probe_errno: int | None
    probe_outcome: str
    error: str | None


def _group_snapshot(pid: int, deadline: float) -> GroupObservation:
    """Observe only while the unreaped session leader pins this group identity."""
    observation: GroupObservation = {
        "started_monotonic": time.monotonic(),
        "finished_monotonic": time.monotonic(),
        "state": "unknown",
        "members": [],
        "stdout": b"",
        "stderr": b"",
        "returncode": None,
        "probe_errno": None,
        "probe_outcome": "not_attempted",
        "error": None,
    }
    try:
        # -g selects the process group; Z is dead, not an executable member.
        # https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man1/ps.1.html
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("group inspection phase exhausted")
        check = subprocess.run(
            ("/bin/ps", "-g", str(pid), "-o", "pid=,pgid=,stat="),
            capture_output=True,
            timeout=remaining,
        )
        observation.update(stdout=check.stdout, stderr=check.stderr, returncode=check.returncode)
        # Acceptance limits only: subprocess.run still captures/waits synchronously.
        if len(check.stdout) + len(check.stderr) > 256 * 1024:
            raise ValueError("ps output limit exceeded")
        if check.returncode != 0 or check.stderr:
            raise ValueError("ps did not return a clean process table")
        if not check.stdout.endswith(b"\n"):
            raise ValueError("incomplete process table")
        lines = check.stdout.split(b"\n")[:-1]
        if len(lines) > 4096:
            raise ValueError("ps row limit exceeded")
        members: list[dict[str, str | int]] = []
        seen: set[int] = set()
        for line in lines:
            fields = line.split()
            if len(fields) != 3:
                raise ValueError("invalid process table row")
            member_pid, pgid, state = fields
            if not re.fullmatch(rb"[0-9]+", member_pid) or int(member_pid) <= 0:
                raise ValueError("invalid process ID")
            if int(member_pid) in seen:
                raise ValueError("duplicate process ID")
            seen.add(int(member_pid))
            if not re.fullmatch(rb"[0-9]+", pgid):
                raise ValueError("invalid process group ID")
            if int(pgid) != pid:
                raise ValueError("ps returned a foreign process group")
            # macOS ps(1): run state, then zero or more documented modifiers.
            # https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man1/ps.1.html
            if not re.fullmatch(rb"[IRSTUZ][+<>AELNSsVWX]*", state):
                raise ValueError("unsupported process state")
            members.append({"pid": int(member_pid), "pgid": int(pgid), "stat": state.decode()})
        if not any(member["pid"] == pid for member in members):
            raise ValueError("unreaped leader missing from group inspection")
        if time.monotonic() >= deadline:
            raise TimeoutError("group inspection deadline exhausted before probe")
        observation["members"] = members
        observation["state"] = (
            "zombie_only"
            if all(str(member["stat"]).startswith("Z") for member in members)
            else "live"
        )
        # Signal 0 checks permissions/existence without delivering a signal.
        # EPERM alone never establishes either executable survival or absence.
        # https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html
        try:
            os.killpg(pid, 0)
            observation["probe_outcome"] = "permitted"
        except OSError as error:
            observation["probe_errno"] = error.errno
            observation["probe_outcome"] = (
                "zombie_only_eperm"
                if error.errno == errno.EPERM and observation["state"] == "zombie_only"
                else "permission_failure"
                if error.errno == errno.EPERM
                else "unknown"
            )
            if error.errno != errno.EPERM:
                observation["state"] = "unknown"
                observation["error"] = str(error)
        if time.monotonic() >= deadline:
            raise TimeoutError("group inspection deadline exhausted after probe")
    except subprocess.TimeoutExpired as error:
        observation.update(stdout=error.stdout or b"", stderr=error.stderr or b"", error=str(error))
    except (OSError, ValueError) as error:
        observation["state"] = "unknown"
        observation["members"] = []
        observation["error"] = str(error)
    observation["finished_monotonic"] = time.monotonic()
    return observation


def _terminate(pid: int, pid_only: bool, plan: Plan, receipt: dict[str, object]) -> float:
    """Finish all signals/observations before returning the one reap deadline."""
    events: list[dict[str, object]] = []
    groups: list[GroupObservation] = []
    receipt.update(signals=events, group_observations=groups)
    observer_ok = True
    signals_ok = True
    leader_exit: float | None = None

    def observe_exit() -> bool:
        nonlocal observer_ok, leader_exit
        try:
            if _exited(pid) and leader_exit is None:
                leader_exit = time.monotonic()
                receipt["leader_exit_monotonic"] = leader_exit
        except OSError as error:
            observer_ok = False
            receipt["exit_observation_error"] = str(error)
        return leader_exit is not None

    def observe(deadline: float) -> bool:
        exited = observe_exit()
        # Do not launch another ps with only a sub-millisecond tail of the
        # phase left. Reserve a 10 ms sampling window inside the existing
        # allowance; kqueue polling continues to the actual phase deadline.
        if not pid_only and (not groups or deadline - time.monotonic() >= 0.01):
            groups.append(_group_snapshot(pid, deadline))
        return exited and (pid_only or groups[-1]["state"] == "zombie_only")

    def send(sig: signal.Signals, deadline: float) -> None:
        nonlocal signals_ok
        event: dict[str, object] = {
            "signal": sig.name.removeprefix("SIG"),
            "scope": "pid" if pid_only else "group",
            "target": pid,
            "monotonic": time.monotonic(),
            "outcome": "sent",
        }
        try:
            if pid_only:
                os.kill(pid, sig)
            else:
                os.killpg(pid, sig)
        except OSError as error:
            event.update(errno=error.errno, error=str(error), outcome="signal_error")
            if not pid_only and error.errno == errno.EPERM and sig == signal.SIGTERM:
                # No group inspection may defer KILL during TERM grace. A
                # later table cannot explain this earlier denied attempt.
                event["outcome"] = "permission_unknown"
            elif not pid_only and error.errno == errno.EPERM:
                observe(deadline)
                event["permission_observation"] = groups[-1]
                event["outcome"] = (
                    "zombie_only_eperm"
                    if leader_exit is not None and groups[-1]["state"] == "zombie_only"
                    else "permission_failure"
                    if groups[-1]["state"] == "live"
                    else "permission_unknown"
                )
            signals_ok = signals_ok and event["outcome"] == "zombie_only_eperm"
        event["finished_monotonic"] = time.monotonic()
        events.append(event)

    term_deadline = time.monotonic() + plan.term_grace
    receipt["term_deadline_monotonic"] = term_deadline
    send(signal.SIGTERM, term_deadline)
    complete = observe_exit()
    while (not pid_only or not complete) and time.monotonic() < term_deadline:
        time.sleep(min(0.005, max(0, term_deadline - time.monotonic())))
        if time.monotonic() < term_deadline:
            complete = observe_exit()
    # This single deadline covers KILL, all following observations AND reaping.
    kill_deadline = (
        min(time.monotonic(), term_deadline) + plan.kill_wait
        if pid_only
        else term_deadline + plan.kill_wait
    )
    receipt["kill_reap_deadline_monotonic"] = kill_deadline
    if not pid_only or not complete:
        # A pinned, exited leader does not establish group absence. Attempt
        # group KILL at T before any potentially blocking table inspection.
        send(signal.SIGKILL, kill_deadline)
        complete = observe(kill_deadline)
        while not complete and time.monotonic() < kill_deadline:
            time.sleep(min(0.005, max(0, kill_deadline - time.monotonic())))
            if time.monotonic() < kill_deadline:
                complete = observe(kill_deadline)
    if pid_only:
        groups.append(_group_snapshot(pid, kill_deadline))
    receipt["termination_ok"] = (
        complete
        and observer_ok
        and signals_ok
        and all(group["state"] != "unknown" for group in groups)
        and groups[-1]["state"] == "zombie_only"
    )
    return kill_deadline


def _call(call: Call, root: Path, plan: Plan) -> dict[str, object]:
    start = time.monotonic()
    receipt: dict[str, object] = {
        "name": call.name,
        "argv": call.argv,
        "budget": call.budget,
        "started_monotonic": start,
        "wall_start_ns": time.time_ns(),
        "reason": "spawn_error",
        "returncode": None,
        "reaped": False,
        "ok": False,
    }
    events: list[dict[str, object]] = []
    receipt["signals"] = events
    with (
        (root / f"{call.name}.stdout").open("xb") as stdout,
        (root / f"{call.name}.stderr").open("xb") as stderr,
    ):
        try:
            # Separate session isolates the command CLI from this controller.
            # File streams avoid communicate() waiting for descendant pipe EOF.
            # https://docs.python.org/3.12/library/subprocess.html#subprocess.Popen
            process = subprocess.Popen(
                call.argv,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
        except OSError as error:
            receipt["error"] = str(error)
        else:
            receipt.update(pid=process.pid, pgid=process.pid)
            receipt["signal_scope"] = "pid" if call.interrupt_after is not None else "group"
            deadline = start + (call.interrupt_after or call.budget)
            try:
                exited = _wait(process.pid, deadline)
            except OSError as error:
                receipt["reason"] = "observation_error"
                receipt["error"] = str(error)
                exited = False
            if exited:
                receipt["reason"] = "exit"
                receipt["leader_exit_monotonic"] = time.monotonic()
                reap_deadline = time.monotonic() + plan.kill_wait
                group = _group_snapshot(process.pid, reap_deadline)
                receipt["group_observations"] = [group]
                receipt["termination_ok"] = group["state"] == "zombie_only"
            else:
                if receipt["reason"] != "observation_error":
                    receipt["reason"] = "interrupted" if call.interrupt_after else "deadline"
                reap_deadline = _terminate(
                    process.pid, call.interrupt_after is not None, plan, receipt
                )
            observations = receipt.get("group_observations")
            if isinstance(observations, list) and observations:
                final = cast(GroupObservation, observations[-1])
                if final["state"] != "zombie_only":
                    receipt["residual_members"] = final["members"]
                    receipt["residual_pgid"] = process.pid
                    receipt["residual_remedy"] = (
                        "Inspect retained member PID/PGID and command identities independently; "
                        "release known fixture markers or clean only revalidated owned members. "
                        "Do not signal this group after leader reap. "
                        "Unknown inspection is not absence."
                    )
            try:
                receipt["returncode"] = process.wait(
                    timeout=max(0, reap_deadline - time.monotonic())
                )
                receipt["reaped"] = True
                receipt["reaped_monotonic"] = time.monotonic()
                receipt["within_process_bounds"] = time.monotonic() <= reap_deadline
            except subprocess.TimeoutExpired:
                receipt["reap_error"] = "leader unreaped; manual identity inspection required"
            receipt["ok"] = (
                receipt["reason"] == call.expected_reason
                and receipt["returncode"] == call.expected_exit
                and receipt["reaped"] is True
                and receipt["termination_ok"] is True
                and receipt["within_process_bounds"] is True
            )
    if call.expected_stdout is not None:
        receipt["stdout_matches"] = (
            root / f"{call.name}.stdout"
        ).read_bytes() == call.expected_stdout
        receipt["ok"] = receipt["ok"] is True and receipt["stdout_matches"] is True
    receipt["elapsed"] = time.monotonic() - start
    write_json(root / f"{call.name}.json", receipt)
    return receipt


def _safe_call(call: Call, root: Path, plan: Plan) -> dict[str, object]:
    # A raised receipt/read/open error must not skip independent safety actions.
    # This handles exceptions, not a filesystem operation which never returns.
    try:
        return _call(call, root, plan)
    except OSError as error:
        return {
            "name": call.name,
            "argv": call.argv,
            "ok": False,
            "reason": "controller_io_error",
            "error": str(error),
        }


def execute(plan: Plan, evidence_root: Path) -> Path:
    """Execute once, stop cases at first failure, always attempt owned teardown.

    Register intent before acquisition: even a failed start can leave a clone.
    Callers must use only fresh, run-named resources and trusted public commands.
    """
    _validate(plan)
    root = evidence_root.resolve() / plan.run_id
    root.mkdir(parents=True, exist_ok=False)
    all_calls = plan.calls + tuple(
        call
        for resource in plan.resources
        for call in (resource.acquire, resource.cleanup, resource.dispose, resource.inspect)
    )
    subjects = tuple(
        dict.fromkeys(
            (
                Path(__file__).resolve(),
                Path(__file__).with_name("inspection.py").resolve(),
                Path(sys.executable).resolve(),
                Path("/bin/ps"),
                *plan.subjects,
                *(Path(call.argv[0]).resolve() for call in all_calls),
            )
        )
    )
    write_json(root / "plan.json", asdict(plan))
    before = _hashes(subjects)
    write_json(root / "subjects-before.json", before)
    receipts: list[dict[str, object]] = []
    ledger: list[dict[str, object]] = []
    attempted: list[Resource] = []
    unreached = [resource.acquire.name for resource in plan.resources] + [
        call.name for call in plan.calls
    ]
    admission = fork_admission()
    ok = admission["ok"] and not any(value.startswith("unavailable:") for value in before.values())
    try:
        for resource in plan.resources:
            if not ok:
                break
            attempted.append(resource)
            try:
                write_json(root / f"intent-{len(attempted)}.json", asdict(resource))
            except OSError as error:
                receipts.append(
                    {
                        "name": resource.acquire.name,
                        "ok": False,
                        "reason": "intent_io_error",
                        "error": str(error),
                    }
                )
                ok = False
                break
            unreached.remove(resource.acquire.name)
            receipt = _safe_call(resource.acquire, root, plan)
            receipts.append(receipt)
            ok = receipt["ok"] is True
        for call in plan.calls:
            if not ok:
                break
            unreached.remove(call.name)
            receipt = _safe_call(call, root, plan)
            receipts.append(receipt)
            ok = receipt["ok"] is True
    finally:
        for resource in reversed(attempted):
            cleanup = _safe_call(resource.cleanup, root, plan)
            disposal = _safe_call(resource.dispose, root, plan)
            inspection = _safe_call(resource.inspect, root, plan)
            receipts.extend((cleanup, disposal, inspection))
            disposed = disposal["ok"] is True and inspection["ok"] is True
            ledger.append(
                {
                    "kind": resource.kind,
                    "identity": resource.identity,
                    "state": "disposed" if disposed else "uncertain",
                    "cleanup_ok": cleanup["ok"],
                    "remedy_argv": resource.dispose.argv,
                }
            )
            ok = ok and cleanup["ok"] is True and disposed
        after = _hashes(subjects)
        ok = ok and before == after
        write_json(
            root / "result.json",
            {
                "run_id": plan.run_id,
                "ok": ok,
                "fork_admission": admission,
                "calls": receipts,
                "resources": ledger,
                "unreached": unreached,
                "subjects_before": before,
                "subjects_after": after,
            },
        )
    return root
