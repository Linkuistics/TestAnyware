"""Judge one completion_k186.py run against its frozen plan.

    completion_k186_check.py RUN_DIR

The frozen shape has ten serial samples, `serial-01` to `serial-10`, and three
concurrent rounds, `concurrent-1` to `concurrent-3`, of four children each.

A sample passes when all of these hold:

- The host CLI exited 0 inside its host bound of 50 s.
- The host observed EOF on the stdout and the stderr of the CLI.
- The receipt is one JSON object with `ok` true, `exit_code` 0, `timed_out` false,
  `stdout` exactly "ordinary-ok" and `stderr` empty.

A sample fails when a condition is false. A sample is unknown when no condition is
false but the record does not show EOF on a stream. The checker names each failed or
unobserved condition.

The run fails in three cases. A sample of the frozen shape fails. A sample is missing,
duplicate or outside the shape. Or the host evidence is not healthy. Healthy host
evidence has all of these:

- `result.json` has `ok` true and no unreached call.
- No inspection failure, and equal non-empty subject digests before and after.
- No failed, dropped or abandoned storage frame, and an orderly storage close with no
  refused or failed frame.
- Each serial call and each concurrent wrapper call is `ok`, ends with reason `exit`
  and return code 0, and reaches EOF on both streams.

Otherwise the run is unknown when a sample is unknown, and it passes when every sample
passes. The checker keeps the result of each sample that it judged. It prints one JSON
object and exits 0 only when the run passes.
"""

import json
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any, cast

HOST_BOUND = 50.0
EXPECTED = {"ok": True, "exit_code": 0, "timed_out": False, "stdout": "ordinary-ok", "stderr": ""}
SERIAL = tuple(f"serial-{i:02d}" for i in range(1, 11))
ROUNDS = tuple(f"concurrent-{i}" for i in range(1, 4))
ROUND_COUNT = 4
SAMPLES = SERIAL + tuple(f"{name}.{i}" for name in ROUNDS for i in range(ROUND_COUNT))
STREAMS = ("stdout", "stderr")


def judge(
    host_exit: object, elapsed: object, raw: bytes, eof: Mapping[str, object]
) -> dict[str, Any]:
    """Judge one sample. `eof` maps each stream to True, False or None (not recorded)."""
    failures: list[str] = []
    unobserved: list[str] = []
    if host_exit != 0:
        failures.append(f"host exit {host_exit!r}")
    if not isinstance(elapsed, (int, float)) or isinstance(elapsed, bool):
        failures.append(f"elapsed {elapsed!r} is not a number")
    elif elapsed > HOST_BOUND:
        failures.append(f"elapsed {elapsed:.3f} s above {HOST_BOUND:g} s")
    for stream in STREAMS:
        seen = eof.get(stream)
        if seen is None:
            unobserved.append(f"{stream} EOF not recorded")
        elif seen is not True:
            failures.append(f"{stream} EOF not reached")
    receipt: object = None
    try:
        receipt = json.loads(raw)
    except ValueError:
        failures.append("receipt is not JSON")
    else:
        if isinstance(receipt, dict):
            fields = cast(dict[str, object], receipt)
            for key, value in EXPECTED.items():
                if key not in fields:
                    failures.append(f"{key} absent")
                elif fields[key] != value:
                    failures.append(f"{key} {fields[key]!r}")
        else:
            failures.append("receipt is not an object")
    verdict = "fail" if failures else "unknown" if unobserved else "pass"
    return {
        "host_exit": host_exit,
        "elapsed": elapsed,
        "receipt": receipt,
        "verdict": verdict,
        "failures": failures,
        "unobserved": unobserved,
    }


def _mapping(value: object) -> dict[str, Any]:
    """Return `value` when it is a JSON object, else an empty one."""
    return cast(dict[str, Any], value) if isinstance(value, dict) else {}


def _read_json(path: Path) -> object:
    try:
        return json.loads(path.read_bytes())
    except (OSError, ValueError):
        return None


def _call_failures(call: dict[str, Any]) -> list[str]:
    """Name each way that one controller call record is not a clean exit."""
    failures: list[str] = []
    if call.get("ok") is not True:
        failures.append(f"call ok {call.get('ok')!r}")
    if call.get("reason") != "exit":
        failures.append(f"reason {call.get('reason')!r}")
    if call.get("returncode") != 0:
        failures.append(f"return code {call.get('returncode')!r}")
    streams = call.get("streams")
    streams = cast(dict[str, Any], streams) if isinstance(streams, dict) else {}
    for stream in STREAMS:
        record = streams.get(stream)
        completeness = _mapping(record).get("completeness")
        if completeness != "eof":
            failures.append(f"{stream} completeness {completeness!r}")
    return failures


