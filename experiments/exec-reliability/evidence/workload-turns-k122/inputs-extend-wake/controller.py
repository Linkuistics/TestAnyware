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
from worker_capture import (
    BootstrapBounds,
    CloseBounds,
    FailureBounds,
    RequestScope,
    Worker,
    capture_helper,
)


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
    bootstrap: BootstrapBounds = BootstrapBounds()
    failure_bounds: FailureBounds = FailureBounds()
    close_bounds: CloseBounds = CloseBounds()


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
    if not isinstance(cast(object, plan.bootstrap), BootstrapBounds):
        raise ValueError("invalid bootstrap bounds")
    plan.bootstrap.validate()
    if not isinstance(cast(object, plan.failure_bounds), FailureBounds):
        raise ValueError("invalid failure bounds")
    if not isinstance(cast(object, plan.close_bounds), CloseBounds):
        raise ValueError("invalid close bounds")
    plan.failure_bounds.validate()
    plan.close_bounds.validate()
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


class _WorkloadWait:
    """Observe one pinned workload leader per turn under a fixed cutoff."""

    def __init__(self, pid: int, deadline: float) -> None:
        self.pid = pid
        self.deadline = deadline
        self.exited = False
        self.finished = False

    def advance(self) -> float | None:
        """Observe at most once; return the next absolute wake, or finish."""
        if self.finished:
            return None
        now = time.monotonic()
        if now >= self.deadline:
            self.finished = True
            return None
        try:
            exited = _exited(self.pid)
        except OSError:
            self.finished = True
            raise
        observed = time.monotonic()
        if observed >= self.deadline:
            self.finished = True
            return None
        if exited:
            self.exited = True
            self.finished = True
            return None
        return observed + 0.005


def _wait(pid: int, deadline: float) -> bool:
    owner = _WorkloadWait(pid, deadline)
    while (wake := owner.advance()) is not None:
        time.sleep(min(0.005, max(0.0, wake - time.monotonic())))
    return owner.exited


class GroupObservation(TypedDict):
    started_monotonic: float
    finished_monotonic: float
    query_attempted: bool
    helper: dict[str, object] | None
    state: Literal["live", "zombie_only", "unknown"]
    members: list[dict[str, str | int]]
    stdout: bytes
    stderr: bytes
    returncode: int | None
    probe_started_monotonic: float | None
    probe_finished_monotonic: float | None
    probe_errno: int | None
    probe_outcome: str
    error: str | None


class _WorkloadProcess(subprocess.Popen[bytes]):
    def __del__(self) -> None:
        # Popen.__del__ polls/reaps and registers survivors for later _cleanup.
        # Workload ownership ends only through our explicit deadline-bound reap.
        # https://github.com/python/cpython/blob/v3.15.1/Lib/subprocess.py
        pass


_workload_owners: dict[int, _WorkloadProcess] = {}


class _LeaderReap:
    """Own exact-child reaping after signal/probe authority has closed."""

    def __init__(
        self, process: _WorkloadProcess, deadline: float, receipt: dict[str, object]
    ) -> None:
        self.process = process
        self.deadline = deadline
        self.receipt = receipt
        self.first_attempt = True
        self.finished = False
        self.attempts = 0
        receipt["authority_closed_monotonic"] = time.monotonic()
        receipt["reap_deadline_monotonic"] = deadline
        receipt["within_process_bounds"] = False
        receipt["reap_attempts"] = 0

    def _retain_residual(self) -> None:
        self.finished = True
        self.receipt.setdefault("reap_error", "leader remains unreaped at fixed deadline")
        self.receipt["residual_leader_pid"] = self.process.pid
        self.receipt["residual_leader_remedy"] = (
            "Leader ownership is retained without further controller signals or waits. "
            "Independently inspect the recorded PID/PGID and command identity; "
            "only an external owner may reconcile this residual."
        )

    def advance(self) -> float | None:
        """Attempt at most one WNOHANG; return the next absolute wake, or finish."""
        if self.finished:
            return None
        # Even a late first turn attempts one reap. Later polls spend only K.
        if not self.first_attempt and time.monotonic() >= self.deadline:
            self._retain_residual()
            return None
        self.first_attempt = False
        self.attempts += 1
        self.receipt["reap_attempts"] = self.attempts
        try:
            # A positive PID restricts the wait; WNOHANG returns immediately.
            # https://docs.python.org/3/library/os.html#os.waitpid
            waited_pid, status = os.waitpid(self.process.pid, os.WNOHANG)
        except OSError as error:
            self.receipt["reap_last_result"] = "error"
            self.receipt["reap_error"] = str(error)
            self.receipt["reap_errno"] = error.errno
            self._retain_residual()
            return None
        self.receipt["reap_last_result"] = (
            "reaped" if waited_pid == self.process.pid else "not_ready"
        )
        if waited_pid == self.process.pid:
            self.process.returncode = os.waitstatus_to_exitcode(status)
            finished = time.monotonic()
            self.receipt.update(
                returncode=self.process.returncode,
                reaped=True,
                reaped_monotonic=finished,
                within_process_bounds=finished <= self.deadline,
            )
            del _workload_owners[self.process.pid]
            self.finished = True
            return None
        observed = time.monotonic()
        if observed >= self.deadline:
            self._retain_residual()
            return None
        return min(self.deadline, observed + 0.005)


