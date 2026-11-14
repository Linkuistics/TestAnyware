"""Interim per-query worker: isolate helper creation from the controller.

This is not yet the persistent worker/gated-publication/shared scheduler in the
inspection spec. Interrupted creation is conservatively unknown, never absence.
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
        inspection.reset_after_fork({control, result})
        ready = {"token": token, "pid": os.getpid(), "pgid": os.getpgrp(), "sid": os.getsid(0)}
        _write(control, json.dumps(ready).encode() + b"\n", deadline)
        _write(control, b"intent\n", deadline)
        capture = inspection.capture_helper(argv, deadline)
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
        elif key.endswith("monotonic"):
            optional = key in {"created_monotonic", "kill_monotonic", "reaped_monotonic"}
            if not (optional and value is None) and (
                type(value) not in {int, float}
                or not isinstance(value, int | float)
                or not 0 <= value <= deadline
            ):
                raise ValueError("invalid worker timestamp")
        elif key in {"error", "residual_remedy"}:
            if value is not None and not isinstance(value, str):
                raise ValueError("invalid worker error")
        elif key == "close_errors":
            if not isinstance(value, list) or not all(
                isinstance(item, str) for item in cast(list[object], value)
            ):
                raise ValueError("invalid worker close errors")
        elif key == "admission":
            if not isinstance(value, dict) or cast(dict[str, object], value).get("ok") is not True:
                raise ValueError("worker helper admission failed")
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
    if receipt.kill_errno is not None and receipt.kill_monotonic is None:
        raise ValueError("worker kill errno without kill timestamp")
    capture_deadline = deadline - min(0.05, max(0, deadline - receipt.started_monotonic) / 4)
    if receipt.capture_deadline_monotonic != capture_deadline:
        raise ValueError("worker capture cutoff mismatch")
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
    buffers = [bytearray(), bytearray()]
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
                    chunk = os.read(fd, TURN_CAP // 2)
                except BlockingIOError:
                    continue
                if not chunk:
                    del streams[fd]
                else:
                    buffers[index].extend(chunk)
                if len(buffers[index]) > (4096 if index == 0 else FRAME_CAP + HEADER.size):
                    raise ValueError("worker channel cap exceeded")
            if not owner.ready and b"\n" in buffers[0]:
                line, _, tail = buffers[0].partition(b"\n")
                identity = _load_json(line)
                if identity != {
                    "token": token,
                    "pid": owner.pid,
                    "pgid": owner.pid,
                    "sid": owner.pid,
                }:
                    raise ValueError("invalid worker readiness")
                owner.ready = True
                buffers[0] = bytearray(tail)
            if owner.ready and bytes(buffers[0]) == b"intent\n":
                intent = True
            if not streams and time.monotonic() < query_deadline:
                received = time.monotonic()
                if not owner.ready or not intent or len(buffers[1]) < HEADER.size:
                    raise ValueError("incomplete worker publication")
                size = HEADER.unpack(buffers[1][: HEADER.size])[0]
                if size > FRAME_CAP or len(buffers[1]) != HEADER.size + size:
                    raise ValueError("invalid worker frame length")
                decoded_receipt = _decode(
                    bytes(buffers[1][HEADER.size :]),
                    token,
                    argv,
                    query_deadline,
                    dispatched,
                    received,
                )
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
            while not owner.reaped and owner.reap_errno is None and time.monotonic() < deadline:
                if time.monotonic() >= kill_at:
                    _signal(owner, signal.SIGKILL)
                    kill_at = float("inf")
                _reap(owner)
                if not owner.reaped:
                    time.sleep(min(0.001, max(0, deadline - time.monotonic())))
            _reap(owner)
            receipt.worker = {
                **asdict(owner),
                "intent": intent,
                "unknown_helper_creation": not complete,
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
            if not complete or not owner.reaped:
                receipt.residual_remedy = (
                    "Interrupted helper creation remains unknown; worker reap/group signals do not "
                    "prove helper death. Independently reconcile exact worker/helper fixture "
                    "identities. An unreaped worker retains strong ownership without later signals."
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
