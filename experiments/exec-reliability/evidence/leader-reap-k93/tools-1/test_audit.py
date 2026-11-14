"""Corruption controls for the final runnable auditor's validation boundaries."""

import copy
import errno
import os
import shutil
import sys
from pathlib import Path

import audit
import pytest


def test_unrelated_archive_cannot_satisfy_subject():
    verdict = audit.load(audit.OLD / "green-2/verdict.json")
    audit.check_subjects(verdict, "green-2")
    subject = next(name for name in verdict["subjects_before"] if name.endswith("controller.py"))
    for field in ("subjects_before", "subjects_after"):
        verdict[field][subject] = audit.digest(audit.OLD / "Taskfile.green-2.yml")
    with pytest.raises(AssertionError, match="subject identity"):
        audit.check_subjects(verdict, "green-2")


@pytest.mark.parametrize("defect", ["missing", "extra", "absolute"])
def test_inventory_is_exactly_tracked(defect):
    inventory = {"paths": {"experiments/exec-reliability/evidence/one": {}}}
    expected = set(inventory["paths"])
    audit.check_inventory(inventory, expected)
    if defect == "missing":
        inventory["paths"].clear()
    else:
        inventory["paths"]["/tmp/extra" if defect == "absolute" else "ignored-local-file"] = {}
    with pytest.raises(AssertionError, match="tracked inventory equality"):
        audit.check_inventory(inventory, expected)


@pytest.mark.parametrize(
    "field,value",
    [
        ("attempts", []),
        ("reap_errno", errno.EPERM),
        ("reap_deadline_monotonic", 101.0),
        ("reap_attempts", 0),
        ("reap_last_result", "not_ready"),
    ],
)
def test_child_corruption_is_rejected(field, value):
    item = {
        "pid": 123,
        "deadline": 100.0,
        "outcome": "missing",
        "before": {"state": "zombie"},
        "after": {"state": "absent"},
        "attempts": [[123, os.WNOHANG]],
        "receipt": {
            "authority_closed_monotonic": 100.0,
            "reap_deadline_monotonic": 100.0,
            "within_process_bounds": False,
            "reaped": False,
            "residual_leader_pid": 123,
            "reap_errno": errno.ECHILD,
            "reap_attempts": 1,
            "reap_last_result": "error",
        },
    }
    audit.check_child(item, False, False)
    damaged = copy.deepcopy(item)
    (damaged if field == "attempts" else damaged["receipt"])[field] = value
    with pytest.raises(AssertionError):
        audit.check_child(damaged, False, False)


def test_duplicate_test_identity_is_rejected(tmp_path):
    (tmp_path / "pytest.xml").write_text(
        '<testsuite><testcase classname="one" name="case"/>'
        '<testcase classname="one" name="case"/></testsuite>'
    )
    with pytest.raises(AssertionError, match="duplicate case identity"):
        audit.cases(tmp_path)


def test_relocated_historical_subjects(tmp_path, monkeypatch):
    # A different physical root is necessary: resolving to the original checkout
    # would hide the old absolute-path defect.
    old = tmp_path / "experiments/exec-reliability/evidence/leader-reap-k88"
    (old / "green-inputs").mkdir(parents=True)
    shutil.copyfile(audit.OLD / "green-inputs/controller.py", old / "green-inputs/controller.py")
    shutil.copyfile(audit.OLD / "Taskfile.green-2.yml", old / "Taskfile.green-2.yml")
    with monkeypatch.context() as patch:
        patch.setattr(audit, "REPO", tmp_path)
        patch.setattr(audit, "OLD", old)
        assert audit.resolve(audit.PREFIX + "Taskfile.yml") == tmp_path / "Taskfile.yml"
        assert audit.historical_subject(audit.PREFIX + "Taskfile.yml", "green-2") == (
            old / "Taskfile.green-2.yml"
        )
        subject = audit.PREFIX + "experiments/exec-reliability/controller.py"
        sha = audit.digest(old / "green-inputs/controller.py")
        audit.check_subjects(
            {"subjects_before": {subject: sha}, "subjects_after": {subject: sha}}, "full-1"
        )
        assert audit.resolve(sys.executable) == Path(sys.executable)


@pytest.mark.parametrize("defect", ["missing-inventory", "unrelated-archive"])
def test_final_entrypoint_rejects_corruption(tmp_path, monkeypatch, defect):
    original_load = audit.load

    def load(path):
        value = original_load(path)
        if defect == "missing-inventory" and path == audit.ROOT / "prior-tracked.json":
            del value["paths"][next(iter(value["paths"]))]
        if defect == "unrelated-archive" and path == audit.OLD / "green-2/verdict.json":
            subject = next(
                name for name in value["subjects_before"] if name.endswith("controller.py")
            )
            for field in ("subjects_before", "subjects_after"):
                value[field][subject] = audit.digest(audit.OLD / "Taskfile.green-2.yml")
        return value

    output = tmp_path / "audit.json"
    monkeypatch.setattr(audit, "load", load)
    monkeypatch.setattr(sys, "argv", [str(audit.ROOT / "audit.py"), str(output)])
    with pytest.raises(
        AssertionError,
        match=(
            "tracked inventory equality" if defect == "missing-inventory" else "subject identity"
        ),
    ):
        audit.main()
    assert not output.exists()