def _reap_leader(process: _WorkloadProcess, deadline: float, receipt: dict[str, object]) -> None:
    """Consume reap turns synchronously until the exact child or fixed cutoff."""
    owner = _LeaderReap(process, deadline, receipt)
    while not owner.finished:
        wake = owner.advance()
        if wake is not None:
            remaining = wake - time.monotonic()
            if remaining > 0:
                time.sleep(remaining)


def _group_snapshot(
    pid: int, deadline: float, *, worker: Worker | None = None, signal_epoch: int = 0
) -> GroupObservation:
    """Observe only while the unreaped session leader pins this group identity."""
    observation: GroupObservation = {
        "started_monotonic": time.monotonic(),
        "finished_monotonic": time.monotonic(),
        "query_attempted": False,
        "helper": None,
        "state": "unknown",
        "members": [],
        "stdout": b"",
        "stderr": b"",
        "returncode": None,
        "probe_started_monotonic": None,
        "probe_finished_monotonic": None,
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
        observation["query_attempted"] = True
        argv = ("/bin/ps", "-g", str(pid), "-o", "pid=,pgid=,stat=")
        check = (
            capture_helper(argv, deadline)
            if worker is None
            else capture_helper(
                argv, deadline, worker=worker, scope=RequestScope(pid, pid, signal_epoch, "final")
            )
        )
        observation["helper"] = asdict(check)
        observation.update(stdout=check.stdout, stderr=check.stderr, returncode=check.returncode)
        if check.error is not None or not check.reaped:
            raise ValueError(check.error or "inspection helper unreaped")
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
            observation["probe_started_monotonic"] = time.monotonic()
            try:
                os.killpg(pid, 0)
            finally:
                observation["probe_finished_monotonic"] = time.monotonic()
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
    except (OSError, ValueError) as error:
        observation["state"] = "unknown"
        observation["members"] = []
        observation["error"] = str(error)
    observation["finished_monotonic"] = time.monotonic()
    return observation


def _final_snapshot(
    pid: int,
    deadline: float,
    leader_exit: float | None,
    *,
    worker: Worker | None = None,
    signal_epoch: int = 0,
) -> GroupObservation:
    """Spend the sole table request only after exit and before the cutoff."""
    now = time.monotonic()
    if leader_exit is not None and leader_exit <= now < deadline:
        if worker is None:
            return _group_snapshot(pid, deadline)
        return _group_snapshot(pid, deadline, worker=worker, signal_epoch=signal_epoch)
    return {
        "started_monotonic": now,
        "finished_monotonic": now,
        "query_attempted": False,
        "helper": None,
        "state": "unknown",
        "members": [],
        "stdout": b"",
        "stderr": b"",
        "returncode": None,
        "probe_started_monotonic": None,
        "probe_finished_monotonic": None,
        "probe_errno": None,
        "probe_outcome": "not_attempted",
        "error": "no observed leader exit before inspection cutoff"
        if leader_exit is None
        else "inspection cutoff exhausted before final query",
    }


class _InspectionFailure(TypedDict):
    call: str
    observation: GroupObservation


@dataclass
class _InspectionState:
    failure: _InspectionFailure | None = None
    worker: Worker | None = None

    def snapshot(
        self,
        call: str,
        pid: int,
        deadline: float,
        leader_exit: float | None,
        *,
        signal_epoch: int = 0,
    ) -> GroupObservation:
        if self.failure is not None:
            now = time.monotonic()
            return {
                "started_monotonic": now,
                "finished_monotonic": now,
                "query_attempted": False,
                "helper": None,
                "state": "unknown",
                "members": [],
                "stdout": b"",
                "stderr": b"",
                "returncode": None,
                "probe_started_monotonic": None,
                "probe_finished_monotonic": None,
                "probe_errno": None,
                "probe_outcome": "not_attempted",
                "error": f"local inspection unavailable after {self.failure['call']}",
            }
        observation = (
            _final_snapshot(pid, deadline, leader_exit)
            if self.worker is None
            else _final_snapshot(
                pid, deadline, leader_exit, worker=self.worker, signal_epoch=signal_epoch
            )
        )
        if observation["query_attempted"] and observation["state"] == "unknown":
            self.failure = {"call": call, "observation": observation}
        return observation


class _Termination:
    """Advance pinned workload signals and exit notification without waiting."""

    def __init__(self, pid: int, pid_only: bool, plan: Plan, receipt: dict[str, object]) -> None:
        self.pid = pid
        self.pid_only = pid_only
        self.plan = plan
        self.receipt = receipt
        self.events: list[dict[str, object]] = []
        self.groups: list[GroupObservation] = []
        receipt.update(signals=self.events, group_observations=self.groups)
        self.observer_ok = True
        self.leader_exit: float | None = None
        self.term_sent = False
        self.kill_sent = False
        self.finished = False
        # Freeze authority before publishing a receipt or touching the target.
        started = time.monotonic()
        self.term_deadline = started + plan.term_grace
        self.kill_deadline = self.term_deadline + plan.kill_wait
        self.inspection_deadline = self.kill_deadline - min(0.1, plan.kill_wait / 4)
        receipt["term_deadline_monotonic"] = self.term_deadline
        self._publish_deadlines()

    def _publish_deadlines(self) -> None:
        self.receipt["kill_reap_deadline_monotonic"] = self.kill_deadline
        self.receipt["inspection_deadline_monotonic"] = self.inspection_deadline

    def _observe_exit(self) -> None:
        if self.leader_exit is not None or not self.observer_ok:
            return
        # Exhausting the observation window does not mean the observer failed.
        if time.monotonic() >= self.inspection_deadline:
            return
        try:
            exited = _exited(self.pid)
            observed = time.monotonic()
            if observed >= self.inspection_deadline:
                self.observer_ok = False
                self.receipt["exit_observation_error"] = "exit observation returned after cutoff"
            elif exited:
                self.leader_exit = observed
                self.receipt["leader_exit_monotonic"] = observed
        except OSError as error:
            self.observer_ok = False
            self.receipt["exit_observation_error"] = str(error)

    def _send(self, sig: signal.Signals) -> None:
        event: dict[str, object] = {
            "signal": sig.name.removeprefix("SIG"),
            "scope": "pid" if self.pid_only else "group",
            "target": self.pid,
            "outcome": "sent",
        }
        try:
            admitted = time.monotonic()
            event["monotonic"] = admitted
            if admitted >= self.inspection_deadline:
                event["outcome"] = "deadline_exhausted"
            elif self.pid_only:
                os.kill(self.pid, sig)
            else:
                os.killpg(self.pid, sig)
        except OSError as error:
            event.update(errno=error.errno, error=str(error), outcome="signal_error")
            if not self.pid_only and error.errno == errno.EPERM:
                event["outcome"] = "permission_unknown"
        event["finished_monotonic"] = time.monotonic()
        self.events.append(event)

    def advance(self) -> float | None:
        """Do at most one exit observation and return the next absolute wake."""
        if self.finished:
            return None
        if not self.term_sent:
            self.term_sent = True
            self._send(signal.SIGTERM)
        self._observe_exit()
        if self.pid_only and self.leader_exit is not None:
            self.kill_deadline = min(self.leader_exit, self.term_deadline) + self.plan.kill_wait
            self.inspection_deadline = self.kill_deadline - min(0.1, self.plan.kill_wait / 4)
            self._publish_deadlines()
            self.finished = True
            return None
        now = time.monotonic()
        if now >= self.term_deadline and not self.kill_sent:
            # An exited leader never cancels the mandatory group KILL at T.
            self.kill_sent = True
            self._send(signal.SIGKILL)
            now = time.monotonic()
        if now >= self.inspection_deadline or (self.kill_sent and self.leader_exit is not None):
            self.finished = True
            return None
        if self.kill_sent:
            return min(now + 0.005, self.inspection_deadline)
        if self.leader_exit is not None:
            return min(self.term_deadline, self.inspection_deadline)
        return min(now + 0.005, self.term_deadline, self.inspection_deadline)


def _terminate(
    pid: int,
    pid_only: bool,
    plan: Plan,
    receipt: dict[str, object],
    inspection: _InspectionState,
) -> float:
    """Compatibility driver; the shared scheduler can instead advance the owner."""
    owner = _Termination(pid, pid_only, plan, receipt)
    while (wake := owner.advance()) is not None:
        delay = wake - time.monotonic()
        if delay > 0:
            time.sleep(delay)
    final = inspection.snapshot(
        str(receipt["name"]),
        pid,
        owner.inspection_deadline,
        owner.leader_exit,
        signal_epoch=len(owner.events),
    )
    owner.groups.append(final)
    last_signal = owner.events[-1]
    if not pid_only and last_signal["signal"] == "KILL" and last_signal.get("errno") == errno.EPERM:
        last_signal["permission_observation"] = final
        last_signal["outcome"] = (
            "zombie_only_eperm"
            if final["state"] == "zombie_only"
            else "permission_failure"
            if final["state"] == "live"
            else "permission_unknown"
        )
    receipt["termination_ok"] = (
        owner.leader_exit is not None
        and owner.observer_ok
        and all(event["outcome"] in {"sent", "zombie_only_eperm"} for event in owner.events)
        and final["state"] == "zombie_only"
    )
    return owner.kill_deadline


def _call(call: Call, root: Path, plan: Plan, inspection: _InspectionState) -> dict[str, object]:
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
            process = _WorkloadProcess(
                call.argv,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
        except OSError as error:
            receipt["error"] = str(error)
        else:
            _workload_owners[process.pid] = process
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
                leader_exit = time.monotonic()
                receipt["leader_exit_monotonic"] = leader_exit
                reap_deadline = leader_exit + plan.kill_wait
                inspection_deadline = reap_deadline - min(0.1, plan.kill_wait / 4)
                receipt["inspection_deadline_monotonic"] = inspection_deadline
                group = inspection.snapshot(
                    call.name, process.pid, inspection_deadline, leader_exit
                )
                receipt["group_observations"] = [group]
                receipt["termination_ok"] = group["state"] == "zombie_only"
            else:
                if receipt["reason"] != "observation_error":
                    receipt["reason"] = "interrupted" if call.interrupt_after else "deadline"
                reap_deadline = _terminate(
                    process.pid, call.interrupt_after is not None, plan, receipt, inspection
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
            _reap_leader(process, reap_deadline, receipt)
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


def _safe_call(
    call: Call, root: Path, plan: Plan, inspection: _InspectionState
) -> dict[str, object]:
    # A raised receipt/read/open error must not skip independent safety actions.
    # This handles exceptions, not a filesystem operation which never returns.
    try:
        return _call(call, root, plan, inspection)
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
                Path(__file__).with_name("worker_capture.py").resolve(),
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
    worker = Worker(
        run_id=plan.run_id,
        bootstrap=plan.bootstrap,
        failure_bounds=plan.failure_bounds,
        close_bounds=plan.close_bounds,
    )
    inspection_state = _InspectionState(worker=worker)
    ledger: list[dict[str, object]] = []
    attempted: list[Resource] = []
    unreached = [resource.acquire.name for resource in plan.resources] + [
        call.name for call in plan.calls
    ]
    admission = fork_admission()
    ok = admission["ok"] and not any(value.startswith("unavailable:") for value in before.values())
    try:
        if ok:
            ok = worker.start()
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
            receipt = _safe_call(resource.acquire, root, plan, inspection_state)
            receipts.append(receipt)
            ok = receipt["ok"] is True
        for call in plan.calls:
            if not ok:
                break
            unreached.remove(call.name)
            receipt = _safe_call(call, root, plan, inspection_state)
            receipts.append(receipt)
            ok = receipt["ok"] is True
    finally:
        try:
            for resource in reversed(attempted):
                cleanup = _safe_call(resource.cleanup, root, plan, inspection_state)
                disposal = _safe_call(resource.dispose, root, plan, inspection_state)
                inspection = _safe_call(resource.inspect, root, plan, inspection_state)
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
        finally:
            worker.close()
        ok = ok and worker.failure is None
        after = _hashes(subjects)
        ok = ok and before == after
        write_json(
            root / "result.json",
            {
                "run_id": plan.run_id,
                "ok": ok,
                "fork_admission": admission,
                "inspection_failure": inspection_state.failure,
                "inspection_worker": worker.evidence(),
                "calls": receipts,
                "resources": ledger,
                "unreached": unreached,
                "subjects_before": before,
                "subjects_after": after,
            },
        )
    return root
