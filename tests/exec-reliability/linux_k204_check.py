"""Judge one linux_k204.py run against its frozen plan.

    linux_k204_check.py RUN_DIR

The checker gives four results:

- `evidence`: complete, incomplete or absent.
- `survival`: issue #14 failure B in the exact fork/wait controls. The value is
  reproduced, not-reproduced or unknown.
- `fixture_survival`: the same question for the r5 `timeout` and `cancel` children.
  The value is listed, not-listed or unknown.
- `stall`: issue #14 failure A, the post-timeout inspection stall. The value is
  reproduced, not-reproduced or unknown.

The Linux agent answers a deadline with exit -1, empty stdout and the stderr
`Process timed out`. Its receipt has no timeout flag, so the CLI receipt shows
`timed_out` false. A guest timeout is therefore exit -1 with that stderr. The field
`timed_out` is recorded and never proves cleanup.

Each guest receipt must be one typed object: schema 1.0, `ok` true, the planned
command, an integer `exit_code`, a boolean `timed_out` and string streams. A raw
record must have schema `k204-raw/2`, the planned request, HTTP status 200 and a
JSON body with an integer `exitCode` and string streams. Each call has a role:

- `timeout`: `cli-sleep`, `raw-sleep`, `cli-fork`, `raw-fork` and `r5-timeout`. The
  result must be a guest timeout. The overhead is the host elapsed time minus the
  guest timeout of 2 s. It includes the host transport, so it is an upper bound on
  the agent's work after its deadline. An overhead above 4 s, the 2 s TERM grace
  plus the 2 s kill/drain/reap bound, is a violation.
- `observe`: the plan's own observations. Each must exit 0 and not time out.
- `r5-observe`: the three r5 observations. Each must exit 0 and not time out.
- `cleanup`: `control-cleanup`. A receipt that ends with `cleanup confirmed` and
  exits 0 is confirmed. One that ends with `cleanup uncertain` is uncertain.
- `ordinary`: the identity, the process tables and the r5 bootstrap. Each must exit
  0 and not time out.
- `health`: agent health. The CLI must exit 0, and the receipt must show a reachable
  agent.
- `r5`: the other r5 guest cases. The guest exit must equal the exit of the original
  r5 run, clone 1 of `linux-03`.
- `transfer`: the uploads and downloads. A download must hold the pinned bytes.
  `r5-missing-artifact` must fail with a typed error receipt.
- `cancel`: `r5-cancel`. The controller must interrupt the CLI after 2 s.

An own observation prints `<label>-<role> pid=N`, the `ps` row when `ps` lists the
PID, and `<label>-<role> ps-exit=E`. In a valid observation a PID is `present` when
its row follows and `ps` exited 0. A present row with a stat that starts with Z is
`zombie`. A present row whose command is not the planned process is `other`. A PID
without a row is `absent` only when `ps` exited 1. Every other case is `unknown`.
An r5 observation masks the `ps` status, so a missing row there is `not-listed`.

A fork control reproduces failure B when its timeout is valid and the late
observation shows its child present or zombie. The late observation runs at least
5 s after the control returned, so a listed child outlived the 4 s bound. The
control does not reproduce failure B when its timeout is valid and both the first
and the late observation show its child absent. Failure B is reproduced when one
control reproduces it. It is not reproduced when both controls do not. Every other
case is unknown. The shell gets the same states, and a present shell in the late
observation is a violation.

An observation or an ordinary command stalls when its receipt is a guest timeout,
when its host elapsed time exceeds its guest budget, or when the host deadline ends
the call. A stall is admitted as a reproduction only with a valid local guest
timeout receipt, EOF on both streams, a valid intentional timeout earlier in the
shape and no run-level provenance failure. This is revision 3 of the k4 rule.

The evidence is complete when the k186 run-level checks pass, every call of the
shape has one record and ends as its role expects, and the ledger shows the clone
disposed with no residual process.

The checker prints one JSON object. It exits 0 when `survival` and `stall` are both
determined and the clone was disposed. It exits 1 otherwise.
"""

import json
import re
import sys
from pathlib import Path
from typing import Any, cast

