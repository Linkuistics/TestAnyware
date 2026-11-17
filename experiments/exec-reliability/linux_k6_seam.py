"""Run the k6 Linux disconnect cases in one container and print the raw evidence.

Usage:
  linux_k6_seam.py RUN cases
  linux_k6_seam.py RUN agent PORT serial|threading

RUN is the run tag: 12 decimal digits that the runner derives from its run ID.
`cases` runs every case and prints one JSON object on stdout. Bytes are hex.
The script judges nothing. `linux_k6_check.py` judges the object. `agent` is
the agent launcher that `cases` starts once for each case.

The script runs inside one fresh container. The container mounts the unchanged
agent at /agent, the k5 desktop stubs at /stub and this directory at /seam.
Client, agent and process table share one Linux kernel and one loopback.

Each case starts a fresh agent. The client sends the issue #14 fork/wait shape
with a guest timeout of 20 s on a raw socket. At 2 s the case acts on the
original connection. Then it sends health, an ordinary exec and a download of
the partial file, one after the other, each on its own connection.

- `serial-control` reads the original response, then sends the follow-ups.
- `serial-concurrent` keeps the original connection open and reading.
- `serial-close` closes the original connection.
- `serial-reset` resets the original connection.
- `serial-half-close` shuts down the write half and keeps reading.
- `host-deadline` keeps reading, and first sends a health request with a 5 s
  client deadline. When the deadline passes, it closes that connection.
- `threaded-reset` resets the original connection. Its agent serves the
  unchanged `AgentRequestHandler` from `ThreadingHTTPServer`. It is the
  identified experimental control `experimental: threading-server`.

Two watchers sample every 50 ms. Each recorded change keeps the times of its
sample and of the sample before it. The process watcher records the payload shell
and its `sleep` child. The handler kills the shell at the deadline, so the exit
of the shell marks the end of the original handler. The socket watcher records
each row of /proc/net/tcp that has the agent port. On the listening row,
`rx_queue` is the accept-queue length. On an agent connection row, `rx_queue` is
the count of request bytes that the agent has not read.

The sleep of each case is `120.<digit>` seconds followed by the run tag. After
each case the script kills the shell and the sleep of the token with a pidfd,
after it reads the start ticks again. It then stops the agent.
"""

from __future__ import annotations

import importlib
import json
import os
import platform
import re
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from linux_k5_seam import kill_here, proc_rows

GUEST_TIMEOUT = 20  # the issue #14 guest timeout
ACT_AFTER = 2.0  # the case acts on the original connection at this time
FOLLOW_BOUND = 30.0  # the ordinary bound of one follow-up request
TRANSFER_BOUND = 150.0  # the bound of the partial-file download
HOST_DEADLINE = 5.0  # the client deadline of the `host-deadline` health request
ORDINARY_TIMEOUT = 10  # the guest timeout of the ordinary follow-up exec
ORIGINAL_BOUND = GUEST_TIMEOUT + 20.0  # the raw host exec ceiling
OBSERVE_BOUND = 35.0  # deadline 20 s + terminate wait 3 s + 12 s of margin
READY_BOUND = 10.0  # a fresh agent must accept a connection within this
STOP_WAIT = 2.0  # the TERM wait, and then the KILL wait, of one agent
KILL_WAIT = 2.0  # owned processes must be gone within this after SIGKILL
POLL = 0.05
FIRST_PORT = 18648
ORDINARY = "printf ordinary-ok"
PARTIAL = b"partial\n"
RUN_TAG = re.compile(r"^[0-9]{12}$")
LISTEN = "0A"


@dataclass(frozen=True)
class Case:
    name: str
    server: str  # "serial" or "threading"
    action: str  # "none", "keep", "close", "reset", "half-close" or "deadline"


CASES = (
    Case("serial-control", "serial", "none"),
    Case("serial-concurrent", "serial", "keep"),
    Case("serial-close", "serial", "close"),
    Case("serial-reset", "serial", "reset"),
    Case("serial-half-close", "serial", "half-close"),
    Case("host-deadline", "serial", "deadline"),
    Case("threaded-reset", "threading", "reset"),
)


def now_ns() -> int:
    return time.monotonic_ns()


def token(index: int, tag: str) -> str:
    return f"120.{index + 1}{tag}"


def payload(sleep: str, directory: str) -> str:
    """The fork/wait shape of issue #14, as in `disconnect_k13_burst.payload`."""
    return "\n".join(
        (
            f"mkdir -p {directory}",
            f"sleep {sleep} &",
            "child=$!",
            f"printf '%s\\n' \"$child\" > {directory}/child.pid",
            f"printf 'partial\\n' > {directory}/partial",
            "wait",
        )
    )


