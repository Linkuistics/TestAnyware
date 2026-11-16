"""Controls for the k186 live completion tooling (leaf macos-completion-live-k191).

Each control breaks one property that the review k190 named and expects the tool to
refuse it. The controls need no VM. Fake executables stand in for the CLI and the
outer commands.
"""

import copy
import json
import os
import shutil
import stat
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import completion_k186_check as checker

HERE = Path(__file__).resolve().parent
RETAINED = HERE / "evidence/k186-completion-macos-01/b1256cae4068418dac994e03f0392f6b"
RECEIPT = b'{"ok": true, "exit_code": 0, "timed_out": false, "stdout": "ordinary-ok", "stderr": ""}'
EOF: dict[str, object] = {"stdout": True, "stderr": True}


def _executable(path: Path, text: str) -> Path:
    path.write_text(text)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


# F1: the receipt must be one object with each expected field.


@pytest.mark.parametrize("raw", [b"null", b"[]", b"1", b'"ok"', b"{}", b'{"ok": true}'])
def test_receipt_that_is_not_a_full_object_fails(raw: bytes) -> None:
    assert checker.judge(0, 0.1, raw, EOF)["verdict"] == "fail"


def test_exact_receipt_passes() -> None:
    assert checker.judge(0, 0.1, RECEIPT, EOF)["verdict"] == "pass"


def test_unrecorded_eof_is_unknown_and_unreached_eof_fails() -> None:
    assert checker.judge(0, 0.1, RECEIPT, {"stdout": None, "stderr": True})["verdict"] == "unknown"
    assert checker.judge(0, 0.1, RECEIPT, {"stdout": False, "stderr": True})["verdict"] == "fail"


# F2: the run must have the frozen shape and healthy host evidence.


def _complete_run(tmp_path: Path) -> Path:
    """Copy the retained run and add the EOF fields that the repaired wrapper records."""
    run = tmp_path / "run"
    shutil.copytree(RETAINED, run)
    for name in checker.ROUNDS:
        path = run / f"{name}.stdout"
        round_ = json.loads(path.read_bytes())
        for child in round_["children"]:
            child["stdout_eof"] = child["stderr_eof"] = True
        path.write_text(json.dumps(round_))
    return run


def _edit_result(run: Path, edit: Callable[[dict[str, Any]], None]) -> None:
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    edit(result)
    path.write_text(json.dumps(result))


def test_retained_run_is_unknown_because_round_eof_was_not_recorded() -> None:
    report = checker.check(RETAINED)
    assert report["verdict"] == "unknown"
    assert report["missing"] == [] and report["run_failures"] == []
    assert all(report["samples"][name]["verdict"] == "pass" for name in checker.SERIAL)
    rounds = [key for key in report["samples"] if key.startswith("concurrent-")]
    assert len(rounds) == 12
    assert all(report["samples"][key]["verdict"] == "unknown" for key in rounds)


def test_complete_run_passes(tmp_path: Path) -> None:
    report = checker.check(_complete_run(tmp_path))
    assert (report["verdict"], report["passed"]) == ("pass", 22)


def test_run_with_no_calls_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)
    _edit_result(run, lambda result: _set(result, ("calls",), []))
    report = checker.check(run)
    assert report["verdict"] == "fail" and len(report["missing"]) == 22


def test_missing_result_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)
    (run / "result.json").unlink()
    assert checker.check(run)["verdict"] == "fail"


def test_round_with_three_children_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)
    path = run / "concurrent-2.stdout"
    round_ = json.loads(path.read_bytes())
    round_["children"].pop()
    path.write_text(json.dumps(round_))
    report = checker.check(run)
    assert report["verdict"] == "fail" and report["missing"] == ["concurrent-2.3"]


def test_duplicate_child_index_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)
    path = run / "concurrent-1.stdout"
    round_ = json.loads(path.read_bytes())
    round_["children"][3]["index"] = 0
    path.write_text(json.dumps(round_))
    report = checker.check(run)
    assert report["verdict"] == "fail" and "concurrent-1.3" in report["missing"]


