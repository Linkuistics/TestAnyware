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
from collections.abc import Callable
from contextlib import closing
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Literal, TypedDict, cast

import storage_client
import writer
from inspection import fork_admission
from retention import IDENTITY, STORAGE_CLOSE, observe_file
from storage_client import OUTBOX_CAP, READINESS_CEILING, WRITER, StorageClient
from turns import Turn, run_turns
from worker_capture import (
    BootstrapBounds,
    CloseBounds,
    FailureBounds,
    RequestScope,
    Worker,
    capture_helper,
)
from writer import MAX_PAYLOAD

# docs/specs/diagnostic-evidence-storage.md, "Phases and bounds": each intent
# gate adds at most 2 seconds before its acquisition.
INTENT_GATE_CEILING = 2.0


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
    readiness_bound: float = READINESS_CEILING
    intent_gate: float = INTENT_GATE_CEILING
    outbox_cap: int = OUTBOX_CAP


def _json_bytes(value: object) -> bytes:
    return json.dumps(value, indent=2, default=_encode).encode() + b"\n"


def _pin(data: bytes) -> dict[str, object]:
    return {"length": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def write_json(path: Path, value: object) -> None:
    """Store one JSON record. Neither route replaces an earlier record.

    A record for a run directory that `execute()` is writing goes to that
    run's writer. The writer creates its file exclusively. Any other path is
    created exclusively by this process.
    """
    data = _json_bytes(value)
    storage = _run_storage.get(path.parent)
    if storage is not None:
        storage.record(path.name, data)
        return
    with path.open("xb") as stream:
        stream.write(data)


def _encode(value: object) -> str:
    if isinstance(value, bytes):
        return value.hex()
    if isinstance(value, Path):
        return str(value)
    raise TypeError(type(value).__name__)


# The writer now hashes the subjects. This function stays as the seam that
# the bound validation tests patch: a plan that fails validation must never
# reach subject hashing, the writer or a fork.
def _hashes(subjects: tuple[Path, ...]) -> dict[str, str]:  # pyright: ignore[reportUnusedFunction]
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
    if not (math.isfinite(plan.readiness_bound) and 0 < plan.readiness_bound <= READINESS_CEILING):
        raise ValueError(f"the readiness bound must be in (0, {READINESS_CEILING:g}] s")
    if not (math.isfinite(plan.intent_gate) and 0 < plan.intent_gate <= INTENT_GATE_CEILING):
        raise ValueError(f"the intent gate must be in (0, {INTENT_GATE_CEILING:g}] s")
    cap = cast(object, plan.outbox_cap)
    if not isinstance(cap, int) or isinstance(cap, bool) or cap <= 0:
        raise ValueError("the outbox cap must be a positive integer")
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
        reserved = {
            "plan",
            "result",
            "subjects-before",
            Path(IDENTITY).stem,
            Path(STORAGE_CLOSE).stem,
        }
        if call.name in reserved or call.name.startswith("intent-"):
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
        return min(self.deadline, observed + 0.005)


def _wait(pid: int, deadline: float, *, drive: Callable[[Turn], None] = run_turns) -> bool:
    owner = _WorkloadWait(pid, deadline)
    drive(Turn(owner.advance))
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


def _reap_leader(
    process: _WorkloadProcess,
    deadline: float,
    receipt: dict[str, object],
    *,
    drive: Callable[[Turn], None] = run_turns,
) -> None:
    """Consume reap turns synchronously until the exact child or fixed cutoff."""
    owner = _LeaderReap(process, deadline, receipt)
    drive(Turn(owner.advance, safety=True))


def _group_snapshot(
    pid: int,
    deadline: float,
    *,
    worker: Worker | None = None,
    signal_epoch: int = 0,
    defer_retirement: Callable[[Turn], None] | None = None,
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

    def retain_retirement(turn: Turn) -> None:
        assert defer_retirement is not None

        def advance() -> float | None:
            wake = turn.advance()
            observation["helper"] = asdict(check)
            return wake

        defer_retirement(Turn(advance, safety=turn.safety, readiness=turn.readiness))

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
                argv,
                deadline,
                worker=worker,
                scope=RequestScope(pid, pid, signal_epoch, "final"),
                **({"defer_retirement": retain_retirement} if defer_retirement is not None else {}),
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
            # ps prints E only for P_WEXIT before SZOMB. It prints "?" when it
            # reads no thread state, as for an exiting task without threads.
            # "?" alone can also mean an unreadable task, so it needs E.
            # https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/print.c
            # https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/tasks.c
            if not re.fullmatch(rb"[IRSTUZ][+<>AELNSsVWX]*", state) and not re.fullmatch(
                rb"\?[+<>ALNSsVWX]*E[+<>ALNSsVWX]*", state
            ):
                raise ValueError("unsupported process state")
            members.append({"pid": int(member_pid), "pgid": int(pgid), "stat": state.decode()})
        if not any(member["pid"] == pid for member in members):
            raise ValueError("unreaped leader missing from group inspection")
        if time.monotonic() >= deadline:
            raise TimeoutError("group inspection deadline exhausted before probe")
        observation["members"] = members
        # Z is dead. E is P_WEXIT, from P_LEXIT, which exit sets once after it
        # holds every other thread and never clears, so an E member never runs
        # user code again. Kqueue reports ESRCH once proc_find fails, after the
        # last thread ends but before SZOMB, so the leader can still show ?E
        # (no threads, not yet a zombie) after its observed exit.
        # https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exit.c
        # https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c
        observation["state"] = (
            "zombie_only"
            if all(
                str(member["stat"]).startswith("Z") or "E" in str(member["stat"])
                for member in members
            )
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
    defer_retirement: Callable[[Turn], None] | None = None,
) -> GroupObservation:
    """Spend the sole table request only after exit and before the cutoff."""
    now = time.monotonic()
    if leader_exit is not None and leader_exit <= now < deadline:
        if worker is None:
            return _group_snapshot(pid, deadline)
        return _group_snapshot(
            pid,
            deadline,
            worker=worker,
            signal_epoch=signal_epoch,
            **({"defer_retirement": defer_retirement} if defer_retirement is not None else {}),
        )
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
    retirements: list[Turn] = field(default_factory=list[Turn])
    # The run's storage. Its turn drains the outbox beside every command wait.
    storage: _RunStorage | None = None

    def advance_worker(self) -> float | None:
        assert self.worker is not None
        if self.retirements:
            turn = self.retirements[0]
            wake = turn.advance()
            if wake is not None:
                return wake
            self.retirements.pop(0)
            # Close reads the request's original cutoff and attempted signals.
            # Never close its channels before cleanup-only parsing has finished.
            self.worker.begin_close()
            return time.monotonic()
        return self.worker.advance_close()

    def worker_readiness(self) -> tuple[tuple[int, ...], tuple[int, ...]]:
        if self.retirements and self.retirements[0].readiness is not None:
            return self.retirements[0].readiness()
        return (), ()

    def close(self) -> None:
        assert self.worker is not None
        try:
            run_turns(Turn(self.advance_worker, safety=True, readiness=self.worker_readiness))
        finally:
            # Also finalizes descriptors if an owner callback raises.
            self.worker.close()

    def drive(self, turn: Turn) -> None:
        """Advance failed worker shutdown and storage beside the active command.

        A finished command yields to the next phase immediately. The worker
        retains its close state and fixed cutoffs across every such phase.
        The storage turn drains the outbox and reads acknowledgements, so a
        storage failure is seen while the command runs. It never delays the
        command, because each storage turn is bounded and nonblocking.
        """
        worker = self.worker
        shutdown = not (
            worker is None
            or (not worker.closed and not self.retirements)
            or (worker.shutdown_finished is not None)
        )
        if not shutdown and self.storage is None:
            run_turns(turn)
            return
        finished = False

        def advance_call() -> float | None:
            nonlocal finished
            wake = turn.advance()
            finished = wake is None
            return wake

        def advance_worker() -> float | None:
            if finished:
                return None
            return self.advance_worker()

        turns = [Turn(advance_call, safety=turn.safety, readiness=turn.readiness)]
        if shutdown:
            turns.append(Turn(advance_worker, safety=True, readiness=self.worker_readiness))
        storage = self.storage
        if storage is not None:

            def advance_storage() -> float | None:
                if finished:
                    return None
                return storage.client.advance()

            turns.append(Turn(advance_storage, readiness=storage.client.interests))
        run_turns(*turns)

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
                pid,
                deadline,
                leader_exit,
                worker=self.worker,
                signal_epoch=signal_epoch,
                defer_retirement=self.retirements.append,
            )
        )
        if observation["query_attempted"] and observation["state"] == "unknown":
            self.failure = {"call": call, "observation": observation}
            if self.worker is not None:
                self.worker.failure = self.worker.failure or f"local inspection failed after {call}"
                if not self.retirements:
                    self.worker.begin_close()
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
    inspection.drive(Turn(owner.advance, safety=True))
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
                exited = _wait(process.pid, deadline, drive=inspection.drive)
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
            _reap_leader(process, reap_deadline, receipt, drive=inspection.drive)
            receipt["ok"] = (
                receipt["reason"] == call.expected_reason
                and receipt["returncode"] == call.expected_exit
                and receipt["reaped"] is True
                and receipt["termination_ok"] is True
                and receipt["within_process_bounds"] is True
            )
    # Pin each raw stream after reap and the termination decision. A surviving
    # writer can still append later; retention.verify compares with these pins.
    observed: dict[str, bytes | None] = {}
    streams: dict[str, dict[str, object]] = {}
    for stream in ("stdout", "stderr"):
        observed[stream], streams[stream] = observe_file(root / f"{call.name}.{stream}")
    receipt["streams"] = streams
    if any(data is None for data in observed.values()):
        receipt["ok"] = False
    if call.expected_stdout is not None:
        # Judge exactly the pinned bytes, never a second read.
        receipt["stdout_matches"] = observed["stdout"] == call.expected_stdout
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