def http_request(method: str, target: str, port: int, body: bytes = b"") -> bytes:
    head = f"{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n"
    if method == "POST":
        head += f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
    return head.encode() + b"\r\n" + body


def exec_request(port: int, command: str, timeout: int) -> bytes:
    body = json.dumps({"command": command, "timeout": timeout}).encode()
    return http_request("POST", "/exec", port, body)


# ---- watchers ----


def tcp_rows(port: int) -> list[tuple[str, str, str, int, int]]:
    """The /proc/net/tcp rows with `port` at either end: local, remote, state, tx, rx."""
    wanted = f"{port:04X}"
    rows: list[tuple[str, str, str, int, int]] = []
    for line in Path("/proc/net/tcp").read_text().splitlines()[1:]:
        fields = line.split()
        local, remote, state, queues = fields[1], fields[2], fields[3], fields[4]
        if local.endswith(":" + wanted) or remote.endswith(":" + wanted):
            tx, rx = queues.split(":")
            rows.append((local, remote, state, int(tx, 16), int(rx, 16)))
    return sorted(rows)


class Watch:
    """Sample one source every POLL seconds and keep each change with its time."""

    def __init__(self, sample: Any) -> None:
        self.sample = sample
        self.changes: list[dict[str, Any]] = []
        self.stopped = threading.Event()
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def _run(self) -> None:
        """Each change keeps `prev_ns`, the time of the last sample before it.

        The change happened in the interval from `prev_ns` to `ns`.
        """
        last: Any = None
        prev_ns: int | None = None
        while not self.stopped.is_set():
            try:
                value = self.sample()
            except OSError as error:
                value = {"error": str(error)}
            ns = now_ns()
            if value != last:
                self.changes.append({"ns": ns, "prev_ns": prev_ns, "value": value})
                last = value
            prev_ns = ns
            self.stopped.wait(POLL)

    def stop(self) -> list[dict[str, Any]]:
        self.stopped.set()
        self.thread.join(timeout=2 * POLL + 1.0)
        return self.changes


def payload_rows(sleep: str) -> list[dict[str, Any]]:
    """The payload shell and its sleep child: role, pid, state, ppid and start ticks."""
    found: list[dict[str, Any]] = []
    for row in proc_rows():
        argv = row["argv"]
        if argv == ["sleep", sleep]:
            role = "child"
        elif argv[:2] == ["/bin/bash", "-c"] and f"sleep {sleep} &" in " ".join(argv):
            role = "shell"
        else:
            continue
        found.append(
            {
                "role": role,
                "pid": row["pid"],
                "state": row["state"],
                "ppid": row["ppid"],
                "pgid": row["pgid"],
                "start_ticks": row["start_ticks"],
            }
        )
    return found


# ---- one connection ----


class Exchange:
    """One request on its own socket, read to EOF or to its deadline."""

    def __init__(self, name: str, port: int, request: bytes, bound: float) -> None:
        self.name = name
        self.port = port
        self.request = request
        self.bound = bound
        self.record: dict[str, Any] = {"name": name, "bound": bound}
        self.sock: socket.socket | None = None
        self.done = threading.Event()

    def open(self) -> None:
        self.record["start_ns"] = now_ns()
        sock = socket.create_connection(("127.0.0.1", self.port), timeout=self.bound)
        self.sock = sock
        self.record["connected_ns"] = now_ns()
        self.record["local_port"] = sock.getsockname()[1]
        sock.sendall(self.request)
        self.record["sent_ns"] = now_ns()

    def read(self) -> None:
        """Read until EOF, an error or the deadline. The record keeps every outcome."""
        assert self.sock is not None
        received = bytearray()
        deadline = self.record["start_ns"] + int(self.bound * 1e9)
        try:
            while True:
                left = (deadline - now_ns()) / 1e9
                if left <= 0:
                    self.record["bound_exceeded"] = True
                    break
                self.sock.settimeout(left)
                try:
                    chunk = self.sock.recv(65536)
                except TimeoutError:
                    self.record["bound_exceeded"] = True
                    break
                if not chunk:
                    break
                if not received:
                    self.record["first_byte_ns"] = now_ns()
                received.extend(chunk)
        except OSError as error:
            self.record["error"] = f"{type(error).__name__}: {error}"
        finally:
            self.record["end_ns"] = now_ns()
            self.record["response_hex"] = bytes(received).hex()
            self.done.set()

    def run(self) -> dict[str, Any]:
        try:
            self.open()
        except OSError as error:
            self.record["error"] = f"{type(error).__name__}: {error}"
            self.record["end_ns"] = now_ns()
            self.done.set()
            return self.record
        self.read()
        self.close()
        return self.record

    def close(self) -> None:
        if self.sock is not None:
            self.sock.close()


