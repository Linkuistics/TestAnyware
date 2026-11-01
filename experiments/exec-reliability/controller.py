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
from typing import Literal


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
            else:
                if receipt["reason"] != "observation_error":
                    receipt["reason"] = "interrupted" if call.interrupt_after else "deadline"
                for sig, grace in (
                    (signal.SIGTERM, plan.term_grace),
                    (signal.SIGKILL, plan.kill_wait),
                ):
                    try:
                        if call.interrupt_after is not None:
                            os.kill(process.pid, sig)
                        else:
                            os.killpg(process.pid, sig)
                        events.append(
                            {"signal": sig.name.removeprefix("SIG"), "monotonic": time.monotonic()}
                        )
                    except ProcessLookupError:
                        events.append(
                            {
                                "signal": sig.name.removeprefix("SIG"),
                                "monotonic": time.monotonic(),
                                "absent": True,
                            }
                        )
                    except OSError as error:
                        events.append(
                            {
                                "signal": sig.name.removeprefix("SIG"),
                                "monotonic": time.monotonic(),
                                "error": str(error),
                            }
                        )
                    if sig == signal.SIGTERM:
                        # Give the entire declared group its grace, even if its leader exits.
                        time.sleep(grace)
            try:
                receipt["returncode"] = process.wait(timeout=plan.kill_wait)
                receipt["reaped"] = True
            except subprocess.TimeoutExpired:
                receipt["reap_error"] = "leader unreaped; manual identity inspection required"
            receipt["ok"] = (
                receipt["reason"] == call.expected_reason
                and receipt["returncode"] == call.expected_exit
                and receipt["reaped"] is True
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
                Path(sys.executable).resolve(),
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
    ok = not any(value.startswith("unavailable:") for value in before.values())
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
                "calls": receipts,
                "resources": ledger,
                "unreached": unreached,
                "subjects_before": before,
                "subjects_after": after,
            },
        )
    return root
