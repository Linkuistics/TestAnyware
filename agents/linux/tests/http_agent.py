"""Reach a Linux agent over raw HTTP, in this process or as its own process.

The responsiveness, cancellation, shutdown and download controls use the
transport seam: real sockets and the real request handler. `AgentProcess`
starts `python3 -m testanyware_agent` through its `main` entry, so a control
that uses it runs unchanged against an older agent checkout. `raw_exchange`
reads every byte until the agent closes the connection, so it sees bytes past
the declared `Content-Length`.
"""

from __future__ import annotations

import http.client
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any

AGENT_ROOT = Path(__file__).resolve().parent.parent

# The launcher installs the AT-SPI stand-ins of tests/agent_server.py, applies
# an optional seam module, and then runs the unchanged entry point.
LAUNCHER = """
import importlib, sys
sys.path.insert(0, sys.argv[1])
from tests.agent_server import agent_request_handler
agent_request_handler()
if sys.argv[3]:
    importlib.import_module(sys.argv[3])
from testanyware_agent.__main__ import main
sys.argv = ["testanyware_agent", "--port", sys.argv[2]]
main()
"""


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


@dataclass
class RawReply:
    status: int
    headers: dict[str, str]
    body: bytes  # the bytes up to the declared length
    extra: bytes  # the bytes after the declared length, before the close
    declared: int | None


def raw_exchange(port: int, request: bytes, timeout: float = 10.0) -> RawReply:
    """Send one request and read until the agent closes the connection."""
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as s:
        s.sendall(request)
        chunks = []
        while chunk := s.recv(65536):
            chunks.append(chunk)
    data = b"".join(chunks)
    head, _, rest = data.partition(b"\r\n\r\n")
    lines = head.decode("latin-1").split("\r\n")
    status = int(lines[0].split()[1])
    headers = {}
    for line in lines[1:]:
        name, _, value = line.partition(":")
        headers[name.strip().lower()] = value.strip()
    declared = int(headers["content-length"]) if "content-length" in headers else None
    if declared is None:
        return RawReply(status, headers, rest, b"", None)
    return RawReply(status, headers, rest[:declared], rest[declared:], declared)


def post_request(path: str, body: bytes = b"") -> bytes:
    return (
        f"POST {path} HTTP/1.1\r\nHost: agent\r\nContent-Length: {len(body)}\r\n"
        "Connection: close\r\n\r\n"
    ).encode() + body


def post_json(port: int, path: str, body: dict[str, Any], timeout: float = 30.0) -> Any:
    """POST a JSON body and return `(status, decoded reply)`."""
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    try:
        payload = json.dumps(body).encode()
        conn.request("POST", path, body=payload, headers={"Content-Type": "application/json"})
        response = conn.getresponse()
        return response.status, json.loads(response.read())
    finally:
        conn.close()


def get_health(port: int, timeout: float) -> tuple[int, Any]:
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    try:
        conn.request("GET", "/health")
        response = conn.getresponse()
        return response.status, json.loads(response.read())
    finally:
        conn.close()


def timed(call: Any, *args: Any, **kwargs: Any) -> tuple[float, Any]:
    started = time.monotonic()
    result = call(*args, **kwargs)
    return time.monotonic() - started, result


class AgentProcess:
    """One agent process on a free port, with its capture root in `captures`."""

    def __init__(self, captures: Path, seam_module: str = "") -> None:
        self.port = free_port()
        env = {
            **os.environ,
            "TESTANYWARE_AGENT_CAPTURE_ROOT": str(captures),
            "PYTHONDONTWRITEBYTECODE": "1",
        }
        self.process = subprocess.Popen(
            [sys.executable, "-c", LAUNCHER, str(AGENT_ROOT), str(self.port), seam_module],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self._wait_listening(10.0)

    def _wait_listening(self, limit: float) -> None:
        end = time.monotonic() + limit
        while time.monotonic() < end:
            if self.process.poll() is not None:
                raise RuntimeError(f"the agent exited early: {self.output()}")
            try:
                socket.create_connection(("127.0.0.1", self.port), timeout=0.2).close()
                return
            except OSError:
                time.sleep(0.02)
        raise RuntimeError("the agent did not listen within its start limit")

    def output(self) -> str:
        if self.process.poll() is None:
            return ""
        out, err = self.process.communicate()
        return (out + err).decode(errors="replace")

    def terminate_and_time(self, limit: float) -> float | None:
        """Send SIGTERM and return the seconds to the agent exit, or None."""
        started = time.monotonic()
        self.process.send_signal(signal.SIGTERM)
        try:
            self.process.wait(limit)
        except subprocess.TimeoutExpired:
            return None
        return time.monotonic() - started

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(10)
        for stream in (self.process.stdout, self.process.stderr):
            if stream is not None:
                stream.close()


@contextmanager
def in_process_agent() -> Iterator[int]:
    """Serve the production server class on a free loopback port in this process."""
    from tests.agent_server import agent_request_handler

    agent_request_handler()  # installs the AT-SPI stand-ins before the import below
    from testanyware_agent import server

    agent = server.make_server("127.0.0.1", 0)
    thread = threading.Thread(target=agent.serve_forever, daemon=True)
    thread.start()
    try:
        yield int(agent.server_address[1])
    finally:
        agent.shutdown()
        agent.server_close()
        thread.join(5)
