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
from typing import Literal, NotRequired, TypedDict, cast

import storage_client
import writer
from inspection import fork_admission
from retention import IDENTITY, STORAGE_CLOSE
from storage_client import OUTBOX_CAP, READINESS_CEILING, REAP_GRACE, WRITER, StorageClient
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

# docs/specs/diagnostic-evidence-storage.md, "Phases and bounds": the declared
# publication window W is at most 10 seconds.
PUBLICATION_CEILING = 10.0

# docs/specs/diagnostic-evidence-storage.md, "Workload and teardown
# descriptors": a turn reads at most 64 KiB from each stream. The final drain
# after leader reap makes at most 16 such reads for each stream.
STREAM_READ = 64 * 1024
FINAL_READS = 16


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
    publication_window: float = PUBLICATION_CEILING
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


def _validate(plan: Plan, evidence_root: Path | None = None) -> None:
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
    window = plan.publication_window
    if not (math.isfinite(window) and 0 < window <= PUBLICATION_CEILING):
        raise ValueError(f"the publication window must be in (0, {PUBLICATION_CEILING:g}] s")
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
        if evidence_root is not None:
            _refuse_evidence_executables(resource, evidence_root)
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


def _refuse_evidence_executables(resource: Resource, evidence_root: Path) -> None:
    """Refuse a teardown executable that is relative or under the evidence root.

    The comparison is lexical. `os.path.abspath` joins the working directory
    and normalises, and it reads nothing under the evidence root.
    https://docs.python.org/3.12/library/os.path.html#os.path.abspath
    POSIX lets `abspath` keep exactly two leading slashes, and `commonpath`
    always returns one. The root takes the `commonpath` spelling so that the
    two sides of the comparison use the same form.
    https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap04.html#tag_04_16
    """
    root = os.path.commonpath([os.path.abspath(evidence_root)])
    for call in (resource.cleanup, resource.dispose, resource.inspect):
        executable = call.argv[0] if call.argv else ""
        if not os.path.isabs(executable):
            raise ValueError("a teardown executable must be an absolute path")
        if os.path.commonpath([os.path.normpath(executable), root]) == root:
            raise ValueError("a teardown executable must not lie under the evidence root")


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
    # The exception type of a raised error, beside its message in `error`.
    error_type: NotRequired[str]


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
            self.receipt["reap_error_type"] = type(error).__name__
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
                observation["error_type"] = type(error).__name__
        if time.monotonic() >= deadline:
            raise TimeoutError("group inspection deadline exhausted after probe")
    except (OSError, ValueError) as error:
        observation["state"] = "unknown"
        observation["members"] = []
        observation["error"] = str(error)
        observation["error_type"] = type(error).__name__
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


class _Stream:
    """One captured stream: its read end and the bytes read so far."""

    def __init__(self, record: str, fd: int) -> None:
        self.record = record
        self.fd: int | None = fd
        self.data = bytearray()
        self.digest = hashlib.sha256()
        self.eof = False
        self.reads = 0
        self.final_reads = 0
        self.completeness: Literal["eof", "writer_open", "cutoff"] | None = None


