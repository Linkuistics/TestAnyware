"""Persistent and standalone workers with gated helper creation publication.

Controller runs reuse one direct child. Shared scheduling remains open;
interrupted unpublished creation is never absence.
"""

from __future__ import annotations

import errno
import json
import os
import select
import signal
import struct
import time
import uuid
from dataclasses import asdict, dataclass, field, fields
from typing import cast

import inspection
from inspection import ForkAdmission, HelperCapture

FRAME_CAP = 1024 * 1024
TURN_CAP = 64 * 1024
HEADER = struct.Struct("!I")


@dataclass(frozen=True)
class BootstrapBounds:
    readiness: float = 1
    term_grace: float = 2
    kill_reap: float = 2

    def validate(self) -> None:
        for name, ceiling in (("readiness", 1), ("term_grace", 2), ("kill_reap", 2)):
            value = getattr(self, name)
            if type(value) not in {int, float} or not 0 < value <= ceiling:
                raise ValueError(f"bootstrap {name} must be positive and at most {ceiling} seconds")


@dataclass(frozen=True)
class FailureBounds:
    term_grace: float = 2
    kill_reap: float = 2

    def validate(self) -> None:
        for name in ("term_grace", "kill_reap"):
            value = getattr(self, name)
            if type(value) not in {int, float} or not 0 < value <= 2:
                raise ValueError(f"failure {name} must be positive and at most 2 seconds")


@dataclass(frozen=True)
class CloseBounds:
    grace: float = 0.05
    kill_reap: float = 2

    def validate(self) -> None:
        for name in ("grace", "kill_reap"):
            value = getattr(self, name)
            if type(value) not in {int, float} or not 0 < value <= 2:
                raise ValueError(f"close {name} must be positive and at most 2 seconds")


@dataclass(frozen=True)
class RequestScope:
    workload_pid: int
    workload_pgid: int
    signal_epoch: int
    phase: str


def _request_identity(value: object) -> dict[str, object]:
    """Validate types before equality: JSON true must never match epoch 1."""
    if not isinstance(value, dict):
        raise ValueError("invalid request identity object")
    raw = cast(dict[str, object], value)
    strings = {"run_id", "worker_token", "request_token", "phase"}
    integers = {"worker_pid", "workload_pid", "workload_pgid", "signal_epoch"}
    if set(raw) != strings | integers | {"deadline_monotonic"}:
        raise ValueError("invalid request identity fields")
    for key in strings:
        item = raw[key]
        if not isinstance(item, str) or not item.strip() or len(item) > 256 or "\x00" in item:
            raise ValueError("invalid request identity text")
    for key in integers:
        item = raw[key]
        if type(item) is not int or item < (0 if key == "signal_epoch" else 1):
            raise ValueError("invalid request identity integer")
    end = raw["deadline_monotonic"]
    if (
        type(end) not in {int, float}
        or not isinstance(end, int | float)
        or not 0 < end < float("inf")
    ):
        raise ValueError("invalid request identity deadline")
    return raw


def _check_identity(value: object, expected: dict[str, object]) -> None:
    if _request_identity(value) != expected:
        raise ValueError("request identity mismatch")


@dataclass
class _Owner:
    pid: int
    shutdown_deadline_monotonic: float
    ready: bool = False
    reaped: bool = False
    ownership_lost: bool = False
    returncode: int | None = None
    signals: list[dict[str, object]] = field(default_factory=list[dict[str, object]])
    reap_errno: int | None = None


_owners: dict[int, _Owner] = {}
# Cleanup obligations are evidence, never authority to signal a non-child PID.
# Only a matching helper reap receipt retires one; worker exit does not.
_helper_obligations: dict[str, dict[str, object]] = {}


def _reap(owner: _Owner) -> None:
    if owner.reaped or owner.reap_errno is not None:
        return
    # Residual ownership pins identity; it does not renew the fixed allowance.
    if time.monotonic() >= owner.shutdown_deadline_monotonic:
        return
    try:
        pid, status = os.waitpid(owner.pid, os.WNOHANG)
    except OSError as error:
        owner.reap_errno = error.errno
        if error.errno == errno.ECHILD:
            owner.ownership_lost = True
            _owners.pop(owner.pid, None)
        return
    if pid == owner.pid:
        owner.reaped = True
        owner.returncode = os.waitstatus_to_exitcode(status)
        _owners.pop(owner.pid, None)


def _signal(owner: _Owner, sig: signal.Signals) -> None:
    # The unreaped direct child pins the identity; never poll inside a signal.
    if owner.reaped or owner.reap_errno is not None:
        return
    now = time.monotonic()
    if now >= owner.shutdown_deadline_monotonic:
        return
    event: dict[str, object] = {"signal": sig.name, "monotonic": now}
    try:
        if owner.ready:
            os.killpg(owner.pid, sig)
        else:
            os.kill(owner.pid, sig)
    except OSError as error:
        event["errno"] = error.errno
    owner.signals.append(event)


def _write(fd: int, data: bytes, deadline: float) -> None:
    while data:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("worker publication deadline exhausted")
        _, ready, _ = select.select([], [fd], [], min(0.005, remaining))
        if ready:
            try:
                count = os.write(fd, data[:TURN_CAP])
            except BlockingIOError:
                continue
            data = data[count:]


def _publish(
    control: int,
    result: int,
    token: str,
    argv: tuple[str, ...],
    deadline: float,
    *,
    identity: dict[str, object] | None = None,
) -> None:
    envelope = {"identity": identity} if identity is not None else {}
    intent = json.dumps({"intent": token, **envelope}).encode() + b"\n" if envelope else b"intent\n"
    _write(control, intent, deadline)

    def publish_created(identity: inspection.HelperIdentity) -> None:
        message = {**identity, "token": token, "deadline_monotonic": deadline, **envelope}
        _write(control, json.dumps(message, allow_nan=False).encode() + b"\n", deadline)

    def publish_fork_error(failure: inspection.ForkFailure) -> None:
        message = {**failure, "token": token, "deadline_monotonic": deadline, **envelope}
        _write(control, json.dumps(message, allow_nan=False).encode() + b"\n", deadline)

    capture = inspection.capture_helper(
        argv, deadline, publish_created=publish_created, publish_fork_error=publish_fork_error
    )
    body = asdict(capture)
    body.update(stdout=capture.stdout.hex(), stderr=capture.stderr.hex(), token=token, **envelope)
    payload = json.dumps(body, allow_nan=False).encode()
    if len(payload) > FRAME_CAP:
        raise ValueError("worker frame cap exceeded")
    _write(result, HEADER.pack(len(payload)) + payload, deadline)


