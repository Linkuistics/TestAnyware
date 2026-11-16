"""Red/green control for the k151 child-absence verdict.

Each case feeds real /bin/ps results to two classifiers. The k149 predicate is
copied from failure-controls-k149/scope_readiness.py line 127. The k151 verdict
is imported from the sibling scope_readiness.py. A case passes when the
classifier gives the expected verdict. The k149 predicate must fail the cases
without a retained identity and with a ps usage error. The k151 verdict must
pass every case.

A last check proves that the supplementary case-path scan finds a live process
whose argv names a case path, and finds nothing once that process is reaped.
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import cast

sys.path.insert(0, str(Path(__file__).resolve().parent))
from scope_readiness import child_verdict, probe_child, scan_case_path  # noqa: E402


def k149_absent(child_text: str, audit: list[str]) -> bool:
    """The k149 predicate, unchanged."""
    return not child_text.isdigit() or (bool(audit) and audit[-1] == "")


def k149_view(child_text: str, audit: list[dict[str, object]]) -> bool:
    # k149 retained only the stripped stdout of each probe.
    return k149_absent(child_text, [str(item["stdout"]).strip() for item in audit])


def k151_view(pid: int | None, audit: list[dict[str, object]]) -> bool | None:
    status, _ = child_verdict(pid, audit)
    return {"absent": True, "present": False}.get(status)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=Path)
    out = parser.parse_args().out.resolve()
    out.mkdir(parents=True)

    live = subprocess.Popen(("/bin/sleep", "30"))
    gone = subprocess.Popen(("/usr/bin/true",))
    gone.wait()
    time.sleep(0.05)
    try:
        inputs: list[tuple[str, str, int | None, bool | None]] = [
            # name, peer stdout, retained PID, expected absence
            ("no-identity", "", None, None),
            ("usage-error-too-large", "99999999999", 99999999999, None),
            ("usage-error-negative", "-5", -5, None),
            ("reaped-child", str(gone.pid), gone.pid, True),
            ("live-child", str(live.pid), live.pid, False),
        ]
        cases: list[dict[str, object]] = []
        for name, text, pid, expected in inputs:
            if pid is None:
                audit: list[dict[str, object]] = []
            elif name == "live-child":
                # One query is enough; probe_child would poll for 1 s.
                probe = subprocess.run(
                    ("/bin/ps", "-o", "pid=,stat=", "-p", str(pid)),
                    capture_output=True,
                    check=False,
                )
                audit = [
                    {
                        "monotonic": time.monotonic(),
                        "returncode": probe.returncode,
                        "stdout": probe.stdout.decode(),
                        "stderr": probe.stderr.decode(),
                    }
                ]
            else:
                audit = probe_child(pid)
            old, new = k149_view(text, audit), k151_view(pid, audit)
            cases.append(
                {
                    "case": name,
                    "pid": pid,
                    "audit": audit,
                    "expected_absent": expected,
                    "k149_absent": old,
                    "k149_pass": old is expected,
                    "k151_absent": new,
                    "k151_status": child_verdict(pid, audit),
                    "k151_pass": new is expected,
                }
            )
    finally:
        live.kill()
        live.wait()
    marker = out / "work-scan" / "release"
    holder = subprocess.Popen(
        (sys.executable, "-I", "-c", "import time; time.sleep(30)", str(marker))
    )
    time.sleep(0.2)
    try:
        scan_live = scan_case_path(marker)
    finally:
        holder.kill()
        holder.wait()
    scan_gone = scan_case_path(marker)
    live_matches = cast(list[str], scan_live["matches"])
    scan: dict[str, object] = {
        "holder_pid": holder.pid,
        "live": scan_live,
        "reaped": scan_gone,
        "pass": len(live_matches) == 1
        and str(holder.pid) in live_matches[0]
        and scan_gone["matches"] == []
        and bool(scan_gone["scanner_row_listed"]),
    }
    k149_failed = [item["case"] for item in cases if not item["k149_pass"]]
    k151_failed = [item["case"] for item in cases if not item["k151_pass"]]
    result: dict[str, object] = {
        "control": "k151-child-absence-verdict",
        "case_path_scan_control": scan,
        "k149_failed": k149_failed,
        "k151_failed": k151_failed,
        "cases": cases,
    }
    (out / "absence-controls.json").write_text(json.dumps(result, indent=2) + "\n")
    print("k149 failed:", k149_failed)
    print("k151 failed:", k151_failed)
    print("case-path scan control:", scan["pass"])
    if k151_failed or not k149_failed or not scan["pass"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