from completion_k186_check import (  # the k186 host evidence checks, reused unchanged
    _mapping,  # pyright: ignore[reportPrivateUsage]
    _read_json,  # pyright: ignore[reportPrivateUsage]
    _run_failures,  # pyright: ignore[reportPrivateUsage]
)
from linux_k204 import (
    CONTROL_CLEANUP,
    FINAL_CLEANUP,
    FIXTURE,
    FORK_LABELS,
    GUEST_TIMEOUT,
    ISSUE_SLEEP,
    LATE_WAIT,
    control_calls,
    r5_calls,
)
from timeout_k4_check import (  # the k4 typed helpers, reused unchanged
    _envelope,  # pyright: ignore[reportPrivateUsage]
    _eof,  # pyright: ignore[reportPrivateUsage]
    _typed,  # pyright: ignore[reportPrivateUsage]
)

GRACE = 2.0  # TERM grace bound
REAP = 2.0  # kill/drain/reap bound
OVERHEAD_LIMIT = GRACE + REAP
TIMEOUT_STDERR = "Process timed out"
RESOURCE = ("acquire", "cleanup", "dispose", "inspect")
_PLACEHOLDER = "/vm-placeholder"
_TOOLS = {"cli": _PLACEHOLDER, "python": _PLACEHOLDER}
_CALLS = [
    *control_calls(_TOOLS, _PLACEHOLDER, Path("/")),
    *r5_calls(_PLACEHOLDER, _PLACEHOLDER, FIXTURE, Path("/")),
]
SHAPE = tuple(call.name for call in _CALLS)
CONTROLS = tuple(name for name in SHAPE if not name.startswith("r5-"))
R5 = tuple(name for name in SHAPE if name.startswith("r5-"))
# The planned guest command and guest timeout of each `file exec` and raw call.
RAW = ("raw-sleep", "raw-fork")
COMMANDS = {
    call.name: call.argv[-1] for call in _CALLS if call.argv[2] == "exec" or call.name in RAW
}
# The raw argv ends with the guest timeout and the command.
GUEST_BUDGET = {
    call.name: int(call.argv[-2] if call.name in RAW else call.argv[6])
    for call in _CALLS
    if call.name in COMMANDS
}
TIMEOUTS = ("cli-sleep", "raw-sleep", "cli-fork", "raw-fork", "r5-timeout")
FORK_OBSERVATIONS = {label: f"observe-{label}" for label in FORK_LABELS}
LATE = "observe-forks-late"
AFTER_CLEANUP = "observe-after-control-cleanup"
OWN_OBSERVATIONS = (*FORK_OBSERVATIONS.values(), LATE, AFTER_CLEANUP)
R5_OBSERVATIONS = ("r5-observe-timeout", "r5-observe-cancel", "r5-observe-after-cleanup")
HEALTH = ("health-before", "health-mid", "r5-health")
# The guest path of each transfer call: the last guest-side argument.
TRANSFERS = {
    call.name: call.argv[6] if call.argv[2] == "upload" else call.argv[5]
    for call in _CALLS
    if call.argv[2] in {"upload", "download"}
}
# The guest exit codes of the original r5 run, clone 1 of linux-03.
R5_ORIGINAL = {
    "r5-inventory": 0,
    "r5-install": 0,
    "r5-intact": 0,
    "r5-online": 0,
    "r5-nonzero": 23,
    "r5-bad-install": 1,
    "r5-guest-cleanup": 0,
}
_CASES = json.loads((FIXTURE / "cases.json").read_bytes())
# The pinned bytes of each download, by its local name.
DOWNLOADS = {
    **{name: bytes.fromhex(value) for name, value in _CASES["expected"].items()},
    "timeout.partial": b"partial timeout\n",
    "cancel.partial": b"partial cancel\n",
    **{f"{label}.partial": b"partial-file\n" for label in FORK_LABELS},
}
MISSING = "r5-missing-artifact"
_INCOMPLETENESS = ("result ok ", "unreached ")
_RECEIPT_TYPES: dict[str, type] = {
    "command": str,
    "exit_code": int,
    "timed_out": bool,
    "stdout": str,
    "stderr": str,
}
OWN_PID = re.compile(r"^(?P<label>[a-z]+-[a-z]+)-(?P<role>shell|child) pid=(?P<pid>\d+)$")
OWN_EXIT = re.compile(r"^(?P<label>[a-z]+-[a-z]+)-(?P<role>shell|child) ps-exit=(?P<exit>\d+)$")
OWN_UNREADABLE = re.compile(r"^(?P<label>[a-z]+-[a-z]+)-(?P<role>shell|child) pid-file=unreadable$")
_LSTART = r"\w{3} \w{3} +\d+ \d\d:\d\d:\d\d \d{4}"
OWN_ROW = re.compile(
    r"^\s*(?P<pid>\d+)\s+(?P<ppid>\d+)\s+(?P<pgid>\d+)\s+(?P<sid>\d+)\s+(?P<stat>\S+)\s+"
    rf"(?P<lstart>{_LSTART})\s+(?P<args>.*)$"
)
R5_PID = re.compile(r"^(?P<label>[a-z]+) pid=(?P<pid>\d+)$")
R5_ROW = re.compile(r"^\s*(?P<pid>\d+)\s+(?P<ppid>\d+)\s+(?P<stat>\S+)\s+(?P<comm>.+)$")


