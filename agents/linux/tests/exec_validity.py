"""The validity rules of an exec record, as a test oracle.

Source of truth: docs/specs/guest-command-lifecycle.md, sections "Added fields",
"Termination fields", "Validity" and "Linux projection". A record that breaks a
rule returns one message per broken rule. An empty list means a valid record.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import re
from typing import Any

from testanyware_agent.models import LeaderStatus, Outcome, linux_legacy_projection

EXEC_ID = re.compile(r"[A-Za-z0-9._-]{1,64}")
OUTCOME_FIELDS = {
    "exited": {"kind", "exitCode"},
    "signalled": {"kind", "signal", "signalName"},
    "timed_out": {"kind", "timeoutSeconds"},
    "cancelled": {"kind", "beforeStart"},
    "spawn_failed": {"kind", "errno", "errorName", "message"},
}
LIFECYCLE_FIELDS = {
    "leaderPid",
    "groupId",
    "leaderStartTime",
    "spawnMs",
    "durationMs",
    "ownerLatenessMs",
    "finalisedAfterMs",
}
TERMINATION_FIELDS = {
    "termSentAfterMs",
    "killSentAfterMs",
    "leaderStatus",
    "leaderExitAfterMs",
    "groupEmpty",
    "groupEmptyAfterMs",
    "withinBounds",
}


def _is_ms(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def stream_bytes(record: dict, name: str) -> bytes:
    return base64.b64decode(record["evidence"][name]["base64"], validate=True)


def _check_stream(name: str, stream: Any, errors: list[str]) -> bytes | None:
    if not isinstance(stream, dict):
        errors.append(f"evidence.{name} is not an object")
        return None
    try:
        data = base64.b64decode(stream.get("base64", ""), validate=True)
    except (binascii.Error, ValueError):
        errors.append(f"evidence.{name}.base64 does not decode")
        return None
    if stream.get("bytes") != len(data):
        errors.append(f"evidence.{name}.bytes is not the decoded length")
    if stream.get("sha256") != hashlib.sha256(data).hexdigest():
        errors.append(f"evidence.{name}.sha256 does not match the bytes")
    file_bytes = stream.get("fileBytes")
    if not _is_ms(file_bytes):
        errors.append(f"evidence.{name}.fileBytes is not a count")
        return data
    capture = stream.get("capture")
    if capture == "complete":
        if len(data) != file_bytes or "reason" in stream:
            errors.append(f"evidence.{name} is complete but not whole")
    elif capture == "incomplete":
        if len(data) >= file_bytes:
            errors.append(f"evidence.{name} is incomplete but holds fileBytes")
        if stream.get("reason") not in ("deadline", "read_error"):
            errors.append(f"evidence.{name}.reason is not deadline or read_error")
    else:
        errors.append(f"evidence.{name}.capture is {capture!r}")
    return data


def _outcome_model(outcome: dict) -> Outcome:
    kind = outcome["kind"]
    if kind == "exited":
        return Outcome("exited", exit_code=outcome["exitCode"])
    if kind == "signalled":
        return Outcome("signalled", signal=outcome["signal"])
    if kind == "timed_out":
        return Outcome("timed_out", timeout_seconds=outcome["timeoutSeconds"])
    if kind == "cancelled":
        return Outcome("cancelled", before_start=outcome["beforeStart"])
    return Outcome("spawn_failed", errno=outcome["errno"], message=outcome["message"])


def _check_termination(record: dict, errors: list[str]) -> None:
    t = record["termination"]
    if not isinstance(t, dict) or set(t) != TERMINATION_FIELDS:
        errors.append("termination does not hold exactly its fields")
        return
    finalised = record["lifecycle"]["finalisedAfterMs"]
    term, kill = t["termSentAfterMs"], t["killSentAfterMs"]
    if not (_is_ms(term) and _is_ms(kill) and term <= kill <= finalised):
        errors.append("termSentAfterMs <= killSentAfterMs <= finalisedAfterMs does not hold")
    status = t["leaderStatus"]
    exit_at = t["leaderExitAfterMs"]
    if status == {"kind": "unavailable"}:
        if exit_at is not None:
            errors.append("an unavailable leader status has a leaderExitAfterMs")
        if t["groupEmpty"] != "no":
            errors.append("an unavailable leader status does not give groupEmpty no")
    else:
        valid = status in (
            LeaderStatus("exited", status.get("exitCode")).to_dict(),
            LeaderStatus("signalled", status.get("signal")).to_dict(),
        )
        if not valid:
            errors.append(f"leaderStatus {status!r} is malformed")
        if not (_is_ms(exit_at) and term <= exit_at <= finalised):
            errors.append("leaderExitAfterMs is not between termSentAfterMs and finalisedAfterMs")
    empty, empty_at = t["groupEmpty"], t["groupEmptyAfterMs"]
    if empty not in ("yes", "no", "unknown"):
        errors.append(f"groupEmpty is {empty!r}")
    if empty == "yes":
        bounds_ok = _is_ms(empty_at) and kill <= empty_at <= finalised
        if bounds_ok and _is_ms(exit_at):
            bounds_ok = empty_at >= exit_at
        if not bounds_ok:
            errors.append("groupEmptyAfterMs is outside its order")
    elif empty_at is not None:
        errors.append("groupEmptyAfterMs is set although groupEmpty is not yes")
    within = empty == "yes" and kill <= 2000 and finalised - kill <= 2000
    if t["withinBounds"] is not within:
        errors.append("withinBounds is not the derived value")


def record_errors(record: dict, *, exec_id: str | None = None) -> list[str]:
    errors: list[str] = []
    rid = record.get("id")
    if not isinstance(rid, str) or not EXEC_ID.fullmatch(rid):
        errors.append(f"id {rid!r} is not a valid exec id")
    if exec_id is not None and rid != exec_id:
        errors.append(f"id {rid!r} is not the requested {exec_id!r}")
    if record.get("legacyProjection") != "linux":
        errors.append("legacyProjection is not linux")
    outcome = record.get("outcome")
    kind = outcome.get("kind") if isinstance(outcome, dict) else None
    if kind not in OUTCOME_FIELDS or set(outcome) != OUTCOME_FIELDS[kind]:
        errors.append(f"outcome {outcome!r} is malformed")
        return errors
    if kind == "exited" and not 0 <= outcome["exitCode"] <= 255:
        errors.append("an exited code is outside 0 to 255")
    evidence = record.get("evidence")
    if not isinstance(evidence, dict) or set(evidence) != {"stdout", "stderr"}:
        errors.append("evidence does not hold exactly stdout and stderr")
        return errors
    out = _check_stream("stdout", evidence["stdout"], errors)
    err = _check_stream("stderr", evidence["stderr"], errors)
    lifecycle = record.get("lifecycle")
    if not isinstance(lifecycle, dict) or set(lifecycle) != LIFECYCLE_FIELDS:
        errors.append("lifecycle does not hold exactly its fields")
        return errors
    for name in ("spawnMs", "ownerLatenessMs", "finalisedAfterMs"):
        if not _is_ms(lifecycle[name]):
            errors.append(f"lifecycle.{name} is not a count of milliseconds")
    started = kind != "spawn_failed" and not (kind == "cancelled" and outcome["beforeStart"])
    identity = [lifecycle["leaderPid"], lifecycle["groupId"], lifecycle["durationMs"]]
    if started and not all(_is_ms(v) for v in identity):
        errors.append("a started command lacks its leader identity or duration")
    if not started and any(v is not None for v in [*identity, lifecycle["leaderStartTime"]]):
        errors.append("a command that never started has a leader identity")
    has_remaining = record.get("remainingGroup") is not None
    if (kind in ("exited", "signalled")) != has_remaining:
        errors.append("remainingGroup is not present exactly for exited and signalled")
    if record.get("remainingGroup") not in (None, "none", "present", "unknown"):
        errors.append("remainingGroup has an unknown value")
    terminated = started and kind in ("timed_out", "cancelled")
    if terminated != (record.get("termination") is not None):
        errors.append("termination is not present exactly for a started termination")
    elif terminated:
        _check_termination(record, errors)
    if out is not None and err is not None:
        legacy = linux_legacy_projection(_outcome_model(outcome), out, err)
        actual = {k: record.get(k) for k in ("exitCode", "stdout", "stderr")}
        if actual != legacy:
            errors.append(f"legacy fields {actual!r} are not the projection {legacy!r}")
        if "timedOut" in record:
            errors.append("the Linux projection carries timedOut")
    return errors
