"""k3 old-path detector for the macOS synchronous-completion timeout.

The script builds `probe/main.swift` with the unchanged agent file
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`. It then runs a
frozen trial plan against that build. With `--variant single-hop`, it also
builds a detector control. The control replaces the nested dispatch in
`waitForExit` with polling on the outer worker. The control is evidence for the
detector, not a proposed repair.

Every input, bound and hash goes into OUT/plan.json before the first trial.
OUT must not exist. The script exits 0 only when the observed verdict equals
`--expect`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
RUNNER = ROOT / "agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift"
PROBE = HERE / "probe/main.swift"
SELF = Path(__file__).resolve()

CONDITIONS = ("unsaturated", "free-2", "free-1")
TRIALS = 5
GUEST_TIMEOUT_SECONDS = 3
TRIAL_BOUND_SECONDS = 20.0
BUILD_BOUND_SECONDS = 300.0
FAST_SECONDS = 1.0

NESTED = """                let semaphore = DispatchSemaphore(value: 0)
                DispatchQueue.global().async {
                    process.waitUntilExit()
                    semaphore.signal()
                }
                let result = semaphore.wait(timeout: .now() + .seconds(seconds))
                continuation.resume(returning: result == .success)"""

SINGLE_HOP = """                let deadline = DispatchTime.now() + .seconds(seconds)
                while process.isRunning && DispatchTime.now() < deadline {
                    usleep(10_000)
                }
                continuation.resume(returning: !process.isRunning)"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def subjects() -> dict[str, str]:
    return {str(p.relative_to(ROOT)): sha256(p) for p in (RUNNER, PROBE, SELF)}


def tool_version(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=60, check=False)
    return (done.stdout + done.stderr).strip()


def sysctl(name: str) -> str:
    return tool_version(["sysctl", "-n", name])


def build(out: Path, variant: str) -> dict[str, Any]:
    source = RUNNER
    if variant == "single-hop":
        text = RUNNER.read_text()
        if text.count(NESTED) != 1:
            raise SystemExit("the nested waitForExit block is not present exactly once")
        source = out / "ProcessRunner-single-hop.swift"
        source.write_text(text.replace(NESTED, SINGLE_HOP))
    binary = out / f"probe-{variant}"
    argv = [
        "swiftc",
        "-O",
        "-swift-version",
        "5",
        "-module-name",
        "K3Probe",
        str(source),
        str(PROBE),
        "-o",
        str(binary),
    ]
    start = time.monotonic()
    done = subprocess.run(
        argv, capture_output=True, text=True, timeout=BUILD_BOUND_SECONDS, check=False
    )
    (out / f"build-{variant}.stdout").write_text(done.stdout)
    (out / f"build-{variant}.stderr").write_text(done.stderr)
    if done.returncode != 0:
        raise SystemExit(f"build of {variant} failed with exit {done.returncode}")
    return {
        "variant": variant,
        "argv": argv,
        "source_sha256": sha256(source),
        "binary_sha256": sha256(binary),
        "seconds": time.monotonic() - start,
    }


def run_trial(out: Path, variant: str, condition: str, index: int) -> dict[str, Any]:
    name = f"{variant}-{condition}-{index:02d}"
    argv = [str(out / f"probe-{variant}"), condition, str(GUEST_TIMEOUT_SECONDS)]
    start = time.monotonic()
    record: dict[str, Any] = {"name": name, "argv": argv, "monotonic_start": start}
    try:
        done = subprocess.run(argv, capture_output=True, timeout=TRIAL_BOUND_SECONDS, check=False)
        stdout, stderr, code = done.stdout, done.stderr, done.returncode
        record["bounded_out"] = False
    except subprocess.TimeoutExpired as expired:
        stdout = expired.stdout or b""
        stderr = expired.stderr or b""
        code = None
        record["bounded_out"] = True
    record["seconds"] = time.monotonic() - start
    record["exit"] = code
    (out / f"{name}.stdout").write_bytes(stdout)
    (out / f"{name}.stderr").write_bytes(stderr)
    try:
        record["receipt"] = json.loads(stdout.decode())
    except (UnicodeDecodeError, json.JSONDecodeError):
        record["receipt"] = None
    record["class"] = classify(record)
    with (out / f"{name}.json").open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
    return record


