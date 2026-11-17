"""Run one exec in a fresh interpreter whose `RLIMIT_NPROC` is 1.

The k5 matrix produced the old spawn failure this way. The limit makes the
next fork fail with EAGAIN, because the user already runs more than one
process. The limit does not bind uid 0. A fresh interpreter starts with one
thread, so its fork is safe even when the test process runs server threads.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

AGENT_ROOT = Path(__file__).resolve().parent.parent

CHILD = """
import json, resource, sys, time
_, hard = resource.getrlimit(resource.RLIMIT_NPROC)
resource.setrlimit(resource.RLIMIT_NPROC, (1, hard))
entry, body = sys.argv[1], json.load(sys.stdin)
if entry == "owner":
    from testanyware_agent import exec_owner
    reply = exec_owner.run_command(
        body["command"], body["timeout"], "ctl-1", time.monotonic()
    ).to_dict()
else:
    from testanyware_agent import system_endpoints
    reply = list(system_endpoints.handle_exec(body))
sys.stdout.write(json.dumps(reply, ensure_ascii=False))
"""


def with_one_process_limit(entry: str, body: dict) -> Any:
    """Return the owner record (`entry="owner"`) or the handler's
    `[status, reply]` (`entry="handler"`), computed under the limit."""
    if os.getuid() == 0:
        raise RuntimeError("RLIMIT_NPROC does not bind uid 0; run the controls as a normal user")
    path = os.pathsep.join(filter(None, [str(AGENT_ROOT), os.environ.get("PYTHONPATH")]))
    result = subprocess.run(
        [sys.executable, "-c", CHILD, entry],
        input=json.dumps(body),
        capture_output=True,
        text=True,
        timeout=30,
        env={**os.environ, "PYTHONPATH": path},
        check=True,
    )
    return json.loads(result.stdout)