def role(name: str) -> str:
    """Return the role of one call of the frozen shape."""
    if name in TIMEOUTS:
        return "timeout"
    if name in OWN_OBSERVATIONS:
        return "observe"
    if name in R5_OBSERVATIONS:
        return "r5-observe"
    if name == "control-cleanup":
        return "cleanup"
    if name in HEALTH:
        return "health"
    if name == "r5-cancel":
        return "cancel"
    if name in TRANSFERS:
        return "transfer"
    if name in R5_ORIGINAL:
        return "r5"
    return "ordinary"


def _planned(role_name: str, args: str) -> bool:
    # ps shows each control character of the shell's argument as `?`.
    if role_name == "child":
        return args.strip() == ISSUE_SLEEP
    return args.startswith("/bin/bash -c ")


def own_processes(stdout: str) -> dict[str, dict[str, Any]]:
    """Parse one own observation into a state for each `<label>-<role>`.

    The states here are the raw parse: present, zombie, other, unlisted or unknown.
    judge() later turns `unlisted` into `absent` or `unknown`.
    """
    lines = stdout.splitlines()
    found: dict[str, dict[str, Any]] = {}
    for index, line in enumerate(lines):
        stripped = line.strip()
        unreadable = OWN_UNREADABLE.match(stripped)
        if unreadable is not None:
            key = f"{unreadable['label']}-{unreadable['role']}"
            found[key] = {"pid": None, "state": "unknown", "why": "pid file unreadable"}
            continue
        match = OWN_PID.match(stripped)
        if match is None:
            continue
        key, pid = f"{match['label']}-{match['role']}", int(match["pid"])
        state: dict[str, Any] = {"pid": pid, "state": "unknown", "ps_exit": None}
        rest = lines[index + 1 : index + 3]
        row = OWN_ROW.match(rest[0]) if rest else None
        exit_line = rest[1] if row is not None and len(rest) > 1 else (rest[0] if rest else "")
        ended = OWN_EXIT.match(exit_line.strip())
        if ended is not None and f"{ended['label']}-{ended['role']}" == key:
            state["ps_exit"] = int(ended["exit"])
        if row is not None and int(row["pid"]) == pid and state["ps_exit"] == 0:
            state |= {
                "ppid": int(row["ppid"]),
                "pgid": int(row["pgid"]),
                "sid": int(row["sid"]),
                "stat": row["stat"],
                "lstart": row["lstart"],
                "args": row["args"],
            }
            if row["stat"].startswith("Z"):
                state["state"] = "zombie"
            elif _planned(match["role"], row["args"]):
                state["state"] = "present"
            else:
                state["state"] = "other"
        elif row is None and state["ps_exit"] == 1:
            state["state"] = "unlisted"
        found[key] = state
    return found


