"""Judge one timeout_k4.py run against its frozen plan.

    timeout_k4_check.py RUN_DIR

The checker gives two results. The first is `evidence`: complete, incomplete or
absent. The second is `stall`, the post-timeout inspection stall of issue #14
failure A: reproduced, not-reproduced or unknown.

Each call of the frozen shape has a role. The role decides the judgement:

- `timeout`: `issue-sleep`, `issue-child` and `r3-timeout`. The receipt must show
  `timed_out` true and `exit_code` -1. The overhead is the host elapsed time minus
  the guest timeout of 2 s. The overhead includes the host transport, so it is an
  upper bound on the agent's work after its deadline. An overhead above 4 s, the
  sum of the 2 s TERM grace and the 2 s kill/drain/reap bound, is a violation.
- `observe`: `issue-observe`, `after-02-observe`, `after-08-observe` and the three
  r3 observations. An observation stalls when its receipt shows `timed_out` true,
  when its host elapsed time exceeds the guest budget of 30 s, or when the host
  deadline ends the call. The checker parses each `<label> pid=N` line and the `ps`
  line after it. A child is `present` with its parent PID, `absent`, or `unknown`.
- `ordinary`: every other guest command. The receipt must show `timed_out` false.
- `health`: agent health. The host CLI must exit 0.
- `cancel`: `r3-cancel`. The controller must interrupt the CLI after 2 s.
- `r3`: the other r3 cases. The checker records the guest exit code beside the
  exit code of the original r3 run.

Revision 2 also counts an `ordinary` guest command whose receipt shows `timed_out`
true as a stall, because each one runs after an intentional timeout. Revision 1,
frozen before run 02, counted observations only.

The stall is reproduced when one observation stalls. That result needs only the
evidence of the stalled call, so it stands when other evidence is incomplete. The
stall is not reproduced when every observation completed and the evidence is
complete. A failed or missing observation never proves absence, so every other case
is unknown.

The evidence is complete when all of these hold:

- The k186 run-level checks pass (`completion_k186_check._run_failures`).
- Every call of the frozen shape has one record, ends as the plan expects, and
  reaches EOF on both streams, except the interrupted call.
- The ledger shows the clone disposed, and the controller names no residual process.

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
from timeout_k4 import GUEST_TIMEOUT, OBSERVE, after_calls, r3_calls

GRACE = 2.0  # TERM grace bound
REAP = 2.0  # kill/drain/reap bound
OVERHEAD_LIMIT = GRACE + REAP
STREAMS = ("stdout", "stderr")
RESOURCE = ("acquire", "cleanup", "dispose", "inspect")
CONDITION_1 = ("health-before", "issue-sleep", "issue-child", "issue-observe")
_PLACEHOLDER = "/vm-placeholder"
AFTER = tuple(call.name for call in after_calls(_PLACEHOLDER, _PLACEHOLDER))
_R3_CALLS = r3_calls(_PLACEHOLDER, _PLACEHOLDER, Path("/"), Path("/"))
R3 = tuple(call.name for call in _R3_CALLS)
SHAPE = CONDITION_1 + AFTER + R3
TIMEOUTS = ("issue-sleep", "issue-child", "r3-timeout")
OBSERVATIONS = (
    "issue-observe",
    "after-02-observe",
    "after-08-observe",
    "r3-observe-timeout",
    "r3-observe-cancel",
    "r3-observe-after-cleanup",
)
HEALTH = ("health-before", "after-01-health", "after-10-health")
TRANSFERS = tuple(call.name for call in _R3_CALLS if call.argv[2] in {"upload", "download"})
# The guest exit codes of the original r3 run, clone 1 of macos-03.
R3_ORIGINAL = {"r3-nonzero": 23, "r3-bad-install": 71}
PID_LINE = re.compile(r"^(?P<label>[a-z]+) pid=(?P<pid>\d+)$")
PS_LINE = re.compile(r"^\s*(?P<pid>\d+)\s+(?P<ppid>\d+)\s+(?P<stat>\S+)\s+(?P<comm>.+)$")


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
    if name.startswith("r3-"):
        return "r3"
    return "ordinary"


def children(stdout: str) -> list[dict[str, Any]]:
    """Parse the `<label> pid=N` lines of an observation and the `ps` line after each."""
    lines = stdout.splitlines()
    found: list[dict[str, Any]] = []
    for index, line in enumerate(lines):
        match = PID_LINE.match(line.strip())
        if match is None:
            continue
        pid = int(match["pid"])
        state: dict[str, Any] = {"label": match["label"], "pid": pid, "state": "absent"}
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


def judge(name: str, call: dict[str, Any], receipt: dict[str, Any] | None) -> dict[str, Any]:
    """Judge one call record and its receipt by the role of the call."""
    kind = role(name)
    elapsed = call.get("elapsed")
    elapsed = float(elapsed) if isinstance(elapsed, (int, float)) else None
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
        "failures": [],
        "violations": [],
    }
    failures: list[str] = sample["failures"]
    if call.get("ok") is not True:
        failures.append(f"call ok {call.get('ok')!r}, reason {call.get('reason')!r}")
    if kind != "cancel":
        failures.extend(_eof(call))
    guest = kind in {"timeout", "observe", "ordinary"} or (kind == "r3" and name not in TRANSFERS)
    if guest and (receipt is None or receipt.get("ok") is not True):
        failures.append("receipt absent or not ok")
    if kind == "timeout":
        if sample["timed_out"] is not True or sample["exit_code"] != -1:
            failures.append(f"timeout receipt {sample['exit_code']!r}/{sample['timed_out']!r}")
        if elapsed is not None:
            sample["overhead"] = round(elapsed - GUEST_TIMEOUT, 3)
            if sample["overhead"] > OVERHEAD_LIMIT:
                sample["violations"].append(
                    f"overhead {sample['overhead']:.3f} s above {OVERHEAD_LIMIT:g} s"
                )
    elif kind == "observe":
        stalled = (
            sample["timed_out"] is True
            or (elapsed is not None and elapsed > OBSERVE)
            or call.get("reason") == "deadline"
        )
        sample["stalled"] = stalled
        sample["children"] = children(str(sample["stdout"] or ""))
        if stalled:
            # The printed PID lines show that the guest shell ran. The command then
            # blocked after them, in `ps` or in the agent's response path.
            for child in sample["children"]:
                if child["state"] == "absent":
                    child["state"] = "unknown"
    elif kind == "ordinary" and sample["timed_out"] is not False:
        failures.append(f"timed_out {sample['timed_out']!r}")
        # Revision 2: every ordinary guest command runs after an intentional
        # timeout, so a guest timeout here is also a post-timeout stall.
        sample["stalled"] = sample["timed_out"] is True
    elif kind == "r3" and name in R3_ORIGINAL:
        sample["original_exit_code"] = R3_ORIGINAL[name]
    elif kind == "cancel":
        sample["signals"] = call.get("signals")
        if call.get("reason") != "interrupted":
            failures.append(f"reason {call.get('reason')!r}, expected interrupted")
    return sample


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


def check(run: Path) -> dict[str, Any]:
    """Judge one run directory and return the checker's JSON object."""
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
    unexpected = sorted({str(name) for name in names} - set(SHAPE) - set(RESOURCE))
    if unexpected:
        failures.append(f"calls outside the shape {unexpected}")
    disposal = _disposal(run)
    by_name = {str(record.get("name")): record for record in records}
    samples: dict[str, Any] = {}
    for name in SHAPE:
        call = by_name.get(name)
        if call is not None:
            samples[name] = judge(name, call, _receipt(run, name))
    missing = [name for name in SHAPE if name not in samples]
    failed = [name for name, sample in samples.items() if sample["failures"]]
    observations = [samples[name] for name in OBSERVATIONS if name in samples]
    stalled = [name for name in SHAPE if samples.get(name, {}).get("stalled") is True]
    complete = not failures and not missing and not failed and not disposal
    if stalled:
        stall = "reproduced"
    elif complete and len(observations) == len(OBSERVATIONS):
        stall = "not-reproduced"
    else:
        stall = "unknown"
    evidence = "absent" if not result else "complete" if complete else "incomplete"
    return {
        "run": str(run),
        "checker_revision": 2,
        "evidence": evidence,
        "stall": stall,
        "stalled": stalled,
        "violations": {n: s["violations"] for n, s in samples.items() if s["violations"]},
        "missing": missing,
        "failed": failed,
        "run_failures": failures,
        "disposal_failures": disposal,
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
