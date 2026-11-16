"""Reproduce k145's interrupted live-group shape with declared child readiness.

The k145 failure ran test_review_live_group_is_failure_even_with_expected_leader_exit
with interrupt_after=0.3 and term_grace=0.15. Its receipt shows PID TERM, then
PID KILL at the TERM deadline, then leader exit -9. It has no readiness time.

This tool runs the same controller call against one frozen input directory. A
separate writer process declares child readiness at a fixed offset after the
planned TERM. The scope child receives a ready path under a missing directory,
so only the writer can declare readiness. Inputs whose peer offers held-child
use that child mode with an existing directory instead, because those peers
also write a spawned declaration beside the ready path. The writer records its
own write time.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

INTERRUPT_AFTER = 0.3
TERM_GRACE = 0.15
KILL_WAIT = 0.3
# Offsets after the planned TERM. None means readiness is never declared.
OFFSETS: tuple[float | None, ...] = (-0.2, 0.05, 0.13, None)
WRITER = """
import os, sys, time
target, ready = float(sys.argv[1]), sys.argv[2]
while time.monotonic() < target:
    time.sleep(min(0.001, max(0.0, target - time.monotonic())))
os.makedirs(os.path.dirname(ready), exist_ok=True)
with open(ready, "wb") as stream:
    stream.write(b"ready")
print(time.monotonic(), flush=True)
"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def case(inputs: Path, out: Path, offset: float | None) -> dict[str, object]:
    import controller  # noqa: PLC0415  (imported from the frozen inputs)

    label = "never" if offset is None else f"{offset:+.3f}"
    work = out / f"work-{label}"
    work.mkdir()
    release, ready = work / "release", work / "late" / "ready"
    peer = inputs / "peer.py"
    held = "held-child" in peer.read_text()
    child_mode: tuple[str, ...] = ()
    if held:
        ready.parent.mkdir()
        child_mode = ("held-child",)
    call = controller.Call(
        "live-exit",
        (sys.executable, str(peer), "scope-parent", str(release), str(ready), *child_mode),
        5,
        interrupt_after=INTERRUPT_AFTER,
        expected_reason="interrupted",
    )
    plan = controller.Plan(
        uuid.uuid4().hex,
        (call,),
        (),
        (peer, Path("/bin/sleep")),
        term_grace=TERM_GRACE,
        kill_wait=KILL_WAIT,
    )
    planned = time.monotonic()
    writer = None
    if offset is not None:
        target = planned + INTERRUPT_AFTER + offset
        writer = subprocess.Popen(
            (sys.executable, "-I", "-c", WRITER, repr(target), str(ready)),
            stdout=subprocess.PIPE,
        )
    try:
        path = controller.execute(plan, out / "evidence")
    finally:
        release.write_bytes(b"release")
    written = None
    if writer is not None:
        stdout, _ = writer.communicate(timeout=10)
        written = float(stdout.decode().strip())
    receipt = json.loads((path / "live-exit.json").read_text())
    signals = receipt.get("signals", [])
    term = next((item for item in signals if item["signal"] == "TERM"), None)
    kill = next((item for item in signals if item["signal"] == "KILL"), None)
    child_text = (path / "live-exit.stdout").read_text().strip()
    audit: list[str] = []
    if child_text.isdigit():
        end = time.monotonic() + 1
        while time.monotonic() < end:
            probe = subprocess.run(
                ("/bin/ps", "-o", "pid=,stat=", "-p", child_text),
                capture_output=True,
                check=False,
            )
            audit.append(probe.stdout.decode().strip())
            if probe.returncode != 0:
                break
            time.sleep(0.005)
    return {
        "offset_after_planned_term": offset,
        "readiness_method": "held-child" if held else "missing-directory",
        "run": str(path.relative_to(out)),
        "planned_monotonic": planned,
        "started_monotonic": receipt.get("started_monotonic"),
        "term": term,
        "kill": kill,
        "term_deadline_monotonic": receipt.get("term_deadline_monotonic"),
        "ready_written_monotonic": written,
        "ready_after_term": None
        if written is None or term is None
        else written - term["monotonic"],
        "leader_exit_monotonic": receipt.get("leader_exit_monotonic"),
        "returncode": receipt.get("returncode"),
        "reason": receipt.get("reason"),
        "child_stdout": child_text,
        "child_audit": audit,
        "child_absent": not child_text.isdigit() or (bool(audit) and audit[-1] == ""),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--repeat", type=int, default=3)
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    out.mkdir(parents=True)
    sys.path.insert(0, str(inputs))
    subjects = sorted(inputs.glob("*.py"))
    before = {path.name: sha256(path) for path in subjects}
    cases = [
        case(inputs, out / f"repeat-{index}", offset)
        for index in range(args.repeat)
        for offset in OFFSETS
        if (out / f"repeat-{index}").mkdir(exist_ok=True) is None
    ]
    after = {path.name: sha256(path) for path in subjects}
    result = {
        "control": "k145-interrupted-live-group-readiness",
        "inputs": str(inputs),
        "python": sys.version,
        "pid": os.getpid(),
        "interrupt_after": INTERRUPT_AFTER,
        "term_grace": TERM_GRACE,
        "kill_wait": KILL_WAIT,
        "subjects_before": before,
        "subjects_after": after,
        "subjects_unchanged": before == after,
        "cases": cases,
    }
    (out / "scope-readiness.json").write_text(json.dumps(result, indent=2) + "\n")
    for item in cases:
        print(item["offset_after_planned_term"], item["returncode"], item["ready_after_term"])


if __name__ == "__main__":
    main()