def _child(control: int, result: int, token: str, argv: tuple[str, ...], deadline: float) -> None:
    # Never unwind this fork into inherited atexit hooks, owners or destructors.
    # https://docs.python.org/3.12/library/os.html#os._exit
    try:
        global _owners
        _owners = {}
        global _helper_obligations
        _helper_obligations = {}
        inspection.reset_after_fork({control, result})
        ready = {"token": token, "pid": os.getpid(), "pgid": os.getpgrp(), "sid": os.getsid(0)}
        _write(control, json.dumps(ready).encode() + b"\n", deadline)
        _publish(control, result, token, argv, deadline)
        os._exit(0)
    except BaseException:
        os._exit(70)


def _service(command: int, control: int, result: int, session: str, deadline: float) -> None:
    """Reset once, then handle one admitted request at a time without an ACK."""
    try:
        global _owners, _helper_obligations
        _owners, _helper_obligations = {}, {}
        inspection.reset_after_fork({command, control, result})
        identity = {
            "session": session,
            "pid": os.getpid(),
            "pgid": os.getpgrp(),
            "sid": os.getsid(0),
        }
        _write(control, json.dumps(identity).encode() + b"\n", deadline)
        pending = bytearray()
        run_id: object = None
        used_tokens: set[str] = set()
        while True:
            readable, _, _ = select.select([command], [], [], 0.05)
            if not readable:
                continue
            chunk = os.read(command, 4097 - len(pending))
            if not chunk:
                if pending:
                    raise ValueError("partial worker command at close")
                os._exit(0)
            pending.extend(chunk)
            if len(pending) > 4096:
                raise ValueError("worker command cap exceeded")
            if b"\n" not in pending:
                continue
            line, _, tail = pending.partition(b"\n")
            if tail:
                raise ValueError("queued worker commands forbidden")
            pending.clear()
            request = _load_json(line)
            if not isinstance(request, dict):
                raise ValueError("invalid worker command")
            raw = cast(dict[str, object], request)
            if (
                set(raw) != {"session", "token", "argv", "deadline", "identity"}
                or raw["session"] != session
            ):
                raise ValueError("worker command session mismatch")
            token, argv, end = raw["token"], raw["argv"], raw["deadline"]
            if (
                not isinstance(token, str)
                or not token
                or not isinstance(argv, list)
                or not argv
                or not all(
                    isinstance(arg, str) and arg and "\x00" not in arg
                    for arg in cast(list[object], argv)
                )
                or type(end) not in (int, float)
            ):
                raise ValueError("invalid worker command values")
            end = cast(float, end)
            if not time.monotonic() < end < float("inf"):
                raise ValueError("worker command deadline exhausted")
            identity = _request_identity(raw["identity"])
            if (
                identity["worker_token"] != session
                or identity["worker_pid"] != os.getpid()
                or identity["request_token"] != token
                or identity["deadline_monotonic"] != end
                or (run_id is not None and identity["run_id"] != run_id)
                or token in used_tokens
            ):
                raise ValueError("worker command identity mismatch")
            run_id = identity["run_id"]
            used_tokens.add(token)
            ready = {
                "token": token,
                "pid": os.getpid(),
                "pgid": os.getpgrp(),
                "sid": os.getsid(0),
                "identity": identity,
            }
            _write(control, json.dumps(ready).encode() + b"\n", end)
            _publish(control, result, token, tuple(cast(list[str], argv)), end, identity=identity)
            _write(
                control, json.dumps({"complete": token, "identity": identity}).encode() + b"\n", end
            )
    except BaseException:
        os._exit(70)


