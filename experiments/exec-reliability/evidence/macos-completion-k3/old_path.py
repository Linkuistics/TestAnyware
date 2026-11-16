"""k3 old-path detector for the macOS synchronous-completion timeout.

The script builds `probe/main.swift` with the unchanged agent file
`agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`. It then runs a
frozen trial plan against that build. With `--variant single-hop`, it also
builds a detector control. The control replaces the nested dispatch in
`waitForExit` with polling on the outer worker. The control is evidence for the
detector, not a proposed repair.

Every input, bound and hash goes into OUT/plan.json before the first trial.
OUT must not exist. The script exits 0 only when the observed verdict equals
`--expect`, the calibration-cap check passes and every trial leaves no owned
process or capture file behind.

Each probe runs in its own session. When a probe reaches the trial bound, the
script records the identity of the probe and its descendants and the capture
files that they hold open. It then kills the probe's process group and each
identified descendant, moves the held capture files into OUT, and audits the
result within a cleanup bound. Every trial also compares the capture files in
the per-user temporary directory before and after the trial.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import signal
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
CLEANUP_BOUND_SECONDS = 5.0
CAP_CHECK_BLOCKERS = 8
CAPTURE_PREFIX = "testanyware-exec-"
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


def pids_of(argv: list[str]) -> list[int]:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=10, check=False)
    return [int(word) for word in done.stdout.split()]


def descendants(pid: int) -> list[int]:
    found: list[int] = []
    frontier = [pid]
    while frontier:
        children = pids_of(["pgrep", "-P", str(frontier.pop())])
        found.extend(children)
        frontier.extend(children)
    return found


def identity(pid: int) -> str | None:
    """Return the start time and command of PID, or None when it is gone."""
    done = subprocess.run(
        ["ps", "-o", "lstart=,command=", "-p", str(pid)],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    return done.stdout.strip() or None


def signal_owned(owned: dict[int, str]) -> list[int]:
    signalled: list[int] = []
    for pid, recorded in owned.items():
        if identity(pid) != recorded:
            continue
        try:
            os.kill(pid, signal.SIGKILL)
            signalled.append(pid)
        except ProcessLookupError:
            pass
    return signalled


def capture_dir() -> Path:
    """Return the per-user directory where Foundation puts temporary files.

    Darwin Foundation takes `FileManager.default.temporaryDirectory` from this
    directory and ignores TMPDIR, so the harness cannot give the unchanged
    runner a directory of its own.
    """
    return Path(tool_version(["getconf", "DARWIN_USER_TEMP_DIR"])).resolve()


def captures(directory: Path) -> set[str]:
    return {entry.name for entry in directory.iterdir() if entry.name.startswith(CAPTURE_PREFIX)}


def open_captures(pids: list[int]) -> list[Path]:
    """Return the runner capture files that the given processes hold open."""
    if not pids:
        return []
    done = subprocess.run(
        ["lsof", "-nP", "-a", "-p", ",".join(str(pid) for pid in pids), "-Fn"],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    found: set[Path] = set()
    for line in done.stdout.splitlines():
        if line.startswith("n") and Path(line[1:]).name.startswith(CAPTURE_PREFIX):
            found.add(Path(line[1:]).resolve())
    return sorted(found)


def contain(
    proc: subprocess.Popen[bytes],
    out: Path,
    name: str,
    directory: Path,
    before: set[str],
    bounded_out: bool,
) -> tuple[bytes, bytes, dict[str, Any]]:
    """Kill, reap and audit everything that the trial owns.

    A probe that reached the bound still holds its capture files open, because
    the runner closes them only after the wait. The harness names those files
    by the open handles of the identified processes, kills the processes and
    moves the files into OUT/NAME.captures/. Any other new capture file in the
    per-user directory is unattributed, stays in place and makes the trial
    unclean.
    """
    pgid = proc.pid
    owned: dict[int, str] = {}
    candidates = [proc.pid, *descendants(proc.pid)] if bounded_out else []
    candidates += pids_of(["pgrep", "-g", str(pgid)])
    for pid in candidates:
        found = identity(pid)
        if found is not None:
            owned.setdefault(pid, found)
    held = open_captures(list(owned))
    group_signalled = False
    if owned:
        try:
            os.killpg(pgid, signal.SIGKILL)
            group_signalled = True
        except ProcessLookupError:
            pass
    signalled = signal_owned(owned)
    stdout, stderr = b"", b""
    if bounded_out:
        try:
            stdout, stderr = proc.communicate(timeout=CLEANUP_BOUND_SECONDS)
        except subprocess.TimeoutExpired:
            pass
    deadline = time.monotonic() + CLEANUP_BOUND_SECONDS
    survivors = [pid for pid, recorded in owned.items() if identity(pid) == recorded]
    while survivors and time.monotonic() < deadline:
        time.sleep(0.05)
        survivors = [pid for pid, recorded in owned.items() if identity(pid) == recorded]
    moved: list[str] = []
    if held:
        keep = out / f"{name}.captures"
        keep.mkdir()
        for path in held:
            if path.exists():
                shutil.move(path, keep / path.name)
                moved.append(path.name)
    unattributed = sorted(captures(directory) - before)
    audit = {
        "owned": {str(pid): found for pid, found in owned.items()},
        "group_signalled": group_signalled,
        "signalled": signalled,
        "survivors": survivors,
        "held_captures": [str(path) for path in held],
        "moved_captures": moved,
        "unattributed_captures": unattributed,
        "leaked": not bounded_out and bool(owned or unattributed),
        "clean": not survivors and not unattributed and len(moved) == len(held),
    }
    return stdout, stderr, audit


def run_probe(out: Path, name: str, argv: list[str], bound_seconds: float) -> dict[str, Any]:
    directory = capture_dir()
    before = captures(directory)
    start = time.monotonic()
    record: dict[str, Any] = {
        "name": name,
        "argv": argv,
        "capture_dir": str(directory),
        "monotonic_start": start,
    }
    proc = subprocess.Popen(
        argv,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    record["pid"] = proc.pid
    try:
        stdout, stderr = proc.communicate(timeout=bound_seconds)
        record["bounded_out"] = False
    except subprocess.TimeoutExpired:
        record["bounded_out"] = True
        stdout, stderr = b"", b""
    record["seconds"] = time.monotonic() - start
    late_stdout, late_stderr, record["cleanup"] = contain(
        proc, out, name, directory, before, record["bounded_out"]
    )
    stdout += late_stdout
    stderr += late_stderr
    record["cleanup_seconds"] = time.monotonic() - start - record["seconds"]
    record["exit"] = None if record["bounded_out"] else proc.returncode
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


def run_trial(
    out: Path, variant: str, condition: str, index: int, bound_seconds: float
) -> dict[str, Any]:
    name = f"{variant}-{condition}-{index:02d}"
    argv = [str(out / f"probe-{variant}"), condition, str(GUEST_TIMEOUT_SECONDS)]
    return run_probe(out, name, argv, bound_seconds)


def cap_check(out: Path, variant: str, bound_seconds: float) -> dict[str, Any]:
    """Make calibration fail at a small cap and require that the probe
    releases every blocker that it admitted."""
    argv = [
        str(out / f"probe-{variant}"),
        "free-1",
        str(GUEST_TIMEOUT_SECONDS),
        str(CAP_CHECK_BLOCKERS),
    ]
    record = run_probe(out, f"{variant}-cap-check", argv, bound_seconds)
    receipt = record["receipt"]
    fields: dict[str, Any] = receipt if isinstance(receipt, dict) else {}  # pyright: ignore[reportUnknownVariableType]
    passed = (
        record["class"] == "calibration-failed"
        and fields.get("saturated_at") is None
        and fields.get("admitted") == CAP_CHECK_BLOCKERS
        and fields.get("released") == CAP_CHECK_BLOCKERS
        and record["cleanup"]["clean"]
        and not record["cleanup"]["leaked"]
    )
    return {"name": record["name"], "class": record["class"], "passed": passed}


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
    parser.add_argument("--trial-bound-seconds", type=float, default=TRIAL_BOUND_SECONDS)
    args = parser.parse_args()
    out: Path = args.out.resolve()
    variant: str = args.variant
    bound_seconds: float = args.trial_bound_seconds
    out.mkdir(parents=True, exist_ok=False)

    before = subjects()
    plan = {
        "variant": variant,
        "expect": args.expect,
        "conditions": list(CONDITIONS),
        "trials_per_condition": TRIALS,
        "order": "for each index, unsaturated then free-2 then free-1",
        "guest_timeout_seconds": GUEST_TIMEOUT_SECONDS,
        "trial_bound_seconds": bound_seconds,
        "cleanup_bound_seconds": CLEANUP_BOUND_SECONDS,
        "cap_check_blockers": CAP_CHECK_BLOCKERS,
        "ownership": "each probe in its own session, captures named by open handles",
        "capture_dir": str(capture_dir()),
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
    capped = cap_check(out, variant, bound_seconds)
    print(f"{capped['name']}: {capped['class']} passed={capped['passed']}", flush=True)
    trials: list[dict[str, Any]] = []
    for index in range(1, TRIALS + 1):
        for condition in CONDITIONS:
            trial = run_trial(out, variant, condition, index, bound_seconds)
            trials.append(trial)
            print(f"{trial['name']}: {trial['class']} {trial['seconds']:.3f}s", flush=True)

    after = subjects()
    summary = verdict(trials, variant)
    summary["build"] = built
    summary["subjects_after"] = after
    summary["subjects_unchanged"] = after == before
    summary["expect"] = args.expect
    summary["cap_check"] = capped
    summary["bounded_out"] = [t["name"] for t in trials if t["bounded_out"]]
    summary["leaked"] = [t["name"] for t in trials if t["cleanup"]["leaked"]]
    summary["unclean"] = [t["name"] for t in trials if not t["cleanup"]["clean"]]
    owned_clean = not summary["leaked"] and not summary["unclean"]
    summary["owned_resources_clean"] = owned_clean
    summary["matches_expectation"] = (
        summary["verdict"] == args.expect and after == before and capped["passed"] and owned_clean
    )
    with (out / "summary.json").open("x") as stream:
        json.dump(summary, stream, indent=2, sort_keys=True)
    print(f"verdict: {summary['verdict']} (expected {args.expect})")
    return 0 if summary["matches_expectation"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
