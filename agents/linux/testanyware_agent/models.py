from __future__ import annotations

import base64
import errno as errno_module
import hashlib
import signal as signal_module
from dataclasses import dataclass, field


@dataclass
class ElementInfo:
    role: str
    enabled: bool
    focused: bool
    child_count: int
    showing: bool = True
    actions: list[str] = field(default_factory=list)
    label: str | None = None
    value: str | None = None
    description: str | None = None
    id: str | None = None
    position_x: float | None = None
    position_y: float | None = None
    size_width: float | None = None
    size_height: float | None = None
    platform_role: str | None = None
    children: list[ElementInfo] | None = None

    def to_dict(self) -> dict:
        d: dict = {"role": self.role, "enabled": self.enabled, "focused": self.focused,
                   "showing": self.showing}
        if self.label is not None:
            d["label"] = self.label
        if self.value is not None:
            d["value"] = self.value
        if self.description is not None:
            d["description"] = self.description
        if self.id is not None:
            d["id"] = self.id
        if self.position_x is not None:
            d["positionX"] = self.position_x
        if self.position_y is not None:
            d["positionY"] = self.position_y
        if self.size_width is not None:
            d["sizeWidth"] = self.size_width
        if self.size_height is not None:
            d["sizeHeight"] = self.size_height
        d["childCount"] = self.child_count
        d["actions"] = self.actions
        if self.platform_role is not None:
            d["platformRole"] = self.platform_role
        if self.children is not None:
            d["children"] = [c.to_dict() for c in self.children]
        return d


@dataclass
class WindowInfo:
    title: str | None
    window_type: str
    size_width: float
    size_height: float
    position_x: float
    position_y: float
    app_name: str
    focused: bool
    elements: list[ElementInfo] | None = None

    def to_dict(self) -> dict:
        d: dict = {}
        if self.title is not None:
            d["title"] = self.title
        d["windowType"] = self.window_type
        d["sizeWidth"] = self.size_width
        d["sizeHeight"] = self.size_height
        d["positionX"] = self.position_x
        d["positionY"] = self.position_y
        d["appName"] = self.app_name
        d["focused"] = self.focused
        if self.elements is not None:
            d["elements"] = [e.to_dict() for e in self.elements]
        return d


# ---- Exec record ---------------------------------------------------------
#
# The typed record of one guest command (ADR-0023). The source of truth is
# docs/specs/guest-command-lifecycle.md, sections "Exec record" and
# "Termination fields". The legacy fields `exitCode`, `stdout` and `stderr`
# come first and keep the values of the old Linux agent.

LINUX_PROJECTION = "linux"


@dataclass
class StreamEvidence:
    """The captured bytes of one stream, up to the size at finalisation."""

    data: bytes
    file_bytes: int
    complete: bool
    reason: str | None = None  # `deadline` or `read_error` when incomplete

    def to_dict(self) -> dict:
        d: dict = {
            "base64": base64.b64encode(self.data).decode("ascii"),
            "bytes": len(self.data),
            "fileBytes": self.file_bytes,
            "sha256": hashlib.sha256(self.data).hexdigest(),
            "capture": "complete" if self.complete else "incomplete",
        }
        if not self.complete:
            d["reason"] = self.reason
        return d


EMPTY_STREAM = StreamEvidence(data=b"", file_bytes=0, complete=True)


@dataclass
class Outcome:
    """The first terminal outcome that the owner chose.

    `kind` is `exited`, `signalled`, `timed_out`, `cancelled` or
    `spawn_failed`. Only the fields of that kind are set.
    """

    kind: str
    exit_code: int | None = None
    signal: int | None = None
    timeout_seconds: float | None = None
    before_start: bool | None = None
    errno: int | None = None
    message: str | None = None

    def to_dict(self) -> dict:
        if self.kind == "exited":
            return {"kind": "exited", "exitCode": self.exit_code}
        if self.kind == "signalled":
            assert self.signal is not None
            try:
                name = signal_module.Signals(self.signal).name
            except ValueError:
                name = f"SIG{self.signal}"
            return {"kind": "signalled", "signal": self.signal, "signalName": name}
        if self.kind == "timed_out":
            return {"kind": "timed_out", "timeoutSeconds": self.timeout_seconds}
        if self.kind == "cancelled":
            return {"kind": "cancelled", "beforeStart": self.before_start}
        assert self.kind == "spawn_failed" and self.errno is not None
        return {
            "kind": "spawn_failed",
            "errno": self.errno,
            "errorName": errno_module.errorcode.get(self.errno, f"E{self.errno}"),
            "message": self.message,
        }