def test_duplicate_call_name_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)

    def duplicate(result: dict[str, Any]) -> None:
        serial = next(call for call in result["calls"] if call["name"] == "serial-01")
        result["calls"].append(copy.deepcopy(serial))

    _edit_result(run, duplicate)
    assert checker.check(run)["verdict"] == "fail"


UNHEALTHY: dict[str, tuple[tuple[str, ...], object]] = {
    "result": (("ok",), False),
    "unreached": (("unreached",), ["pool"]),
    "inspection": (("inspection_failure",), "lost"),
    "subjects": (("subjects_after", "/x"), "changed"),
    "storage": (("storage", "frames", "dropped"), 1),
    "serial-call": (("call:serial-03", "ok"), False),
    "round-call": (("call:concurrent-2", "ok"), False),
    "round-eof": (("call:concurrent-3", "streams", "stdout", "completeness"), "deadline"),
}


def _set(result: dict[str, Any], path: tuple[str, ...], value: object) -> None:
    """Set one field of result.json. A `call:NAME` step selects the call of that name."""
    node: dict[str, Any] = result
    for step in path[:-1]:
        if step.startswith("call:"):
            calls: list[dict[str, Any]] = node["calls"]
            node = next(call for call in calls if call["name"] == step.removeprefix("call:"))
        else:
            node = node[step]
    node[path[-1]] = value


@pytest.mark.parametrize("case", sorted(UNHEALTHY))
def test_unhealthy_host_evidence_fails(tmp_path: Path, case: str) -> None:
    run = _complete_run(tmp_path)
    path, value = UNHEALTHY[case]
    _edit_result(run, lambda result: _set(result, path, value))
    assert checker.check(run)["verdict"] == "fail"


def test_storage_close_that_is_not_orderly_fails(tmp_path: Path) -> None:
    run = _complete_run(tmp_path)
    (run / "storage-close.json").write_text(
        json.dumps({"state": "orderly", "refused": 0, "failed": 1})
    )
    assert checker.check(run)["verdict"] == "fail"


def test_checker_exit_follows_the_verdict(tmp_path: Path) -> None:
    def exit_of(run: Path) -> int:
        argv = [sys.executable, "-I", str(HERE / "completion_k186_check.py"), str(run)]
        return subprocess.run(argv, capture_output=True, check=False).returncode

    assert exit_of(_complete_run(tmp_path)) == 0
    assert exit_of(RETAINED) == 1


# F3: the wrapper must read each pipe to EOF, not only to the reap of the leader.

FAKE_CLI = """#!/bin/sh
# The leader writes the first half of the receipt. A descendant keeps stdout and writes
# the second half after the leader exits.
printf '{"ok": true, "exit_code": 0,'
(sleep "$FAKE_DELAY"; printf ' "timed_out": false, "stdout": "ordinary-ok", "stderr": ""}') &
exit 0
"""


def _round(tmp_path: Path, delay: str) -> tuple[int, dict[str, Any]]:
    cli = _executable(tmp_path / "cli", FAKE_CLI)
    argv = [sys.executable, "-I", str(HERE / "concurrent_k186.py"), "2", "10", str(cli), "vm"]
    done = subprocess.run(
        argv, capture_output=True, check=False, timeout=30, env={**os.environ, "FAKE_DELAY": delay}
    )
    return done.returncode, json.loads(done.stdout)


def test_wrapper_reads_output_written_after_the_leader_reap(tmp_path: Path) -> None:
    code, round_ = _round(tmp_path, "0.3")
    assert code == 0
    for child in round_["children"]:
        assert (child["stdout_eof"], child["stderr_eof"]) == (True, True)
        assert bytes.fromhex(child["stdout_hex"]) == RECEIPT