# docs/specs/diagnostic-evidence-storage.md, "Result snapshot": the
# subjects-after wait ends by S + W/2. Until storage-publication-k168 adds the
# publication window, the wait uses the largest W/2, which is 5 seconds.
SUBJECTS_AFTER_BOUND = 5.0


@dataclass(frozen=True)
class StoragePhase:
    """One storage phase: its absolute start and deadline, end and outcome."""

    started: float
    deadline: float
    finished: float | None
    outcome: str


@dataclass(frozen=True)
class StorageOutcome:
    """The storage state that `execute()` reports in memory after the run.

    The publication bounds and the terminal storage section arrive with
    storage-publication-k168. Until then, `publication` says `complete` only
    for an `ok` close acknowledgement and a reaped writer.
    """

    readiness: str
    readiness_reason: str | None
    first_failure: str | None
    forward_stop: str | None
    publication: str
    writer_reaped: bool
    exit_status: int | None
    residual: bool
    killed_at: float | None
    reaped_at: float | None
    drops: dict[str, int]
    abandoned: dict[str, int]
    phases: dict[str, StoragePhase]
    snapshot: dict[str, object] | None
    close_ack: dict[str, object] | None


class RunDirectory(Path):
    """The run directory. `storage` holds the run's storage outcome.

    Python 3.12 supports subclasses of `Path`
    (https://docs.python.org/3.12/whatsnew/3.12.html#pathlib). A derived path,
    such as `directory / "result.json"`, has no storage outcome.
    """

    storage: StorageOutcome | None = None