def r5_children(stdout: str) -> dict[str, dict[str, Any]]:
    """Parse one r5 observation: `<label> pid=N` and the masked `ps` row after it."""
    lines = stdout.splitlines()
    found: dict[str, dict[str, Any]] = {}
    for index, line in enumerate(lines):
        match = R5_PID.match(line.strip())
        if match is None:
            continue
        pid = int(match["pid"])
        state: dict[str, Any] = {"pid": pid, "state": "unlisted"}
        row = R5_ROW.match(lines[index + 1]) if index + 1 < len(lines) else None
        if row is not None and int(row["pid"]) == pid:
            state |= {
                "state": "zombie" if row["stat"].startswith("Z") else "present",
                "ppid": int(row["ppid"]),
                "stat": row["stat"],
                "comm": row["comm"].strip(),
            }
        found[match["label"]] = state
    return found


def _receipt(run: Path, name: str) -> dict[str, Any] | None:
    value = _read_json(run / f"{name}.stdout")
    return cast(dict[str, Any], value) if isinstance(value, dict) else None


def receipt_failures(receipt: dict[str, Any] | None, command: str | None) -> list[str]:
    """Name each way that one guest exec receipt is not the typed planned receipt."""
    if receipt is None:
        return ["receipt absent or not one JSON object"]
    failures: list[str] = []
    if receipt.get("schema_version") != "1.0" or receipt.get("ok") is not True:
        failures.append(f"receipt {receipt.get('schema_version')!r}/ok {receipt.get('ok')!r}")
    for field, kind in _RECEIPT_TYPES.items():
        if not _typed(receipt.get(field), kind):
            failures.append(f"receipt {field} {receipt.get(field)!r} is not {kind.__name__}")
    if command is not None and receipt.get("command") != command:
        failures.append("receipt command differs from the plan")
    return failures


def raw_receipt(
    record: dict[str, Any] | None, name: str
) -> tuple[dict[str, Any] | None, list[str]]:
    """Turn one raw record into the guest receipt fields, and name each record failure.

    The receipt gets `exit_code` from `exitCode` and has no `timed_out` field, as the
    agent sends none.
    """
    if record is None:
        return None, ["raw record absent or not one JSON object"]
    failures: list[str] = []
    if record.get("schema") != "k204-raw/2":
        failures.append(f"raw schema {record.get('schema')!r}")
    planned = {"command": COMMANDS[name], "timeout": GUEST_BUDGET[name]}
    if record.get("request") != planned:
        failures.append("raw request differs from the plan")
    if record.get("http_status") != 200 or record.get("error") is not None:
        failures.append(f"raw http {record.get('http_status')!r}, error {record.get('error')!r}")
    body = record.get("body")
    if not isinstance(body, dict):
        failures.append("raw body is not one JSON object")
        return None, failures
    fields = cast(dict[str, Any], body)
    receipt = {
        "command": COMMANDS[name],
        "exit_code": fields.get("exitCode"),
        "timed_out": None,
        "stdout": fields.get("stdout"),
        "stderr": fields.get("stderr"),
        "raw_seconds": record.get("seconds"),
    }
    if not _typed(receipt["exit_code"], int):
        failures.append(f"raw exitCode {receipt['exit_code']!r} is not int")
    for stream in ("stdout", "stderr"):
        if not isinstance(receipt[stream], str):
            failures.append(f"raw {stream} {receipt[stream]!r} is not str")
    return receipt, failures


def _guest_timeout(receipt: dict[str, Any] | None) -> bool:
    if receipt is None:
        return False
    return receipt.get("timed_out") is True or (
        receipt.get("exit_code") == -1 and receipt.get("stderr") == TIMEOUT_STDERR
    )