def test_wrapper_marks_a_pipe_held_past_the_grace_as_incomplete(tmp_path: Path) -> None:
    code, round_ = _round(tmp_path, "3")
    assert code == 1
    for child in round_["children"]:
        assert child["stdout_eof"] is False
        raw = bytes.fromhex(child["stdout_hex"])
        eof = {"stdout": child["stdout_eof"], "stderr": child["stderr_eof"]}
        assert checker.judge(child["returncode"], child["elapsed"], raw, eof)["verdict"] == "fail"


# F4 and F5: the runner must exit 1 on any failure and keep tool digests around the check.

STUB = """#!/bin/sh
case "$*" in
  *completion_k186.py*)
    printf 'verdict %s run %s\\n' "$STUB_VERDICT" "$STUB_RUN"
    exit "$STUB_PROBE_EXIT" ;;
  *completion_k186_check.py*) echo '{}'; exit "$STUB_CHECK_EXIT" ;;
esac
exit 0
"""


def _runner(tmp_path: Path, verdict: str, probe_exit: str, check_exit: str) -> tuple[int, Path]:
    """Run the runner in a copy of the repository layout, with every outer command faked."""
    repo = tmp_path / "repo"
    tools = repo / "experiments/exec-reliability"
    tools.mkdir(parents=True)
    for name in (
        "completion_k186_run.sh",
        "completion_k186.py",
        "completion_k186_check.py",
        "concurrent_k186.py",
        "routes_k19.py",
        "controller.py",
        "pyproject.toml",
        "uv.lock",
    ):
        shutil.copy2(HERE / name, tools / name)
    (repo / "scripts").mkdir()
    shutil.copy2(HERE.parents[1] / "scripts/vm_inventory.py", repo / "scripts/vm_inventory.py")
    shutil.copy2(HERE.parents[1] / "Taskfile.yml", repo / "Taskfile.yml")
    bin_ = tmp_path / "bin"
    bin_.mkdir()
    for name in ("uv", "jj", "testanyware", "codesign", "tart", "pgrep"):
        _executable(bin_ / name, STUB)
    env = {
        **os.environ,
        "PATH": f"{bin_}:{os.environ['PATH']}",
        "STUB_VERDICT": verdict,
        "STUB_RUN": str(tmp_path / "out/run"),
        "STUB_PROBE_EXIT": probe_exit,
        "STUB_CHECK_EXIT": check_exit,
    }
    out = tmp_path / "out"
    argv = ["sh", str(tools / "completion_k186_run.sh"), str(out)]
    done = subprocess.run(argv, cwd=repo, env=env, capture_output=True, check=False, timeout=30)
    return done.returncode, Path(f"{out}-outer")


@pytest.mark.parametrize(
    ("verdict", "probe_exit", "check_exit", "expected"),
    [("True", "0", "0", 0), ("False", "0", "0", 1), ("True", "1", "0", 1), ("True", "0", "1", 1)],
    ids=["clean", "false-verdict", "probe-exit", "check-exit"],
)
def test_runner_exit_follows_probe_and_check(
    tmp_path: Path, verdict: str, probe_exit: str, check_exit: str, expected: int
) -> None:
    code, outer = _runner(tmp_path, verdict, probe_exit, check_exit)
    assert code == expected
    assert (outer / "run.exit").read_text().strip() == str(expected)
    before = (outer / "tooling-before.sha256").read_text()
    assert before == (outer / "tooling-after.sha256").read_text()
    assert "completion_k186_check.py" in before and "completion_k186_run.sh" in before


def test_probe_subjects_include_the_receipt_checker_and_runner() -> None:
    import completion_k186

    subjects = completion_k186.plan(
        {"python": sys.executable, "cli": "/bin/sh", "tart": "/bin/sh"}
    ).subjects
    names = {Path(subject).name for subject in subjects}
    assert {"completion_k186_check.py", "completion_k186_run.sh", "concurrent_k186.py"} <= names
