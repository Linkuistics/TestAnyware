"""Compatibility control: the legacy projection equals the old Linux replies.

Row "Legacy projection" of the required-control table in
docs/specs/guest-command-lifecycle.md. The reference is
`fixtures/legacy_linux_replies.json`. The k5 matrix recorded it from the Linux
agent before k217, through raw HTTP and the checkout Host CLI.

The old Host CLI reads only `exitCode`, `stdout`, `stderr` and `timedOut`. It
ignores other keys, because its `ExecResult` does not deny unknown fields
(cli-rs/crates/testanyware-protocol/src/agent_requests.rs). So equal legacy
fields and an absent `timedOut` give the same text output, the same JSON
envelope and the same exit code that the fixture records for that CLI.
"""

from __future__ import annotations

import http.client
import json
import threading
from collections.abc import Iterator
from http.server import HTTPServer
from pathlib import Path

import pytest

from testanyware_agent import exec_owner
from tests.agent_server import agent_request_handler
from tests.exec_validity import record_errors
from tests.limits import with_one_process_limit

pytestmark = pytest.mark.linux_kernel

FIXTURE = Path(__file__).parent / "fixtures" / "legacy_linux_replies.json"
CASES = json.loads(FIXTURE.read_text())["cases"]
LEGACY_KEYS = ("exitCode", "stdout", "stderr")


@pytest.fixture(autouse=True)
def capture_root(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv(exec_owner.CAPTURE_ROOT_ENV, str(tmp_path / "captures"))


@pytest.fixture(scope="module")
def agent_port() -> Iterator[int]:
    server = HTTPServer(("127.0.0.1", 0), agent_request_handler())
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_address[1]
    finally:
        server.shutdown()
        server.server_close()


def post_exec(port: int, body: dict) -> tuple[int, dict]:
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=30)
    try:
        conn.request("POST", "/exec", json.dumps(body), {"Content-Type": "application/json"})
        response = conn.getresponse()
        return response.status, json.loads(response.read())
    finally:
        conn.close()


@pytest.mark.parametrize("case", CASES, ids=[case["case"] for case in CASES])
def test_legacy_projection_equals_the_recorded_old_reply(case: dict, agent_port: int) -> None:
    body = {"command": case["command"], "timeout": case["timeout"]}
    if case["case"] == "spawn-failure":
        # The HTTP server cannot run under the process limit, so the call goes
        # to the handler and through the JSON encoding of the server.
        status, reply = with_one_process_limit("handler", body)
    else:
        status, reply = post_exec(agent_port, body)

    old = case["body"]
    assert set(old) == set(LEGACY_KEYS)
    assert status == case["httpStatus"]
    assert {key: reply[key] for key in LEGACY_KEYS} == old
    assert "timedOut" not in reply
    assert record_errors(reply) == []


def test_reference_cli_receipts_follow_from_the_legacy_fields() -> None:
    """Check that the fixture's CLI receipts derive from its HTTP bodies alone.

    This is the premise of the control above: equal legacy fields give equal
    CLI output.
    """
    for case in CASES:
        old = case["body"]
        text = case["cliText"]
        assert text["returncode"] == old["exitCode"] % 256, case["case"]
        assert bytes.fromhex(text["stdoutHex"]) == old["stdout"].encode()
        stderr = old["stderr"] + "\n" if old["stderr"] else ""
        assert bytes.fromhex(text["stderrHex"]) == stderr.encode()
        envelope = case["cliJson"]["stdout"]
        assert envelope["exit_code"] == old["exitCode"]
        assert (envelope["stdout"], envelope["stderr"]) == (old["stdout"], old["stderr"])
        assert envelope["timed_out"] is False