class _RunStorage:
    """Send one run's records to its writer and keep the storage state.

    Storage failure is sticky. Once storage is not ready, `healthy()` is
    False and the controller stops forward progress.
    """

    def __init__(self, plan: Plan, root: Path) -> None:
        self.plan = plan
        self.root = root
        # Read the module globals at call time, so a test can substitute the
        # writer command or the client.
        self.client = StorageClient(
            WRITER, readiness_bound=plan.readiness_bound, outbox_cap=plan.outbox_cap
        )
        self.records: dict[int, str] = {}
        self.digests: dict[str, dict[str, object]] = {}
        self.phases: dict[str, StoragePhase] = {}
        self.forward_stop: str | None = None
        # The frame that a controller-detected first failure names. An error
        # acknowledgement names its own frame in the client's failure_ack.
        self.failure_context: dict[str, object] | None = None
        self.close_ack: dict[str, object] | None = None
        self.result_snapshot: dict[str, object] | None = None

    # Readiness ---------------------------------------------------------------

    def start(self, plan: bytes, subjects: tuple[Path, ...]) -> dict[str, str]:
        """Start the writer and return the subjects-before digests.

        Without readiness, every subject digest is `unavailable:storage`.
        """
        client = self.client
        self.records[client.next_seq] = "plan"
        self.digests["plan"] = _pin(plan)
        names = [str(subject) for subject in subjects]
        client.start(str(self.root.parent), self.root.name, plan, names)
        if client.spawned_at is not None and client.readiness_deadline is not None:
            self.phases["readiness"] = StoragePhase(
                client.spawned_at,
                client.readiness_deadline,
                client.ready_at if client.readiness == "ready" else client.failed_at,
                client.readiness,
            )
        ack = client.readiness_ack
        if client.storage == "ready" and ack is not None:
            if ack.get("run_dir") != str(self.root):
                client.fail("run_dir")
        if not self.healthy() or ack is None:
            return {name: "unavailable:storage" for name in names}
        digests = ack.get("subjects")
        if not isinstance(digests, dict) or set(cast(dict[str, object], digests)) != set(names):
            client.fail("protocol")
            return {name: "unavailable:storage" for name in names}
        return cast(dict[str, str], digests)

    def healthy(self) -> bool:
        if self.client.storage != "ready":
            self._stop()
        return self.forward_stop is None

    def _stop(self) -> None:
        if self.forward_stop is None:
            client = self.client
            self.forward_stop = client.first_failure or client.readiness_reason or client.storage

    def _fail(self, cause: str, operation: str, record: str, sequence: int | None) -> None:
        """Fail storage for a cause that the controller detects, and keep its frame.

        The context is kept only when this call sets the first failure. A
        later error acknowledgement cannot replace it, because the client
        keeps a failure acknowledgement only while storage is ready. An
        unqueued frame has no sequence number.
        """
        client = self.client
        if client.storage == "ready":
            self.failure_context = {
                "sequence": sequence,
                "operation": operation,
                "record": record,
            }
        client.fail(cause)

    # Records -----------------------------------------------------------------

    def record(self, name: str, data: bytes) -> int | None:
        """Serialise one record into the outbox. Return its first sequence number.

        A record larger than one frame becomes a create and appends. A drop
        fails storage. The result record gets no digest entry.
        """
        client = self.client
        if name != "result":
            self.digests[name] = _pin(data)
        first: int | None = None
        for offset in range(0, max(len(data), 1), MAX_PAYLOAD):
            chunk = data[offset : offset + MAX_PAYLOAD]
            if offset:
                op = "append"
            else:
                op = "result" if name == "result" else "create"
            seq = client.enqueue(op, name, chunk)
            if seq is None:
                if client.drops.get(name):
                    self._fail("outbox_drop", op, name, None)
                self._stop()
                return first
            self.records[seq] = name
            first = seq if first is None else first
        return first

    # P2: intent gate ---------------------------------------------------------

    def gate(self, index: int, data: bytes, drive: Callable[[Turn], None]) -> bool:
        """Store intent record `index` and wait for its acknowledgement.

        The wait runs as scheduler turns and ends by the gate deadline. A
        missed, late or failed acknowledgement fails storage, so the
        resource is never acquired.
        """
        if not self.healthy():
            return False
        name = f"intent-{index}"
        started = time.monotonic()
        deadline = started + self.plan.intent_gate
        seq = self.record(f"{name}.json", data)
        client = self.client

        def advance() -> float | None:
            wake = client.advance()
            if seq is None or client.ack(seq) is not None or client.storage != "ready":
                return None
            if time.monotonic() >= deadline:
                return None
            return min(wake, deadline)

        if seq is not None:
            drive(Turn(advance, readiness=client.interests))
        finished = time.monotonic()
        ack = client.ack(seq) if seq is not None else None
        if ack is not None and ack.get("status") == "ok" and finished <= deadline:
            outcome = "acknowledged"
        elif ack is not None and ack.get("status") == "ok":
            outcome = "late"
        elif ack is not None or client.storage != "ready":
            outcome = "failed"
        else:
            outcome = "missed"
        if outcome in ("late", "missed"):
            assert seq is not None
            self._fail("intent_gate", "create", f"{name}.json", seq)
        self.phases[name] = StoragePhase(started, deadline, finished, outcome)
        return outcome == "acknowledged" and self.healthy()

    # After teardown ----------------------------------------------------------

    def subjects_after(self, subjects: tuple[Path, ...]) -> dict[str, str]:
        names = [str(subject) for subject in subjects]
        unavailable = {name: "unavailable:storage" for name in names}
        if not self.healthy():
            return unavailable
        client = self.client
        seq = client.enqueue("subjects", "subjects-after")
        if seq is None:
            # The client was ready, so the outbox cap dropped the frame.
            self._fail("outbox_drop", "subjects", "subjects-after", None)
            self._stop()
            return unavailable
        self.records[seq] = "subjects-after"
        deadline = time.monotonic() + SUBJECTS_AFTER_BOUND
        run_turns(
            client.turn(
                lambda: (
                    client.ack(seq) is not None
                    or client.storage != "ready"
                    or time.monotonic() >= deadline
                )
            )
        )
        ack = client.ack(seq)
        digests = ack.get("subjects") if ack is not None and ack.get("status") == "ok" else None
        if not isinstance(digests, dict) or set(cast(dict[str, object], digests)) != set(names):
            return unavailable
        return cast(dict[str, str], digests)

    def snapshot(self) -> dict[str, object]:
        """Return the result's storage section, as of the result's own sequence."""
        client = self.client
        sequence = client.next_seq
        enqueued = time.monotonic()
        failed = {seq: ack for seq, ack in client.acks.items() if ack.get("status") != "ok"}
        failure: dict[str, object] | None = None
        if client.first_failure is not None:
            ack = client.failure_ack
            failure = {
                "cause": client.first_failure,
                "sequence": None,
                "operation": None,
                "record": None,
                "errno": None,
            }
            if ack is not None:
                seq = ack.get("seq")
                failure["sequence"] = seq
                failure["operation"] = ack.get("op")
                failure["record"] = self.records.get(seq) if isinstance(seq, int) else None
                failure["errno"] = ack.get("errno")
            elif self.failure_context is not None:
                failure.update(self.failure_context)
        readiness = self.phases.get("readiness")
        section: dict[str, object] = {
            "sequence": sequence,
            "enqueued_monotonic": enqueued,
            "publication": "pending",
            "readiness": {
                "outcome": client.readiness,
                "reason": client.readiness_reason,
                "started": readiness.started if readiness else None,
                "deadline": readiness.deadline if readiness else None,
                "finished": readiness.finished if readiness else None,
            },
            "frames": {
                "enqueued": sequence - 1,
                "acknowledged": len(client.acks),
                "failed": len(failed),
                "dropped": sum(client.drops.values()),
                "abandoned": sum(client.abandoned.values()),
            },
            "drops": dict(client.drops),
            "abandoned": dict(client.abandoned),
            "first_failure": failure,
            "outbox_high_water": client.outbox_high_water,
            "forward_stop": self.forward_stop,
            "intent_gates": {
                name: asdict(phase) for name, phase in self.phases.items() if name != "readiness"
            },
            "digests": dict(self.digests),
            "writer_reported": {
                "subjects-before": (client.readiness_ack or {}).get("subjects_before"),
            },
        }
        self.result_snapshot = section
        return section

    # Close -------------------------------------------------------------------

    def close(self) -> None:
        """Request the close transaction and drive turns until the writer settles.

        storage-publication-k168 bounds this wait. A stalled writer can still
        hold it in this slice. A dropped close frame fails storage, so a live
        writer reads the end of its frame pipe and exits.
        """
        client = self.client
        seq: int | None = None
        if client.storage == "ready":
            seq = client.request_close()
            if seq is None:
                # A ready client returns None only when the outbox cap drops the frame.
                self._fail("outbox_drop", "close", "close", None)
        run_turns(client.turn(lambda: False))
        if seq is not None:
            self.close_ack = client.ack(seq)

    def finish(self) -> None:
        """End an unsettled writer after an exception: fail storage, KILL, reap."""
        client = self.client
        if client.pid is None or client.reaped_at is not None or client.residual:
            return
        client.fail("controller_exception")
        client.kill()
        run_turns(client.turn(lambda: True))

    def outcome(self) -> StorageOutcome:
        client = self.client
        self.healthy()
        if client.residual:
            publication = "residual"
        elif client.killed_at is not None:
            publication = "killed"
        elif client.storage == "closed" and client.reaped_at is not None:
            publication = "complete"
        else:
            publication = "incomplete"
        return StorageOutcome(
            readiness=client.readiness,
            readiness_reason=client.readiness_reason,
            first_failure=client.first_failure,
            forward_stop=self.forward_stop,
            publication=publication,
            writer_reaped=client.reaped_at is not None,
            exit_status=client.exit_status,
            residual=client.residual,
            killed_at=client.killed_at,
            reaped_at=client.reaped_at,
            drops=dict(client.drops),
            abandoned=dict(client.abandoned),
            phases=dict(self.phases),
            snapshot=self.result_snapshot,
            close_ack=self.close_ack,
        )