class _Capture:
    """Drain one call's stdout and stderr pipes and send the bytes to the writer.

    The controller keeps only the nonblocking read ends. `os.pipe()` makes
    both ends non-inheritable (PEP 446), and `Popen` duplicates the write
    ends onto descriptors 1 and 2 in the child only.
    https://docs.python.org/3.12/library/os.html#os.pipe
    """

    def __init__(self, name: str, storage: _RunStorage | None) -> None:
        self.storage = storage
        self.streams: dict[str, _Stream] = {}
        self.write_ends: list[int] = []
        try:
            for stream in ("stdout", "stderr"):
                read_end, write_end = os.pipe()
                self.streams[stream] = _Stream(f"{name}.{stream}", read_end)
                self.write_ends.append(write_end)
                os.set_blocking(read_end, False)
        except OSError:
            self.close()
            raise
        for captured in self.streams.values():
            self._send(captured, b"", create=True)

    def _send(self, captured: _Stream, chunk: bytes, *, create: bool = False) -> None:
        if self.storage is not None:
            self.storage.stream(captured.record, chunk, create=create)

    def close_write_ends(self) -> None:
        """Close the controller's write ends, so EOF depends only on the call."""
        while self.write_ends:
            os.close(self.write_ends.pop())

    def _read(self, captured: _Stream) -> Literal["data", "again", "eof"]:
        assert captured.fd is not None
        try:
            chunk = os.read(captured.fd, STREAM_READ)
        except BlockingIOError:
            return "again"
        if not chunk:
            captured.eof = True
            return "eof"
        captured.data += chunk
        captured.digest.update(chunk)
        self._send(captured, chunk)
        return "data"

    def drain(self) -> None:
        """Make at most one read of each open stream."""
        for captured in self.streams.values():
            if captured.fd is not None and not captured.eof:
                captured.reads += 1
                self._read(captured)

    def advance(self) -> float:
        self.drain()
        return time.monotonic() + 0.005

    def readiness(self) -> tuple[tuple[int, ...], tuple[int, ...]]:
        reads = tuple(
            captured.fd
            for captured in self.streams.values()
            if captured.fd is not None and not captured.eof
        )
        return reads, ()

    def final_drain(self) -> None:
        """Read each stream until EOF or EAGAIN, at most 16 times, and close it.

        The drain never waits. EAGAIN means a descendant still holds a write
        end, so the stream is `writer_open`. The read limit gives `cutoff`.
        """
        for captured in self.streams.values():
            if captured.fd is None:
                continue
            outcome: Literal["data", "again", "eof"] = "eof" if captured.eof else "data"
            while outcome == "data" and captured.final_reads < FINAL_READS:
                captured.final_reads += 1
                outcome = self._read(captured)
            captured.completeness = (
                "eof" if outcome == "eof" else "writer_open" if outcome == "again" else "cutoff"
            )
            os.close(captured.fd)
            captured.fd = None

    def close(self) -> None:
        """Close every descriptor that remains, also after an error.

        Each stream record gets its digest entry from the bytes in memory.
        """
        self.close_write_ends()
        for captured in self.streams.values():
            if captured.fd is not None:
                os.close(captured.fd)
                captured.fd = None
            if self.storage is not None:
                self.storage.digests[captured.record] = _pin(bytes(captured.data))

    def receipts(self) -> dict[str, dict[str, object]]:
        return {
            stream: {
                "record": captured.record,
                "length": len(captured.data),
                "sha256": captured.digest.hexdigest(),
                "completeness": captured.completeness,
                "reads": captured.reads,
                "final_reads": captured.final_reads,
            }
            for stream, captured in self.streams.items()
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
    # The active call's streams. Its turn drains the pipes beside the command.
    capture: _Capture | None = None
    # The null device for command input, opened before readiness. A command
    # start after readiness then opens no file in the controller.
    null: int | None = None

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
        if not shutdown and self.storage is None and self.capture is None:
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
        capture = self.capture
        if capture is not None:

            def advance_capture() -> float | None:
                if finished:
                    return None
                return capture.advance()

            turns.append(Turn(advance_capture, readiness=capture.readiness))
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
            self.receipt["exit_observation_error_type"] = type(error).__name__

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
            event.update(
                errno=error.errno,
                error=str(error),
                error_type=type(error).__name__,
                outcome="signal_error",
            )
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
    capture = _Capture(call.name, inspection.storage)
    inspection.capture = capture
    try:
        try:
            # Separate session isolates the command CLI from this controller.
            # The controller drains the pipes in its own turns and never waits
            # for EOF, so a descendant that holds a write end cannot hold it.
            # https://docs.python.org/3.12/library/subprocess.html#subprocess.Popen
            process = _WorkloadProcess(
                call.argv,
                stdin=subprocess.DEVNULL if inspection.null is None else inspection.null,
                stdout=capture.write_ends[0],
                stderr=capture.write_ends[1],
                cwd="/",
                start_new_session=True,
            )
        except OSError as error:
            receipt["error"] = str(error)
            receipt["error_type"] = type(error).__name__
            process = None
        finally:
            capture.close_write_ends()
        if process is not None:
            _supervise(call, plan, inspection, process, receipt)
        # The final drain follows the termination decision and leader reap.
        # It reads what the leader left in the pipes, and it never waits.
        capture.final_drain()
    finally:
        inspection.capture = None
        capture.close()
    streams = capture.receipts()
    receipt["streams"] = streams
    if any(stream["completeness"] != "eof" for stream in streams.values()):
        receipt["ok"] = False
    if call.expected_stdout is not None:
        # Judge exactly the bytes in memory, never a readback.
        receipt["stdout_matches"] = bytes(capture.streams["stdout"].data) == call.expected_stdout
        receipt["ok"] = receipt["ok"] is True and receipt["stdout_matches"] is True
    receipt["elapsed"] = time.monotonic() - start
    write_json(root / f"{call.name}.json", receipt)
    return receipt


def _supervise(
    call: Call,
    plan: Plan,
    inspection: _InspectionState,
    process: _WorkloadProcess,
    receipt: dict[str, object],
) -> None:
    """Wait for the leader, decide termination, and reap the leader."""
    start = cast(float, receipt["started_monotonic"])
    _workload_owners[process.pid] = process
    receipt.update(pid=process.pid, pgid=process.pid)
    receipt["signal_scope"] = "pid" if call.interrupt_after is not None else "group"
    deadline = start + (call.interrupt_after or call.budget)
    try:
        exited = _wait(process.pid, deadline, drive=inspection.drive)
    except OSError as error:
        receipt["reason"] = "observation_error"
        receipt["error"] = str(error)
        receipt["error_type"] = type(error).__name__
        exited = False
    if exited:
        receipt["reason"] = "exit"
        leader_exit = time.monotonic()
        receipt["leader_exit_monotonic"] = leader_exit
        reap_deadline = leader_exit + plan.kill_wait
        inspection_deadline = reap_deadline - min(0.1, plan.kill_wait / 4)
        receipt["inspection_deadline_monotonic"] = inspection_deadline
        group = inspection.snapshot(call.name, process.pid, inspection_deadline, leader_exit)
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
            "error_type": type(error).__name__,
        }