@dataclass
class Worker:
    """One controller-owned session. Failure and retirement never permit restart."""

    session_token: str = field(default_factory=lambda: uuid.uuid4().hex)
    run_id: str = field(default_factory=lambda: uuid.uuid4().hex)
    owner: _Owner | None = None
    failure: str | None = None
    channels: list[int] = field(default_factory=list[int])
    command: int | None = None
    control: int | None = None
    result: int | None = None
    started: bool = False
    closed: bool = False
    admission: ForkAdmission | None = None
    startup_started: float | None = None
    startup_finished: float | None = None
    shutdown_started: float | None = None
    shutdown_finished: float | None = None
    kill_at: float | None = None
    bootstrap: BootstrapBounds = field(default_factory=BootstrapBounds)
    failure_bounds: FailureBounds = field(default_factory=FailureBounds)
    close_bounds: CloseBounds = field(default_factory=CloseBounds)
    shutdown: dict[str, object] | None = None
    startup_deadline: float | None = None
    readiness_finished: float | None = None
    readiness_outcome: str | None = None
    bootstrap_shutdown: dict[str, object] | None = None

    def start(self) -> bool:
        if self.started or self.closed or self.failure is not None:
            return self.started and not self.closed and self.failure is None
        if not isinstance(cast(object, self.bootstrap), BootstrapBounds):
            raise ValueError("invalid bootstrap bounds")
        self.bootstrap.validate()
        self._validate_shutdown_bounds()
        self.startup_started = time.monotonic()
        deadline = self.startup_started + self.bootstrap.readiness
        self.startup_deadline = deadline
        try:
            self.admission = inspection.fork_admission()
            if not self.admission["ok"]:
                raise ValueError(f"worker startup admission rejected: {self.admission['reason']}")
            for _ in range(3):
                pair = inspection.capture_pipe()
                self.channels.extend(pair)
            command_read, command_write, control_read, control_write, result_read, result_write = (
                self.channels
            )
            blocked = signal.valid_signals() - {signal.SIGKILL, signal.SIGSTOP}
            previous = signal.pthread_sigmask(signal.SIG_BLOCK, blocked)
            try:
                self.admission = inspection.fork_admission()
                if not self.admission["ok"]:
                    raise ValueError(
                        f"worker startup admission rejected: {self.admission['reason']}"
                    )
                if time.monotonic() >= deadline:
                    raise TimeoutError("worker startup deadline exhausted before fork")
                # https://docs.python.org/3.12/library/os.html#os.fork
                pid = os.fork()
                if pid == 0:
                    _service(
                        command_read, control_write, result_write, self.session_token, deadline
                    )
                    os._exit(71)
                self.owner = _Owner(pid, float("inf"))
                _owners[pid] = self.owner
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous)
            for fd in (command_read, control_write, result_write):
                os.close(fd)
                self.channels.remove(fd)
            self.command, self.control, self.result = command_write, control_read, result_read
            pending = bytearray()
            while time.monotonic() < deadline:
                ready, _, _ = select.select(
                    [control_read], [], [], min(0.005, max(0, deadline - time.monotonic()))
                )
                if not ready:
                    continue
                chunk = os.read(control_read, 4097 - len(pending))
                pending.extend(chunk)
                if not chunk or len(pending) > 4096:
                    raise ValueError("invalid worker startup frame")
                if b"\n" in pending:
                    line, _, tail = pending.partition(b"\n")
                    if tail or _load_json(line) != {
                        "session": self.session_token,
                        "pid": pid,
                        "pgid": pid,
                        "sid": pid,
                    }:
                        raise ValueError("invalid worker startup identity")
                    self.readiness_finished = time.monotonic()
                    self.readiness_outcome = "late"
                    if self.readiness_finished >= deadline:
                        raise TimeoutError("late worker readiness")
                    self.readiness_outcome = "ready"
                    self.owner.ready = True
                    self.started = True
                    return True
            raise TimeoutError("worker readiness deadline exhausted")
        except (OSError, ValueError) as error:
            if self.readiness_finished is None:
                self.readiness_finished = time.monotonic()
            if self.readiness_outcome is None:
                self.readiness_outcome = "expired" if isinstance(error, TimeoutError) else "failed"
            self.failure = str(error)
            self.close()
            return False
        finally:
            if self.readiness_finished is None:
                self.readiness_finished = time.monotonic()
            self.startup_finished = time.monotonic()

    def _validate_shutdown_bounds(self) -> None:
        if not isinstance(cast(object, self.failure_bounds), FailureBounds):
            raise ValueError("invalid failure bounds")
        if not isinstance(cast(object, self.close_bounds), CloseBounds):
            raise ValueError("invalid close bounds")
        self.failure_bounds.validate()
        self.close_bounds.validate()

    def close(self) -> None:
        if self.closed:
            return
        self._validate_shutdown_bounds()
        self.closed = True
        self.shutdown_started = time.monotonic()
        owner = self.owner
        failed_bootstrap = (
            self.startup_started is not None and not self.started and self.failure is not None
        )
        # EOF is the orderly request. A failed close becomes failure shutdown
        # within this same timeline, never a fresh grace after the error.
        if self.command is not None and self.command in self.channels:
            try:
                os.close(self.command)
            except OSError as error:
                self.failure = self.failure or f"worker command close failed: {error}"
            self.channels.remove(self.command)
        old_end = owner.shutdown_deadline_monotonic if owner else float("inf")
        retiring = owner is not None and (old_end != float("inf") or bool(owner.signals))
        mode = (
            "orderly"
            if self.failure is None and not retiring and (owner is None or owner.ready)
            else "failure"
        )
        if failed_bootstrap:
            mode = "bootstrap"
            grace, reap_budget = self.bootstrap.term_grace, self.bootstrap.kill_reap
        elif mode == "orderly":
            grace, reap_budget = self.close_bounds.grace, self.close_bounds.kill_reap
        else:
            grace = self.failure_bounds.term_grace
            reap_budget = self.failure_bounds.kill_reap
        phase_start = self.shutdown_started
        term_sent = mode == "orderly"
        kill_sent = False
        prior_kill = None
        if owner is not None:
            for attempt in owner.signals:
                if attempt["signal"] == "SIGTERM":
                    term_sent = True
                    phase_start = min(phase_start, cast(float, attempt["monotonic"]))
                elif attempt["signal"] == "SIGKILL":
                    kill_sent = term_sent = True
                    attempt_time = cast(float, attempt["monotonic"])
                    prior_kill = (
                        attempt_time if prior_kill is None else min(prior_kill, attempt_time)
                    )
        # Reserve the declared reap allowance inside any earlier finite cutoff.
        # A prior attempted signal (including denial) never receives another grace.
        kill_at = (
            prior_kill
            if prior_kill is not None
            else min(
                old_end, phase_start + grace + 0.003, max(self.shutdown_started, old_end - reap_budget)
            )
        )
        self.kill_at = kill_at
        end = min(old_end, kill_at + reap_budget)
        grace_finished = prior_kill
        reap_started = prior_kill
        if owner is not None:
            owner.shutdown_deadline_monotonic = end
            while time.monotonic() < end:
                _reap(owner)
                if owner.reaped or owner.reap_errno is not None:
                    break
                now = time.monotonic()
                if not term_sent and now < end:
                    _signal(owner, signal.SIGTERM)
                    term_sent = True
                if not kill_sent and kill_at <= now < end:
                    attempts = len(owner.signals)
                    _signal(owner, signal.SIGKILL)
                    if len(owner.signals) > attempts:
                        # _signal rechecks the cutoff immediately before the
                        # syscall. A suppressed late call never entered this phase.
                        reap_started = cast(float, owner.signals[-1]["monotonic"])
                        grace_finished = reap_started
                        kill_sent = True
                time.sleep(min(0.001, max(0, end - time.monotonic())))
            _reap(owner)
            if not owner.reaped or owner.returncode != 0:
                self.failure = self.failure or "worker close did not return cleanly"
        for fd in self.channels:
            try:
                os.close(fd)
            except OSError as error:
                self.failure = self.failure or f"worker channel close failed: {error}"
        self.channels.clear()
        self.shutdown_finished = time.monotonic()
        overrun = self.shutdown_finished > end
        outcome = (
            "not_created" if owner is None else "reaped" if owner.reaped else "residual_ownership"
        )
        grace_finished = self.shutdown_finished if grace_finished is None else grace_finished
        phases = {}
        for name, start, cutoff, finish, result in (
            (
                "grace",
                None if prior_kill is not None else phase_start,
                kill_at,
                grace_finished,
                "already_retiring"
                if prior_kill is not None
                else "escalated"
                if reap_started is not None
                else outcome,
            ),
            (
                "kill_reap",
                reap_started,
                end,
                self.shutdown_finished,
                outcome if reap_started is not None else "not_entered",
            ),
        ):
            phases[name] = {
                "started_monotonic": start,
                "deadline_monotonic": cutoff,
                "finished_monotonic": finish,
                "outcome": result,
                "timing_outcome": (
                    "late" if finish > cutoff else "at_deadline" if finish == cutoff else "on_time"
                ),
                "overrun": finish > cutoff,
                "lateness_seconds": max(0.0, finish - cutoff),
                "deadline_exhausted": finish >= cutoff,
            }
        self.shutdown = {
            "mode": mode,
            "started_monotonic": self.shutdown_started,
            "term_deadline_monotonic": kill_at,
            "kill_deadline_monotonic": end,
            "finished_monotonic": self.shutdown_finished,
            "overrun": overrun,
            "lateness_seconds": max(0.0, self.shutdown_finished - end),
            "deadline_exhausted": self.shutdown_finished >= end,
            "outcome": outcome,
            "signals": list(owner.signals) if owner is not None else [],
            "phases": phases,
        }
        if grace_finished > kill_at and prior_kill is None:
            self.failure = f"{self.failure}; {mode} grace overrun"
        if failed_bootstrap:
            self.bootstrap_shutdown = self.shutdown
        if overrun:
            self.failure = f"{self.failure}; {mode} shutdown overrun"

    def evidence(self) -> dict[str, object]:
        return {
            "run_id": self.run_id,
            "session_token": self.session_token,
            "owner": asdict(self.owner) if self.owner else None,
            "failure": self.failure,
            "admission": self.admission,
            "bootstrap_bounds": asdict(self.bootstrap),
            "failure_bounds": asdict(self.failure_bounds),
            "close_bounds": asdict(self.close_bounds),
            "shutdown": self.shutdown,
            "startup_started_monotonic": self.startup_started,
            "startup_deadline_monotonic": self.startup_deadline,
            "readiness_finished_monotonic": self.readiness_finished,
            "readiness_outcome": self.readiness_outcome,
            "readiness_lateness_seconds": (
                max(0.0, self.readiness_finished - self.startup_deadline)
                if self.readiness_finished is not None and self.startup_deadline is not None
                else None
            ),
            "readiness_overrun": (
                self.readiness_finished > self.startup_deadline
                if self.readiness_finished is not None and self.startup_deadline is not None
                else None
            ),
            "startup_finished_monotonic": self.startup_finished,
            "bootstrap_shutdown": self.bootstrap_shutdown,
            "shutdown_started_monotonic": self.shutdown_started,
            "shutdown_finished_monotonic": self.shutdown_finished,
            "closed": self.closed,
            "residual_remedy": (
                "Retained exact worker child requires independent identity reconciliation and "
                "reap. Shutdown authority is closed; no replacement or later runtime signals."
                if self.closed and self.owner is not None and not self.owner.reaped
                else None
            ),
        }


