"""Reconcile cutoff controls, isolated mutants, full suites and prior bytes."""

import hashlib
import json
import os
import signal
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
LIVE = ROOT / "experiments/exec-reliability"
MUTATIONS = {
    "signal-removed": ("if time.monotonic() >= kill_deadline:", "if False:"),
    "entry-removed": ("if time.monotonic() >= cutoff:", "if False:"),
    "return-removed": ("if observed >= cutoff:", "if False:"),
    "signal-equality-removed": (
        "if time.monotonic() >= kill_deadline:",
        "if time.monotonic() > kill_deadline:",
    ),
    "entry-equality-removed": ("if time.monotonic() >= cutoff:", "if time.monotonic() > cutoff:"),
    "return-equality-removed": ("if observed >= cutoff:", "if observed > cutoff:"),
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def files(folder: Path) -> list[Path]:
    return sorted(
        p
        for p in folder.rglob("*")
        if p.is_file()
        and not any(part in {"__pycache__", ".pytest_cache", ".ruff_cache"} for part in p.parts)
    )


def cases(path: Path) -> dict[str, dict[str, object]]:
    result = {}
    for case in ET.parse(path).iter("testcase"):
        key = f"{case.attrib['classname']}::{case.attrib['name']}"
        assert key not in result and case.find("skipped") is None, key
        failure = case.find("failure")
        error = case.find("error")
        result[key] = {
            "status": "failed" if failure is not None or error is not None else "passed",
            "seconds": float(case.attrib["time"]),
            "failure": "" if failure is None else failure.attrib["message"],
        }
    return result


def controls(stem: str, parameters: list[str]) -> set[str]:
    return {f"test_controller::test_termination_cutoff_{stem}[{p}]" for p in parameters}


def main() -> None:
    output = BASE / os.environ.get("DIAGNOSTIC_AUDIT", "audit-1")
    output.mkdir(exist_ok=False)
    names = ["red-1", "red-2", "red-3", "red-4", "green-1", "green-2", "full-1", "full-2"]
    names.extend(f"{name}-1" for name in MUTATIONS)
    inputs = [p for p in BASE.iterdir() if p.is_dir() and p.name.endswith("inputs")]
    subjects = [Path(__file__), BASE / "prior-evidence.json", ROOT / "Taskfile.yml"]
    subjects.extend(BASE.glob("run_condition*.py"))
    subjects.extend(p for folder in inputs for p in files(folder))
    subjects.extend(p for name in names for p in files(BASE / name))
    subjects.extend([*LIVE.glob("*.py"), LIVE / "pyproject.toml", LIVE / "uv.lock"])
    prior = json.loads((BASE / "prior-evidence.json").read_text())
    subjects.extend(ROOT / name for name in prior)
    before = {str(p): digest(p) for p in subjects}
    (output / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    late_signals = controls(
        "refuses_late_signals", [f"{x}-{p}" for x in (0.0, 0.01) for p in (False, True)]
    )
    equality_signals = controls("refuses_late_signals", [f"0.0-{p}" for p in (False, True)])
    expired = controls("refuses_expired_first_turn", ["False", "True"])
    entry = controls("observer_entry_at_equality", ["False", "True"])
    late_return = controls("checks_observer_return", ["True-False", "True-True"])
    timely_return = controls("checks_observer_return", ["False-False", "False-True"])
    real = controls("real_delayed_observer", ["False", "True"])
    all_controls = late_signals | expired | entry | late_return | timely_return | real
    expected = {
        "red-2": late_signals | expired | late_return | real,
        "red-3": late_signals | expired | late_return | real,
        "red-4": all_controls - timely_return,
        "green-1": set(),
        "green-2": set(),
        "full-2": set(),
        "signal-removed-1": late_signals | expired | real,
        "entry-removed-1": expired | entry,
        "return-removed-1": late_return,
        "signal-equality-removed-1": equality_signals | expired,
        "entry-equality-removed-1": entry,
        "return-equality-removed-1": late_return,
        "full-1": {
            "test_controller::test_review_later_zombie_table_cannot_repair_earlier_eperm",
            "test_controller::test_review_checker_unreaped_timeout_still_retains_verdict",
            *{
                f"test_controller::test_review_{stem}[{p}]"
                for stem in (
                    "exited_leader_does_not_hide_a_surviving_group_member",
                    "live_group_is_failure_even_with_expected_leader_exit",
                )
                for p in (False, True)
            },
        },
    }
    tables, summaries, real_audits = {}, {}, {}
    runner_versions = {digest(p) for p in BASE.glob("run_condition*.py")}
    for name in names:
        folder = BASE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, value in verdict["subjects_after"].items():
            source = Path(path)
            if source == BASE / "run_condition.py":
                assert value in runner_versions, name
            elif source.is_relative_to(BASE) or source == ROOT / "Taskfile.yml":
                assert digest(source) == value, source
            else:
                assert source.name in {"python", "ps", "sh", "sleep", "true", "false"}, source
        for path, value in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == value, path
        checks = verdict["checks"]
        assert all(c["reaped"] for c in checks), name
        if name == "red-1":
            assert not verdict["ok"] and checks[0]["timed_out"], name
            assert checks[0]["returncode"] == -signal.SIGTERM
            assert not (folder / "pytest.xml").exists()
            summaries[name] = {"checks": checks, "cases": "timeout, incomplete"}
            continue
        assert all(not c["timed_out"] for c in checks), name
        table = cases(folder / "pytest.xml")
        tables[name] = table
        failed = {key for key, value in table.items() if value["status"] == "failed"}
        assert failed == expected[name], (name, failed, expected[name])
        assert verdict["ok"] is (not failed), name
        assert checks[0]["returncode"] == (1 if failed else 0), name
        assert all(c["returncode"] == 0 for c in checks[1:]), name
        summaries[name] = {"checks": checks, "cases": len(table), "failed": sorted(failed)}
        if name.endswith("removed-1"):
            for key in failed:
                message = str(table[key]["failure"])
                needle = (
                    "SIGKILL"
                    if key in late_signals
                    else "target OS operation admitted"
                    if key in expired
                    else "exit observation admitted"
                    if key in entry
                    else "leader_exit_monotonic"
                    if key in late_return
                    else "and False"
                )
                assert needle in message, (name, key, message)
        audits = []
        for path in sorted((folder / "runs").glob("termination-cutoff-*.json")):
            data = json.loads(path.read_text())
            pid, receipt = data["pid"], data["receipt"]
            cutoff = receipt["kill_reap_deadline_monotonic"]
            assert data["pinned"]["pid"] == data["after"]["pid"] == pid
            assert data["pinned"]["state"] == "zombie" and data["after"]["state"] == "absent"
            assert data["returncode"] in (-signal.SIGTERM, -signal.SIGKILL)
            assert all(
                e["target"] == pid and e["scope"] == data["scope"] for e in receipt["signals"]
            )
            assert all(s[0] == data["scope"] for s in data["signals"])
            late = [s for s in data["signals"] if s[2] >= cutoff]
            bad_signal = name in {"red-3", "red-4", "signal-removed-1"}
            assert bool(late) is bad_signal, (name, path)
            assert [s[1] for s in data["signals"]] == (
                [signal.SIGTERM, signal.SIGKILL] if bad_signal else [signal.SIGTERM]
            ), path
            assert receipt["termination_ok"] is False
            assert not receipt["group_observations"][0]["query_attempted"]
            audits.append({"file": path.name, "pid": pid, "scope": data["scope"], "late": late})
        if name not in {"red-2"}:
            assert len(audits) == 2 and {a["scope"] for a in audits} == {"pid", "group"}, name
        real_audits[name] = audits

    assert len(all_controls) == 14 and set(tables["green-2"]) == all_controls
    assert set(tables["red-3"]) == set(tables["green-1"]) == all_controls - entry
    assert set(tables["red-4"]) == all_controls
    assert set(tables["full-1"]) == set(tables["full-2"])
    assert len(tables["full-2"]) == 448 and all_controls <= set(tables["full-2"])
    for label, (old, new) in MUTATIONS.items():
        assert set(tables[f"{label}-1"]) == all_controls
        for path in files(BASE / "green-v2-inputs"):
            expected_bytes = path.read_bytes()
            if path.name == "controller.py":
                assert expected_bytes.count(old.encode()) == 1
                expected_bytes = expected_bytes.replace(old.encode(), new.encode(), 1)
            assert (BASE / f"{label}-inputs" / path.name).read_bytes() == expected_bytes, path
    for path in files(BASE / "green-v2-inputs"):
        expected_bytes = path.read_bytes()
        if path.name == "self_check.py":
            expected_bytes = expected_bytes.replace(
                b'HERE.parents[1] / "Taskfile.yml"', b'HERE / "Taskfile.yml"'
            )
        assert (BASE / "green-v3-inputs" / path.name).read_bytes() == expected_bytes, path
        assert (LIVE / path.name).read_bytes() == path.read_bytes(), path
    assert digest(BASE / "green-v3-inputs/Taskfile.yml") == digest(ROOT / "Taskfile.yml")
    assert (BASE / "red-v4-inputs/controller.py").read_bytes() == (
        BASE / "red-v3-inputs/controller.py"
    ).read_bytes()
    for path in files(BASE / "green-v2-inputs"):
        if path.name != "controller.py":
            assert (BASE / "red-v4-inputs" / path.name).read_bytes() == path.read_bytes(), path
    differences = [name for name, value in prior.items() if before[str(ROOT / name)] != value]
    after = {path: digest(Path(path)) for path in before}
    result = {
        "ok": before == after and not differences,
        "subjects_before": before,
        "subjects_after": after,
        "checks": summaries,
        "cases": tables,
        "real_child_audits": real_audits,
        "prior_file_entries_checked": len(prior),
        "prior_differences": differences,
        "inventory_limit": "File bytes; caches, symlink text and directory symlinks excluded.",
        "external_tools": "Recorded historical executable hashes; no current-byte assumption.",
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                "ok": result["ok"],
                "prior_entries": len(prior),
                "prior_differences": differences,
                "conditions": len(names),
            }
        )
    )
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