def act(original: Exchange, action: str) -> dict[str, Any]:
    """Apply the case action to the original connection, and record its time."""
    assert original.sock is not None
    record: dict[str, Any] = {"action": action, "ns": now_ns()}
    if action == "close":
        original.sock.close()
    elif action == "reset":
        original.sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        original.sock.close()
    elif action == "half-close":
        original.sock.shutdown(socket.SHUT_WR)
    record["done_ns"] = now_ns()
    return record


# ---- agents ----


def agent(port: int, kind: str) -> None:
    """Serve the unchanged agent handler. `threading` is the experimental control."""
    sys.path[:0] = ["/stub", "/agent"]
    server: Any = importlib.import_module("testanyware_agent.server")

    if kind == "serial":
        server.run_server(port)  # the unchanged entry: a plain HTTPServer
        return
    from http.server import ThreadingHTTPServer

    ThreadingHTTPServer(("0.0.0.0", port), server.AgentRequestHandler).serve_forever()


def start_agent(tag: str, port: int, kind: str, logs: Path) -> tuple[subprocess.Popen[bytes], Any]:
    argv = [sys.executable, "-E", "-s", "-B", __file__, tag, "agent", str(port), kind]
    out = (logs / f"agent-{port}.out").open("wb")
    child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=out, stderr=out)
    return child, out


def wait_ready(port: int) -> dict[str, Any]:
    start = now_ns()
    deadline = start + int(READY_BOUND * 1e9)
    while now_ns() < deadline:
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1.0).close()
            return {"ready": True, "seconds": (now_ns() - start) / 1e9}
        except OSError:
            time.sleep(POLL)
    return {"ready": False, "seconds": (now_ns() - start) / 1e9}


def stop_agent(child: subprocess.Popen[bytes]) -> list[dict[str, Any]]:
    steps: list[dict[str, Any]] = []
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if child.poll() is not None:
            break
        child.send_signal(sig)
        try:
            child.wait(timeout=STOP_WAIT)
        except subprocess.TimeoutExpired:
            steps.append({"signal": sig.name, "exited": False})
            continue
        steps.append({"signal": sig.name, "exited": True})
    steps.append({"returncode": child.poll()})
    return steps


# ---- one case ----


def wait_until(mark_ns: int) -> None:
    left = (mark_ns - now_ns()) / 1e9
    if left > 0:
        time.sleep(left)


def follow_ups(case: Case, port: int, directory: str) -> list[dict[str, Any]]:
    steps: list[tuple[str, bytes, float]] = []
    if case.action == "deadline":
        steps.append(("health-deadline", http_request("GET", "/health", port), HOST_DEADLINE))
    steps += [
        ("health", http_request("GET", "/health", port), FOLLOW_BOUND),
        ("ordinary", exec_request(port, ORDINARY, ORDINARY_TIMEOUT), FOLLOW_BOUND),
        (
            "download",
            http_request("POST", f"/download?path={directory}/partial", port),
            TRANSFER_BOUND,
        ),
    ]
    return [Exchange(name, port, request, bound).run() for name, request, bound in steps]


def handler_gone(watch: Watch) -> bool:
    """True once the shell was seen and the latest sample no longer holds it."""
    seen = any(
        any(row.get("role") == "shell" for row in change["value"]) for change in watch.changes
    )
    latest: list[Any] = watch.changes[-1]["value"] if watch.changes else []
    return seen and not any(row.get("role") == "shell" for row in latest)


def cleanup(sleep: str) -> dict[str, Any]:
    """Kill the shell and the sleep of this token, then confirm that both are gone."""
    targets = [(row["pid"], row["start_ticks"]) for row in payload_rows(sleep)]
    kills = kill_here(targets)
    deadline = now_ns() + int(KILL_WAIT * 1e9)
    left = payload_rows(sleep)
    while left and now_ns() < deadline:
        time.sleep(POLL)
        left = payload_rows(sleep)
    return {"kills": kills, "left": left}


