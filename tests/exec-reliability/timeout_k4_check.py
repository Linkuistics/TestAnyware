"""Judge one timeout_k4.py run against its frozen plan.

    timeout_k4_check.py RUN_DIR

The checker gives two results. The first is `evidence`: complete, incomplete or
absent. The second is `stall`, the post-timeout inspection stall of issue #14
failure A: reproduced, not-reproduced or unknown.

Each guest receipt must be one typed object: schema 1.0, `ok` true, the planned
command, an integer `exit_code`, a boolean `timed_out` and string streams. Each call
of the frozen shape has a role. The role decides the expected outcome:

- `timeout`: `issue-sleep`, `issue-child` and `r3-timeout`. The receipt must show
  `timed_out` true and `exit_code` -1. The overhead is the host elapsed time minus
  the guest timeout of 2 s. The overhead includes the host transport, so it is an
  upper bound on the agent's work after its deadline. An overhead above 4 s, the
  sum of the 2 s TERM grace and the 2 s kill/drain/reap bound, is a violation.
- `observe`: `issue-observe`, `after-02-observe`, `after-08-observe` and the three
  r3 observations. The receipt must show `timed_out` false. The plan's own
  observation exits 0 when `ps` lists the child and 1 when it does not. The r3
  observation masks the `ps` status with `|| true`, so it exits 0.
- `ordinary`: the other after-timeout guest commands. The receipt must show
  `timed_out` false and exit 0.
- `health`: agent health. The host CLI must exit 0, and the receipt must show a
  reachable agent.
- `r3`: the other r3 guest cases. The guest exit code must equal the exit code of
  the original r3 run, clone 1 of `macos-03`.
- `transfer`: the r3 uploads and downloads. The receipt must name the planned guest
  path and a byte count. A download must hold the pinned bytes of the original run.
  `r3-missing-artifact` must fail with a typed error receipt.
- `cancel`: `r3-cancel`. The controller must interrupt the CLI after 2 s.

A child that an observation names with a `<label> pid=N` line is `present` when the
next line is its `ps` row. Without a row the child is `absent` only in a valid,
complete observation of the plan's own payload that exits 1 with empty stderr. In
a valid r3 observation the child is `not-listed`, because the fixture masks a `ps`
failure. In every other case the child is `unknown`.

An observation or an ordinary command stalls when its receipt shows `timed_out`
true, when its host elapsed time exceeds the guest budget of 30 s, or when the host
deadline ends the call. Revision 1, frozen before run 02, counted observations only.
Revision 2 also counted ordinary commands. Revision 3 admits a stall as a
reproduction only when the stalled call has valid local evidence: a successful host
call, EOF on both streams, a typed receipt with `timed_out` true and exit -1, and a
valid intentional timeout earlier in the shape. Run-level provenance failures block
admission. An unrelated later failure does not.

The stall is reproduced when one admitted stall exists. The stall is not reproduced
when no call stalled and the evidence is complete. A failed or missing observation
never proves absence, so every other case is unknown.

The evidence is complete when all of these hold:

- The k186 run-level checks pass (`completion_k186_check._run_failures`).
- Every call of the frozen shape has one record, ends as its role expects, and
  reaches EOF on both streams, except the interrupted call.
- The ledger shows the clone disposed, and the controller names no residual process.

The report also exposes the resource cleanup. Its state is `confirmed` only when the
cleanup receipt is valid, exits 0 and ends with `cleanup confirmed`. A receipt that
ends with `cleanup uncertain` gives `uncertain`. Any other receipt gives `unknown`.
Clone disposal stays the independent guarantee.

The checker prints one JSON object. It exits 0 when the stall result is reproduced
or not-reproduced and the clone was disposed. It exits 1 otherwise.
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
from timeout_k4 import (
    FIXTURE,
    GUEST_TIMEOUT,
    OBSERVE,
    OWNED_CLEANUP,
    after_calls,
    issue_calls,
    r3_calls,
)

GRACE = 2.0  # TERM grace bound
REAP = 2.0  # kill/drain/reap bound
OVERHEAD_LIMIT = GRACE + REAP
STREAMS = ("stdout", "stderr")
RESOURCE = ("acquire", "cleanup", "dispose", "inspect")
_PLACEHOLDER = "/vm-placeholder"
_CALLS = [
    *issue_calls(_PLACEHOLDER, _PLACEHOLDER),
    *after_calls(_PLACEHOLDER, _PLACEHOLDER),
    *r3_calls(_PLACEHOLDER, _PLACEHOLDER, FIXTURE, Path("/")),
]
CONDITION_1 = ("health-before", *(call.name for call in issue_calls("", "")))
AFTER = tuple(call.name for call in after_calls(_PLACEHOLDER, _PLACEHOLDER))
R3 = tuple(call.name for call in _CALLS if call.name.startswith("r3-"))
SHAPE = CONDITION_1 + AFTER + R3
# The planned guest command of each `file exec` call.
COMMANDS = {call.name: call.argv[-1] for call in _CALLS if call.argv[2] == "exec"}
TIMEOUTS = ("issue-sleep", "issue-child", "r3-timeout")
OWN_OBSERVATIONS = ("issue-observe", "after-02-observe", "after-08-observe")
R3_OBSERVATIONS = ("r3-observe-timeout", "r3-observe-cancel", "r3-observe-after-cleanup")
OBSERVATIONS = OWN_OBSERVATIONS + R3_OBSERVATIONS
HEALTH = ("health-before", "after-01-health", "after-10-health", "r3-health")
# The guest path of each transfer call: the last guest-side argument.
TRANSFERS = {
    call.name: call.argv[6] if call.argv[2] == "upload" else call.argv[5]
    for call in _CALLS
    if call.argv[2] in {"upload", "download"}
}
# The guest exit codes of the original r3 run, clone 1 of macos-03.
R3_ORIGINAL = {
    "r3-bootstrap-dir": 0,
    "r3-inventory": 0,
    "r3-install": 0,
    "r3-intact": 0,
    "r3-online": 0,
    "r3-nonzero": 23,
    "r3-bad-install": 71,
    "r3-guest-cleanup": 0,
}
_CASES = json.loads((FIXTURE / "cases.json").read_bytes())
# The pinned bytes of each download, by its local name. The partial files are the
# fixed text that guest.sh writes.
DOWNLOADS = {
    **{name: bytes.fromhex(value) for name, value in _CASES["expected"].items()},
    "timeout.partial": b"partial timeout\n",
    "cancel.partial": b"partial cancel\n",
}
MISSING = "r3-missing-artifact"
# The k186 run-level failures that state incompleteness, not damaged provenance.
_INCOMPLETENESS = ("result ok ", "unreached ")
PID_LINE = re.compile(r"^(?P<label>[a-z]+) pid=(?P<pid>\d+)$")
PS_LINE = re.compile(r"^\s*(?P<pid>\d+)\s+(?P<ppid>\d+)\s+(?P<stat>\S+)\s+(?P<comm>.+)$")
_RECEIPT_TYPES: dict[str, type] = {
    "command": str,
    "exit_code": int,
    "timed_out": bool,
    "stdout": str,
    "stderr": str,
}


def role(name: str) -> str:
    """Return the role of one call of the frozen shape."""
    if name in TIMEOUTS:
        return "timeout"
    if name in OBSERVATIONS:
        return "observe"
    if name in HEALTH:
        return "health"
    if name == "r3-cancel":
        return "cancel"
    if name in TRANSFERS:
        return "transfer"
    if name.startswith("r3-"):
        return "r3"
    return "ordinary"


def children(stdout: str) -> list[dict[str, Any]]:
    """Parse the `<label> pid=N` lines of an observation and the `ps` line after each.

    A child without a following `ps` row is `unlisted`. The judgement decides later
    whether the observation can show it absent.
    """
    lines = stdout.splitlines()
    found: list[dict[str, Any]] = []
    for index, line in enumerate(lines):
        match = PID_LINE.match(line.strip())
        if match is None:
            continue
        pid = int(match["pid"])
        state: dict[str, Any] = {"label": match["label"], "pid": pid, "state": "unlisted"}
        following = lines[index + 1] if index + 1 < len(lines) else ""
        ps = PS_LINE.match(following)
        if ps is not None and int(ps["pid"]) == pid:
            state |= {
                "state": "present",
                "ppid": int(ps["ppid"]),
                "stat": ps["stat"],
                "comm": ps["comm"].strip(),
            }
        found.append(state)
    return found


def _receipt(run: Path, name: str) -> dict[str, Any] | None:
    value = _read_json(run / f"{name}.stdout")
    return cast(dict[str, Any], value) if isinstance(value, dict) else None


def _eof(call: dict[str, Any]) -> list[str]:
    streams = _mapping(call.get("streams"))
    failures: list[str] = []
    for stream in STREAMS:
        completeness = _mapping(streams.get(stream)).get("completeness")
        if completeness != "eof":
            failures.append(f"{stream} completeness {completeness!r}")
    return failures


def _typed(value: object, kind: type) -> bool:
    # A JSON boolean is a Python int, so an integer field refuses it explicitly.
    return isinstance(value, kind) and (kind is bool or not isinstance(value, bool))


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


def _envelope(receipt: dict[str, Any] | None, ok: bool) -> list[str]:
    if receipt is None:
        return ["receipt absent or not one JSON object"]
    if receipt.get("schema_version") != "1.0" or receipt.get("ok") is not ok:
        return [f"receipt {receipt.get('schema_version')!r}/ok {receipt.get('ok')!r}"]
    return []


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
    local_name = TRANSFERS[name].rsplit("/", 1)[1]
    if name.startswith("r3-download-"):
        local = run.parent / "r3-downloads" / local_name
        actual = local.read_bytes() if local.is_file() else None
        if actual is None or actual != DOWNLOADS[local_name]:
            failures.append(f"download {local_name} differs from the pinned bytes")
        elif size != len(actual):
            failures.append(f"bytes {size!r}, local file {len(actual)}")
    elif size != len((FIXTURE / local_name).read_bytes()):
        failures.append(f"bytes {size!r} differ from the fixture {local_name}")
    return failures


def _expected_exits(name: str) -> set[int]:
    kind = role(name)
    if kind == "timeout":
        return {-1}
    if name in OWN_OBSERVATIONS:
        return {0, 1}
    if kind == "r3":
        return {R3_ORIGINAL[name]}
    return {0}


def judge(
    name: str, call: dict[str, Any], receipt: dict[str, Any] | None, run: Path | None = None
) -> dict[str, Any]:
    """Judge one call record and its receipt by the role of the call.

    `local` holds the failures of the host call, the streams and the receipt type.
    `failures` adds the failures of the expected outcome.
    """
    kind = role(name)
    elapsed = call.get("elapsed")
    elapsed = float(elapsed) if isinstance(elapsed, (int, float)) else None
    guest = name in COMMANDS and kind != "cancel"
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
        "local": [],
        "failures": [],
        "violations": [],
    }
    local: list[str] = sample["local"]
    if call.get("ok") is not True and kind != "cancel":
        local.append(f"call ok {call.get('ok')!r}, reason {call.get('reason')!r}")
    if kind != "cancel":
        local.extend(_eof(call))
    if guest:
        local.extend(receipt_failures(receipt, COMMANDS[name]))
    elif kind == "health":
        local.extend(_envelope(receipt, ok=True))
        if receipt is not None and receipt.get("reachable") is not True:
            local.append(f"health reachable {receipt.get('reachable')!r}")
    elif kind == "transfer":
        local.extend(_transfer(run or Path("/nonexistent"), name, receipt))
    failures: list[str] = sample["failures"]
    failures.extend(local)
    exit_code, timed_out = sample["exit_code"], sample["timed_out"]
    if guest:
        if kind != "timeout" and timed_out is not False:
            failures.append(f"timed_out {timed_out!r}")
        if exit_code not in _expected_exits(name):
            failures.append(f"exit {exit_code!r}, expected {sorted(_expected_exits(name))}")
    if kind == "r3":
        sample["original_exit_code"] = R3_ORIGINAL[name]
    if kind == "timeout":
        if timed_out is not True:
            failures.append(f"timeout receipt {exit_code!r}/{timed_out!r}")
        if elapsed is not None:
            sample["overhead"] = round(elapsed - GUEST_TIMEOUT, 3)
            if sample["overhead"] > OVERHEAD_LIMIT:
                sample["violations"].append(
                    f"overhead {sample['overhead']:.3f} s above {OVERHEAD_LIMIT:g} s"
                )
    if kind in {"observe", "ordinary"}:
        sample["stalled"] = (
            timed_out is True
            or (elapsed is not None and elapsed > OBSERVE)
            or call.get("reason") == "deadline"
        )
        # The stalled call shows the guest phenomenon only with valid local evidence
        # of a guest timeout. check() adds the earlier intentional timeout.
        sample["local_stall"] = (
            sample["stalled"] and not local and timed_out is True and exit_code == -1
        )
    if kind == "observe":
        sample["children"] = children(str(sample["stdout"] or ""))
        valid = not failures and not sample["stalled"]
        for child in sample["children"]:
            if child["state"] != "unlisted":
                continue
            if valid and name in R3_OBSERVATIONS:
                child["state"] = "not-listed"
            elif valid and exit_code == 1 and sample["stderr"] == "":
                child["state"] = "absent"
            else:
                child["state"] = "unknown"
    if kind == "cancel":
        sample["signals"] = call.get("signals")
        if call.get("reason") != "interrupted" or call.get("ok") is not True:
            failures.append(f"call ok {call.get('ok')!r}, reason {call.get('reason')!r}")
    return sample


def cleanup_state(call: dict[str, Any] | None, receipt: dict[str, Any] | None) -> dict[str, Any]:
    """Return the state of the resource cleanup: confirmed, uncertain or unknown."""
    failures = receipt_failures(receipt, OWNED_CLEANUP)
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
    ledger = summary.get("ledger")
    entries = [_mapping(entry) for entry in cast(list[object], ledger or [])]
    if not entries or any(entry.get("state") != "disposed" for entry in entries):
        failures.append(f"ledger not disposed: {[entry.get('state') for entry in entries]}")
    if summary.get("residuals") != []:
        failures.append(f"residuals {summary.get('residuals')!r}")
    return failures


def check(
    run: Path, shape: tuple[str, ...] = SHAPE, observations: tuple[str, ...] = OBSERVATIONS
) -> dict[str, Any]:
    """Judge one run directory and return the checker's JSON object.

    The defaults give the k4 shape. timeout_k193_check.py passes the r3 calls alone.
    """
    failures: list[str] = []
    result = _read_json(run / "result.json")
    if not isinstance(result, dict):
        failures.append("result.json is not one JSON object")
        result = {}
    result = cast(dict[str, Any], result)
    calls = result.get("calls")
    records = [
        cast(dict[str, Any], c) for c in cast(list[object], calls or []) if isinstance(c, dict)
    ]
    names: list[object] = [record.get("name") for record in records]
    if result:
        failures.extend(_run_failures(run, result, names))
    unexpected = sorted({str(name) for name in names} - set(shape) - set(RESOURCE))
    if unexpected:
        failures.append(f"calls outside the shape {unexpected}")
    disposal = _disposal(run)
    by_name = {str(record.get("name")): record for record in records}
    samples: dict[str, Any] = {}
    for name in shape:
        call = by_name.get(name)
        if call is not None:
            samples[name] = judge(name, call, _receipt(run, name), run)
    missing = [name for name in shape if name not in samples]
    failed = [name for name, sample in samples.items() if sample["failures"]]
    seen = [samples[name] for name in observations if name in samples]
    stalled = [name for name in shape if samples.get(name, {}).get("stalled") is True]
    provenance = [f for f in failures if not f.startswith(_INCOMPLETENESS)]
    admitted: list[str] = []
    for index, name in enumerate(shape):
        sample = samples.get(name, {})
        earlier = [samples.get(prior) for prior in shape[:index] if prior in TIMEOUTS]
        timeout = any(prior is not None and not prior["failures"] for prior in earlier)
        if sample.get("local_stall") is True and timeout and not provenance:
            admitted.append(name)
    complete = not failures and not missing and not failed and not disposal
    if admitted:
        stall = "reproduced"
    elif complete and not stalled and len(seen) == len(observations):
        stall = "not-reproduced"
    else:
        stall = "unknown"
    evidence = "absent" if not result else "complete" if complete else "incomplete"
    return {
        "run": str(run),
        "checker_revision": 3,
        "evidence": evidence,
        "stall": stall,
        "stalled": stalled,
        "admitted": admitted,
        "violations": {n: s["violations"] for n, s in samples.items() if s["violations"]},
        "missing": missing,
        "failed": failed,
        "run_failures": failures,
        "provenance_failures": provenance,
        "disposal_failures": disposal,
        "cleanup": cleanup_state(by_name.get("cleanup"), _receipt(run, "cleanup")),
        "unreached": result.get("unreached"),
        "samples": samples,
    }


def main() -> int:
    report = check(Path(sys.argv[1]))
    print(json.dumps(report, indent=2))
    determined = report["stall"] in {"reproduced", "not-reproduced"}
    return 0 if determined and not report["disposal_failures"] else 1


if __name__ == "__main__":
    sys.exit(main())