def classify(record: dict[str, Any]) -> str:
    receipt = record["receipt"]
    if record["bounded_out"] or record["exit"] != 0 or not isinstance(receipt, dict):
        return "harness-failure"
    fields: dict[str, Any] = receipt  # pyright: ignore[reportUnknownVariableType]
    if fields.get("outcome") == "calibration-failed":
        return "calibration-failed"
    if fields.get("outcome") != "returned" or fields.get("stdout") != "ordinary-ok":
        return "unexpected"
    if fields.get("timed_out") is True and fields.get("exit_code") == -1:
        return "completed-then-timed-out"
    run_seconds = fields.get("run_seconds")
    if (
        fields.get("timed_out") is False
        and fields.get("exit_code") == 0
        and isinstance(run_seconds, float)
        and run_seconds < FAST_SECONDS
    ):
        return "prompt"
    return "unexpected"


def verdict(trials: list[dict[str, Any]], variant: str) -> dict[str, Any]:
    by_condition: dict[str, list[str]] = {c: [] for c in CONDITIONS}
    for trial in trials:
        if trial["name"].startswith(variant + "-"):
            condition = trial["name"][len(variant) + 1 : -3]
            by_condition[condition].append(trial["class"])
    controls_prompt = all(
        cls == "prompt" for c in ("unsaturated", "free-2") for cls in by_condition[c]
    )
    starved = by_condition["free-1"]
    if not controls_prompt:
        result = "controls-failed"
    elif all(cls == "completed-then-timed-out" for cls in starved):
        result = "defect"
    elif all(cls == "prompt" for cls in starved):
        result = "clean"
    else:
        result = "mixed"
    return {"variant": variant, "classes": by_condition, "verdict": result}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    parser.add_argument("--variant", choices=("old", "single-hop"), default="old")
    parser.add_argument("--expect", choices=("defect", "clean"), required=True)
    args = parser.parse_args()
    out: Path = args.out.resolve()
    variant: str = args.variant
    out.mkdir(parents=True, exist_ok=False)

    before = subjects()
    plan = {
        "variant": variant,
        "expect": args.expect,
        "conditions": list(CONDITIONS),
        "trials_per_condition": TRIALS,
        "order": "for each index, unsaturated then free-2 then free-1",
        "guest_timeout_seconds": GUEST_TIMEOUT_SECONDS,
        "trial_bound_seconds": TRIAL_BOUND_SECONDS,
        "build_bound_seconds": BUILD_BOUND_SECONDS,
        "fast_seconds": FAST_SECONDS,
        "subjects_before": before,
        "swiftc": tool_version(["swiftc", "--version"]),
        "python": sys.version,
        "host": platform.platform(),
        "hw_activecpu": sysctl("hw.activecpu"),
        "wq_max_constrained_threads": sysctl("kern.wq_max_constrained_threads"),
        "pid": os.getpid(),
    }
    with (out / "plan.json").open("x") as stream:
        json.dump(plan, stream, indent=2, sort_keys=True)

    built = build(out, variant)
    trials: list[dict[str, Any]] = []
    for index in range(1, TRIALS + 1):
        for condition in CONDITIONS:
            trial = run_trial(out, variant, condition, index)
            trials.append(trial)
            print(f"{trial['name']}: {trial['class']} {trial['seconds']:.3f}s", flush=True)

    after = subjects()
    summary = verdict(trials, variant)
    summary["build"] = built
    summary["subjects_after"] = after
    summary["subjects_unchanged"] = after == before
    summary["expect"] = args.expect
    summary["matches_expectation"] = summary["verdict"] == args.expect and after == before
    with (out / "summary.json").open("x") as stream:
        json.dump(summary, stream, indent=2, sort_keys=True)
    print(f"verdict: {summary['verdict']} (expected {args.expect})")
    return 0 if summary["matches_expectation"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