def _transfer(run: Path, name: str, receipt: dict[str, Any] | None) -> list[str]:
    if name == MISSING:
        failures = _envelope(receipt, ok=False)
        if not failures and cast(dict[str, Any], receipt).get("code") != "DOWNLOAD_FAILED":
            failures.append(f"error code {cast(dict[str, Any], receipt).get('code')!r}")
        return failures
    failures = _envelope(receipt, ok=True)
    if failures:
        return failures
    fields = cast(dict[str, Any], receipt)
    if fields.get("remote") != TRANSFERS[name]:
        failures.append(f"remote {fields.get('remote')!r}, expected {TRANSFERS[name]!r}")
    size = fields.get("bytes")
    if not _typed(size, int):
        failures.append(f"bytes {size!r} is not int")
    remote_name = TRANSFERS[name].rsplit("/", 1)[1]
    if name.startswith("r5-upload-"):
        if size != len((FIXTURE / remote_name).read_bytes()):
            failures.append(f"bytes {size!r} differ from the fixture {remote_name}")
        return failures
    if name.startswith("r5-download-"):
        local_name, folder = remote_name, "r5-downloads"
    else:
        label = name.removeprefix("download-").removesuffix("-partial")
        local_name, folder = f"{label}.partial", "k204-downloads"
    local = run.parent / folder / local_name
    actual = local.read_bytes() if local.is_file() else None
    if actual is None or actual != DOWNLOADS[local_name]:
        failures.append(f"download {local_name} differs from the pinned bytes")
    elif size != len(actual):
        failures.append(f"bytes {size!r}, local file {len(actual)}")
    return failures


def _expected_exits(name: str) -> set[int]:
    kind = role(name)
    if kind == "timeout":
        return {-1}
    if kind == "r5":
        return {R5_ORIGINAL[name]}
    return {0}


def judge(
    name: str, call: dict[str, Any], record: dict[str, Any] | None, run: Path | None = None
) -> dict[str, Any]:
    """Judge one call record and its receipt by the role of the call.

    `local` holds the failures of the host call, the streams and the receipt type.
    `failures` adds the failures of the expected outcome.
    """
    kind = role(name)
    elapsed = call.get("elapsed")
    elapsed = float(elapsed) if isinstance(elapsed, (int, float)) else None
    local: list[str] = []
    receipt = record
    if name in RAW:
        receipt, raw_failures = raw_receipt(record, name)
        local.extend(raw_failures)
    sample: dict[str, Any] = {
        "role": kind,
        "reason": call.get("reason"),
        "returncode": call.get("returncode"),
        "started_monotonic": call.get("started_monotonic"),
        "elapsed": elapsed,
        "exit_code": None if receipt is None else receipt.get("exit_code"),
        "timed_out": None if receipt is None else receipt.get("timed_out"),
        "stdout": None if receipt is None else receipt.get("stdout"),
        "stderr": None if receipt is None else receipt.get("stderr"),
        "local": local,
        "failures": [],
        "violations": [],
    }
    if name in RAW and receipt is not None:
        sample["raw_seconds"] = receipt.get("raw_seconds")
    guest = name in COMMANDS and kind != "cancel"
    if call.get("ok") is not True and kind != "cancel":
        local.append(f"call ok {call.get('ok')!r}, reason {call.get('reason')!r}")
    if kind != "cancel":
        local.extend(_eof(call))
    if guest and name not in RAW:
        local.extend(receipt_failures(record, COMMANDS[name]))
    elif kind == "health":
        local.extend(_envelope(record, ok=True))
        if record is not None and record.get("reachable") is not True:
            local.append(f"health reachable {record.get('reachable')!r}")
    elif kind == "transfer":
        local.extend(_transfer(run or Path("/nonexistent"), name, record))
    failures: list[str] = sample["failures"]
    failures.extend(local)
    timeout = _guest_timeout(receipt)
    exit_code = sample["exit_code"]
    if guest:
        if kind != "timeout" and timeout:
            failures.append(f"guest timeout {exit_code!r}/{sample['stderr']!r}")
        if exit_code not in _expected_exits(name):
            failures.append(f"exit {exit_code!r}, expected {sorted(_expected_exits(name))}")
    if kind == "r5":
        sample["original_exit_code"] = R5_ORIGINAL[name]
    if kind == "timeout":
        if not timeout:
            failures.append(f"no guest timeout: {exit_code!r}/{sample['stderr']!r}")
        for field, value in (("overhead", elapsed), ("raw_overhead", sample.get("raw_seconds"))):
            if not isinstance(value, (int, float)):
                continue
            sample[field] = round(float(value) - GUEST_TIMEOUT, 3)
            if sample[field] > OVERHEAD_LIMIT:
                sample["violations"].append(
                    f"{field} {sample[field]:.3f} s above {OVERHEAD_LIMIT:g} s"
                )
    if kind in {"observe", "r5-observe", "ordinary", "cleanup"}:
        sample["stalled"] = (
            timeout
            or (elapsed is not None and elapsed > GUEST_BUDGET.get(name, GUEST_TIMEOUT))
            or call.get("reason") == "deadline"
        )
        sample["local_stall"] = sample["stalled"] and not local and timeout
    valid = not failures and not sample.get("stalled", False)
    if kind == "observe":
        processes = own_processes(str(sample["stdout"] or ""))
        for state in processes.values():
            if state["state"] == "unlisted":
                state["state"] = "absent" if valid else "unknown"
            elif not valid:
                state["state"] = "unknown"
        sample["processes"] = processes
    if kind == "r5-observe":
        children = r5_children(str(sample["stdout"] or ""))
        for state in children.values():
            if not valid:
                state["state"] = "unknown"
            elif state["state"] == "unlisted":
                state["state"] = "not-listed"
        sample["children"] = children
    if kind == "cleanup":
        sample["cleanup"] = cleanup_state(call, record, CONTROL_CLEANUP)
    if kind == "cancel":
        sample["signals"] = call.get("signals")
        if call.get("reason") != "interrupted" or call.get("ok") is not True:
            failures.append(f"call ok {call.get('ok')!r}, reason {call.get('reason')!r}")
    return sample