@dataclass
class Lifecycle:
    leader_pid: int | None
    group_id: int | None
    leader_start_time: str | None
    spawn_ms: int
    duration_ms: int | None
    owner_lateness_ms: int
    finalised_after_ms: int

    def to_dict(self) -> dict:
        return {
            "leaderPid": self.leader_pid,
            "groupId": self.group_id,
            "leaderStartTime": self.leader_start_time,
            "spawnMs": self.spawn_ms,
            "durationMs": self.duration_ms,
            "ownerLatenessMs": self.owner_lateness_ms,
            "finalisedAfterMs": self.finalised_after_ms,
        }


@dataclass
class LeaderStatus:
    """The leader exit status as the kernel delivered it, or unavailable."""

    kind: str  # `exited`, `signalled` or `unavailable`
    value: int | None = None  # the exit code or the signal number

    def to_dict(self) -> dict:
        if self.kind == "exited":
            return {"kind": "exited", "exitCode": self.value}
        if self.kind == "signalled":
            return {"kind": "signalled", "signal": self.value}
        return {"kind": "unavailable"}


@dataclass
class Termination:
    """The steps of one termination, in milliseconds from T0, the outcome."""

    term_sent_after_ms: int
    kill_sent_after_ms: int
    leader_status: LeaderStatus
    leader_exit_after_ms: int | None
    group_empty: str  # `yes`, `no` or `unknown`
    group_empty_after_ms: int | None

    def within_bounds(self, finalised_after_ms: int) -> bool:
        return (
            self.group_empty == "yes"
            and self.kill_sent_after_ms <= 2000
            and finalised_after_ms - self.kill_sent_after_ms <= 2000
        )

    def to_dict(self, finalised_after_ms: int) -> dict:
        return {
            "termSentAfterMs": self.term_sent_after_ms,
            "killSentAfterMs": self.kill_sent_after_ms,
            "leaderStatus": self.leader_status.to_dict(),
            "leaderExitAfterMs": self.leader_exit_after_ms,
            "groupEmpty": self.group_empty,
            "groupEmptyAfterMs": self.group_empty_after_ms,
            "withinBounds": self.within_bounds(finalised_after_ms),
        }


def _decode_like_old_agent(data: bytes) -> str:
    # The old agent read both streams with `subprocess.run(text=True)`, which
    # decodes with the locale encoding (UTF-8 on the guest) and translates
    # universal newlines. Source: CPython 3.12 Lib/subprocess.py,
    # `Popen._translate_newlines`
    # (https://github.com/python/cpython/blob/3.12/Lib/subprocess.py).
    return data.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")


def linux_legacy_projection(outcome: Outcome, stdout: bytes, stderr: bytes) -> dict:
    """The legacy fields that the old Linux agent gave for this outcome.

    The old Linux agent never sent `timedOut`, so the projection omits it for
    every outcome. A reader treats an absent `timedOut` as false.
    """
    if outcome.kind in ("exited", "signalled"):
        try:
            out = _decode_like_old_agent(stdout)
            err = _decode_like_old_agent(stderr)
        except UnicodeDecodeError as e:
            return {"exitCode": -1, "stdout": "", "stderr": str(e)}
        code = outcome.exit_code if outcome.kind == "exited" else -(outcome.signal or 0)
        return {"exitCode": code, "stdout": out.rstrip(), "stderr": err.rstrip()}
    if outcome.kind == "timed_out":
        return {"exitCode": -1, "stdout": "", "stderr": "Process timed out"}
    if outcome.kind == "cancelled":
        return {"exitCode": -1, "stdout": "", "stderr": "Process cancelled"}
    return {"exitCode": -1, "stdout": "", "stderr": outcome.message or ""}


@dataclass
class ExecRecord:
    """The record of one guest command, as the exec reply carries it."""

    exec_id: str
    outcome: Outcome
    stdout: StreamEvidence
    stderr: StreamEvidence
    lifecycle: Lifecycle
    remaining_group: str | None  # set exactly for `exited` and `signalled`
    termination: Termination | None  # set exactly for a started termination

    def to_dict(self) -> dict:
        d = linux_legacy_projection(self.outcome, self.stdout.data, self.stderr.data)
        d.update({
            "id": self.exec_id,
            "outcome": self.outcome.to_dict(),
            "evidence": {"stdout": self.stdout.to_dict(), "stderr": self.stderr.to_dict()},
            "lifecycle": self.lifecycle.to_dict(),
            "remainingGroup": self.remaining_group,
            "termination": (
                None
                if self.termination is None
                else self.termination.to_dict(self.lifecycle.finalised_after_ms)
            ),
            "legacyProjection": LINUX_PROJECTION,
        })
        return d
