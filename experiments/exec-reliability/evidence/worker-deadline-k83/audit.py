"""Reconcile deadline controls, exact-child audits and frozen evidence bytes."""

import hashlib
import json
import os
import signal
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
LIVE = ROOT / "experiments/exec-reliability"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cases(path: Path) -> dict[str, str]:
    result = {}
    for case in ET.parse(path).iter("testcase"):
        key = f"{case.attrib['classname']}::{case.attrib['name']}"
        assert key not in result, key
        result[key] = (
            "failed"
            if case.find("failure") is not None or case.find("error") is not None
            else "skipped"
            if case.find("skipped") is not None
            else "passed"
        )
    return result


def main() -> None:
    output = BASE / "audit-1"
    output.mkdir(exist_ok=False)
    verdicts = {
        name: json.loads((BASE / name / "verdict.json").read_text())
        for name in ("red-1", "red-2", "green-1", "full-1")
    }
    subjects = {
        Path(path)
        for verdict in verdicts.values()
        for path in (*verdict["artifact_sha256"], *verdict["subjects_after"])
    }
    subjects.update(BASE / name / "verdict.json" for name in verdicts)
    subjects.update([Path(__file__), BASE / "prior-evidence.json"])
    subjects.update(LIVE.glob("*.py"))
    before = {str(path): digest(path) for path in sorted(subjects)}
    (output / "declaration.json").write_text(json.dumps({"subjects_before": before}, indent=2))
    for name, verdict in verdicts.items():
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_after"].items():
            assert digest(Path(path)) == expected, path
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, path
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"])
        assert verdict["ok"] is (not name.startswith("red-")), name
    red = cases(BASE / "red-2/pytest.xml")
    green = cases(BASE / "green-1/pytest.xml")
    full = cases(BASE / "full-1/pytest.xml")
    assert len(red) == len(green) == 8 and red.keys() == green.keys()
    assert set(red.values()) == {"failed"} and set(green.values()) == {"passed"}
    failures = list(ET.parse(BASE / "red-2/pytest.xml").iter("failure"))
    assert len(failures) == 8
    assert all("assert not [" in item.attrib["message"] for item in failures)
    assert len(full) == 432 and set(full.values()) == {"passed"}
    assert all(full[key] == value for key, value in green.items())
    for path in (BASE / "green-inputs").iterdir():
        if not path.is_file():
            continue
        current = ROOT / "Taskfile.yml" if path.name == "Taskfile.yml" else LIVE / path.name
        if path.name == "self_check.py":
            # The full-suite checker additionally freezes the actual fixture executables.
            added = (
                '        Path("/bin/sh"),\n'
                '        Path("/bin/sleep"),\n'
                '        Path("/usr/bin/true"),\n'
                '        Path("/usr/bin/false"),\n'
            )
            assert current.read_text().count(added) == 1
            assert path.read_text() == current.read_text().replace(added, "", 1)
        else:
            assert digest(path) == digest(current), path
        if path.name != "worker_capture.py":
            assert digest(path) == digest(BASE / "red2-inputs" / path.name), path
    receipts = {}
    for name in verdicts:
        paths = sorted((BASE / name / "runs").glob("worker-deadline-audit-*.json"))
        raw = {
            data["worker"]["pid"]: (path, data)
            for path in (BASE / name / "runs").glob("worker-worker-deadline-*.json")
            for data in [json.loads(path.read_text())]
        }
        expected_count = 6 if name == "red-1" else 8
        assert len(paths) == len(raw) == expected_count, name
        receipts[name] = []
        for path in paths:
            data = json.loads(path.read_text())
            audit, calls, end = data["audit"], data["calls"], data["deadline"]
            raw_path, receipt = raw[audit["pid"]]
            assert audit["reaped"] and audit["adapter"] == "deferred parent WNOHANG observation"
            assert os.WIFSIGNALED(audit["status"])
            assert os.WTERMSIG(audit["status"]) in (signal.SIGTERM, signal.SIGKILL)
            assert calls and all(call["pid"] == audit["pid"] for call in calls)
            assert all(call["flags"] == os.WNOHANG for call in calls if "flags" in call)
            late = [call for call in calls if call["time"] >= end]
            if name.startswith("red-"):
                assert late, path
            else:
                assert not late, path
                assert audit["source"] == "independent exact-child reap"
                worker = receipt["worker"]
                assert receipt["error"] and receipt["residual_remedy"]
                assert not worker["reaped"] and worker["shutdown_deadline_monotonic"] == end
                expected_signal = "killpg" if worker["ready"] else "kill"
                signals = [call["operation"] for call in calls if "signal" in call]
                assert signals and set(signals) == {expected_signal}
            receipts[name].append({"receipt": str(raw_path), "audit": str(path)})
    prior = json.loads((BASE / "prior-evidence.json").read_text())
    # This original inventory hashes file bytes, including symlink-file targets.
    # It does not inventory symlink text or symlinks to directories.
    after_prior = {name: digest(ROOT / name) for name in prior}
    differences = [name for name in prior if prior[name] != after_prior[name]]
    after = {str(path): digest(path) for path in sorted(subjects)}
    result = {
        "ok": before == after and not differences,
        "subjects_before": before,
        "subjects_after": after,
        "red_cases": red,
        "green_cases": green,
        "full_cases": full,
        "real_child_receipts": receipts,
        "prior_file_entries_checked": len(prior),
        "prior_inventory_limit": "file bytes only; symlink text and directory links excluded",
        "prior_differences": differences,
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"audit ok={result['ok']}; {len(prior)} prior file entries; evidence {output}")
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
