"""Freeze one input set for a k149 control, with an optional declared mutation."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    # Remove the reap cutoff gate: every later _reap reaches waitpid.
    "reap-ignores-cutoff": (
        "worker_capture.py",
        "if now >= owner.shutdown_deadline_monotonic and owner.reap_attempts:",
        "if False:",
    ),
    # Remove the signal cutoff gate: a later _signal still sends.
    "signal-ignores-cutoff": (
        "worker_capture.py",
        "    now = time.monotonic()\n"
        "    if now >= owner.shutdown_deadline_monotonic:\n"
        "        return\n",
        "    now = time.monotonic()\n",
    ),
    # Stale admission (k149 review): the reap gate reads a clock 1 s old, so a
    # later reap passes the cutoff check with an early admission time.
    "stale-reap-admission": (
        "worker_capture.py",
        "    now = time.monotonic()\n"
        "    if now >= owner.shutdown_deadline_monotonic and owner.reap_attempts:",
        "    now = time.monotonic() - 1.0\n"
        "    if now >= owner.shutdown_deadline_monotonic and owner.reap_attempts:",
    ),
    # Stale admission (k149 review): the signal gate reads a clock 1 s old.
    "stale-signal-admission": (
        "worker_capture.py",
        "    now = time.monotonic()\n"
        "    if now >= owner.shutdown_deadline_monotonic:\n"
        "        return\n",
        "    now = time.monotonic() - 1.0\n"
        "    if now >= owner.shutdown_deadline_monotonic:\n"
        "        return\n",
    ),
    # Remove k116's repair: cleanup reads control EOF but does not record it.
    "omit-cleanup-eof": (
        "worker_capture.py",
        "                                        if not chunk:\n"
        "                                            closed_channels.add(0)\n",
        "",
    ),
    # Fixture stall: the scope parent starts 0.4 s late, so TERM at 0.3 s
    # meets the default disposition before the handler exists (k85).
    "peer-startup-stall": (
        "peer.py",
        "        release, ready, *child_mode = args\n",
        "        release, ready, *child_mode = args\n        time.sleep(0.4)\n",
    ),
    # Drop late helper identities: a late publication creates no obligation.
    "late-identity-dropped": (
        "worker_capture.py",
        'if not any(known["pid"] == identity["pid"] for known in helper_identities):',
        'if not late and not any(known["pid"] == identity["pid"] for known in helper_identities):',
    ),
    # Fixture stall: the TERM-handled worker loses the processor for 30 ms
    # after it delivers the late publication. The runtime KILLs it at grace.
    "fixture-stall-after-late-write": (
        "test_worker_capture.py",
        "            os.write(fd, data)  # Late terminal/reap must never retire the obligation.\n",
        "            os.write(fd, data)  # Late terminal/reap must never retire the obligation.\n"
        "            time.sleep(0.03)\n",
    ),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--mutant", choices=MUTATIONS)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    for path in [*sorted(source.glob("*.py")), source / "pyproject.toml", source / "uv.lock"]:
        shutil.copy2(path, destination / path.name)
    taskfile = source / "Taskfile.yml"
    if not taskfile.is_file():
        taskfile = source.parents[1] / "Taskfile.yml"
    shutil.copy2(taskfile, destination / "Taskfile.yml")
    if args.mutant:
        name, before, after = MUTATIONS[args.mutant]
        path = destination / name
        text = path.read_text()
        if text.count(before) != 1:
            raise ValueError(f"mutation {args.mutant}: expected exactly one source site")
        path.write_text(text.replace(before, after))
    (destination / "preparation.json").write_text(
        json.dumps({"source": str(source), "mutation": args.mutant}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
