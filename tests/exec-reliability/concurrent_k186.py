"""One round of concurrent exact guest requests (leaf macos-completion-live-k186).

    concurrent_k186.py COUNT BOUND CLI VM

The script starts COUNT copies of
`CLI file exec --vm VM --timeout 30 --json "printf ordinary-ok"` in one tight loop.
Each copy runs in its own process session. Each copy has its own host bound of BOUND
seconds from its own start. At that bound the script sends TERM to the copy's group,
waits at most 2 s, sends KILL and waits at most 2 s more for the reap.

A reap does not end the capture. A descendant of the copy can keep a pipe open, and
the bytes read up to the reap can then be a prefix of the output. After the last reap,
the script reads each open pipe for at most 1 s more, until EOF. It records for each
stream whether it saw EOF. It sends no signal after a reap, because the group of a
reaped leader can be empty and its number reusable.

The script prints one JSON object on stdout. It keeps the raw stdout and stderr of
every copy as hex, `stdout_eof` and `stderr_eof`, its return code or signal, and its
monotonic start and end. The script does not interpret the guest receipt. A later
checker reads it. The script exits 0 only when every copy exited 0 and both of its
streams reached EOF.
"""

import json
import os
import signal
import subprocess
import sys
import time
from typing import IO

COMMAND = "printf ordinary-ok"
GUEST_TIMEOUT = "30"
TERM_GRACE = 2.0
KILL_WAIT = 2.0
CAPTURE_GRACE = 1.0
POLL = 0.005


def _drain(stream: IO[bytes] | None) -> tuple[bytes, bool]:
    """Read what the pipe holds now, and say whether the read reached EOF.

    An empty pipe that a writer still holds raises BlockingIOError: that is not EOF.
    Only an empty read is EOF. A descendant that keeps the pipe cannot block us.
    """
    assert stream is not None
    fd = stream.fileno()
    os.set_blocking(fd, False)
    chunks: list[bytes] = []
    while True:
        try:
            chunk = os.read(fd, 65536)
        except BlockingIOError:
            return b"".join(chunks), False
        if not chunk:
            return b"".join(chunks), True
        chunks.append(chunk)


def _signal(pgid: int, number: signal.Signals) -> str:
    try:
        os.killpg(pgid, number)
    except OSError as error:
        return type(error).__name__
    return "sent"


def main() -> int:
    count, bound, cli, vm = int(sys.argv[1]), float(sys.argv[2]), sys.argv[3], sys.argv[4]
    argv = [cli, "file", "exec", "--vm", vm, "--timeout", GUEST_TIMEOUT, "--json", COMMAND]
    children: list[dict[str, object]] = []
    procs: list[subprocess.Popen[bytes]] = []
    sent_by_child: list[list[list[object]]] = []
    for index in range(count):
        started = time.monotonic()
        proc = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        procs.append(proc)
        sent_by_child.append([])
        children.append(
            {
                "index": index,
                "argv": argv,
                "pid": proc.pid,
                "started_monotonic": started,
                "wall_start_ns": time.time_ns(),
                "signals": sent_by_child[index],
                "status": None,
                "ended_monotonic": None,
                "reason": None,
            }
        )
    out: list[bytes] = [b""] * count
    err: list[bytes] = [b""] * count
    out_eof = [False] * count
    err_eof = [False] * count

    def drain(index: int) -> None:
        proc = procs[index]
        if not out_eof[index]:
            chunk, out_eof[index] = _drain(proc.stdout)
            out[index] += chunk
        if not err_eof[index]:
            chunk, err_eof[index] = _drain(proc.stderr)
            err[index] += chunk

    while any(child["status"] is None for child in children):
        now = time.monotonic()
        for index, child in enumerate(children):
            if child["status"] is not None:
                continue
            proc = procs[index]
            drain(index)
            pid, status = os.waitpid(proc.pid, os.WNOHANG)
            if pid == proc.pid:
                child["status"] = status
                child["ended_monotonic"] = time.monotonic()
                child["reason"] = child["reason"] or "exit"
                continue
            elapsed = now - float(child["started_monotonic"])  # type: ignore[arg-type]
            sent = sent_by_child[index]
            if elapsed >= bound and not sent:
                sent.append(["TERM", now, _signal(proc.pid, signal.SIGTERM)])
                child["reason"] = "deadline"
            elif elapsed >= bound + TERM_GRACE and len(sent) == 1:
                sent.append(["KILL", now, _signal(proc.pid, signal.SIGKILL)])
            elif elapsed >= bound + TERM_GRACE + KILL_WAIT:
                child["status"] = "unreaped"
                child["ended_monotonic"] = now
        time.sleep(POLL)
    capture_deadline = time.monotonic() + CAPTURE_GRACE
    while True:
        for index in range(count):
            drain(index)
        if all(out_eof) and all(err_eof) or time.monotonic() >= capture_deadline:
            break
        time.sleep(POLL)
    for index, child in enumerate(children):
        proc = procs[index]
        for stream in (proc.stdout, proc.stderr):
            assert stream is not None
            stream.close()
        child["stdout_eof"] = out_eof[index]
        child["stderr_eof"] = err_eof[index]
        status = child["status"]
        if isinstance(status, int):
            child["returncode"] = os.waitstatus_to_exitcode(status)
        child["elapsed"] = float(child["ended_monotonic"]) - float(child["started_monotonic"])  # type: ignore[arg-type]
        child["stdout_hex"] = out[index].hex()
        child["stderr_hex"] = err[index].hex()
    print(json.dumps({"count": count, "bound": bound, "children": children}, indent=2))
    healthy = all(child.get("returncode") == 0 for child in children)
    healthy = healthy and all(out_eof) and all(err_eof)
    return 0 if healthy else 1


if __name__ == "__main__":
    sys.exit(main())