def run_case(index: int, case: Case, tag: str, root: Path) -> dict[str, Any]:
    port = FIRST_PORT + index
    sleep = token(index, tag)
    directory = str(root / case.name)
    record: dict[str, Any] = {"case": case.name, "server": case.server, "port": port}
    record["token"] = sleep
    child, log = start_agent(tag, port, case.server, root)
    try:
        record["ready"] = wait_ready(port)
        if not record["ready"]["ready"]:
            return record
        processes = Watch(lambda: payload_rows(sleep))
        sockets = Watch(lambda: tcp_rows(port))
        request = exec_request(port, payload(sleep, directory), GUEST_TIMEOUT)
        original = Exchange("original", port, request, ORIGINAL_BOUND)
        original.open()
        start = original.record["start_ns"]
        record["start_ns"] = start
        reader: threading.Thread | None = None
        if case.action not in ("close", "reset"):
            reader = threading.Thread(target=original.read, daemon=True)
            reader.start()
        wait_until(start + int(ACT_AFTER * 1e9))
        if case.action == "none":
            original.done.wait(ORIGINAL_BOUND + 1.0)
        else:
            record["act"] = act(original, case.action)
        record["follow"] = follow_ups(case, port, directory)
        deadline = start + int(OBSERVE_BOUND * 1e9)
        while not handler_gone(processes) and now_ns() < deadline:
            time.sleep(POLL)
        if reader is not None:
            reader.join(timeout=max(0.0, (deadline - now_ns()) / 1e9))
        original.close()
        record["original"] = original.record
        record["after_handler"] = {"ns": now_ns(), "rows": payload_rows(sleep)}
        record["evidence"] = evidence(directory)
        record["cleanup"] = cleanup(sleep)
        record["processes"] = processes.stop()
        record["sockets"] = sockets.stop()
    finally:
        record["agent_stop"] = stop_agent(child)
        log.close()
        record["agent_log_hex"] = (root / f"agent-{port}.out").read_bytes().hex()
    return record


def evidence(directory: str) -> dict[str, Any]:
    """The partial file and the child PID file, read on this kernel."""
    found: dict[str, Any] = {}
    for name in ("partial", "child.pid"):
        try:
            found[name] = Path(directory, name).read_bytes().hex()
        except OSError as error:
            found[name] = f"unreadable: {error}"
    return found


def owned_by_run(argv: list[str], tag: str) -> bool:
    """True for a payload sleep, a payload shell or an agent launcher of this run.

    The container init and this script also name the tag, but only as their
    own arguments. Neither is a payload or an agent, so neither matches.
    """
    if (
        len(argv) == 2
        and argv[0] == "sleep"
        and argv[1].startswith("120.")
        and argv[1].endswith(tag)
    ):
        return True
    if argv[:2] == ["/bin/bash", "-c"] and len(argv) >= 3 and f"{tag} &" in argv[2]:
        return True
    return tag in argv and argv[argv.index(tag) + 1 :][:1] == ["agent"]


def final_sweep(tag: str) -> dict[str, Any]:
    """Every process that still names the run tag, and the kill of each."""
    rows = [row for row in proc_rows() if owned_by_run(row["argv"], tag)]
    kills = kill_here([(row["pid"], row["start_ticks"]) for row in rows])
    return {"rows": rows, "kills": kills}


def bounds() -> dict[str, Any]:
    return {
        "guest_timeout": GUEST_TIMEOUT,
        "act_after": ACT_AFTER,
        "follow_bound": FOLLOW_BOUND,
        "transfer_bound": TRANSFER_BOUND,
        "host_deadline": HOST_DEADLINE,
        "ordinary_timeout": ORDINARY_TIMEOUT,
        "original_bound": ORIGINAL_BOUND,
        "observe_bound": OBSERVE_BOUND,
        "ready_bound": READY_BOUND,
        "stop_wait": STOP_WAIT,
        "kill_wait": KILL_WAIT,
        "poll": POLL,
    }


def cases(tag: str) -> dict[str, Any]:
    result: dict[str, Any] = {
        "tag": tag,
        "python": sys.version,
        "kernel": platform.release(),
        "uid": os.getuid(),
        "pid": os.getpid(),
        "bounds": bounds(),
        "cases": [],
    }
    root = Path(tempfile.mkdtemp(prefix="k6-"))
    try:
        for index, case in enumerate(CASES):
            result["cases"].append(run_case(index, case, tag, root))
    finally:
        result["final_sweep"] = final_sweep(tag)
    return result


def main(argv: list[str]) -> int:
    if len(argv) >= 3 and RUN_TAG.fullmatch(argv[1]) and argv[2] == "agent" and len(argv) == 5:
        agent(int(argv[3]), argv[4])
        return 0
    if len(argv) == 3 and RUN_TAG.fullmatch(argv[1]) and argv[2] == "cases":
        print(json.dumps(cases(argv[1])), flush=True)
        return 0
    print(__doc__, file=sys.stderr)
    return 64


if __name__ == "__main__":
    sys.exit(main(sys.argv))