@dataclass(frozen=True)
class StoragePhase:
    """One storage phase: its absolute start and deadline, end and outcome."""

    started: float
    deadline: float
    finished: float | None
    outcome: str


@dataclass(frozen=True)
class StorageOutcome:
    """The terminal storage section that the controller reports in memory.

    docs/specs/diagnostic-evidence-storage.md, "Terminal outcome". The
    `publication` field is one of these values:

    - `complete`: an `ok` close acknowledgement arrived before P, and the
      controller reaped the writer.
    - `incomplete`: the close acknowledgement reported an error or never
      arrived, and the writer exited before P.
    - `killed`: the controller sent KILL to the writer, and reaped it. In a
      run that returns, the KILL came at P.
    - `residual`: the writer was unreaped at P plus 2 seconds.

    `identity` holds the length and SHA-256 digest of the identity record
    from an `ok` close acknowledgement. A caller can anchor that digest.
    """

    readiness: str
    readiness_reason: str | None
    first_failure: str | None
    forward_stop: str | None
    publication: str
    writer_reaped: bool
    exit_status: int | None
    residual: bool
    residual_pid: int | None
    killed_at: float | None
    reaped_at: float | None
    drops: dict[str, int]
    abandoned: dict[str, int]
    frames: dict[str, int]
    phases: dict[str, StoragePhase]
    snapshot: dict[str, object] | None
    close_ack: dict[str, object] | None
    identity: dict[str, object] | None


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
        # The publication phase, with S as its start and P = S + W as its deadline.
        self.publication: StoragePhase | None = None
        # True when the controller ended the writer with KILL: at P, or after
        # an exception. A KILL after a readiness failure does not count.
        self.controller_kill = False

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

    def stream(self, name: str, chunk: bytes, *, create: bool = False) -> None:
        """Send one stream chunk as a create or an append frame.

        A chunk holds at most 64 KiB, so it fits one frame. A drop fails
        storage. The capture keeps the bytes in memory in every case.
        """
        seq = self.client.enqueue("create" if create else "append", name, chunk)
        if seq is None:
            if self.client.drops.get(name):
                self._fail("outbox_drop", "create" if create else "append", name, None)
            self._stop()
            return
        self.records[seq] = name

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

    def begin_publication(self) -> StoragePhase:
        """Start publication at S, once, and return its phase with P = S + W.

        The controller calls this after the last teardown action and the
        settled worker close. A direct call of `subjects_after` or `close`
        starts the phase itself.
        """
        if self.publication is None:
            started = time.monotonic()
            deadline = started + self.plan.publication_window
            self.publication = StoragePhase(started, deadline, None, "pending")
            self.phases["publication"] = self.publication
        return self.publication

    def subjects_after(self, subjects: tuple[Path, ...]) -> dict[str, str]:
        """Request the subjects-after digests and wait for them until S + W/2.

        That instant is the only cutoff. A digest that has not arrived by
        then is `unavailable:storage`.
        """
        phase = self.begin_publication()
        cutoff = phase.started + (phase.deadline - phase.started) / 2
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
        run_turns(
            self._bounded_turn(
                lambda: client.ack(seq) is not None or client.storage != "ready", cutoff
            )
        )
        # As at the intent gate, the turn ends at the first turn that reads the
        # acknowledgement. A read after the cutoff is late.
        ack = client.ack(seq) if time.monotonic() <= cutoff else None
        digests = ack.get("subjects") if ack is not None and ack.get("status") == "ok" else None
        if not isinstance(digests, dict) or set(cast(dict[str, object], digests)) != set(names):
            return unavailable
        return cast(dict[str, str], digests)

    def snapshot(self) -> dict[str, object]:
        """Return the result's storage section, as of the result's own sequence."""
        client = self.client
        sequence = client.next_seq
        enqueued = time.monotonic()
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
            "frames": self._frame_counts(),
            "drops": dict(client.drops),
            "abandoned": dict(client.abandoned),
            "first_failure": failure,
            "outbox_high_water": client.outbox_high_water,
            "forward_stop": self.forward_stop,
            "intent_gates": {
                name: asdict(phase)
                for name, phase in self.phases.items()
                if name.startswith("intent-")
            },
            "digests": dict(self.digests),
            "writer_reported": {
                "subjects-before": (client.readiness_ack or {}).get("subjects_before"),
            },
        }
        self.result_snapshot = section
        return section

    def _frame_counts(self) -> dict[str, int]:
        client = self.client
        failed = [ack for ack in client.acks.values() if ack.get("status") != "ok"]
        return {
            "enqueued": client.next_seq - 1,
            "acknowledged": len(client.acks),
            "failed": len(failed),
            "dropped": sum(client.drops.values()),
            "abandoned": sum(client.abandoned.values()),
        }

    def _bounded_turn(self, until: Callable[[], bool], deadline: float) -> Turn:
        """Return a client turn that ends when `until` holds or at `deadline`.

        Before the deadline, the wake never passes it, so the turn ends at the
        deadline and not at the next idle wake. Due reap work still runs.
        """
        inner = self.client.turn(lambda: until() or time.monotonic() >= deadline)

        def advance() -> float | None:
            wake = inner.advance()
            if wake is None or time.monotonic() >= deadline:
                return wake
            return min(wake, deadline)

        return Turn(advance, readiness=self.client.interests)

    # Close -------------------------------------------------------------------

    def close(self) -> None:
        """Send one close frame and run publication to its terminal outcome.

        The turns drain the outbox and read acknowledgements until P. A writer
        whose `ok` close acknowledgement a turn read before P gets no KILL, and
        turns reap it by P plus 2 seconds. A read at or after P is late, so it
        cannot make publication complete. At P, an unfinished writer receives
        KILL, with the same reap
        bound. An unreaped writer at P plus 2 seconds becomes a residual, so it
        never blocks the controller. A dropped close frame fails storage, and a
        live writer then reads the end of its frame pipe and exits.
        """
        phase = self.begin_publication()
        client = self.client
        seq: int | None = None
        if client.storage == "ready":
            seq = client.request_close()
            if seq is None:
                # A ready client returns None only when the outbox cap drops the frame.
                self._fail("outbox_drop", "close", "close", None)
        run_turns(self._bounded_turn(lambda: False, phase.deadline))
        reap_deadline = phase.deadline + REAP_GRACE
        if client.pid is not None and client.reaped_at is None and not client.residual:
            if self._closed_before_deadline():
                client.expect_exit(reap_deadline)
            elif client.killed_at is None:
                client.kill(reap_deadline)
                self.controller_kill = client.killed_at is not None
            run_turns(client.turn(lambda: True))
        if seq is not None:
            self.close_ack = client.ack(seq)
        self.publication = StoragePhase(
            phase.started, phase.deadline, time.monotonic(), self._publication_outcome()
        )
        self.phases["publication"] = self.publication

    def finish(self) -> None:
        """End an unsettled writer after an exception: fail storage, KILL, reap."""
        client = self.client
        if client.pid is None or client.reaped_at is not None or client.residual:
            return
        client.fail("controller_exception")
        if client.killed_at is None:
            client.kill()
            self.controller_kill = client.killed_at is not None
        run_turns(client.turn(lambda: True))

    def _closed_before_deadline(self) -> bool:
        """Return whether a turn read the `ok` close acknowledgement before P."""
        client = self.client
        phase = self.publication
        return (
            phase is not None
            and client.storage == "closed"
            and client.closed_at is not None
            and client.closed_at < phase.deadline
        )

    def _publication_outcome(self) -> str:
        client = self.client
        if client.residual:
            return "residual"
        if self.controller_kill:
            return "killed"
        if self._closed_before_deadline() and client.reaped_at is not None:
            return "complete"
        return "incomplete"

    def outcome(self) -> StorageOutcome:
        client = self.client
        self.healthy()
        close_ack = self.close_ack
        identity: dict[str, object] | None = None
        # A late close acknowledgement stays in the section, but no caller can
        # anchor an identity from it.
        if close_ack is not None and self._closed_before_deadline():
            pin = close_ack.get("identity")
            if isinstance(pin, dict):
                identity = dict(cast(dict[str, object], pin))
        return StorageOutcome(
            readiness=client.readiness,
            readiness_reason=client.readiness_reason,
            first_failure=client.first_failure,
            forward_stop=self.forward_stop,
            publication=self._publication_outcome(),
            writer_reaped=client.reaped_at is not None,
            exit_status=client.exit_status,
            residual=client.residual,
            residual_pid=client.pid if client.residual else None,
            killed_at=client.killed_at,
            reaped_at=client.reaped_at,
            drops=dict(client.drops),
            abandoned=dict(client.abandoned),
            frames=self._frame_counts(),
            phases=dict(self.phases),
            snapshot=self.result_snapshot,
            close_ack=close_ack,
            identity=identity,
        )


