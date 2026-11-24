"""Judge one acceptance clone against its frozen plan (leaf acceptance-plan-k223).

    acceptance_k223_check.py CLONE_DIR

CLONE_DIR is one `<os>-<n>` directory of acceptance_k223.py. The checker reads
JSON mode only. It never reads text output, because text mode keeps the trimmed
legacy streams. It compares the decoded `evidence` bytes with the byte oracle.

The checker separates two questions:

- Validity: `record_failures` applies the validity rules of the spec, section
  "Validity rules", to a `file exec` envelope or a `file cancel` record. It
  accepts both leader orders and a record with `leaderStatus` `unavailable`.
- Acceptance: `bound_failures` applies the bounds of the root brief and the
  spec to a valid record. A valid record whose group did not end within the
  bounds fails acceptance.

Every ceiling and bound of the root brief and the spec is a limit here:

| Limit | Value | Observation |
|---|---|---|
| Ordinary observation and health | 30 s | Host elapsed time |
| Inventory | 60 s | Host elapsed time of `baseline` and `inspect` |
| Raw host exec | Guest budget + 20 s | Host elapsed time of each `file exec` |
| Intentional guest timeout | 2 s | `timeout_seconds`, `duration_ms` from 1999 to 2500 |
| Transfer | 150 s | Host elapsed time of each upload and download |
| Independent disposal | 45 s | Host elapsed time of `dispose` |
| Guest cleanup | 30 s | Host elapsed time of `cleanup` |
| Termination grace | 2 s | `kill_sent_after_ms` |
| SIGKILL, drain and reap | 2 s | `finalised_after_ms` minus `kill_sent_after_ms` |
| CLI SIGTERM, then SIGKILL | 2 s, then 2 s | Controller signals of `interrupt` |
| CLI SIGTERM to signal death | 2 s | Leader exit after the controller's SIGTERM |
| Spawn | 1 s | `spawn_ms` |
| Owner lateness | 0.5 s | `owner_lateness_ms` |
| Outcome to published record | 2 s, or 4 s with termination | `finalised_after_ms` |
| Cancellation reply with wait 5 s | 5 s | Host elapsed time of each `file cancel` |
| Health during a running command | 1 s | Host elapsed time of the helper's health |
| Boot | From the manifest | Host elapsed time of `acquire` |
| Interruption child poll | 4 s of waits at most | The number of polls in `interrupt-child` |

The 1999 ms floor of the timeout allows one millisecond of rounding, because
each agent rounds the duration down.

The manifest also declares a slot bound. `vm start` runs without `--wait`, so
no clone waits for a slot. A full slot fails `acquire` at once with
`VM_LIMIT_REACHED`. The slot bound therefore has no observation in this trial.

The checker also judges three properties of the run itself:

- Identity: the `identity` output gives the frozen agent digest, and the
  `app-identity` helper shows the frozen app build and executable. The app
  loaded the frozen configuration file, which has not changed since its
  evaluation, and `doctor` resolves the frozen Tart.
- Freeze: the manifest still has its frozen digest. Each frozen path, the
  manifest and each fixture file had its frozen digest before and after the
  run. Each checker module has the digest that the run's subjects recorded.
  The interpreter of the plan is a frozen path. `golden-disk.json` gives the
  frozen golden disk digest before and after the run.
- Execution: the recorded calls have the plan's names in the plan's order,
  each with its planned argv, and their start times never decrease.

Health during the explicit cancellation must start after the latest possible
spawn. That moment is the earlier reply end minus `duration_ms` and
`finalised_after_ms`, because both replies follow the published record. The
reply latency after publication stays inside the moment, and the derivation
takes the guest and host clock rates as equal.

The checker runs four corruption controls on independent in-memory copies of
the clone's evidence. Each damaged copy must fail:

1. `appended-lf`: the `intact` record with a line feed after its stdout. The
   copy updates the length and the digest, so only the byte oracle rejects it.
2. `flipped-byte`: the downloaded `artifact.bin` with one bit changed.
3. `missing-outcome`: the `timeout` record without `outcome`.
4. `contradictory-bounds` and `contradictory-order`: the `timeout` record with
   one changed termination claim. The first sets `groupEmpty` to `no` and
   keeps `withinBounds` true. The second puts `groupEmptyAfterMs` before
   `killSentAfterMs`. When `killSentAfterMs` is 0, it puts
   `leaderExitAfterMs` after `finalisedAfterMs` instead.

Then the checker reads the originals again. Their digests must not change, and
each original must still pass.

A download that succeeds proves its bytes only. On the app route, a guest reset
after a complete body reads as success, so no result here claims reset
detection.

The checker prints one JSON object. It exits 0 only when the clone passes.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import re
import sys
from collections.abc import Callable
from dataclasses import asdict
from pathlib import Path
from typing import Any, cast

import acceptance_k223 as trial
from completion_k186_check import _run_failures  # pyright: ignore[reportPrivateUsage]
from controller import _json_bytes  # pyright: ignore[reportPrivateUsage]

RESOURCE = ("acquire", "cleanup", "dispose", "inspect")
KINDS = ("exited", "signalled", "timed_out", "cancelled", "spawn_failed")
TIMEOUT_FLOOR_MS = trial.GUEST_TIMEOUT * 1000 - 1
TIMEOUT_CEILING_MS = trial.GUEST_TIMEOUT * 1000 + trial.LATENESS_MS
CANCEL_WINDOW = 0.5  # the cancellation starts at most 0.5 s after its 2 s trigger
EXEC_ID = re.compile(r"[A-Za-z0-9._-]{1,64}")
LEGACY_KEYS = ("exit_code", "stdout", "stderr", "timed_out")
TYPED_KEYS = (
    "exec_id",
    "legacy_projection",
    "outcome",
    "evidence",
    "lifecycle",
    "termination",
    "remaining_group",
)

Json = dict[str, Any]


def _mapping(value: object) -> Json:
    return cast(Json, value) if isinstance(value, dict) else {}


def _integer(value: object) -> bool:
    # A JSON boolean is a Python int, so an integer field refuses it explicitly.
    return isinstance(value, int) and not isinstance(value, bool)


def _time(value: object) -> bool:
    return _integer(value) and cast(int, value) >= 0


def decode(value: object) -> bytes | None:
    """Strict base64, or None when the value is not valid base64 text."""
    if not isinstance(value, str):
        return None
    try:
        return base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError):
        return None


def _rstrip_macos(text: str) -> str:
    # Foundation `\s+$` matches Unicode White_Space, as `str.isspace` does,
    # except U+001C to U+001F, which Python counts and ICU does not.
    end = len(text)
    while end and text[end - 1].isspace() and not "\x1c" <= text[end - 1] <= "\x1f":
        end -= 1
    return text[:end]


def projection(name: str, outcome: Json, out: bytes, err: bytes) -> tuple[Any, ...]:
    """The legacy fields that one platform's projection gives.

    The result is `(exit_code, stdout, stderr, timed_out)`. `stderr` is None for
    a Linux decoder message, which the spec does not compare. Spec, section
    "Legacy fields".
    """
    kind = outcome.get("kind")
    if name == "macos":

        def text(data: bytes) -> str:
            try:
                return _rstrip_macos(data.decode("utf-8"))
            except UnicodeDecodeError:
                return ""

        stdout, stderr = text(out), text(err)

        def noted(note: str) -> str:
            return note if not stderr else f"{stderr}\n[{note}]"

        if kind == "exited":
            return (outcome.get("exit_code"), stdout, stderr, False)
        if kind == "signalled":
            return (outcome.get("signal"), stdout, stderr, False)
        if kind == "timed_out":
            note = f"Process timed out after {outcome.get('timeout_seconds')}s"
            return (-1, stdout, noted(note), True)
        if kind == "cancelled":
            return (-1, stdout, noted("Process cancelled"), False)
        return (-1, "", outcome.get("message"), False)
    if kind == "timed_out":
        return (-1, "", "Process timed out", False)
    if kind == "cancelled":
        return (-1, "", "Process cancelled", False)
    if kind == "spawn_failed":
        return (-1, "", outcome.get("message"), False)
    code = outcome.get("exit_code") if kind == "exited" else -cast(int, outcome.get("signal"))
    try:
        # subprocess text mode: strict UTF-8, universal newlines, then rstrip.
        texts = [
            data.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n").rstrip()
            for data in (out, err)
        ]
    except UnicodeDecodeError:
        return (-1, "", None, False)
    return (code, texts[0], texts[1], False)


def _outcome_failures(outcome: Json) -> list[str]:
    kind = outcome.get("kind")
    if kind not in KINDS:
        return [f"outcome kind {kind!r}"]
    fields: dict[str, Callable[[object], bool]] = {
        "exited": lambda o: (
            _integer(_mapping(o).get("exit_code"))
            and 0 <= cast(int, _mapping(o)["exit_code"]) <= 255
        ),
        "signalled": lambda o: (
            _integer(_mapping(o).get("signal")) and isinstance(_mapping(o).get("signal_name"), str)
        ),
        "timed_out": lambda o: (
            isinstance(_mapping(o).get("timeout_seconds"), int | float)
            and not isinstance(_mapping(o).get("timeout_seconds"), bool)
        ),
        "cancelled": lambda o: isinstance(_mapping(o).get("before_start"), bool),
        "spawn_failed": lambda o: (
            _integer(_mapping(o).get("errno")) and isinstance(_mapping(o).get("message"), str)
        ),
    }
    return [] if fields[cast(str, kind)](outcome) else [f"outcome {kind} fields {outcome!r}"]


def _stream_failures(name: str, stream: Json) -> tuple[list[str], bytes | None]:
    failures: list[str] = []
    data = decode(stream.get("base64"))
    if data is None:
        return [f"evidence.{name}.base64 does not decode"], None
    size, file_size = stream.get("bytes"), stream.get("file_bytes")
    if not (_time(size) and _time(file_size)):
        return [f"evidence.{name} bytes {size!r} or file_bytes {file_size!r}"], None
    if size != len(data):
        failures.append(f"evidence.{name}.bytes {size} but {len(data)} decoded bytes")
    if stream.get("sha256") != hashlib.sha256(data).hexdigest():
        failures.append(f"evidence.{name}.sha256 is not the digest of the bytes")
    capture, reason = stream.get("capture"), stream.get("reason")
    if capture == "complete":
        if size != file_size:
            failures.append(f"evidence.{name} complete but bytes {size} != {file_size}")
        if reason is not None:
            failures.append(f"evidence.{name} complete with reason {reason!r}")
    elif capture == "incomplete":
        if cast(int, size) >= cast(int, file_size):
            failures.append(f"evidence.{name} incomplete but bytes {size} >= {file_size}")
        if reason not in ("deadline", "read_error"):
            failures.append(f"evidence.{name} reason {reason!r}")
    else:
        failures.append(f"evidence.{name} capture {capture!r}")
    return failures, data


def termination_failures(termination: Json, finalised: int) -> list[str]:
    """The termination block rules of the spec. Both leader orders are valid."""
    failures: list[str] = []
    term, kill = termination.get("term_sent_after_ms"), termination.get("kill_sent_after_ms")
    leader, exit_ = termination.get("leader_status"), termination.get("leader_exit_after_ms")
    empty, empty_at = termination.get("group_empty"), termination.get("group_empty_after_ms")
    if not (_time(term) and _time(kill)):
        return [f"termination term {term!r} or kill {kill!r} is not a time"]
    term, kill = cast(int, term), cast(int, kill)
    if not term <= kill <= finalised:
        failures.append("termination breaks term <= kill <= finalised")
    kind = _mapping(leader).get("kind")
    # The payload of each kind, as the Host CLI writes `LeaderStatus` in JSON mode.
    payload = {"exited": "exit_code", "signalled": "signal", "unavailable": None}
    if kind not in payload:
        failures.append(f"termination leader_status {leader!r}")
    else:
        field = payload[cast(str, kind)]
        keys = {"kind"} if field is None else {"kind", field}
        if set(_mapping(leader)) != keys or (
            field is not None and not _integer(_mapping(leader)[field])
        ):
            failures.append(f"termination leader_status {leader!r} has a malformed payload")
    unavailable = kind == "unavailable"
    if exit_ is None:
        if not unavailable:
            failures.append("termination leader_exit_after_ms null without unavailable")
    elif not _time(exit_):
        failures.append(f"termination leader_exit_after_ms {exit_!r}")
    elif unavailable:
        failures.append("termination leader_exit_after_ms set with unavailable")
    elif not term <= cast(int, exit_) <= finalised:
        failures.append("termination leader exit not between term and finalised")
    if empty not in ("yes", "no", "unknown"):
        failures.append(f"termination group_empty {empty!r}")
    if unavailable and empty != "no":
        failures.append("termination unavailable leader without group_empty no")
    if empty == "yes":
        if not _time(empty_at):
            failures.append("termination group_empty yes without group_empty_after_ms")
        else:
            after_exit = not _time(exit_) or cast(int, empty_at) >= cast(int, exit_)
            if cast(int, empty_at) < kill or not after_exit or cast(int, empty_at) > finalised:
                failures.append("termination group_empty_after_ms out of order")
    elif empty_at is not None:
        failures.append("termination group_empty_after_ms set without group_empty yes")
    derived = (
        empty == "yes" and kill <= trial.ROOT_BOUND_MS and finalised - kill <= trial.ROOT_BOUND_MS
    )
    if termination.get("within_bounds") is not derived:
        failures.append(
            f"termination within_bounds {termination.get('within_bounds')!r} "
            f"but the derived value is {derived}"
        )
    return failures


def record_failures(record: Json, expected_id: str, envelope: bool) -> list[str]:
    """Each broken validity rule of one record. An empty list means valid.

    `envelope` is true for a `file exec` envelope and false for the `record`
    of a `file cancel` reply.
    """
    failures: list[str] = []
    if envelope and record.get("record") != "agent":
        failures.append(f"record {record.get('record')!r}, not agent")
    outcome = record.get("outcome")
    if not isinstance(outcome, dict):
        return [*failures, "the record has no outcome"]
    outcome = cast(Json, outcome)
    failures += _outcome_failures(outcome)
    if failures:
        return failures
    if record.get("exec_id") != expected_id:
        failures.append(f"exec_id {record.get('exec_id')!r} is not {expected_id!r}")
    platform = record.get("legacy_projection")
    if platform not in ("macos", "linux"):
        failures.append(f"legacy_projection {platform!r}")
    evidence = _mapping(record.get("evidence"))
    streams: list[bytes] = []
    for name in ("stdout", "stderr"):
        stream_failures, data = _stream_failures(name, _mapping(evidence.get(name)))
        failures += stream_failures
        if data is not None:
            streams.append(data)
    lifecycle = record.get("lifecycle")
    if not isinstance(lifecycle, dict):
        return [*failures, "the record has no lifecycle"]
    lifecycle = cast(Json, lifecycle)
    for key in ("spawn_ms", "owner_lateness_ms", "finalised_after_ms"):
        if not _time(lifecycle.get(key)):
            failures.append(f"lifecycle.{key} {lifecycle.get(key)!r} is not a time")
    kind = outcome["kind"]
    started = not (kind == "spawn_failed" or (kind == "cancelled" and outcome["before_start"]))
    identity = [lifecycle.get(key) for key in ("leader_pid", "group_id", "duration_ms")]
    if started and not all(_time(value) for value in identity):
        failures.append("a started command lacks leader_pid, group_id or duration_ms")
    if not started and (any(v is not None for v in identity) or lifecycle.get("leader_start_time")):
        failures.append("a command that never started has a leader identity")
    alone = kind in ("exited", "signalled")
    remaining = record.get("remaining_group")
    if alone != (remaining is not None):
        failures.append("remaining_group is not present exactly for exited and signalled")
    if remaining is not None and remaining not in ("none", "present", "unknown"):
        failures.append(f"remaining_group {remaining!r}")
    termination = record.get("termination")
    terminated = started and kind in ("timed_out", "cancelled")
    finalised = lifecycle.get("finalised_after_ms")
    if terminated != (termination is not None) or (
        termination is not None and not isinstance(termination, dict)
    ):
        failures.append("termination is not present exactly for a terminated started command")
    elif isinstance(termination, dict) and _time(finalised):
        failures += termination_failures(cast(Json, termination), cast(int, finalised))
    if len(streams) == 2 and platform in ("macos", "linux"):
        expected = projection(cast(str, platform), outcome, streams[0], streams[1])
        actual = tuple(record.get(key) for key in LEGACY_KEYS)
        if (
            actual[0] != expected[0]
            or actual[1] != expected[1]
            or (expected[2] is not None and actual[2] != expected[2])
            or actual[3] is not expected[3]
        ):
            failures.append(f"legacy fields {actual!r} are not the {platform} projection")
    return failures


def bound_failures(record: Json) -> list[str]:
    """The acceptance bounds of one valid record."""
    failures: list[str] = []
    lifecycle = _mapping(record.get("lifecycle"))
    spawn, lateness = lifecycle.get("spawn_ms"), lifecycle.get("owner_lateness_ms")
    finalised = lifecycle.get("finalised_after_ms")
    termination = record.get("termination")
    if not (_time(spawn) and cast(int, spawn) <= trial.SPAWN_MS):
        failures.append(f"spawn_ms {spawn!r} exceeds {trial.SPAWN_MS}")
    if not (_time(lateness) and cast(int, lateness) <= trial.LATENESS_MS):
        failures.append(f"owner_lateness_ms {lateness!r} exceeds {trial.LATENESS_MS}")
    limit = trial.FINALISE_TERMINATED_MS if termination is not None else trial.FINALISE_MS
    if not (_time(finalised) and cast(int, finalised) <= limit):
        failures.append(f"finalised_after_ms {finalised!r} exceeds {limit}")
    if isinstance(termination, dict):
        block = cast(Json, termination)
        kill = block.get("kill_sent_after_ms")
        if block.get("group_empty") != "yes" or block.get("within_bounds") is not True:
            failures.append(
                f"group_empty {block.get('group_empty')!r} within_bounds "
                f"{block.get('within_bounds')!r}"
            )
        if not (_time(kill) and cast(int, kill) <= trial.ROOT_BOUND_MS):
            failures.append(f"kill_sent_after_ms {kill!r} exceeds the grace bound")
        elif _time(finalised) and cast(int, finalised) - cast(int, kill) > trial.ROOT_BOUND_MS:
            failures.append("SIGKILL to the published record exceeds 2000 ms")
    evidence = _mapping(record.get("evidence"))
    for name in ("stdout", "stderr"):
        if _mapping(evidence.get(name)).get("capture") != "complete":
            failures.append(f"evidence.{name} is not complete")
    return failures


def evidence(record: Json) -> tuple[bytes | None, bytes | None]:
    streams = _mapping(record.get("evidence"))
    return (
        decode(_mapping(streams.get("stdout")).get("base64")),
        decode(_mapping(streams.get("stderr")).get("base64")),
    )


def _read(path: Path) -> bytes | None:
    try:
        return path.read_bytes()
    except OSError:
        return None


def _json(data: bytes | None) -> Json | None:
    if data is None:
        return None
    try:
        value = json.loads(data)
    except ValueError:
        return None
    return cast(Json, value) if isinstance(value, dict) else None


def _flag(argv: list[str], flag: str) -> str:
    return argv[argv.index(flag) + 1] if flag in argv else ""


def _ok_envelope(receipt: Json | None) -> list[str]:
    if receipt is None:
        return ["the stdout is not one JSON object"]
    if receipt.get("schema_version") != "1.0" or receipt.get("ok") is not True:
        return [f"envelope {receipt.get('schema_version')!r}/ok {receipt.get('ok')!r}"]
    return []


def exec_failures(
    envelope: Json | None,
    argv: list[str],
    *,
    outcome: Json,
    stdout: bytes | None = None,
    stderr: bytes | None = None,
) -> list[str]:
    """Judge one `file exec` envelope: validity, bounds, outcome and exact bytes.

    `outcome` holds the required outcome fields. A None byte expectation is not
    compared.
    """
    failures = _ok_envelope(envelope)
    if failures or envelope is None:
        return failures
    if envelope.get("command") != argv[-1]:
        failures.append("the envelope command is not the planned command")
    validity = record_failures(envelope, _flag(argv, "--exec-id"), envelope=True)
    if validity:
        return failures + validity
    failures += bound_failures(envelope)
    actual = _mapping(envelope.get("outcome"))
    for key, value in outcome.items():
        if actual.get(key) != value or isinstance(actual.get(key), bool) != isinstance(value, bool):
            failures.append(f"outcome {key} {actual.get(key)!r}, expected {value!r}")
    out, err = evidence(envelope)
    if stdout is not None and out != stdout:
        failures.append(f"stdout bytes {out!r}, expected {stdout!r}")
    if stderr is not None and err != stderr:
        failures.append(f"stderr bytes {err!r}, expected {stderr!r}")
    return failures


def _snapshot(pid: str, rows: list[str], status: str) -> str:
    """Classify one `ps -p` snapshot of the known child."""
    if status == "1" and not rows:
        return "absent"
    if status != "0" or len(rows) != 1:
        return "unknown"
    fields = rows[0].split()
    if len(fields) < 10 or fields[0] != pid:
        return "unknown"
    if fields[3].startswith("Z"):
        return "zombie"
    return "present" if " ".join(fields[9:]) == "sleep 120" else "other"


def child_states(stdout: bytes | None) -> list[str]:
    """Classify each snapshot of a child check or a child poll, in order.

    The result is `["unknown"]` when the output is not the planned shape.
    """
    if stdout is None:
        return ["unknown"]
    lines = stdout.decode("utf-8", "replace").splitlines()
    if len(lines) < 2 or not re.fullmatch(r"child pid=\d+", lines[0]):
        return ["unknown"]
    pid = lines[0].removeprefix("child pid=")
    states: list[str] = []
    rows: list[str] = []
    for line in lines[1:]:
        status = re.fullmatch(r"child ps-exit=(\d+)", line)
        if status is None:
            rows.append(line)
            continue
        states.append(_snapshot(pid, rows, status.group(1)))
        rows = []
    if rows or not states:
        return ["unknown"]
    return states


def child_state(stdout: bytes | None) -> str:
    """Classify one child check: absent, present, zombie, other or unknown."""
    states = child_states(stdout)
    return states[0] if len(states) == 1 else "unknown"


def child_poll_failures(stdout: bytes | None) -> list[str]:
    """Judge the interruption poll: the known child is absent at its last poll.

    A present or zombie child before the last poll is allowed, because the agent
    may still be within its 2 s grace and 2 s kill interval. A poll that shows
    another command or an unknown state fails, and so does a poll after absence.
    """
    states = child_states(stdout)
    failures: list[str] = []
    if len(states) > trial.CHILD_POLLS:
        failures.append(f"{len(states)} polls, more than {trial.CHILD_POLLS}")
    if any(state not in ("present", "zombie") for state in states[:-1]):
        failures.append(f"known child polls {states!r}")
    elif states[-1] != "absent":
        failures.append(f"known child {states[-1]} after {len(states)} polls")
    return failures


def role(name: str, argv: list[str]) -> str:
    """The role of one call of the frozen shape."""
    if name in RESOURCE:
        return name
    if name == "cancel":
        return "cancel-helper"
    if name == "app-identity":
        return "app-helper"
    if argv[1:3] == ["agent", "health"]:
        return "health"
    if argv[1:3] == ["file", "cancel"]:
        return "file-cancel"
    if argv[1:3] in (["file", "upload"], ["file", "download"]):
        return "transfer"
    return "exec"


def _cancelled_record(record: Json | None, exec_id_: str) -> list[str]:
    """Judge a terminal `file cancel` reply that must carry a cancelled record."""
    failures = _ok_envelope(record)
    if failures or record is None:
        return failures
    if record.get("exec_id") != exec_id_ or record.get("state") != "terminal":
        return [f"cancel reply {record.get('exec_id')!r} state {record.get('state')!r}"]
    inner = _mapping(record.get("record"))
    failures = record_failures(inner, exec_id_, envelope=False)
    if failures:
        return failures
    failures = bound_failures(inner)
    if _mapping(inner.get("outcome")) != {"kind": "cancelled", "before_start": False}:
        failures.append(f"outcome {inner.get('outcome')!r}, expected a started cancellation")
    out, err = evidence(inner)
    if (out, err) != (trial.PARTIAL_STDOUT, trial.PARTIAL_STDERR):
        failures.append(f"partial evidence {out!r} {err!r}")
    return failures


class Clone:
    """The evidence of one clone directory."""

    def __init__(self, directory: Path) -> None:
        self.directory = directory
        self.inventory = _json(_read(directory / "inventory.json")) or {}
        self.summary = _json(_read(directory / "summary.json")) or {}
        self.run = Path(str(self.summary.get("run", directory / "missing-run")))
        self.result = _json(_read(self.run / "result.json")) or {}
        self.bounds = _mapping(self.inventory.get("bounds"))
        plan = _mapping(self.inventory.get("plan"))
        calls = [_mapping(call) for call in cast(list[Any], plan.get("calls") or [])]
        resources = [_mapping(r) for r in cast(list[Any], plan.get("resources") or [])]
        for resource in resources[:1]:
            acquire, *teardown = (_mapping(resource.get(key)) for key in RESOURCE)
            calls = [acquire, *calls, *teardown]
        self.planned = {str(call.get("name")): call for call in calls}
        self.order = [str(call.get("name")) for call in calls]
        records = cast(list[Any], self.result.get("calls") or [])
        self.receipts = {str(_mapping(r).get("name")): _mapping(r) for r in records}

    def argv(self, name: str) -> list[str]:
        return [str(token) for token in cast(list[Any], self.planned[name].get("argv") or [])]

    def stdout(self, name: str) -> bytes | None:
        return _read(self.run / f"{name}.stdout")

    def envelope(self, name: str) -> Json | None:
        return _json(self.stdout(name))


def _receipt_failures(clone: Clone, name: str, expected_exit: int = 0) -> list[str]:
    """The controller receipt of one call: an ordinary reaped exit with its own bytes."""
    receipt = clone.receipts.get(name)
    if receipt is None:
        return ["no controller receipt"]
    failures: list[str] = []
    expected_reason = "interrupted" if name == "interrupt" else "exit"
    if receipt.get("ok") is not True or receipt.get("reason") != expected_reason:
        failures.append(f"controller ok {receipt.get('ok')!r} reason {receipt.get('reason')!r}")
    if receipt.get("returncode") != expected_exit:
        failures.append(f"return code {receipt.get('returncode')!r}, expected {expected_exit}")
    streams = _mapping(receipt.get("streams"))
    stdout = clone.stdout(name)
    for stream in ("stdout", "stderr"):
        if _mapping(streams.get(stream)).get("completeness") != "eof":
            failures.append(f"{stream} completeness {_mapping(streams.get(stream))!r}")
    digest = _mapping(streams.get("stdout")).get("sha256")
    if stdout is None or hashlib.sha256(stdout).hexdigest() != digest:
        failures.append("the stored stdout is not the captured stdout")
    return failures


def _elapsed(clone: Clone, name: str, limit: float) -> list[str]:
    elapsed = clone.receipts.get(name, {}).get("elapsed")
    if not isinstance(elapsed, int | float) or elapsed > limit:
        return [f"host elapsed {elapsed!r} exceeds {limit:g} s"]
    return []


def _health_failures(receipt: Json | None) -> list[str]:
    failures = _ok_envelope(receipt)
    if not failures and receipt is not None and receipt.get("reachable") is not True:
        failures.append(f"health reachable {receipt.get('reachable')!r}")
    return failures


def judge_exec(clone: Clone, name: str) -> list[str]:
    """Judge one `file exec` call of the shape by its name."""
    argv = clone.argv(name)
    guest = int(_flag(argv, "--timeout") or 0)
    failures = _elapsed(clone, name, guest + trial.HOST_MARGIN)
    envelope = clone.envelope(name)
    if name == "interrupt":
        return failures + interrupt_failures(clone)
    failures += _receipt_failures(clone, name)
    ceiling = trial.INVENTORY if name == "baseline" else trial.OBSERVATION
    if name != "timeout":
        failures += _elapsed(clone, name, ceiling)
    exited0: Json = {"kind": "exited", "exit_code": 0}
    if name == "intact":
        return failures + exec_failures(
            envelope,
            argv,
            outcome=exited0,
            stdout=trial.ORACLE["stdout.bin"],
            stderr=trial.ORACLE["stderr.bin"],
        )
    if name == "exit-23":
        return failures + exec_failures(envelope, argv, outcome={"kind": "exited", "exit_code": 23})
    if name == "timeout":
        return failures + timeout_failures(envelope, argv)
    if argv[-1] == trial.ORDINARY:
        return failures + exec_failures(
            envelope, argv, outcome=exited0, stdout=b"ordinary-ok", stderr=b""
        )
    failures += exec_failures(envelope, argv, outcome=exited0)
    out = evidence(envelope)[0] if envelope is not None else None
    if name == "baseline" and (out is None or not out.endswith(b"BASELINE_ABSENT\n")):
        failures.append("the baseline does not end with BASELINE_ABSENT")
    if name == "cleanup" and (out is None or not out.endswith(b"cleanup confirmed\n")):
        failures.append("the guest cleanup is not confirmed")
    if name == "identity":
        failures += agent_identity_failures(clone, out)
    if any(argv[-1] == trial.child_check(case) for case in trial.CHILD_CASES):
        state = child_state(out)
        if state != "absent":
            failures.append(f"known child {state}")
    if argv[-1] == trial.child_poll("interrupt"):
        failures += child_poll_failures(out)
    return failures


LINUX_LISTING = re.compile(r"[0-9a-f]{64}  \./\S.*\.py")


def agent_identity(os_name: str, stdout: bytes | None) -> str | None:
    """The agent identity that the `identity` case printed, or None.

    For macOS it is the digest on the first line, which `shasum` prints for the
    agent binary. For Linux it is the SHA-256 of the `sha256sum` listing lines
    at the start of the output, each with its line feed.
    """
    lines = (stdout or b"").decode("utf-8", "replace").splitlines()
    if os_name == "macos":
        first = (
            re.fullmatch(rf"([0-9a-f]{{64}})  {re.escape(trial.MACOS_AGENT)}", lines[0])
            if lines
            else None
        )
        return first.group(1) if first else None
    listing: list[str] = []
    for line in lines:
        if not LINUX_LISTING.fullmatch(line):
            break
        listing.append(line)
    if not listing:
        return None
    return hashlib.sha256("".join(f"{line}\n" for line in listing).encode()).hexdigest()


def agent_identity_failures(clone: Clone, stdout: bytes | None) -> list[str]:
    expected = _mapping(clone.inventory.get("identities")).get("agent_sha256")
    actual = agent_identity(str(clone.inventory.get("os")), stdout)
    if not isinstance(expected, str) or actual != expected:
        return [f"answering agent identity {actual!r}, frozen {expected!r}"]
    return []


def app_identity_failures(clone: Clone) -> list[str]:
    """The app that answered: its build and its executable equal the freeze."""
    failures = _receipt_failures(clone, "app-identity")
    failures += _elapsed(clone, "app-identity", trial.APP_HELPER_BOUND)
    record = clone.envelope("app-identity")
    if (
        record is None
        or record.get("schema") != "acceptance-k223-app/1"
        or record.get("ok") is not True
    ):
        return failures + ["the app helper record is absent or malformed"]
    doctor = _mapping(record.get("doctor"))
    elapsed = doctor.get("elapsed")
    if not isinstance(elapsed, int | float) or elapsed > trial.OBSERVATION:
        failures.append(f"doctor took {elapsed!r} s")
    report = _json(decode(doctor.get("stdout")))
    running = _mapping(_mapping(_mapping(_mapping(report).get("checks")).get("app")).get("running"))
    identities = _mapping(clone.inventory.get("identities"))
    for key, field in (("app_build", "build"), ("app_executable", "executable")):
        expected = identities.get(key)
        if not isinstance(expected, str) or running.get(field) != expected:
            failures.append(f"answering app {field} {running.get(field)!r}, frozen {expected!r}")
    return failures + tool_identity_failures(_mapping(report), identities)


def tool_identity_failures(report: Json, identities: Json) -> list[str]:
    """The answering app loaded the frozen configuration, and `doctor` resolves the frozen Tart.

    The configuration file can name another Tart. The app reads it only when it
    starts, and `changed_since_evaluation` compares the file now with the file
    that the app evaluated. The Host CLI reads the same file, so its resolved
    Tart is the Tart that the configuration selects.
    """
    checks = _mapping(report.get("checks"))
    configuration = _mapping(_mapping(checks.get("app")).get("configuration"))
    expected = identities.get("app_config")
    failures: list[str] = []
    if (
        not isinstance(expected, str)
        or configuration.get("verdict") != "loaded"
        or configuration.get("file") != expected
        or configuration.get("changed_since_evaluation") is not False
    ):
        failures.append(
            f"answering app configuration {configuration.get('file')!r} "
            f"verdict {configuration.get('verdict')!r} changed "
            f"{configuration.get('changed_since_evaluation')!r}, frozen {expected!r}"
        )
    tools = cast(list[Any], _mapping(checks.get("host_tools")).get("tools") or [])
    tarts = [_mapping(tool).get("path") for tool in tools if _mapping(tool).get("name") == "tart"]
    tart = identities.get("tart")
    if not isinstance(tart, str) or tarts != [tart]:
        failures.append(f"resolved Tart {tarts!r}, frozen {tart!r}")
    return failures


def timeout_failures(envelope: Json | None, argv: list[str]) -> list[str]:
    """The intentional 2 s timeout: outcome, bounds, duration and partial bytes."""
    failures = exec_failures(
        envelope,
        argv,
        outcome={"kind": "timed_out", "timeout_seconds": trial.GUEST_TIMEOUT},
        stdout=trial.PARTIAL_STDOUT,
        stderr=trial.PARTIAL_STDERR,
    )
    duration = _mapping(_mapping(envelope).get("lifecycle")).get("duration_ms")
    if not (_time(duration) and TIMEOUT_FLOOR_MS <= cast(int, duration) <= TIMEOUT_CEILING_MS):
        failures.append(f"duration_ms {duration!r} outside the 2 s timeout bounds")
    return failures


def interrupt_failures(clone: Clone) -> list[str]:
    """The abrupt interruption: SIGTERM after 2 s, signal death within 2 s, no SIGKILL."""
    failures = _receipt_failures(clone, "interrupt", trial.INTERRUPT_EXIT)
    receipt = clone.receipts.get("interrupt", {})
    signals = [_mapping(event) for event in cast(list[Any], receipt.get("signals") or [])]
    if [event.get("signal") for event in signals] != ["TERM"]:
        return failures + [f"controller signals {[e.get('signal') for e in signals]!r}"]
    if signals[0].get("scope") != "pid" or signals[0].get("outcome") != "sent":
        failures.append(f"SIGTERM {signals[0]!r}")
    start, sent = receipt.get("started_monotonic"), signals[0].get("monotonic")
    leader = receipt.get("leader_exit_monotonic")
    if not all(isinstance(v, int | float) for v in (start, sent, leader)):
        return failures + ["the interrupt receipt lacks its monotonic times"]
    if cast(float, sent) - cast(float, start) < trial.CANCEL_AFTER:
        failures.append("SIGTERM came before 2 s")
    if cast(float, leader) - cast(float, sent) > trial.SIGTERM_DEATH:
        failures.append("the Host CLI outlived SIGTERM by more than 2 s")
    return failures


def cancel_helper_failures(clone: Clone) -> list[str]:
    """Explicit cancellation: health during the command, then one waited cancellation."""
    failures = _receipt_failures(clone, "cancel")
    failures += _elapsed(clone, "cancel", trial.HELPER_BOUND)
    record = clone.envelope("cancel")
    if record is None or record.get("schema") != "acceptance-k223-cancel/1":
        return failures + ["the helper record is absent or malformed"]
    if record.get("ok") is not True or record.get("exec_running_at_cancel") is not True:
        failures.append(
            f"helper ok {record.get('ok')!r} running {record.get('exec_running_at_cancel')!r}"
        )
    argv = clone.argv("cancel")
    exec_id_ = _flag(argv, "--exec-id")
    health, cancel, run = (_mapping(record.get(k)) for k in ("health", "cancel", "exec"))
    times = [
        child.get(key)
        for child in (health, cancel, run)
        for key in ("started_after", "ended_after")
    ]
    if not all(isinstance(v, int | float) and not isinstance(v, bool) for v in times):
        return failures + ["the helper record lacks its times"]
    if health.get("returncode") != 0:
        failures.append(f"health exit {health.get('returncode')!r}")
    failures += [f"health: {f}" for f in _health_failures(_json(decode(health.get("stdout"))))]
    health_time = cast(float, health["ended_after"]) - cast(float, health["started_after"])
    if health_time > trial.HEALTH_DURING:
        failures.append(f"health during the command took {health_time:.3f} s")
    trigger = cast(float, cancel["started_after"])
    if not trial.CANCEL_AFTER <= trigger <= trial.CANCEL_AFTER + CANCEL_WINDOW:
        failures.append(f"cancellation started at {trigger:.3f} s")
    if cast(float, health["ended_after"]) > trigger:
        failures.append("health ended after the cancellation started")
    reply_time = cast(float, cancel["ended_after"]) - trigger
    if reply_time > trial.CANCEL_REPLY:
        failures.append(f"the cancellation reply took {reply_time:.3f} s")
    if cancel.get("returncode") != 0:
        failures.append(f"file cancel exit {cancel.get('returncode')!r}")
    reply = _json(decode(cancel.get("stdout")))
    failures += [f"cancel: {f}" for f in _cancelled_record(reply, exec_id_)]
    if run.get("returncode") != 0:
        failures.append(f"file exec exit {run.get('returncode')!r}")
    if cast(float, run["ended_after"]) > trial.CHILD_GUEST + trial.HOST_MARGIN:
        failures.append("file exec exceeded its guest budget plus 20 s")
    envelope = _json(decode(run.get("stdout")))
    exec_argv = [str(token) for token in cast(list[Any], run.get("argv") or [])]
    if not exec_argv or _flag(exec_argv, "--exec-id") != exec_id_ or "--json" not in exec_argv:
        return failures + ["the helper's file exec argv is not the planned one"]
    failures += [
        f"exec: {f}"
        for f in exec_failures(
            envelope,
            exec_argv,
            outcome={"kind": "cancelled", "before_start": False},
            stdout=trial.PARTIAL_STDOUT,
            stderr=trial.PARTIAL_STDERR,
        )
    ]
    if envelope is not None and reply is not None:
        inner = _mapping(reply.get("record"))
        differing = [k for k in (*LEGACY_KEYS, *TYPED_KEYS) if envelope.get(k) != inner.get(k)]
        if differing:
            failures.append(f"the exec and cancel records differ in {differing}")
        failures += overlap_failures(envelope, health, cancel, run)
    return failures


def spawned_by(lifecycle: Json, cancel: Json, run: Json) -> float | None:
    """The latest helper time at which the command can have spawned, or None.

    `duration_ms` runs from the spawn to the outcome, and `finalised_after_ms`
    from the outcome to the published record. Each agent rounds down, so the
    true times are at least these values. Both replies end after the record is
    published, so the spawn came at least their sum before the earlier reply end.
    The latency of that reply after publication stays inside the result.
    """
    duration, finalised = lifecycle.get("duration_ms"), lifecycle.get("finalised_after_ms")
    if not (_time(duration) and _time(finalised)):
        return None
    published_by = min(cast(float, cancel["ended_after"]), cast(float, run["ended_after"]))
    return published_by - (cast(int, duration) + cast(int, finalised)) / 1000


def overlap_failures(envelope: Json, health: Json, cancel: Json, run: Json) -> list[str]:
    """Health ran during the command: it started after the spawn and ended before the trigger.

    The caller checks that health ended before the cancellation started. The
    outcome follows the cancellation, so health then ended before the outcome.
    """
    latest = spawned_by(_mapping(envelope.get("lifecycle")), cancel, run)
    if latest is None:
        return ["the record lacks duration_ms or finalised_after_ms"]
    if cast(float, health["started_after"]) < latest:
        return [
            f"health started at {cast(float, health['started_after']):.3f} s, "
            f"before {latest:.3f} s, the latest possible spawn"
        ]
    return []


def transfer_failures(clone: Clone, name: str) -> list[str]:
    argv = clone.argv(name)
    failures = _elapsed(clone, name, trial.TRANSFER)
    receipt = clone.envelope(name)
    if name == "download-missing":
        failures += _receipt_failures(clone, name, 1)
        if receipt is None or receipt.get("ok") is not False:
            return failures + ["the missing download did not fail with a typed envelope"]
        if receipt.get("code") != "DOWNLOAD_FAILED":
            failures.append(f"error code {receipt.get('code')!r}")
        if Path(argv[6]).exists():
            failures.append("the missing download left a local file")
        return failures
    failures += _receipt_failures(clone, name)
    failures += _ok_envelope(receipt)
    if failures or receipt is None:
        return failures
    upload = argv[2] == "upload"
    local, remote = (Path(argv[5]), argv[6]) if upload else (Path(argv[6]), argv[5])
    if receipt.get("remote") != remote:
        failures.append(f"remote {receipt.get('remote')!r}, expected {remote!r}")
    data = _read(local)
    if data is None:
        return failures + ["the local file is absent"]
    if receipt.get("bytes") != len(data):
        failures.append(f"bytes {receipt.get('bytes')!r} but the file has {len(data)}")
    if not upload:
        expected = trial.ORACLE.get(local.name, trial.PARTIAL)
        if data != expected:
            failures.append(f"downloaded bytes {data.hex()} are not {expected.hex()}")
    return failures


def judge_call(clone: Clone, name: str) -> list[str]:
    """Each failure of one call, by its role."""
    argv = clone.argv(name)
    kind = role(name, argv)
    host_cli = kind in ("exec", "health", "file-cancel", "transfer", "cleanup")
    if host_cli and "--json" not in argv:
        return ["the Host CLI call does not use JSON mode"]
    if kind == "exec" or name == "cleanup":
        failures = judge_exec(clone, name)
        if name == "cleanup":
            failures += _elapsed(clone, name, trial.GUEST_CLEANUP)
        return failures
    if kind == "health":
        failures = _receipt_failures(clone, name) + _elapsed(clone, name, trial.OBSERVATION)
        return failures + _health_failures(clone.envelope(name))
    if kind == "file-cancel":
        failures = _receipt_failures(clone, name) + _elapsed(clone, name, trial.CANCEL_REPLY)
        return failures + _cancelled_record(clone.envelope(name), _flag(argv, "--exec-id"))
    if kind == "cancel-helper":
        return cancel_helper_failures(clone)
    if kind == "app-helper":
        return app_identity_failures(clone)
    if kind == "transfer":
        return transfer_failures(clone, name)
    failures = _receipt_failures(clone, name)
    limits = {
        "acquire": float(clone.bounds.get("acquire", 0)),
        "dispose": trial.DISPOSAL,
        "inspect": trial.INVENTORY,
    }
    return failures + _elapsed(clone, name, limits[name])


def expected_shape(os_name: str) -> list[str]:
    """The call names of the frozen shape, in the order the controller runs them."""
    placeholder = trial.Clone(os_name, 1, "0" * 32, "vm", "/cli", "/python", Path("/"))
    return ["acquire", *(call.name for call in trial.clone_calls(placeholder)), *RESOURCE[1:]]


def _normal(value: object) -> object:
    """The JSON form in which the controller stores a plan: bytes become hex."""
    return json.loads(_json_bytes(value))


def plan_failures(inventory: Json) -> list[str]:
    """Rebuild the canonical plan from the frozen identities and compare each field.

    The identities are the run id, the VM, the golden, the tools and the
    download directory. Every argv, budget and expectation must then equal the
    plan that acceptance_k223.py builds for them.
    """
    plan = _mapping(inventory.get("plan"))
    os_name, index = inventory.get("os"), inventory.get("index")
    try:
        calls = {str(c["name"]): c for c in cast(list[Json], plan["calls"])}
        resource = cast(list[Json], plan["resources"])[0]
        acquire = cast(list[str], resource["acquire"]["argv"])
        dispose = cast(list[str], resource["dispose"]["argv"])
        run_id = str(plan["run_id"])
        clone = trial.Clone(
            os_name=str(os_name),
            index=cast(int, index),
            run_id=run_id,
            vm=acquire[acquire.index("--id") + 1],
            cli=acquire[0],
            python=str(calls["cancel"]["argv"][0]),
            downloads=Path(str(calls["download-artifact-bin"]["argv"][6])).parent,
        )
        golden = acquire[acquire.index("--base") + 1]
        tart = dispose[dispose.index("--tart") + 1]
        acquire_budget = float(_mapping(inventory.get("bounds"))["acquire"])
    except (KeyError, IndexError, TypeError, ValueError):
        return ["the frozen plan lacks an identity of the acceptance shape"]
    failures: list[str] = []
    if clone.vm != f"k223-{os_name}-{index:02d}-{run_id}":
        failures.append(f"VM {clone.vm!r} is not the canonical clone identity")
    expected_calls = [asdict(call) for call in trial.clone_calls(clone)]
    if _normal(plan["calls"]) != _normal(expected_calls):
        failures.append("the frozen calls are not the acceptance calls")
    expected = trial._resource(clone, golden, tart, acquire_budget)  # pyright: ignore[reportPrivateUsage]
    if _normal(resource) != _normal(asdict(expected)):
        failures.append("the frozen resource is not the acceptance resource")
    if (plan.get("term_grace"), plan.get("kill_wait")) != (trial.TERM_GRACE, trial.KILL_WAIT):
        failures.append("the frozen termination bounds are not 2 s and 2 s")
    if _normal(inventory.get("bounds")) != _normal(trial.bounds(_manifest_bounds(inventory))):
        failures.append("the frozen bounds are not the acceptance bounds")
    return failures


def _manifest_bounds(inventory: Json) -> trial.Manifest:
    bounds = _mapping(inventory.get("bounds"))
    return trial.Manifest(
        path=Path("/"),
        cli="",
        tart="",
        goldens={},
        boot=float(bounds.get("boot", 0)),
        slot=float(bounds.get("slot", 0)),
        agents={},
        app_build="",
        app_executable="",
        sha256={},
    )


def _file_sha256(path: Path) -> str | None:
    data = _read(path)
    return None if data is None else hashlib.sha256(data).hexdigest()


def freeze_failures(clone: Clone) -> list[str]:
    """The run measured the frozen candidate, and the checker is the code that ran with it.

    The manifest must still have its frozen digest and give the same digests and
    identities as the inventory. Each frozen path, the manifest and each fixture
    file must have its frozen digest before and after the run. Each checker
    module must have, now, the digest that the controller took after the run.
    """
    failures: list[str] = []
    inventory = clone.inventory
    before = _mapping(clone.result.get("subjects_before"))
    after = _mapping(clone.result.get("subjects_after"))
    manifest_path = Path(str(inventory.get("manifest", "")))
    manifest_sha256 = inventory.get("manifest_sha256")
    frozen = _mapping(inventory.get("frozen_sha256"))
    if not isinstance(manifest_sha256, str) or _file_sha256(manifest_path) != manifest_sha256:
        return [f"the manifest {manifest_path} no longer has its frozen digest"]
    try:
        manifest = trial.read_manifest(manifest_path)
    except (OSError, ValueError) as error:
        return [f"the manifest no longer reads: {error}"]
    if manifest.sha256 != frozen:
        failures.append("the inventory digests are not the manifest digests")
    os_name = str(inventory.get("os"))
    if os_name in trial.OSES and _mapping(inventory.get("identities")) != trial.identities(
        manifest, os_name
    ):
        failures.append("the inventory identities are not the manifest identities")
    fixture = trial.FIXTURES.get(os_name)
    expected = {str(manifest_path): manifest_sha256, **{str(k): str(v) for k, v in frozen.items()}}
    if fixture is not None:
        for name, digest in trial.FIXTURE_SHA256[os_name].items():
            expected[str((fixture / name).absolute())] = digest
    for path, digest in expected.items():
        if before.get(path) != digest or after.get(path) != digest:
            failures.append(
                f"subject {path}: before {before.get(path)!r} after {after.get(path)!r}, "
                f"frozen {digest!r}"
            )
    for module in trial.checker_modules():
        key = str(module.absolute())
        if _file_sha256(module) != after.get(key):
            failures.append(f"checker module {key} differs from the run's subject digest")
    python = clone.argv("app-identity")[:1] if "app-identity" in clone.planned else []
    if not python or str(Path(python[0])) not in frozen:
        failures.append(f"the interpreter {python!r} is not a frozen pin")
    if os_name in trial.OSES:
        failures += golden_disk_failures(clone, manifest, os_name)
    return failures


def golden_disk_failures(clone: Clone, manifest: trial.Manifest, os_name: str) -> list[str]:
    """The golden disk of the clone had its frozen digest before and after the run."""
    record = _json(_read(clone.directory / "golden-disk.json"))
    if record is None:
        return ["the golden disk record is absent"]
    frozen = manifest.golden_disks[os_name]
    failures: list[str] = []
    if record.get("path") != frozen["path"] or record.get("frozen") != frozen["sha256"]:
        failures.append("the golden disk record does not name the frozen disk")
    for phase in ("before", "after"):
        digest = _mapping(record.get(phase)).get("sha256")
        if digest != frozen["sha256"]:
            failures.append(f"golden disk {phase} {digest!r}, frozen {frozen['sha256']!r}")
    return failures


def execution_failures(clone: Clone) -> list[str]:
    """The recorded execution: the planned names in the planned order, with the planned argv."""
    records = [_mapping(r) for r in cast(list[Any], clone.result.get("calls") or [])]
    names = [str(record.get("name")) for record in records]
    failures: list[str] = []
    if names != clone.order:
        failures.append("the recorded call order is not the plan order")
    for record in records:
        name = str(record.get("name"))
        planned = clone.planned.get(name)
        argv = [str(token) for token in cast(list[Any], record.get("argv") or [])]
        if planned is None or argv != clone.argv(name):
            failures.append(f"the receipt argv of {name} is not its planned argv")
    raw = [record.get("started_monotonic") for record in records]
    starts = [
        float(start)
        for start in raw
        if isinstance(start, int | float) and not isinstance(start, bool)
    ]
    if len(starts) != len(raw):
        failures.append("a receipt lacks started_monotonic")
    elif any(later < earlier for earlier, later in zip(starts, starts[1:], strict=False)):
        failures.append("the recorded start times decrease")
    return failures


def _items(clone: Clone) -> dict[str, Any]:
    """The three original items that the corruption controls damage."""
    artifact = clone.directory / "downloads" / "artifact.bin"
    return {
        "intact": (clone.run / "intact.stdout", clone.argv("intact")),
        "artifact": (artifact, None),
        "timeout": (clone.run / "timeout.stdout", clone.argv("timeout")),
    }


def _judge_item(item: str, data: bytes | None, argv: list[str] | None) -> list[str]:
    if item == "artifact":
        expected = trial.ORACLE["artifact.bin"]
        return [] if data == expected else [f"artifact bytes {data!r} are not the oracle"]
    envelope = _json(data)
    if item == "intact":
        return exec_failures(
            envelope,
            cast(list[str], argv),
            outcome={"kind": "exited", "exit_code": 0},
            stdout=trial.ORACLE["stdout.bin"],
            stderr=trial.ORACLE["stderr.bin"],
        )
    return timeout_failures(envelope, cast(list[str], argv))


def _appended_lf(data: bytes) -> bytes:
    envelope = cast(Json, json.loads(data))
    stream = envelope["evidence"]["stdout"]
    damaged = cast(bytes, decode(stream["base64"])) + b"\n"
    stream.update(
        base64=base64.b64encode(damaged).decode("ascii"),
        bytes=len(damaged),
        file_bytes=len(damaged),
        sha256=hashlib.sha256(damaged).hexdigest(),
    )
    return json.dumps(envelope).encode()


def _flipped(data: bytes) -> bytes:
    damaged = bytearray(data)
    damaged[2] ^= 0x01
    return bytes(damaged)


def _without_outcome(data: bytes) -> bytes:
    envelope = cast(Json, json.loads(data))
    del envelope["outcome"]
    return json.dumps(envelope).encode()


def _contradictory_bounds(data: bytes) -> bytes:
    envelope = cast(Json, json.loads(data))
    envelope["termination"].update(group_empty="no", group_empty_after_ms=None)
    return json.dumps(envelope).encode()


def _contradictory_order(data: bytes) -> bytes:
    envelope = cast(Json, json.loads(data))
    termination = envelope["termination"]
    if termination["kill_sent_after_ms"] >= 1:
        termination["group_empty_after_ms"] = termination["kill_sent_after_ms"] - 1
    else:
        termination["leader_exit_after_ms"] = envelope["lifecycle"]["finalised_after_ms"] + 1
    return json.dumps(envelope).encode()


CONTROLS: dict[str, tuple[str, Callable[[bytes], bytes]]] = {
    "appended-lf": ("intact", _appended_lf),
    "flipped-byte": ("artifact", _flipped),
    "missing-outcome": ("timeout", _without_outcome),
    "contradictory-bounds": ("timeout", _contradictory_bounds),
    "contradictory-order": ("timeout", _contradictory_order),
}


def corruption_controls(clone: Clone) -> dict[str, Any]:
    """Damage independent copies, require each to fail, then recheck the originals."""
    items = _items(clone)
    before: dict[str, bytes | None] = {item: _read(path) for item, (path, _) in items.items()}
    digests = {
        item: None if data is None else hashlib.sha256(data).hexdigest()
        for item, data in before.items()
    }
    originals = {item: _judge_item(item, data, items[item][1]) for item, data in before.items()}
    controls: dict[str, Any] = {}
    for name, (item, damage) in CONTROLS.items():
        data = before[item]
        if data is None or originals[item]:
            controls[name] = {"rejected": False, "reason": "the original is absent or fails"}
            continue
        try:
            damaged = damage(bytes(data))
        except (KeyError, TypeError, ValueError, IndexError) as error:
            controls[name] = {"rejected": False, "reason": f"damage failed: {error!r}"}
            continue
        failures = _judge_item(item, damaged, items[item][1])
        controls[name] = {
            "item": item,
            "damaged_sha256": hashlib.sha256(damaged).hexdigest(),
            "rejected": bool(failures),
            "failures": failures,
        }
    after: dict[str, bytes | None] = {item: _read(path) for item, (path, _) in items.items()}
    rechecked = {item: _judge_item(item, data, items[item][1]) for item, data in after.items()}
    unchanged = all(
        data is not None and hashlib.sha256(data).hexdigest() == digests[item]
        for item, data in after.items()
    )
    ok = (
        all(control["rejected"] is True for control in controls.values())
        and not any(originals.values())
        and not any(rechecked.values())
        and unchanged
    )
    return {
        "ok": ok,
        "originals_sha256": digests,
        "originals": originals,
        "controls": controls,
        "originals_after": rechecked,
        "originals_unchanged": unchanged,
    }


def check_clone(directory: Path) -> dict[str, Any]:
    """Judge one clone directory. The result names every failure."""
    clone = Clone(directory)
    failures: list[str] = []
    os_name = str(clone.inventory.get("os"))
    if os_name not in trial.OSES:
        return {"ok": False, "failures": ["inventory.json is absent or names no OS"]}
    if clone.order != expected_shape(os_name):
        return {"ok": False, "failures": ["the frozen plan is not the acceptance shape"]}
    failures += plan_failures(clone.inventory)
    run_plan = _json(_read(clone.run / "plan.json"))
    if run_plan != clone.inventory.get("plan"):
        failures.append("the controller plan differs from the frozen inventory plan")
    names = [_mapping(r).get("name") for r in cast(list[Any], clone.result.get("calls") or [])]
    failures += _run_failures(clone.run, clone.result, cast(list[object], names))
    failures += execution_failures(clone)
    failures += freeze_failures(clone)
    if clone.summary.get("verdict") is not True or clone.summary.get("residuals") != []:
        failures.append(
            f"controller verdict {clone.summary.get('verdict')!r} "
            f"residuals {clone.summary.get('residuals')!r}"
        )
    ledger = [_mapping(entry) for entry in cast(list[Any], clone.summary.get("ledger") or [])]
    if [(e.get("kind"), e.get("state")) for e in ledger] != [("vm", "disposed")]:
        failures.append(f"ledger {ledger!r}")
    cases: dict[str, Any] = {}
    for name in clone.order:
        if name not in clone.receipts:
            cases[name] = {"ok": False, "failures": ["unreached"]}
            continue
        case_failures = judge_call(clone, name)
        cases[name] = {"ok": not case_failures, "failures": case_failures}
    controls = (
        corruption_controls(clone)
        if all(cases.get(n, {}).get("ok") for n in ("intact", "timeout", "download-artifact-bin"))
        else {"ok": False, "reason": "an original item failed or was unreached"}
    )
    ok = not failures and all(case["ok"] for case in cases.values()) and controls["ok"] is True
    return {
        "ok": ok,
        "clone": directory.name,
        "run": str(clone.run),
        "failures": failures,
        "baseline_ok": cases.get("baseline", {}).get("ok") is True,
        "cases": cases,
        "corruption_controls": controls,
    }


def main() -> None:
    verdict = check_clone(Path(sys.argv[1]).resolve())
    print(json.dumps(verdict, indent=2))
    raise SystemExit(0 if verdict["ok"] is True else 1)


if __name__ == "__main__":
    main()