def _run_failures(run: Path, result: dict[str, Any], names: list[object]) -> list[str]:
    """Name each way that the host evidence of the run is not healthy."""
    failures: list[str] = []
    if result.get("ok") is not True:
        failures.append(f"result ok {result.get('ok')!r}")
    if result.get("unreached") != []:
        failures.append(f"unreached {result.get('unreached')!r}")
    if result.get("inspection_failure") is not None:
        failures.append(f"inspection failure {result.get('inspection_failure')!r}")
    before, after = result.get("subjects_before"), result.get("subjects_after")
    if not isinstance(before, dict) or not before or before != after:
        failures.append("subject digests absent or changed")
    storage = result.get("storage")
    storage = cast(dict[str, Any], storage) if isinstance(storage, dict) else {}
    frames = storage.get("frames")
    frames = cast(dict[str, Any], frames) if isinstance(frames, dict) else {}
    for key in ("failed", "dropped", "abandoned"):
        if frames.get(key) != 0:
            failures.append(f"storage frames {key} {frames.get(key)!r}")
    if storage.get("first_failure") is not None:
        failures.append(f"storage first failure {storage.get('first_failure')!r}")
    close = _read_json(run / "storage-close.json")
    close = cast(dict[str, Any], close) if isinstance(close, dict) else {}
    if (close.get("state"), close.get("refused"), close.get("failed")) != ("orderly", 0, 0):
        failures.append(f"storage close {close or None!r}")
    duplicates = sorted({str(name) for name in names if names.count(name) > 1})
    if duplicates:
        failures.append(f"duplicate calls {duplicates}")
    return failures


def _judge_round(
    run: Path, name: str, call: dict[str, Any], samples: dict[str, Any], failures: list[str]
) -> None:
    """Judge the children of one concurrent round into `samples`."""
    failures.extend(f"{name}: {failure}" for failure in _call_failures(call))
    round_ = _read_json(run / f"{name}.stdout")
    if not isinstance(round_, dict):
        failures.append(f"{name}: wrapper output is not one JSON object")
        return
    fields = cast(dict[str, Any], round_)
    if fields.get("count") != ROUND_COUNT:
        failures.append(f"{name}: count {fields.get('count')!r}, expected {ROUND_COUNT}")
    children = fields.get("children")
    if not isinstance(children, list):
        failures.append(f"{name}: children absent")
        return
    for child in cast(list[object], children):
        record = cast(dict[str, Any], child) if isinstance(child, dict) else {}
        index = record.get("index")
        key = f"{name}.{index}"
        if not isinstance(index, int) or isinstance(index, bool) or key not in SAMPLES:
            failures.append(f"{name}: child index {index!r} outside the shape")
            continue
        if key in samples:
            failures.append(f"{key}: duplicate child index")
            samples[key]["verdict"] = "fail"
            samples[key]["failures"].append("duplicate child index")
            continue
        try:
            raw = bytes.fromhex(str(record.get("stdout_hex")))
        except ValueError:
            raw = b"\x00"
        eof = {stream: record.get(f"{stream}_eof") for stream in STREAMS}
        samples[key] = judge(record.get("returncode"), record.get("elapsed"), raw, eof)
        samples[key]["started_monotonic"] = record.get("started_monotonic")


def check(run: Path) -> dict[str, Any]:
    """Judge one run directory and return the checker's JSON object."""
    samples: dict[str, Any] = {}
    failures: list[str] = []
    result = _read_json(run / "result.json")
    if not isinstance(result, dict):
        failures.append("result.json is not one JSON object")
        result = {}
    result = cast(dict[str, Any], result)
    calls = result.get("calls")
    calls = cast(list[object], calls) if isinstance(calls, list) else []
    records = [cast(dict[str, Any], call) for call in calls if isinstance(call, dict)]
    names: list[object] = [record.get("name") for record in records]
    if result:
        failures.extend(_run_failures(run, result, names))
    by_name = {str(record.get("name")): record for record in records}
    for name in SERIAL:
        call = by_name.get(name)
        if call is None:
            continue
        try:
            raw = (run / f"{name}.stdout").read_bytes()
        except OSError:
            raw = b"\x00"
        streams = call.get("streams")
        streams = cast(dict[str, Any], streams) if isinstance(streams, dict) else {}
        eof: dict[str, object] = {}
        for stream in STREAMS:
            record = streams.get(stream)
            completeness = _mapping(record).get("completeness")
            eof[stream] = None if completeness is None else completeness == "eof"
        sample = judge(call.get("returncode"), call.get("elapsed"), raw, eof)
        extra = [f for f in _call_failures(call) if f.startswith(("call ok", "reason"))]
        if extra:
            sample["failures"].extend(extra)
            sample["verdict"] = "fail"
        samples[name] = sample
    for name in ROUNDS:
        call = by_name.get(name)
        if call is not None:
            _judge_round(run, name, call, samples, failures)
    missing = [name for name in SAMPLES if name not in samples]
    verdicts = {sample["verdict"] for sample in samples.values()}
    if failures or missing or "fail" in verdicts:
        verdict = "fail"
    elif "unknown" in verdicts:
        verdict = "unknown"
    else:
        verdict = "pass"
    return {
        "run": str(run),
        "verdict": verdict,
        "expected": len(SAMPLES),
        "passed": sum(1 for sample in samples.values() if sample["verdict"] == "pass"),
        "missing": missing,
        "run_failures": failures,
        "unreached": result.get("unreached"),
        "samples": samples,
    }


def main() -> int:
    report = check(Path(sys.argv[1]))
    print(json.dumps(report, indent=2))
    return 0 if report["verdict"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