# Live runs, by run directory. `write_json` sends their records to the writer.
_run_storage: dict[Path, _RunStorage] = {}


@dataclass(frozen=True)
class RunOutcome:
    """The run outcome that the controller returns to its caller in memory.

    docs/specs/diagnostic-evidence-storage.md, "Terminal outcome". `result`
    is the result record as the controller serialised it, so its verdict and
    its storage section are snapshots. `verdict` is the overall verdict. It
    requires a passing result and the publication outcome `complete`.
    `residuals` names each owned process that the run left behind, including
    a residual inspection worker and a residual writer. A caller learns the
    safety outcome without reading evidence back.
    """

    run_dir: RunDirectory
    verdict: bool
    result: dict[str, object]
    ledger: list[dict[str, object]]
    residuals: list[dict[str, object]]
    storage: StorageOutcome


def _residuals(
    receipts: list[dict[str, object]],
    worker: dict[str, object],
    storage: StorageOutcome,
) -> list[dict[str, object]]:
    """Name each owned process that the run left behind.

    `worker` is the inspection worker's evidence. Its residual remedy is set
    only after a finished close that left the exact child unreaped.
    """
    found: list[dict[str, object]] = []
    for receipt in receipts:
        if "residual_leader_pid" in receipt:
            found.append(
                {"kind": "leader", "call": receipt["name"], "pid": receipt["residual_leader_pid"]}
            )
        if "residual_pgid" in receipt:
            found.append(
                {
                    "kind": "group",
                    "call": receipt["name"],
                    "pgid": receipt["residual_pgid"],
                    "members": receipt.get("residual_members"),
                }
            )
    owner = worker.get("owner")
    remedy = worker.get("residual_remedy")
    if remedy is not None and isinstance(owner, dict):
        pid = cast(dict[str, object], owner).get("pid")
        found.append({"kind": "worker", "pid": pid, "remedy": remedy})
    if storage.residual:
        found.append({"kind": "writer", "pid": storage.residual_pid})
    return found


