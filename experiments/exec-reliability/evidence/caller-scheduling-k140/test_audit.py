"""Missing fixture records must not produce a vacuously clean absence audit."""

import json
from pathlib import Path

import pytest
from run_condition import caller_audits, expired_audits


@pytest.mark.parametrize("damage", ["no-journals", "start-only", "invalid-completion"])
def test_incomplete_fixture_is_unresolved(tmp_path: Path, damage: str) -> None:
    runs = tmp_path / "runs"
    runs.mkdir()
    if damage != "no-journals":
        (runs / "persistent-expired-start-exited-1.json").write_text(
            json.dumps({"schedule": "exited", "pid": 123, "phase": "after-start"})
        )
    if damage == "invalid-completion":
        (runs / "persistent-expired-independent-exited-1.json").write_text("{")
    audits = expired_audits(tmp_path)
    assert len(audits) == 4
    assert {item["schedule"] for item in audits} == {
        "exited",
        "stopped",
        "natural",
        "ownership-lost",
    }
    assert all(item["absent"] is False for item in audits)
    assert all(item["complete"] is False for item in audits)


@pytest.mark.parametrize("damage", ["no-journals", "start-only", "unmatched-end"])
def test_incomplete_caller_accounting_is_unresolved(tmp_path: Path, damage: str) -> None:
    runs = tmp_path / "runs"
    runs.mkdir()
    if damage != "no-journals":
        (runs / "request-start-False-False-False.json").write_text("{}")
    if damage == "unmatched-end":
        (runs / "request-turns-fixture.json").write_text('{"declaration": "other"}')
    audits = caller_audits(tmp_path)
    assert len(audits) == 8
    assert all(item["complete"] is False for item in audits)
    assert all(item["absent"] is False for item in audits)