# Live runs, by run directory. `write_json` sends their records to the writer.
_run_storage: dict[Path, _RunStorage] = {}


def execute(plan: Plan, evidence_root: Path) -> RunDirectory:
    """Execute once, stop cases at first failure, always attempt owned teardown.

    Register intent before acquisition: even a failed start can leave a clone.
    Callers must use only fresh, run-named resources and trusted public commands.
    Every run record goes through the run's writer. The returned path carries
    the storage outcome in its `storage` attribute.
    """
    _validate(plan)
    root = evidence_root.resolve() / plan.run_id
    if os.path.lexists(root):
        raise FileExistsError(errno.EEXIST, os.strerror(errno.EEXIST), str(root))
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
                Path(__file__).with_name("turns.py").resolve(),
                Path(__file__).with_name("retention.py").resolve(),
                Path(storage_client.__file__).resolve(),
                Path(writer.__file__).resolve(),
                Path(sys.executable).resolve(),
                Path("/bin/ps"),
                # The writer hashes from its working directory "/".
                *(subject.absolute() for subject in plan.subjects),
                *(Path(call.argv[0]).resolve() for call in all_calls),
            )
        )
    )
    storage = _RunStorage(plan, root)
    _run_storage[root] = storage
    try:
        before = storage.start(_json_bytes(asdict(plan)), subjects)
        receipts: list[dict[str, object]] = []
        worker = Worker(
            run_id=plan.run_id,
            bootstrap=plan.bootstrap,
            failure_bounds=plan.failure_bounds,
            close_bounds=plan.close_bounds,
        )
        inspection_state = _InspectionState(worker=worker, storage=storage)
        ledger: list[dict[str, object]] = []
        attempted: list[Resource] = []
        unreached = [resource.acquire.name for resource in plan.resources] + [
            call.name for call in plan.calls
        ]
        admission = fork_admission()
        ok = (
            storage.healthy()
            and admission["ok"]
            and not any(value.startswith("unavailable:") for value in before.values())
        )
        try:
            if ok:
                ok = worker.start()
            for index, resource in enumerate(plan.resources, start=1):
                if not ok:
                    break
                # The resource enters teardown only after its intent is stored.
                # A resource whose gate fails is never acquired.
                if not storage.gate(index, _json_bytes(asdict(resource)), inspection_state.drive):
                    ok = False
                    break
                attempted.append(resource)
                unreached.remove(resource.acquire.name)
                receipt = _safe_call(resource.acquire, root, plan, inspection_state)
                receipts.append(receipt)
                ok = receipt["ok"] is True
            for call in plan.calls:
                if not ok or not storage.healthy():
                    ok = False
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
                inspection_state.close()
        ok = ok and worker.failure is None
        after = storage.subjects_after(subjects)
        ok = ok and before == after
        # The snapshot is taken before the verdict uses it: a failure from here
        # on cannot reach the stored result, only the in-memory outcome.
        ok = ok and storage.healthy()
        result: dict[str, object] = {
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
            "storage": storage.snapshot(),
        }
        storage.record("result", _json_bytes(result))
        storage.close()
    finally:
        _run_storage.pop(root, None)
        storage.finish()
    directory = RunDirectory(root)
    directory.storage = storage.outcome()
    return directory