def _load_json(payload: bytes | bytearray) -> object:
    """Treat duplicate keys and decoder stack exhaustion as protocol failures."""

    def unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
        value: dict[str, object] = {}
        for key, item in pairs:
            if key in value:
                raise ValueError("duplicate worker publication key")
            value[key] = item
        return value

    try:
        return json.loads(payload, object_pairs_hook=unique)
    except RecursionError as error:
        raise ValueError("worker publication nesting exceeds decoder limit") from error


def _admission(value: object) -> ForkAdmission:
    if not isinstance(value, dict):
        raise ValueError("invalid worker admission fields")
    raw = cast(dict[str, object], value)
    if set(raw) != set(ForkAdmission.__annotations__):
        raise ValueError("invalid worker admission fields")
    python_threads, native_threads = raw["python_threads"], raw["native_threads"]
    error, error_number = raw["error"], raw["errno"]
    if (
        type(raw["ok"]) is not bool
        or not isinstance(raw["reason"], str)
        or type(python_threads) is not int
        or python_threads < 1
        or (native_threads is not None and (type(native_threads) is not int or native_threads < 1))
        or raw["sigchld"] not in ("SIG_DFL", "SIG_IGN", "handler")
        or raw["native_sigchld"] not in (None, "SIG_DFL", "SIG_IGN", "handler", "SA_NOCLDWAIT")
        or (error is not None and (not isinstance(error, str) or not error.strip()))
        or (error_number is not None and (type(error_number) is not int or error_number <= 0))
    ):
        raise ValueError("invalid worker admission values")
    admission = cast(ForkAdmission, raw)
    if admission["reason"] == "native_observation_error":
        if admission["ok"] or error is None or admission["native_sigchld"] is not None:
            raise ValueError("inconsistent worker admission")
    else:
        reason = (
            "python_threads"
            if python_threads != 1
            else "native_threads"
            if native_threads != 1
            else "sigchld"
            if admission["sigchld"] != "SIG_DFL" or admission["native_sigchld"] != "SIG_DFL"
            else "ready"
        )
        if (
            native_threads is None
            or admission["native_sigchld"] is None
            or error is not None
            or error_number is not None
            or admission["reason"] != reason
            or admission["ok"] != (reason == "ready")
        ):
            raise ValueError("inconsistent worker admission")
    return admission