def cleanup_state(
    call: dict[str, Any] | None, receipt: dict[str, Any] | None, command: str
) -> dict[str, Any]:
    """Return the state of one owned cleanup: confirmed, uncertain or unknown."""
    failures = receipt_failures(receipt, command)
    if call is None or call.get("ok") is not True:
        failures.append("cleanup call absent or not ok")
    lines = str((receipt or {}).get("stdout") or "").splitlines()
    last = lines[-1] if lines else None
    if not failures and last == "cleanup confirmed" and (receipt or {}).get("exit_code") == 0:
        state = "confirmed"
    elif last == "cleanup uncertain":
        state = "uncertain"
    else:
        state = "unknown"
    return {"state": state, "lines": lines, "failures": failures}


def _disposal(run: Path) -> list[str]:
    summary = _mapping(_read_json(run.parent / "summary.json"))
    failures: list[str] = []
    entries = [_mapping(entry) for entry in cast(list[object], summary.get("ledger") or [])]
    if not entries or any(entry.get("state") != "disposed" for entry in entries):
        failures.append(f"ledger not disposed: {[entry.get('state') for entry in entries]}")
    if summary.get("residuals") != []:
        failures.append(f"residuals {summary.get('residuals')!r}")
    return failures


def _state(samples: dict[str, Any], observation: str, key: str) -> str:
    processes = samples.get(observation, {}).get("processes", {})
    return str(processes.get(key, {}).get("state", "unknown"))


def survival(samples: dict[str, Any]) -> dict[str, Any]:
    """Judge failure B for each fork control from its timeout and its observations."""
    controls: dict[str, Any] = {}
    for label in FORK_LABELS:
        timeout = samples.get(label)
        valid = timeout is not None and not timeout["failures"]
        first = FORK_OBSERVATIONS[label]
        states = {
            role_name: {
                "first": _state(samples, first, f"{label}-{role_name}"),
                "late": _state(samples, LATE, f"{label}-{role_name}"),
                "after_cleanup": _state(samples, AFTER_CLEANUP, f"{label}-{role_name}"),
            }
            for role_name in ("shell", "child")
        }
        child = states["child"]
        if valid and child["late"] in {"present", "zombie"}:
            result = "reproduced"
        elif valid and child["first"] == "absent" and child["late"] == "absent":
            result = "not-reproduced"
        else:
            result = "unknown"
        violations = [
            f"{role_name} {states[role_name]['late']} {LATE_WAIT} s after the control returned"
            for role_name in ("shell", "child")
            if states[role_name]["late"] in {"present", "zombie"}
        ]
        controls[label] = {
            "timeout_valid": valid,
            "states": states,
            "result": result,
            "violations": violations,
        }
    results = {control["result"] for control in controls.values()}
    if "reproduced" in results:
        overall = "reproduced"
    elif results == {"not-reproduced"}:
        overall = "not-reproduced"
    else:
        overall = "unknown"
    return {"result": overall, "controls": controls}


