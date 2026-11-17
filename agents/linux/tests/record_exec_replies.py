"""Record the exec replies of the Linux agent as fixtures for the Host CLI.

Run it through `task agent:linux:record-exec-replies`. The task runs it in the
pinned Linux container, so the replies come from a real Linux kernel. Each
fixture holds the request, the HTTP status and the reply body that the agent
sent through its HTTP server. Leaf k218 reads these files in the Host CLI
tests. `test_exec_fixtures.py` checks each file against the validity rules.

Times and process identities differ from run to run. A reader compares the
outcome, the evidence and the legacy fields, and checks the rest by rule.
"""

from __future__ import annotations

import http.client
import json
import os
import sys
import tempfile
import threading
from http.server import HTTPServer
from pathlib import Path

from testanyware_agent import exec_owner
from tests.agent_server import agent_request_handler
from tests.exec_validity import record_errors
from tests.limits import with_one_process_limit

LEGACY = Path(__file__).parent / "fixtures" / "legacy_linux_replies.json"
# The legacy cases, with the shell syntax the old run used, plus cases that
# show the record fields the legacy cases do not reach.
EXTRA = [
    {
        "case": "remaining-group",
        "command": "sleep 3 & printf 'leader done\\n'; exit 0",
        "timeout": 10,
    },
    {"case": "requested-id", "command": "printf 'with id'", "timeout": 10, "id": "k217.fixture-1"},
    {"case": "timeout-child-kill", "command": "trap '' TERM; sleep 30 & wait", "timeout": 2},
]


def cases() -> list[dict]:
    legacy = json.loads(LEGACY.read_text())["cases"]
    chosen = [
        {"case": c["case"], "command": c["command"], "timeout": c["timeout"]}
        for c in legacy
        if c["case"] != "issue-fork-wait"
    ]
    # The old command wrote its child PID under a k16 path. Here the PID goes to /dev/null.
    chosen.append(
        {
            "case": "issue-fork-wait",
            "command": "sleep 120 & p=$!; echo $p > /dev/null; wait",
            "timeout": 2,
        }
    )
    return chosen + EXTRA


def post(port: int, body: dict) -> tuple[int, dict]:
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=30)
    try:
        conn.request("POST", "/exec", json.dumps(body), {"Content-Type": "application/json"})
        response = conn.getresponse()
        return response.status, json.loads(response.read())
    finally:
        conn.close()


def main(out_dir: Path) -> int:
    out_dir.mkdir(parents=True, exist_ok=True)
    os.environ[exec_owner.CAPTURE_ROOT_ENV] = tempfile.mkdtemp(prefix="record-exec-")
    server = HTTPServer(("127.0.0.1", 0), agent_request_handler())
    threading.Thread(target=server.serve_forever, daemon=True).start()
    failures = 0
    try:
        for case in cases():
            request = {k: case[k] for k in ("command", "timeout", "id") if k in case}
            if case["case"] == "spawn-failure":
                status, reply = with_one_process_limit("handler", request)
            else:
                status, reply = post(server.server_address[1], request)
            errors = record_errors(reply, exec_id=request.get("id"))
            if errors:
                failures += 1
                print(f"{case['case']}: invalid record: {errors}", file=sys.stderr)
            fixture = {
                "case": case["case"],
                "source": "agents/linux/tests/record_exec_replies.py (k217)",
                "request": request,
                "httpStatus": status,
                "body": reply,
            }
            path = out_dir / f"{case['case']}.json"
            path.write_text(json.dumps(fixture, indent=2, ensure_ascii=False) + "\n")
            print(f"{case['case']}: {reply['outcome']}")
    finally:
        server.shutdown()
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main(Path(sys.argv[1])))