def _created(
    line: bytes | bytearray, owner: _Owner, token: str, dispatched: float, deadline: float
) -> dict[str, object]:
    decoded = _load_json(line)
    if not isinstance(decoded, dict):
        raise ValueError("invalid helper creation object")
    identity = cast(dict[str, object], decoded)
    if set(identity) != {"token", "pid", "ppid", "pgid", "created_monotonic", "deadline_monotonic"}:
        raise ValueError("invalid helper creation fields")
    stamp = identity["created_monotonic"]
    if (
        identity["token"] != token
        or type(identity["pid"]) is not int
        or identity["pid"] <= 0
        or identity["pid"] == owner.pid
        or type(identity["ppid"]) is not int
        or identity["ppid"] != owner.pid
        or type(identity["pgid"]) is not int
        or identity["pgid"] != owner.pid
        or type(identity["deadline_monotonic"]) not in {int, float}
        or identity["deadline_monotonic"] != deadline
        or type(stamp) not in {int, float}
        or not isinstance(stamp, int | float)
        or not dispatched <= stamp < deadline
        or not stamp <= time.monotonic()
    ):
        raise ValueError("invalid helper creation identity")
    return identity


def _fork_failed(
    line: bytes | bytearray, token: str, dispatched: float, deadline: float
) -> dict[str, object]:
    decoded = _load_json(line)
    if not isinstance(decoded, dict):
        raise ValueError("invalid helper fork failure object")
    failure = cast(dict[str, object], decoded)
    if set(failure) != {"token", "errno", "failed_monotonic", "deadline_monotonic"}:
        raise ValueError("invalid helper fork failure fields")
    stamp = failure["failed_monotonic"]
    if (
        failure["token"] != token
        or type(failure["errno"]) is not int
        or failure["errno"] <= 0
        or type(failure["deadline_monotonic"]) not in {int, float}
        or failure["deadline_monotonic"] != deadline
        or type(stamp) not in {int, float}
        or not isinstance(stamp, int | float)
        or not dispatched <= stamp <= time.monotonic() < deadline
    ):
        raise ValueError("invalid helper fork failure identity")
    return failure


def _decode(
    payload: bytes,
    token: str,
    argv: tuple[str, ...],
    deadline: float,
    dispatched: float,
    received: float,
    *,
    identity: dict[str, object] | None = None,
) -> HelperCapture:
    decoded = _load_json(payload)
    if not isinstance(decoded, dict):
        raise ValueError("invalid worker result object")
    raw = cast(dict[str, object], decoded)
    if identity is not None:
        _check_identity(raw.pop("identity", None), identity)
    if raw.pop("token", None) != token:
        raise ValueError("invalid worker result identity")
    if set(raw) != {item.name for item in fields(HelperCapture)}:
        raise ValueError("invalid worker result fields")
    receipt = HelperCapture()
    for key, value in raw.items():
        if key in {"stdout", "stderr"}:
            if not isinstance(value, str):
                raise ValueError("invalid worker output encoding")
            value = bytes.fromhex(value)
        elif key == "argv":
            if value != list(argv):
                raise ValueError("worker argv mismatch")
            value = argv
        elif key in {"reaped", "ownership_lost"}:
            if type(value) is not bool:
                raise ValueError("invalid worker boolean")
        elif key in {"returncode", "pid", "kill_errno", "reap_errno", "error_errno"}:
            if value is not None and type(value) is not int:
                raise ValueError("invalid worker integer")
            if key.endswith("errno") and isinstance(value, int) and value <= 0:
                raise ValueError("invalid worker errno")
        elif key.endswith("monotonic"):
            optional = key in {"created_monotonic", "kill_monotonic", "reaped_monotonic"}
            if not (optional and value is None) and (
                type(value) not in {int, float}
                or not isinstance(value, int | float)
                or not 0 <= value <= deadline
            ):
                raise ValueError("invalid worker timestamp")
        elif key in {"error", "residual_remedy"}:
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise ValueError("invalid worker error")
        elif key == "close_errors":
            if not isinstance(value, list) or not all(
                isinstance(item, str) and item.strip() for item in cast(list[object], value)
            ):
                raise ValueError("invalid worker close errors")
        elif key == "admission":
            value = _admission(value)
        elif key == "worker":
            if value is not None:
                raise ValueError("nested worker result")
        setattr(receipt, key, value)
    if len(receipt.stdout) + len(receipt.stderr) > 256 * 1024:
        raise ValueError("worker output cap exceeded")
    if receipt.deadline_monotonic != deadline:
        raise ValueError("worker deadline mismatch")
    if (
        not dispatched
        <= receipt.started_monotonic
        <= receipt.finished_monotonic
        <= received
        <= deadline
    ):
        raise ValueError("worker query interval outside dispatch/receipt allowance")
    if not receipt.started_monotonic <= receipt.capture_deadline_monotonic <= deadline:
        raise ValueError("worker capture cutoff outside query allowance")
    previous = receipt.started_monotonic
    for stamp in (receipt.created_monotonic, receipt.kill_monotonic, receipt.reaped_monotonic):
        if stamp is not None:
            if not previous <= stamp <= receipt.finished_monotonic:
                raise ValueError("worker helper timestamps out of order")
            previous = stamp
    if receipt.created_monotonic is None and (
        receipt.kill_monotonic is not None or receipt.reaped_monotonic is not None
    ):
        raise ValueError("worker helper event without creation timestamp")
    if receipt.reaped != (receipt.reaped_monotonic is not None):
        raise ValueError("worker reap state/timestamp mismatch")
    if (receipt.pid is not None) != (receipt.created_monotonic is not None):
        raise ValueError("worker pid/creation timestamp mismatch")
    if receipt.pid is not None and receipt.pid <= 0:
        raise ValueError("invalid worker helper pid")
    if receipt.kill_errno is not None and receipt.kill_monotonic is None:
        raise ValueError("worker kill errno without kill timestamp")
    capture_deadline = deadline - min(0.05, max(0, deadline - receipt.started_monotonic) / 4)
    if receipt.capture_deadline_monotonic != capture_deadline:
        raise ValueError("worker capture cutoff mismatch")
    if (receipt.returncode is not None) != receipt.reaped:
        raise ValueError("worker status/reap mismatch")
    if receipt.reaped and (receipt.reap_errno is not None or receipt.ownership_lost):
        raise ValueError("worker reaped helper has failed ownership")
    if receipt.admission is not None and not receipt.admission["ok"] and receipt.pid is not None:
        raise ValueError("worker helper created after admission refusal")
    if receipt.pid is None and (
        receipt.reap_errno is not None
        or receipt.ownership_lost
        or receipt.stdout
        or receipt.stderr
        or receipt.close_errors
    ):
        raise ValueError("worker helper failure without creation")
    if receipt.ownership_lost != (receipt.reap_errno == errno.ECHILD):
        raise ValueError("worker ECHILD/ownership mismatch")
    if receipt.error is None and (
        not receipt.reaped
        or receipt.error_errno is not None
        or receipt.kill_monotonic is not None
        or receipt.reap_errno is not None
        or receipt.close_errors
        or receipt.residual_remedy is not None
    ):
        raise ValueError("worker failure without error")
    retained = receipt.pid is not None and not receipt.reaped and not receipt.ownership_lost
    if (receipt.residual_remedy is not None) != retained:
        raise ValueError("worker residual ownership mismatch")
    return receipt


