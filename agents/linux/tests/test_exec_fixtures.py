"""Check the recorded Linux exec replies that leaf k218 reads.

`task agent:linux:record-exec-replies` writes them from a Linux container.
This test runs on any host. Each fixture must be a valid record, and the
legacy fields of each legacy case must equal the old Linux reply.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from tests.exec_validity import record_errors, stream_bytes

FIXTURES = Path(__file__).parent / "fixtures"
RECORDS = sorted((FIXTURES / "exec-records").glob("*.json"))
LEGACY = {
    c["case"]: c for c in json.loads((FIXTURES / "legacy_linux_replies.json").read_text())["cases"]
}


def test_fixtures_exist() -> None:
    names = {path.stem for path in RECORDS}
    assert set(LEGACY) <= names
    assert {"remaining-group", "requested-id", "timeout-child-kill"} <= names


@pytest.mark.parametrize("path", RECORDS, ids=[path.stem for path in RECORDS])
def test_fixture_is_a_valid_record(path: Path) -> None:
    fixture = json.loads(path.read_text())
    body = fixture["body"]
    assert fixture["httpStatus"] == 200
    assert record_errors(body, exec_id=fixture["request"].get("id")) == []
    legacy = LEGACY.get(fixture["case"])
    if legacy is not None:
        assert {k: body[k] for k in ("exitCode", "stdout", "stderr")} == legacy["body"]


def test_fixtures_hold_the_named_outcomes() -> None:
    by_case = {path.stem: json.loads(path.read_text())["body"] for path in RECORDS}
    assert by_case["exit-23-partial"]["outcome"] == {"kind": "exited", "exitCode": 23}
    assert by_case["binary"]["evidence"]["stdout"]["base64"] == "AP+AQQo="
    assert stream_bytes(by_case["oracle-text"], "stdout") == b"Explain this change."
    assert by_case["signal-kill"]["outcome"]["signalName"] == "SIGKILL"
    assert by_case["spawn-failure"]["outcome"]["errorName"] == "EAGAIN"
    assert by_case["remaining-group"]["remainingGroup"] == "present"
    assert by_case["requested-id"]["id"] == "k217.fixture-1"
    killed = by_case["timeout-child-kill"]["termination"]
    assert killed["leaderStatus"] == {"kind": "signalled", "signal": 9}
    assert killed["groupEmpty"] == "yes"
    assert by_case["deadline"]["termination"]["leaderStatus"]["signal"] == 15
