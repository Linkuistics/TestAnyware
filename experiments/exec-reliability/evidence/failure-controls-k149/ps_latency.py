"""Measure real /bin/ps latency against the runtime's helper capture bound.

Several historical failures share one shape: a real ps helper ran past its
capture deadline (about 128 ms), the runtime killed and reaped it, and the
group state became unknown. The fixture's own ps audits once exceeded 1 s.
This tool measures the same ps queries idle, under CPU load, and against
exiting processes. Each load process and target is killed and reaped.
"""

import argparse
import json
import os
import signal
import statistics
import subprocess
import sys
import time
from pathlib import Path

GROUP = ("-o", "pid=,pgid=,stat=")
TARGET = (
    "import os,sys,time\n"
    "b=bytearray(int(sys.argv[1]))\n"
    "for i in range(0,len(b),4096): b[i]=1\n"
    "os.write(1,b'r'); time.sleep(30)\n"
)


def summary(values: list[float]) -> dict[str, float | int]:
    ordered = sorted(values)
    return {
        "count": len(ordered),
        "min_ms": ordered[0],
        "median_ms": statistics.median(ordered),
        "p95_ms": ordered[int(len(ordered) * 0.95)],
        "max_ms": ordered[-1],
        "over_128_ms": sum(value > 128 for value in ordered),
    }


def timed(argv: tuple[str, ...]) -> tuple[float, subprocess.CompletedProcess[bytes]]:
    start = time.monotonic()
    result = subprocess.run(argv, capture_output=True, check=False, timeout=5)
    return (time.monotonic() - start) * 1000, result


def loaded(load: int, count: int) -> dict[str, object]:
    busy = [subprocess.Popen((sys.executable, "-c", "while True: pass")) for _ in range(load)]
    try:
        time.sleep(0.2)
        group = ("/bin/ps", "-g", str(os.getpgrp()), *GROUP)
        values = [timed(group)[0] for _ in range(count)]
        usage = [timed(("/bin/ps", "-o"))[0] for _ in range(count // 4)]
    finally:
        for process in busy:
            process.kill()
        returncodes = [process.wait() for process in busy]
    return {
        "load_processes": load,
        "group_query": summary(values),
        "usage_failure": summary(usage),
        "load_reaped": all(code == -signal.SIGKILL for code in returncodes),
    }


def exiting(size: int, count: int) -> dict[str, object]:
    values: list[float] = []
    states: dict[str, int] = {}
    reaped: list[bool] = []
    for _ in range(count):
        target = subprocess.Popen(
            (sys.executable, "-c", TARGET, str(size)),
            stdout=subprocess.PIPE,
            start_new_session=True,
        )
        assert target.stdout is not None
        target.stdout.read(1)
        os.killpg(target.pid, signal.SIGKILL)
        value, result = timed(("/bin/ps", "-g", str(target.pid), *GROUP))
        fields = result.stdout.decode().split()
        state = fields[2] if len(fields) > 2 else "-"
        states[state] = states.get(state, 0) + 1
        values.append(value)
        reaped.append(target.wait() == -signal.SIGKILL)
    return {
        "target_bytes": size,
        "group_query": summary(values),
        "states": states,
        "reaped": all(reaped),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    parser.add_argument("--count", type=int, default=100)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True)
    cores = os.cpu_count() or 1
    result = {
        "control": "real-ps-latency",
        "python": sys.version,
        "cpu_count": cores,
        "process_count": len(
            subprocess.run(("/bin/ps", "-A"), capture_output=True, check=True).stdout.splitlines()
        )
        - 1,
        "loaded": [loaded(load, args.count) for load in (0, cores, cores * 3)],
        "exiting": [exiting(size, args.count // 5) for size in (0, 2_000_000_000)],
    }
    (out / "ps-latency.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    main()