def fixture_survival(samples: dict[str, Any]) -> dict[str, Any]:
    """Report the r5 `timeout` and `cancel` children in each r5 observation."""
    seen = {
        name: {
            label: state.get("state")
            for label, state in samples.get(name, {}).get("children", {}).items()
        }
        for name in R5_OBSERVATIONS
    }
    timeout = samples.get("r5-timeout")
    states = {seen["r5-observe-timeout"].get("timeout"), seen["r5-observe-cancel"].get("timeout")}
    if timeout is None or timeout["failures"]:
        result = "unknown"
    elif states & {"present", "zombie"}:
        result = "listed"
    elif states == {"not-listed"}:
        result = "not-listed"
    else:
        result = "unknown"
    return {"result": result, "observations": seen}


def check(run: Path) -> dict[str, Any]:
    """Judge one run directory and return the checker's JSON object."""
    failures: list[str] = []
    result = _read_json(run / "result.json")
    if not isinstance(result, dict):
        failures.append("result.json is not one JSON object")
        result = {}
    result = cast(dict[str, Any], result)
    records = [
        cast(dict[str, Any], c)
        for c in cast(list[object], result.get("calls") or [])
        if isinstance(c, dict)
    ]
    names: list[object] = [record.get("name") for record in records]
    if result:
        failures.extend(_run_failures(run, result, names))
    unexpected = sorted({str(name) for name in names} - set(SHAPE) - set(RESOURCE))
    if unexpected:
        failures.append(f"calls outside the shape {unexpected}")
    disposal = _disposal(run)
    by_name = {str(record.get("name")): record for record in records}
    samples: dict[str, Any] = {}
    for name in SHAPE:
        call = by_name.get(name)
        if call is not None:
            samples[name] = judge(name, call, _receipt(run, name), run)
    missing = [name for name in SHAPE if name not in samples]
    failed = [name for name, sample in samples.items() if sample["failures"]]
    stalled = [name for name in SHAPE if samples.get(name, {}).get("stalled") is True]
    provenance = [f for f in failures if not f.startswith(_INCOMPLETENESS)]
    admitted: list[str] = []
    for index, name in enumerate(SHAPE):
        earlier = [samples.get(prior) for prior in SHAPE[:index] if prior in TIMEOUTS]
        timeout = any(prior is not None and not prior["failures"] for prior in earlier)
        if samples.get(name, {}).get("local_stall") is True and timeout and not provenance:
            admitted.append(name)
    complete = not failures and not missing and not failed and not disposal
    observed = all(name in samples for name in (*OWN_OBSERVATIONS, *R5_OBSERVATIONS))
    if admitted:
        stall = "reproduced"
    elif complete and not stalled and observed:
        stall = "not-reproduced"
    else:
        stall = "unknown"
    forks = survival(samples)
    violations = {n: s["violations"] for n, s in samples.items() if s["violations"]}
    for label, control in forks["controls"].items():
        if control["violations"]:
            violations.setdefault(LATE, []).extend(f"{label} {v}" for v in control["violations"])
    return {
        "run": str(run),
        "checker_revision": 1,
        "evidence": "absent" if not result else "complete" if complete else "incomplete",
        "survival": forks["result"],
        "fork_controls": forks["controls"],
        "fixture_survival": fixture_survival(samples),
        "stall": stall,
        "stalled": stalled,
        "admitted": admitted,
        "violations": violations,
        "missing": missing,
        "failed": failed,
        "run_failures": failures,
        "provenance_failures": provenance,
        "disposal_failures": disposal,
        "control_cleanup": samples.get("control-cleanup", {}).get("cleanup"),
        "cleanup": cleanup_state(by_name.get("cleanup"), _receipt(run, "cleanup"), FINAL_CLEANUP),
        "unreached": result.get("unreached"),
        "samples": samples,
    }


def main() -> int:
    report = check(Path(sys.argv[1]))
    print(json.dumps(report, indent=2))
    determined = {report["survival"], report["stall"]} <= {"reproduced", "not-reproduced"}
    return 0 if determined and not report["disposal_failures"] else 1


if __name__ == "__main__":
    sys.exit(main())
