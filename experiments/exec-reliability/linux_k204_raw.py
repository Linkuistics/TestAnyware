"""Send one exec request straight to the Linux agent of one VM (leaf linux-timeout-live-k204).

    linux_k204_raw.py VM GUEST_TIMEOUT COMMAND

The script reads the agent address from the VM spec that `testanyware vm start` wrote.
It posts `{"command": COMMAND, "timeout": GUEST_TIMEOUT}` to `/exec` once, without the
Host CLI. The HTTP timeout is the guest timeout plus 10 s, the buffer of the Host CLI.
It prints one JSON record: the request, the monotonic times, the HTTP status, the raw
body bytes as hex, the parsed body and any transport error.

The platform binary `/usr/bin/curl` makes the connection. macOS Local Network privacy
refuses a connection to the guest network from this interpreter with errno 65. A
platform binary connects (docs/research/macos-local-network-platform-binary-exemption.md).
Attempt 01 of k204 recorded that refusal for the earlier urllib transport.

The script exits 0 whenever it prints the record, also for a transport error. The
checker judges the record, so the controller never stops the plan at a raw call. The
script exits 2 for a bad argument list.
"""

import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

HTTP_BUFFER = 10  # the Host CLI adds 10 s to the guest timeout for its HTTP layer
CURL = "/usr/bin/curl"
STATUS_MARK = "k204-http-status="


def spec_path(vm: str) -> Path:
    """Return the VM spec path that the Host CLI writes, under the XDG state directory."""
    state = os.environ.get("XDG_STATE_HOME") or str(Path.home() / ".local" / "state")
    return Path(state) / "testanyware" / "vms" / f"{vm}.json"


def exchange(vm: str, guest_timeout: int, command: str) -> dict[str, Any]:
    path = spec_path(vm)
    request_body = {"command": command, "timeout": guest_timeout}
    record: dict[str, Any] = {
        "schema": "k204-raw/2",
        "transport": CURL,
        "vm": vm,
        "spec_path": str(path),
        "request": request_body,
        "http_timeout": guest_timeout + HTTP_BUFFER,
        "http_status": None,
        "body_hex": None,
        "body": None,
        "error": None,
    }
    try:
        spec_bytes = path.read_bytes()
        record["spec_sha256"] = hashlib.sha256(spec_bytes).hexdigest()
        agent = json.loads(spec_bytes)["agent"]
        url = f"http://{agent['host']}:{int(agent['port'])}/exec"
    except (OSError, ValueError, KeyError, TypeError) as error:
        record["error"] = f"spec: {type(error).__name__}: {error}"
        return record
    record["url"] = url
    data = json.dumps(request_body).encode()
    limit = guest_timeout + HTTP_BUFFER
    argv = [CURL, "-sS", "--max-time", str(limit), "-H", "Content-Type: application/json"]
    argv += ["--data-binary", "@-", "-o", "-", "-w", f"%{{stderr}}{STATUS_MARK}%{{http_code}}\\n"]
    record["started_monotonic_ns"] = time.monotonic_ns()
    raw: bytes | None = None
    try:
        done = subprocess.run([*argv, url], input=data, capture_output=True, timeout=limit + 5)
    except (OSError, subprocess.SubprocessError) as error:
        record["error"] = f"transport: {type(error).__name__}: {error}"
    else:
        stderr = done.stderr.decode(errors="replace")
        record["curl_exit"] = done.returncode
        record["curl_stderr"] = stderr
        marks = [line for line in stderr.splitlines() if line.startswith(STATUS_MARK)]
        status = marks[-1].removeprefix(STATUS_MARK) if marks else ""
        if status.isdigit() and int(status) > 0:
            record["http_status"] = int(status)
            raw = done.stdout
        if done.returncode != 0:
            record["error"] = f"transport: curl exit {done.returncode}: {stderr.strip()}"
    record["ended_monotonic_ns"] = time.monotonic_ns()
    record["seconds"] = (record["ended_monotonic_ns"] - record["started_monotonic_ns"]) / 1e9
    if raw is not None:
        record["body_hex"] = raw.hex()
        try:
            record["body"] = json.loads(raw)
        except ValueError as error:
            record["error"] = f"body: {type(error).__name__}: {error}"
    return record


def main() -> int:
    if len(sys.argv) != 4 or not sys.argv[2].isdigit():
        print(__doc__, file=sys.stderr)
        return 2
    record = exchange(sys.argv[1], int(sys.argv[2]), sys.argv[3])
    print(json.dumps(record, indent=2, sort_keys=True), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
