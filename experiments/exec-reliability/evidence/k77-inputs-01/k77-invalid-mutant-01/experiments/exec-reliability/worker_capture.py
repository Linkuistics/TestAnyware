"""Interim per-query worker: isolate helper creation from the controller.

Helpers are gated by creation publication. Persistent startup and shared
scheduling remain open. Interrupted unpublished creation is never absence.
"""

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


@dataclass
class _Owner:
    pid: int
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
    event: dict[str, object] = {"signal": sig.name, "monotonic": time.monotonic()}
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
        _write(control, b"intent\n", deadline)

        def publish_created(identity: inspection.HelperIdentity) -> None:
            message = {**identity, "token": token, "deadline_monotonic": deadline}
            _write(control, json.dumps(message, allow_nan=False).encode() + b"\n", deadline)

        def publish_fork_error(failure: inspection.ForkFailure) -> None:
            message = {**failure, "token": token, "deadline_monotonic": deadline}
            _write(control, json.dumps(message, allow_nan=False).encode() + b"\n", deadline)

        capture = inspection.capture_helper(
            argv, deadline, publish_created=publish_created, publish_fork_error=publish_fork_error
        )
        body = asdict(capture)
        body.update(stdout=capture.stdout.hex(), stderr=capture.stderr.hex(), token=token)
        payload = json.dumps(body, allow_nan=False).encode()
        if len(payload) > FRAME_CAP:
            raise ValueError("worker frame cap exceeded")
        _write(result, HEADER.pack(len(payload)) + payload, deadline)
        os._exit(0)
    except BaseException:
        os._exit(70)


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
    return cast(dict[str, object], _load_json(line))


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
) -> HelperCapture:
    decoded = _load_json(payload)
    if not isinstance(decoded, dict):
        raise ValueError("invalid worker result object")
    raw = cast(dict[str, object], decoded)
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


def capture_helper(argv: tuple[str, ...], deadline: float) -> HelperCapture:
    """Bound parent waiting even when the worker's helper creation never returns.

    The raw worker fork is admitted, but remains a synchronous OS operation.
    All channel waits and worker shutdown share the caller's original deadline.
    """
    started = time.monotonic()
    reserve = min(0.05, max(0, deadline - started) / 4)
    query_deadline = deadline - reserve
    receipt = HelperCapture(argv=argv, started_monotonic=started, deadline_monotonic=deadline)
    admission: ForkAdmission = inspection.fork_admission()
    receipt.admission = admission
    if not admission["ok"] or time.monotonic() >= query_deadline:
        receipt.error = f"worker not started: {admission['reason']} or exhausted allowance"
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

    def consume_control(cleanup_only: bool) -> None:
        nonlocal control_ready, control_invalid, intent, creation_conflict
        nonlocal helper_identity, fork_failure
        if control_invalid or owner is None or dispatched is None:
            return
        # Deliberate framing mutant: accept an unterminated final JSON line.
        if cleanup_only and buffers[0] and b"\n" not in buffers[0]:
            buffers[0].extend(b"\n")
        while b"\n" in buffers[0]:
            line, _, tail = buffers[0].partition(b"\n")
            buffers[0] = bytearray(tail)
            late = cleanup_only or time.monotonic() >= query_deadline
            try:
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
                    if bytes(line) != b"intent":
                        control_invalid = True
                        raise ValueError("invalid helper creation intent")
                    intent = True
                else:
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
        for _ in range(2):
            pair = os.pipe()
            channels.extend(pair)
            for fd in pair:
                os.set_blocking(fd, False)
        control_read, control_write, result_read, result_write = channels
        blocked = signal.valid_signals() - {signal.SIGKILL, signal.SIGSTOP}
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, blocked)
        try:
            # https://docs.python.org/3.12/library/os.html#os.fork
            dispatched = time.monotonic()
            pid = os.fork()
            if pid == 0:
                _child(control_write, result_write, token, argv, query_deadline)
                os._exit(71)
            owner = _Owner(pid)
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
            if owner.reap_errno is not None:
                raise OSError(owner.reap_errno, "worker ownership observation failed")
            remaining = query_deadline - time.monotonic()
            ready, _, _ = select.select(list(streams), [], [], max(0, min(0.005, remaining)))
            for fd in ready:
                if time.monotonic() >= query_deadline:
                    raise TimeoutError("late worker channel receipt")
                index = streams[fd]
                try:
                    chunk = os.read(
                        fd,
                        min(TURN_CAP // 2, 4097 - control_bytes) if index == 0 else TURN_CAP // 2,
                    )
                except BlockingIOError:
                    continue
                if not chunk:
                    closed_channels.add(index)
                    del streams[fd]
                elif index == 0:
                    # Keep only the admitted prefix; the extra byte detects
                    # overflow and can never complete a protocol line.
                    buffers[0].extend(chunk[: max(0, 4096 - control_bytes)])
                else:
                    buffers[index].extend(chunk)
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
            if not streams and time.monotonic() < query_deadline:
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
    except (OSError, ValueError) as error:
        receipt.error = f"worker capture failed: {error}"
        if isinstance(error, OSError):
            receipt.error_errno = error.errno
    finally:
        if owner is not None:
            _reap(owner)
            if not owner.reaped:
                _signal(owner, signal.SIGTERM)
            # One fixed shutdown reserve, never a renewed per-phase allowance.
            kill_at = min(deadline, time.monotonic() + reserve / 4)
            # Parse buffered controls even after observing worker exit. Never
            # decode late terminal/reap receipts or confirm a late fork failure.
            if not complete:
                consume_control(cleanup_only=True)
            cleanup_open = not complete and control_read is not None and 0 not in closed_channels
            while time.monotonic() < deadline:
                if not owner.reaped and owner.reap_errno is None and time.monotonic() >= kill_at:
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
                "shutdown_deadline_monotonic": deadline,
                "reserve_seconds": reserve,
            }
            if not owner.reaped or owner.returncode != 0:
                receipt.error = (
                    receipt.error or "worker did not exit cleanly within fixed allowance"
                )
            if not owner.reaped or (not complete and not confirmed_failure):
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
        for fd in channels:
            try:
                os.close(fd)
            except OSError as error:
                receipt.close_errors.append(f"worker channel {fd}: {error}")
                receipt.error = receipt.error or "worker channel close failed"
        receipt.finished_monotonic = time.monotonic()
        if receipt.finished_monotonic > deadline:
            receipt.error = receipt.error or "worker fixed deadline exceeded"
    return receipt