def execute(plan: Plan, evidence_root: Path) -> RunDirectory:
    """Execute once, stop cases at first failure, always attempt owned teardown.

    Register intent before acquisition: even a failed start can leave a clone.
    Callers must use only fresh, run-named resources and trusted public commands.
    Every run record goes through the run's writer. The returned path carries
    the storage outcome in its `storage` attribute. `execute_outcome` returns
    the full run outcome.
    """
    return execute_outcome(plan, evidence_root).run_dir


def execute_outcome(plan: Plan, evidence_root: Path) -> RunOutcome:
    """Execute once, as `execute` does, and return the terminal run outcome."""
    _validate(plan, evidence_root)
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
    # P0 opens the null device for command input, so no command start after
    # readiness opens a file in the controller.
    null = os.open(os.devnull, os.O_RDONLY)
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
        inspection_state = _InspectionState(worker=worker, storage=storage, null=null)
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
        # P3 starts after the last teardown action and the settled worker close.
        storage.begin_publication()
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
        os.close(null)
    directory = RunDirectory(root)
    outcome = storage.outcome()
    directory.storage = outcome
    return RunOutcome(
        run_dir=directory,
        verdict=result["ok"] is True and outcome.publication == "complete",
        result=result,
        ledger=ledger,
        residuals=_residuals(
            receipts, cast(dict[str, object], result["inspection_worker"]), outcome
        ),
        storage=outcome,
    )