def capture_helper(
    argv: tuple[str, ...],
    deadline: float,
    *,
    worker: Worker | None = None,
    scope: RequestScope | None = None,
) -> HelperCapture:
    """Bound parent waiting even when the worker's helper creation never returns.

    The raw worker fork is admitted, but remains a synchronous OS operation.
    All channel waits and worker shutdown share the caller's original deadline.
    """
    started = time.monotonic()
    reserve = min(0.05, max(0, deadline - started) / 4)
    query_deadline = deadline - reserve
    receipt = HelperCapture(argv=argv, started_monotonic=started, deadline_monotonic=deadline)
    admission: ForkAdmission | None = (
        inspection.fork_admission() if worker is None else worker.admission
    )
    receipt.admission = admission
    if worker is not None and (not worker.started or worker.closed or worker.failure is not None):
        receipt.error = f"persistent worker unavailable: {worker.failure or 'not running'}"
        receipt.finished_monotonic = time.monotonic()
        return receipt
    if admission is None or not admission["ok"] or time.monotonic() >= query_deadline:
        reason = admission["reason"] if admission else "unavailable"
        receipt.error = f"worker not started: {reason} or exhausted allowance"
        if worker is not None:
            worker.failure = receipt.error
            if worker.owner is not None:
                worker.owner.shutdown_deadline_monotonic = min(
                    worker.owner.shutdown_deadline_monotonic, deadline
                )
            worker.close()
            if worker.owner is not None and not worker.owner.reaped:
                receipt.residual_remedy = (
                    "Request allowance exhausted; independently reconcile the retained exact "
                    "worker child after command EOF. No renewed signal/reap allowance."
                )
        receipt.finished_monotonic = time.monotonic()
        return receipt
    token = uuid.uuid4().hex
    channels: list[int] = []
    owner: _Owner | None = None
    intent = False
    complete = False
    dispatched: float | None = None
    received: float | None = None
    published_times: dict[str, float | None] | None = None
    helper_identity: dict[str, object] | None = None
    helper_identities: list[dict[str, object]] = []
    fork_failure: dict[str, object] | None = None
    creation_conflict = False
    closed_channels: set[int] = set()
    control_bytes = 0
    control_read: int | None = None
    control_ready = False
    control_invalid = False
    buffers = [bytearray(), bytearray()]
    result_size: int | None = None
    request_done = False
    request_identity: dict[str, object] | None = None

    def consume_control(cleanup_only: bool) -> None:
        nonlocal control_ready, control_invalid, intent, creation_conflict
        nonlocal helper_identity, fork_failure, request_done
        if control_invalid or owner is None or dispatched is None:
            return
        while b"\n" in buffers[0]:
            line, _, tail = buffers[0].partition(b"\n")
            buffers[0] = bytearray(tail)
            late = cleanup_only or time.monotonic() >= query_deadline
            try:
                if request_identity is not None:
                    decoded = _load_json(line)
                    if not isinstance(decoded, dict):
                        raise ValueError("invalid persistent control object")
                    message = dict(cast(dict[str, object], decoded))
                    _check_identity(message.pop("identity", None), request_identity)
                    line = json.dumps(message).encode()
                if not control_ready:
                    if _load_json(line) != {
                        "token": token,
                        "pid": owner.pid,
                        "pgid": owner.pid,
                        "sid": owner.pid,
                    }:
                        control_invalid = True
                        raise ValueError("invalid worker readiness")
                    control_ready = True
                    # Late readiness allows parsing only, never wider signals.
                    if not late:
                        owner.ready = True
                elif not intent:
                    if (
                        _load_json(line) != {"intent": token}
                        if request_identity is not None
                        else bytes(line) != b"intent"
                    ):
                        control_invalid = True
                        raise ValueError("invalid helper creation intent")
                    intent = True
                else:
                    message = _load_json(line)
                    if worker is not None and message == {"complete": token}:
                        if request_done:
                            raise ValueError("duplicate worker completion")
                        request_done = True
                        continue
                    if request_done:
                        raise ValueError("worker control after completion")
                    if helper_identity is not None or fork_failure is not None:
                        creation_conflict = True
                    message = _load_json(line)
                    if isinstance(message, dict) and "errno" in message:
                        # Timeliness remains mandatory for no-child authority.
                        if not late:
                            failure = _fork_failed(line, token, dispatched, query_deadline)
                            if not creation_conflict:
                                fork_failure = failure
                    else:
                        identity = _created(line, owner, token, dispatched, query_deadline)
                        if helper_identity is None:
                            helper_identity = identity
                        if not any(known["pid"] == identity["pid"] for known in helper_identities):
                            helper_identities.append(identity)
                            key = (
                                token
                                if identity is helper_identity
                                else f"{token}:{identity['pid']}"
                            )
                            _helper_obligations[key] = dict(identity)
                    if creation_conflict:
                        raise ValueError("duplicate helper creation publication")
            except ValueError:
                creation_conflict = True
                if not late:
                    raise
                if control_invalid:
                    break

    try:
        if worker is not None:
            owner = worker.owner
            if (
                owner is None
                or worker.command is None
                or worker.control is None
                or worker.result is None
            ):
                raise ValueError("persistent worker channels unavailable")
            owner.shutdown_deadline_monotonic = deadline
            scope = scope or RequestScope(os.getpid(), os.getpgrp(), 0, "helper")
            request_identity = _request_identity(
                {
                    "run_id": worker.run_id,
                    "worker_token": worker.session_token,
                    "worker_pid": owner.pid,
                    "request_token": token,
                    **asdict(scope),
                    "deadline_monotonic": query_deadline,
                }
            )
            control_read, result_read = worker.control, worker.result
            dispatched = time.monotonic()
            command = (
                json.dumps(
                    {
                        "session": worker.session_token,
                        "token": token,
                        "argv": argv,
                        "deadline": query_deadline,
                        "identity": request_identity,
                    },
                    allow_nan=False,
                ).encode()
                + b"\n"
            )
            if len(command) > 4096:
                raise ValueError("worker command cap exceeded")
            _write(worker.command, command, query_deadline)
        else:
            for _ in range(2):
                pair = os.pipe()
                channels.extend(pair)
                for fd in pair:
                    os.set_blocking(fd, False)
            control_read, control_write, result_read, result_write = channels
            blocked = signal.valid_signals() - {signal.SIGKILL, signal.SIGSTOP}
            previous = signal.pthread_sigmask(signal.SIG_BLOCK, blocked)
            try:
                # Revalidate after channel/mask preparation and retain that snapshot.
                admission = inspection.fork_admission()
                receipt.admission = admission
                if not admission["ok"]:
                    raise ValueError(f"worker fork admission rejected: {admission['reason']}")
                if time.monotonic() >= query_deadline:
                    raise ValueError("worker query deadline exhausted before creation")
                # https://docs.python.org/3.12/library/os.html#os.fork
                dispatched = time.monotonic()
                pid = os.fork()
                if pid == 0:
                    _child(control_write, result_write, token, argv, query_deadline)
                    os._exit(71)
                owner = _Owner(pid, deadline)
                _owners[pid] = owner
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, previous)
            os.close(control_write)
            channels.remove(control_write)
            os.close(result_write)
            channels.remove(result_write)
        streams = {control_read: 0, result_read: 1}
        while time.monotonic() < query_deadline:
            _reap(owner)
            if worker is not None and owner.reaped:
                raise ValueError("persistent worker exited during request")
            if owner.reap_errno is not None:
                raise OSError(owner.reap_errno, "worker ownership observation failed")
            remaining = query_deadline - time.monotonic()
            ready, _, _ = select.select(list(streams), [], [], max(0, min(0.005, remaining)))
            for fd in ready:
                if time.monotonic() >= query_deadline:
                    raise TimeoutError("late worker channel receipt")
                index = streams[fd]
                if index == 0:
                    read_cap = min(TURN_CAP // 2, 4097 - control_bytes)
                elif result_size is None:
                    read_cap = HEADER.size - len(buffers[1])
                else:
                    # One extra byte detects a trailing/duplicate frame. Do
                    # not retain it or read beyond the declared boundary.
                    read_cap = min(TURN_CAP // 2, HEADER.size + result_size - len(buffers[1]) + 1)
                try:
                    chunk = os.read(fd, read_cap)
                except BlockingIOError:
                    continue
                if not chunk:
                    closed_channels.add(index)
                    del streams[fd]
                    if index == 1 and (
                        result_size is None or len(buffers[1]) != HEADER.size + result_size
                    ):
                        raise ValueError("incomplete worker frame length")
                elif index == 0:
                    # Keep only the admitted prefix; the extra byte detects
                    # overflow and can never complete a protocol line.
                    buffers[0].extend(chunk[: max(0, 4096 - control_bytes)])
                else:
                    if result_size is not None and (
                        len(buffers[1]) + len(chunk) > HEADER.size + result_size
                    ):
                        raise ValueError("invalid worker frame length")
                    buffers[index].extend(chunk)
                    if result_size is None and len(buffers[1]) == HEADER.size:
                        result_size = cast(int, HEADER.unpack(buffers[1])[0])
                        # A JSON object needs at least the two bytes in {}.
                        if not 2 <= result_size <= FRAME_CAP:
                            raise ValueError("invalid worker frame length")
                if index == 0:
                    control_bytes += len(chunk)
                    # Complete identities preceding overflow remain cleanup
                    # evidence, without granting success or signal authority.
                    consume_control(cleanup_only=control_bytes > 4096)
                if (control_bytes if index == 0 else len(buffers[index])) > (
                    4096 if index == 0 else FRAME_CAP + HEADER.size
                ):
                    raise ValueError("worker channel cap exceeded")
            consume_control(cleanup_only=False)
            framed = (
                worker is not None
                and request_done
                and result_size is not None
                and len(buffers[1]) == HEADER.size + result_size
            )
            if (not streams or framed) and time.monotonic() < query_deadline:
                received = time.monotonic()
                # Partial control or terminal bytes cannot preserve no-child
                # authority by failing an earlier framing guard.
                creation_conflict = fork_failure is not None and bool(buffers[0] or buffers[1])
                if not owner.ready or not intent or buffers[0] or len(buffers[1]) < HEADER.size:
                    raise ValueError("incomplete worker publication")
                size = HEADER.unpack(buffers[1][: HEADER.size])[0]
                if size > FRAME_CAP or len(buffers[1]) != HEADER.size + size:
                    raise ValueError("invalid worker frame length")
                # A complete contradictory terminal must not retain no-child
                # authority merely because its own validation fails first.
                creation_conflict = fork_failure is not None
                decoded_receipt = _decode(
                    bytes(buffers[1][HEADER.size :]),
                    token,
                    argv,
                    query_deadline,
                    dispatched,
                    received,
                    identity=request_identity,
                )
                if fork_failure is not None and (
                    decoded_receipt.pid is not None
                    or decoded_receipt.error_errno != fork_failure["errno"]
                    or decoded_receipt.admission is None
                    or not decoded_receipt.admission["ok"]
                    or not decoded_receipt.started_monotonic
                    <= cast(float, fork_failure["failed_monotonic"])
                    <= decoded_receipt.finished_monotonic
                ):
                    creation_conflict = True
                    raise ValueError("worker result/fork failure mismatch")
                creation_conflict = False
                if decoded_receipt.pid is None:
                    if helper_identity is not None:
                        raise ValueError("worker denied published helper creation")
                elif helper_identity is None or (
                    decoded_receipt.pid != helper_identity["pid"]
                    or decoded_receipt.created_monotonic != helper_identity["created_monotonic"]
                ):
                    raise ValueError("worker result/created identity mismatch")
                if time.monotonic() >= query_deadline:
                    raise TimeoutError("worker validation exceeded query deadline")
                receipt = decoded_receipt
                published_times = {
                    item.name: getattr(receipt, item.name)
                    for item in fields(HelperCapture)
                    if item.name.endswith("monotonic")
                }
                receipt.deadline_monotonic = deadline
                complete = True
                if receipt.reaped:
                    _helper_obligations.pop(token, None)
                break
        if not complete:
            raise TimeoutError("worker query deadline exhausted")
        if worker is not None and receipt.error is not None:
            worker.failure = receipt.error
    except (OSError, ValueError) as error:
        receipt.error = f"worker capture failed: {error}"
        if worker is not None:
            worker.failure = receipt.error
        if isinstance(error, OSError):
            receipt.error_errno = error.errno
    finally:
        if owner is not None:
            if worker is None or worker.failure is not None:
                _reap(owner)
                if not owner.reaped:
                    _signal(owner, signal.SIGTERM)
                # One fixed shutdown reserve, never a renewed per-phase allowance.
                kill_at = min(deadline, time.monotonic() + reserve / 4)
                # Parse buffered controls even after observing worker exit. Never
                # decode late terminal/reap receipts or confirm a late fork failure.
                if not complete:
                    consume_control(cleanup_only=True)
                cleanup_open = (
                    not complete and control_read is not None and 0 not in closed_channels
                )
                while time.monotonic() < deadline:
                    if (
                        not owner.reaped
                        and owner.reap_errno is None
                        and time.monotonic() >= kill_at
                    ):
                        _signal(owner, signal.SIGKILL)
                        kill_at = float("inf")
                    _reap(owner)
                    if cleanup_open and control_read is not None:
                        # Nonblocking, one read per turn, cumulative cap plus one
                        # overflow-detection byte; no EOF-dependent wait or renewal.
                        if control_bytes > 4096 or control_invalid:
                            cleanup_open = False
                        else:
                            try:
                                chunk = os.read(control_read, min(TURN_CAP, 4097 - control_bytes))
                            except BlockingIOError:
                                chunk = None
                            except OSError:
                                chunk = None
                                cleanup_open = False
                            if chunk is not None:
                                cleanup_open = bool(chunk)
                                buffers[0].extend(chunk[: max(0, 4096 - control_bytes)])
                                control_bytes += len(chunk)
                                if chunk and fork_failure is not None:
                                    creation_conflict = True
                                consume_control(cleanup_only=True)
                    if (owner.reaped or owner.reap_errno is not None) and not cleanup_open:
                        break
                    time.sleep(min(0.001, max(0, deadline - time.monotonic())))
                _reap(owner)
            if not complete and fork_failure is not None and (buffers[0] or buffers[1]):
                # Incremental rejection can bypass the final decoder. Partial
                # control/terminal data still revokes clean no-child authority.
                creation_conflict = True
            confirmed_failure = (
                fork_failure is not None and not creation_conflict and closed_channels == {0, 1}
            )
            unknown_creation = not complete and helper_identity is None and not confirmed_failure
            if confirmed_failure and fork_failure is not None and not complete:
                receipt.error_errno = cast(int, fork_failure["errno"])
            receipt.worker = {
                **asdict(owner),
                "intent": intent,
                "unknown_helper_creation": unknown_creation,
                "fork_failure": fork_failure,
                "creation_conflict": creation_conflict,
                "helper_identity": helper_identity,
                "helper_identities": helper_identities,
                "helper_obligation_retained": any(
                    key == token or key.startswith(token + ":") for key in _helper_obligations
                ),
                "signal_scope": "group" if owner.ready else "pid",
                "dispatch_monotonic": dispatched,
                "receipt_monotonic": received,
                "published_times": published_times,
                "query_deadline_monotonic": query_deadline,
                "reserve_seconds": reserve,
                "request_token": token,
                "request_identity": request_identity,
                "session_token": worker.session_token if worker is not None else None,
            }
            if (worker is None or worker.failure is not None) and (
                not owner.reaped or owner.returncode != 0
            ):
                receipt.error = (
                    receipt.error or "worker did not exit cleanly within fixed allowance"
                )
            if (worker is None or worker.failure is not None) and (
                not owner.reaped or (not complete and not confirmed_failure)
            ):
                receipt.residual_remedy = (
                    "Retained helper identity requires its own matching reap receipt; unpublished "
                    "interrupted creation remains unknown. Worker reap/group signals do not "
                    "prove helper death. Independently reconcile exact worker/helper fixture "
                    "identities. An unreaped worker retains strong ownership without later signals."
                )
            elif token in _helper_obligations and not receipt.residual_remedy:
                receipt.residual_remedy = (
                    "Retained helper identity requires its own matching reap receipt; "
                    "worker reap does not resolve the helper obligation."
                )
        if worker is not None:
            if worker.failure is not None:
                worker.close()
        for fd in channels:
            try:
                os.close(fd)
            except OSError as error:
                receipt.close_errors.append(f"worker channel {fd}: {error}")
                receipt.error = receipt.error or "worker channel close failed"
        receipt.finished_monotonic = time.monotonic()
        if receipt.finished_monotonic > deadline:
            receipt.error = receipt.error or "worker fixed deadline exceeded"
        if worker is not None:
            if receipt.error is not None and worker.failure is None:
                worker.failure = receipt.error
                worker.close()
                if owner is not None and not owner.reaped:
                    receipt.residual_remedy = receipt.residual_remedy or (
                        "Worker finalization exceeded its request allowance; independently "
                        "reconcile the retained exact child without renewed runtime authority."
                    )
            elif complete and worker.failure is None and owner is not None:
                owner.shutdown_deadline_monotonic = float("inf")
    return receipt
