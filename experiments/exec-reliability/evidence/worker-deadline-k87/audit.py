"""Reconcile frozen timing, isolated guard mutations and prior evidence bytes."""

import hashlib
import json
import os
import signal
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
LIVE = ROOT / "experiments/exec-reliability"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cases(path: Path) -> dict[str, dict[str, object]]:
    result = {}
    for case in ET.parse(path).iter("testcase"):
        key = f"{case.attrib['classname']}::{case.attrib['name']}"
        assert key not in result, key
        failure = case.find("failure")
        error = case.find("error")
        result[key] = {
            "status": "failed" if failure is not None or error is not None else "passed",
            "seconds": float(case.attrib["time"]),
            "failure": "" if failure is None else failure.attrib["message"],
        }
        assert case.find("skipped") is None, key
    return result


def main() -> None:
    output = BASE / (
        sys.argv[1] if len(sys.argv) > 1 else os.environ.get("DIAGNOSTIC_AUDIT", "audit-1")
    )
    output.mkdir(exist_ok=False)
    names = (
        "timing-k81-1",
        "timing-k83-1",
        "timing-k81-2",
        "timing-k83-2",
        "signal-red-1",
        "reap-red-1",
        "green-1",
        "full-1",
        "full-2",
    )
    # Freeze the evidence being reconciled, including declarations and verdicts.
    # This is the only walk; after measurement, hash these exact paths again.
    subjects = [Path(__file__), BASE / "prior-evidence.json"]
    subjects.extend(
        BASE / f"prior-path-comparison{suffix}" for suffix in (".json", ".diff", ".stderr")
    )
    subjects.extend(p for name in names for p in (BASE / name).rglob("*") if p.is_file())
    before = {str(p): digest(p) for p in subjects}
    summaries = {}
    tables = {}
    for name in names:
        folder = BASE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_after"].items():
            source = Path(path)
            # Full verification ran live, but its exact source bytes are archived.
            if name.startswith("full-") and (
                source.parent == LIVE or source == ROOT / "Taskfile.yml"
            ):
                source = BASE / "green-inputs" / source.name
            if source.is_relative_to(BASE):
                assert digest(source) == expected, source
            else:
                # External interpreter/executables are identified by recorded hashes;
                # reproduction does not require today's binaries to match old ones.
                assert source == ROOT / "Taskfile.yml" or source.name in {
                    "python",
                    "ps",
                    "sh",
                    "sleep",
                    "true",
                    "false",
                }, source
                if source == ROOT / "Taskfile.yml":
                    assert digest(BASE / "green-inputs/Taskfile.yml") == expected
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, path
        checks = verdict["checks"]
        assert all(c["reaped"] and not c["timed_out"] for c in checks), name
        table = cases(folder / "pytest.xml")
        tables[name] = table
        failures = {key for key, value in table.items() if value["status"] == "failed"}
        if name.endswith("-red-1"):
            expected_failures = {
                key for key in table if name == "reap-red-1" or "[final-reap-" not in key
            }
            assert failures == expected_failures, (name, failures, expected_failures)
            assert all("assert not [" in table[key]["failure"] for key in failures), name
        elif name.startswith("timing-"):
            # Exploratory timings retain every original failure by identity.
            # A failed replay is never promoted to a successful full check.
            assert len(table) == (424 if "k81" in name else 432), name
        elif name == "full-1":
            assert failures == {
                "test_worker_capture::test_late_created_identity_rejects_invalid_fields[token]"
            }, failures
            assert table[next(iter(failures))]["failure"] == "assert (False)"
        else:
            assert not failures, (name, failures)
        assert verdict["ok"] is (not failures), name
        assert checks[-1]["returncode"] == (1 if failures else 0), name
        summaries[name] = {"checks": checks, "cases": len(table), "failed": sorted(failures)}

    green = tables["green-1"]
    assert len(green) == 10
    assert set(green) == set(tables["signal-red-1"]) == set(tables["reap-red-1"])
    assert set(green) <= set(tables["full-1"]) == set(tables["full-2"])
    assert len(tables["full-2"]) == 434
    controls = {}
    for name in ("signal-red-1", "reap-red-1", "green-1", "full-1", "full-2"):
        folder = BASE / name / "runs"
        paths = list(folder.glob("worker-deadline-audit-*.json"))
        raw = {
            data["worker"]["pid"]: data
            for path in folder.glob("worker-worker-deadline-*.json")
            for data in [json.loads(path.read_text())]
        }
        assert len(paths) == len(raw) == 10, name
        controls[name] = []
        for path in paths:
            data = json.loads(path.read_text())
            audit, calls, end = data["audit"], data["calls"], data["deadline"]
            receipt = raw[audit["pid"]]
            worker = receipt["worker"]
            assert audit["reaped"] and os.WIFSIGNALED(audit["status"]), path
            assert os.WTERMSIG(audit["status"]) in (signal.SIGTERM, signal.SIGKILL), path
            assert calls and all(call["pid"] == audit["pid"] for call in calls)
            assert all(c["flags"] == os.WNOHANG for c in calls if "flags" in c)
            late = [c for c in calls if c["time"] >= end]
            if name == "signal-red-1":
                assert all(c["operation"] in {"kill", "killpg"} for c in late), path
                assert bool(late) is ("final-reap" not in path.name), path
            elif name == "reap-red-1":
                assert late and all(c["operation"] == "waitpid" for c in late), path
            else:
                assert not late and audit["source"] == "independent exact-child reap", path
                assert receipt["error"] and receipt["residual_remedy"]
                assert not worker["reaped"] and worker["shutdown_deadline_monotonic"] == end
            assert data["cleanup_cutoff"] is ("cleanup-cutoff" in path.name), path
            if data["cleanup_cutoff"] and name.endswith("red-1"):
                assert any(c["time"] == end for c in late), path
            expected_signal = "killpg" if worker["ready"] else "kill"
            assert {c["operation"] for c in calls if "signal" in c} == {expected_signal}
            controls[name].append({"audit": path.name, "late": late, "status": audit["status"]})

    frozen = BASE / "green-inputs"
    assert (frozen / "worker_capture.py").read_bytes() == (
        BASE.parent / "worker-deadline-k83/green-inputs/worker_capture.py"
    ).read_bytes()
    for label, guard in (
        ("signal", "    if now >= owner.shutdown_deadline_monotonic:\n        return\n"),
        ("reap", "    if time.monotonic() >= owner.shutdown_deadline_monotonic:\n        return\n"),
    ):
        mutant = BASE / f"{label}-mutant-inputs"
        for source in frozen.iterdir():
            if source.is_file():
                expected = source.read_bytes()
                if source.name == "worker_capture.py":
                    assert expected.count(guard.encode()) == 1
                    expected = expected.replace(guard.encode(), b"", 1)
                assert (mutant / source.name).read_bytes() == expected, (label, source)
    for label, old in (
        ("k81", "probe-intervals-k81/inputs-green"),
        ("k83", "worker-deadline-k83/green-inputs"),
    ):
        source = BASE.parent / old
        relocated = BASE / f"timing-{label}-relocated-inputs"
        for path in [*source.glob("*.py"), source / "uv.lock", source / "pyproject.toml"]:
            expected = path.read_bytes()
            if path.name == "self_check.py":
                expected = expected.replace(
                    b'HERE.parents[1] / "Taskfile.yml"', b'HERE / "Taskfile.yml"'
                )
            assert (relocated / path.name).read_bytes() == expected, path

    tables["retained-k81"] = cases(BASE.parent / "probe-intervals-k81/full-01/pytest.xml")
    tables["retained-k83"] = cases(BASE.parent / "worker-deadline-k83/full-1/pytest.xml")
    comparison = json.loads((BASE / "prior-path-comparison.json").read_text())
    assert comparison["returncode"] == 0
    assert comparison["stdout_bytes"] == comparison["stderr_bytes"] == 0
    assert (BASE / "prior-path-comparison.diff").read_bytes() == b""
    assert (BASE / "prior-path-comparison.stderr").read_bytes() == b""
    timing = {}
    for older, newer in (("retained-k81", "retained-k83"), ("timing-k81-2", "timing-k83-2")):
        left, right = tables[older], tables[newer]
        assert set(left) <= set(right)
        added = sorted(set(right) - set(left))
        deltas = {key: right[key]["seconds"] - left[key]["seconds"] for key in left}
        timing[f"{older}:{newer}"] = {
            "added": {key: right[key]["seconds"] for key in added},
            "added_seconds": sum(right[key]["seconds"] for key in added),
            "shared_delta_seconds": sum(deltas.values()),
            "shared_deltas": deltas,
        }
    prior = json.loads((BASE / "prior-evidence.json").read_text())
    differences = [name for name, value in prior.items() if digest(ROOT / name) != value]
    after = {path: digest(Path(path)) for path in before}
    result = {
        "ok": before == after and not differences,
        "subjects_before": before,
        "subjects_after": after,
        "checks": summaries,
        "cases": tables,
        "controls": controls,
        "timing": timing,
        "prior_file_entries_checked": len(prior),
        "prior_differences": differences,
        "inventory_limit": (
            "file bytes including file symlink targets; "
            "cache paths, link text and directory links excluded"
        ),
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"audit ok={result['ok']}; {len(prior)} prior entries; evidence {output}")
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
