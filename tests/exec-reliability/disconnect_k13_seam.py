"""Run the k13 host disconnect cases against a loopback agent and print the raw evidence.

Usage:
  disconnect_k13_seam.py RUN AGENT APP CLI

RUN is the run tag: 12 decimal digits that the runner derives from its run ID.
AGENT is a loopback build of the unchanged agent. APP and CLI are checkout builds.

The script prints one JSON object on stdout. Bytes are hex. The script judges
nothing. `disconnect_k13_check.py` judges the object.

Each case sends the issue #14 fork/wait shape with a guest timeout of 20 s. The
agent runs that shell on this host, so the script sees the shell and its child
in the process table. The shell exits when the handler kills it at the deadline,
so the exit of the shell marks the end of the original handler.

- `http-control` reads the response on the open connection.
- `http-concurrent` also sends a health request and an ordinary exec at 2 s.
- `http-close` closes the connection at 2 s and then sends the two requests.
- `http-reset` resets the connection at 2 s and then sends the two requests.
- `http-half-close` shuts down only the write half at 2 s, sends the two
  requests and keeps reading the original connection.
- `cli-concurrent` sends the command through the CLI and a test-mode app. It
  sends the CLI health and ordinary exec at 2 s without an interruption.
- `cli-term` sends TERM to the command CLI at 2 s, waits at most 2 s, then
  sends KILL and waits at most 2 s. It then sends the two CLI requests.

The sleep of each case is `120.<digit>` seconds followed by the run tag, for
example `sleep 120.3004718293846`. The token is unique to one run, so it names
a process of this run. After each case, the script kills the payload sleep and
the payload shell of the token, after it checks the start time and the command
again. A process that only mentions the token is not killed.
"""

from __future__ import annotations

import http.client
import json
import os
import re
import select
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

from outcomes_k12_seam import (
    Owned,
    free_port,
    now_ns,
    processes,
    tcp_connect,
    unix_connect,
    wait_connect,
    write_spec,
)

GUEST_TIMEOUT = 20  # the issue #14 guest timeout
ACT_AFTER = 2.0  # the interruption, or the concurrent requests, happen at this time
FOLLOW_BOUND = 30.0  # the ordinary bound of one follow-up request
ORDINARY_TIMEOUT = 10  # the guest timeout of the ordinary follow-up exec
TERM_GRACE = 2.0  # the faithful TERM grace of the command CLI
KILL_WAIT = 2.0  # the faithful kill and reap bound of the command CLI
OBSERVE_BOUND = 35.0  # deadline 20 s + terminate wait 3 s + 12 s of margin
ORIGINAL_BOUND = GUEST_TIMEOUT + 20.0  # the raw host exec ceiling
PIDFILE_BOUND = 5.0  # the shell writes the child PID file within this
POLL = 0.05
ORDINARY = "printf ordinary-ok"
RUN_TAG = re.compile(r"^[0-9]{12}$")


@dataclass(frozen=True)
class Case:
    name: str
    digit: int
    path: str  # "http" or "cli"
    action: str  # "none", "concurrent", "close", "reset", "half-close", "term"


CASES = (
    Case("http-control", 1, "http", "none"),
    Case("http-concurrent", 2, "http", "concurrent"),
    Case("http-close", 3, "http", "close"),
    Case("http-reset", 4, "http", "reset"),
    Case("http-half-close", 5, "http", "half-close"),
    Case("cli-concurrent", 6, "cli", "concurrent"),
    Case("cli-term", 7, "cli", "term"),
)


def token(case: Case, tag: str) -> str:
    return f"120.{case.digit}{tag}"


def payload(case: Case, tag: str, directory: Path) -> str:
    """The issue #14 fork/wait shape, with a run token and a private directory."""
    return "\n".join(
        (
            f"mkdir -p {directory}",
            f"sleep {token(case, tag)} & child=$!",
            f"printf '%s\\n' \"$child\" > {directory}/child.pid",
            f"printf 'partial\\n' > {directory}/partial",
            "wait",
        )
    )


# ---- process observation ----


def row(pid: int) -> dict[str, Any] | None:
    """The parent PID, start time and command of one process, or None when it is gone."""
    listing = subprocess.run(
        ["/bin/ps", "-p", str(pid), "-o", "ppid=,lstart=,command="],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    ).stdout.strip()
    parts = listing.split(None, 6)
    if len(parts) < 7 or not parts[0].isdigit():
        return None
    return {"ppid": int(parts[0]), "start": " ".join(parts[1:6]), "command": parts[6]}


class Watch:
    """The first and last sight of one process identity, polled on one clock."""

    def __init__(self, role: str, pid: int, seen: dict[str, Any]) -> None:
        self.role = role
        self.pid = pid
        self.start: str = seen["start"]
        self.command: str = seen["command"]
        self.ppid: int = seen["ppid"]
        self.first_ns = now_ns()
        self.last_seen_ns = self.first_ns
        self.gone_ns: int | None = None
        self.gone_reason: str | None = None

    def poll(self) -> None:
        if self.gone_ns is not None:
            return
        seen = row(self.pid)
        stamp = now_ns()
        if seen is None:
            self.gone_ns, self.gone_reason = stamp, "absent"
        elif (seen["start"], seen["command"]) != (self.start, self.command):
            self.gone_ns, self.gone_reason = stamp, "pid-reused"
        else:
            self.last_seen_ns = stamp

    def record(self) -> dict[str, Any]:
        return {
            "role": self.role,
            "pid": self.pid,
            "ppid": self.ppid,
            "start": self.start,
            "command": self.command,
            "first_ns": self.first_ns,
            "last_seen_ns": self.last_seen_ns,
            "gone_ns": self.gone_ns,
            "gone_reason": self.gone_reason,
        }


def find_identities(directory: Path, deadline_ns: int) -> dict[str, Any]:
    """Read the child PID file, then record the child and its parent shell."""
    pidfile = directory / "child.pid"
    while now_ns() < deadline_ns:
        try:
            text = pidfile.read_text()
        except FileNotFoundError:
            time.sleep(POLL)
            continue
        if text.endswith("\n") and text.strip().isdigit():
            child_pid = int(text)
            child = row(child_pid)
            if child is None:
                return {"error": f"child {child_pid} gone before the first sight"}
            shell = row(child["ppid"])
            if shell is None:
                return {"error": f"shell {child['ppid']} gone before the first sight"}
            return {
                "child": Watch("child", child_pid, child),
                "shell": Watch("shell", child["ppid"], shell),
            }
        time.sleep(POLL)
    return {"error": "no child PID file within the bound"}


def owned(command: str, tokens: tuple[str, ...], work: str | None = None) -> bool:
    """Whether a command is a program that this run started.

    The run starts three shapes: the payload `sleep TOKEN`, the agent's
    `/bin/bash -c` payload shell and a binary inside the private build directory.
    A command that only mentions a token or the directory is not owned, for
    example `grep TOKEN` or `tail -f WORK/log`.
    """
    for value in tokens:
        if command == f"sleep {value}":
            return True
        if command.startswith("/bin/bash -c ") and f"sleep {value} & child=$!" in command:
            return True
    return work is not None and command.startswith(f"{work}/")


def sweep(needle: str) -> list[dict[str, Any]]:
    """Record and kill each owned process of this run's token: its payload shell or sleep."""
    found: list[dict[str, Any]] = []
    for pid, start, command in processes():
        if pid == os.getpid() or not owned(command, (needle,)):
            continue
        seen = row(pid)
        if seen is None or (seen["start"], seen["command"]) != (start, command):
            found.append({"pid": pid, "start": start, "command": command, "killed": False})
            continue
        try:
            os.kill(pid, signal.SIGKILL)
            killed = True
        except ProcessLookupError:
            killed = False
        found.append({"pid": pid, "start": start, "command": command, "killed": killed})
    return found


# ---- the original request ----


class RawExec:
    """One exec request on a socket that the case can close, reset or half-close."""

    def __init__(self, port: int, command: str) -> None:
        body = json.dumps({"command": command, "timeout": GUEST_TIMEOUT}).encode()
        head = (
            "POST /exec HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n"
            f"Content-Length: {len(body)}\r\nConnection: close\r\n\r\n"
        ).encode()
        self.request = head + body
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=5)
        self.chunks: list[tuple[int, str]] = []
        self.eof_ns: int | None = None
        self.error: str | None = None
        self.error_ns: int | None = None
        self.reader: threading.Thread | None = None

    def send(self) -> int:
        self.sock.sendall(self.request)
        return now_ns()

    def read(self, deadline_ns: int) -> None:
        """Read until EOF, an error or the deadline. Each chunk keeps its arrival time."""
        try:
            while now_ns() < deadline_ns:
                left = (deadline_ns - now_ns()) / 1e9
                ready, _, _ = select.select([self.sock], [], [], max(0.0, min(left, 0.5)))
                if not ready:
                    continue
                data = self.sock.recv(65536)
                if not data:
                    self.eof_ns = now_ns()
                    return
                self.chunks.append((now_ns(), data.hex()))
        except OSError as error:
            self.error = f"{type(error).__name__}: {error}"
            self.error_ns = now_ns()

    def start_reader(self, deadline_ns: int) -> None:
        self.reader = threading.Thread(target=self.read, args=(deadline_ns,), daemon=True)
        self.reader.start()

    def record(self) -> dict[str, Any]:
        return {
            "request_hex": self.request.hex(),
            "chunks": [{"ns": ns, "hex": data} for ns, data in self.chunks],
            "eof_ns": self.eof_ns,
            "error": self.error,
            "error_ns": self.error_ns,
        }


class CliExec:
    """One command CLI, its streams drained by threads, its signals recorded."""

    def __init__(self, cli: str, env: dict[str, str], command: str) -> None:
        self.argv = [
            cli,
            "file",
            "exec",
            "--vm",
            "normal",
            "--timeout",
            str(GUEST_TIMEOUT),
            "--json",
            command,
        ]
        self.child = subprocess.Popen(  # noqa: S603 - the checkout CLI
            self.argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            start_new_session=True,
        )
        self.start_ns = now_ns()
        self.out: dict[str, bytes] = {"stdout": b"", "stderr": b""}
        self.eof_ns: dict[str, int | None] = {"stdout": None, "stderr": None}
        self.signals: list[dict[str, Any]] = []
        self.exit_ns: int | None = None
        self.status: int | None = None
        self.threads = [
            threading.Thread(target=self.drain, args=(name,), daemon=True)
            for name in ("stdout", "stderr")
        ]
        for thread in self.threads:
            thread.start()

    def drain(self, name: str) -> None:
        stream = self.child.stdout if name == "stdout" else self.child.stderr
        assert stream is not None
        while True:
            data = os.read(stream.fileno(), 65536)
            if not data:
                self.eof_ns[name] = now_ns()
                return
            self.out[name] += data

    def wait(self, seconds: float) -> bool:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            pid, status = os.waitpid(self.child.pid, os.WNOHANG)
            if pid:
                self.exit_ns = now_ns()
                self.status = status
                self.child.returncode = os.waitstatus_to_exitcode(status)
                return True
            time.sleep(0.005)
        return False

    def signal(self, sig: signal.Signals) -> None:
        try:
            os.kill(self.child.pid, sig)  # only the CLI PID, never its group
            result = "sent"
        except ProcessLookupError:
            result = "ProcessLookupError"
        self.signals.append({"signal": sig.name, "ns": now_ns(), "result": result})

    def interrupt(self) -> None:
        self.signal(signal.SIGTERM)
        if self.wait(TERM_GRACE):
            return
        self.signal(signal.SIGKILL)
        self.wait(KILL_WAIT)

    def finish(self, seconds: float) -> None:
        if self.exit_ns is None and not self.wait(seconds):
            self.signal(signal.SIGKILL)
            self.wait(KILL_WAIT)
        for thread in self.threads:
            thread.join(timeout=1.0)

    def record(self) -> dict[str, Any]:
        return {
            "argv": self.argv,
            "pid": self.child.pid,
            "start_ns": self.start_ns,
            "signals": self.signals,
            "exit_ns": self.exit_ns,
            "wait_status": self.status,
            "returncode": self.child.returncode if self.exit_ns is not None else None,
            "stdout_hex": self.out["stdout"].hex(),
            "stderr_hex": self.out["stderr"].hex(),
            "stdout_eof_ns": self.eof_ns["stdout"],
            "stderr_eof_ns": self.eof_ns["stderr"],
        }


# ---- follow-up requests ----


def http_follow(port: int, kind: str) -> dict[str, Any]:
    record: dict[str, Any] = {"kind": kind, "path": "http", "start_ns": now_ns()}
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=FOLLOW_BOUND)
    try:
        if kind == "health":
            connection.request("GET", "/health")
        else:
            body = json.dumps({"command": ORDINARY, "timeout": ORDINARY_TIMEOUT}).encode()
            record["request_hex"] = body.hex()
            connection.request(
                "POST", "/exec", body=body, headers={"Content-Type": "application/json"}
            )
        response = connection.getresponse()
        record["status"] = response.status
        record["body_hex"] = response.read().hex()
    except OSError as error:
        record["transport_error"] = f"{type(error).__name__}: {error}"
    finally:
        connection.close()
    record["end_ns"] = now_ns()
    return record


def cli_follow(cli: str, env: dict[str, str], kind: str) -> dict[str, Any]:
    if kind == "health":
        argv = [cli, "agent", "health", "--vm", "normal", "--json"]
    else:
        timeout = str(ORDINARY_TIMEOUT)
        argv = [cli, "file", "exec", "--vm", "normal", "--timeout", timeout, "--json", ORDINARY]
    record: dict[str, Any] = {"kind": kind, "path": "cli", "argv": argv, "start_ns": now_ns()}
    child = subprocess.Popen(  # noqa: S603 - the checkout CLI
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        start_new_session=True,
    )
    try:
        stdout, stderr = child.communicate(timeout=FOLLOW_BOUND)
        record["bound_exceeded"] = False
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        stdout, stderr = child.communicate()
        record["bound_exceeded"] = True
    record["end_ns"] = now_ns()
    record["returncode"] = child.returncode
    record["stdout_hex"] = stdout.hex()
    record["stderr_hex"] = stderr.hex()
    return record


# ---- one case ----


def abort(raw: RawExec, how: str) -> None:
    if how == "reset":
        raw.sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
    if how == "half-close":
        raw.sock.shutdown(socket.SHUT_WR)
    else:
        raw.sock.close()


def run_case(
    case: Case, tag: str, root: Path, port: int, cli: str, env: dict[str, str]
) -> dict[str, Any]:
    directory = root / f"c{case.digit}"
    command = payload(case, tag, directory)
    record: dict[str, Any] = {
        "case": case.name,
        "path": case.path,
        "action": case.action,
        "token": token(case, tag),
        "command": command,
    }
    raw: RawExec | None = None
    cli_exec: CliExec | None = None
    if case.path == "http":
        raw = RawExec(port, command)
        start_ns = raw.send()
        if case.action in ("none", "concurrent", "half-close"):
            raw.start_reader(start_ns + int(ORIGINAL_BOUND * 1e9))
    else:
        cli_exec = CliExec(cli, env, command)
        start_ns = cli_exec.start_ns
    record["start_ns"] = start_ns
    found = find_identities(directory, start_ns + int(PIDFILE_BOUND * 1e9))
    watches = [found[k] for k in ("shell", "child") if k in found]
    if "error" in found:
        record["identity_error"] = found["error"]
    act_ns = start_ns + int(ACT_AFTER * 1e9)
    while now_ns() < act_ns:
        for watch in watches:
            watch.poll()
        time.sleep(POLL)
    record["act_ns"] = now_ns()
    if raw is not None and case.action in ("close", "reset", "half-close"):
        abort(raw, case.action)
    if cli_exec is not None and case.action == "term":
        cli_exec.interrupt()
    record["act_done_ns"] = now_ns()
    follow: list[dict[str, Any]] = []
    if case.action != "none":
        for kind in ("health", "ordinary"):
            follow.append(
                http_follow(port, kind) if case.path == "http" else cli_follow(cli, env, kind)
            )
            for watch in watches:
                watch.poll()
    record["follow"] = follow
    observe_ns = start_ns + int(OBSERVE_BOUND * 1e9)
    while now_ns() < observe_ns:
        for watch in watches:
            watch.poll()
        if all(watch.gone_ns is not None for watch in watches):
            break
        time.sleep(POLL)
    record["identities"] = [watch.record() for watch in watches]
    if raw is not None:
        if raw.reader is not None:
            raw.reader.join(timeout=max(0.0, (start_ns + ORIGINAL_BOUND * 1e9 - now_ns()) / 1e9))
        if case.action not in ("close", "reset"):
            raw.sock.close()
        record["original"] = raw.record()
    if cli_exec is not None:
        cli_exec.finish(max(0.0, (start_ns + ORIGINAL_BOUND * 1e9 - now_ns()) / 1e9))
        record["original"] = cli_exec.record()
    record["sweep"] = sweep(token(case, tag))
    record["end_ns"] = now_ns()
    return record


def seam(tag: str, agent: str, app: str, cli: str) -> dict[str, Any]:
    result: dict[str, Any] = {"seam": "k13-host", "tag": tag, "servers": [], "stops": []}
    owned: list[Owned] = []
    root = Path(tempfile.mkdtemp(prefix="k13", dir="/tmp"))  # a socket path has 103 bytes at most
    result["root"] = str(root)
    try:
        state = root / "xdg-state" / "testanyware"
        data = root / "xdg-data" / "testanyware"
        tools, tmp, run = root / "tools", root / "tmp", root / "run"
        for directory in (state / "vms", data, tools, tmp, run):
            directory.mkdir(parents=True)
        run.chmod(0o700)
        sock = run / "testanyware.sock"
        port = free_port()
        base = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
        app_argv = [
            app,
            "--test-mode",
            "--socket",
            str(sock),
            "--state-dir",
            str(state),
            "--data-dir",
            str(data),
            "--tool-dir",
            str(tools),
        ]
        for role, argv, env, connect in (
            ("agent", [agent, str(port)], base, tcp_connect(port)),
            ("app", app_argv, {**base, "TMPDIR": str(tmp)}, unix_connect(sock)),
        ):
            server = Owned(role, argv, env)
            owned.append(server)
            ready = wait_connect(connect, server.child)
            result["servers"].append(
                {
                    "role": role,
                    "argv": argv,
                    "pid": server.child.pid,
                    "start": server.start,
                    "ready": ready,
                }
            )
            if not ready:
                result["error"] = f"{role} did not accept a connection"
                return result
        write_spec(state, "normal", port)
        cli_env = {
            **base,
            "HOME": str(root),
            "TMPDIR": str(tmp),
            "XDG_STATE_HOME": str(root / "xdg-state"),
            "XDG_DATA_HOME": str(root / "xdg-data"),
            "TESTANYWARE_APP_SOCKET": str(sock),
        }
        result["cases"] = [run_case(case, tag, root, port, cli, cli_env) for case in CASES]
        return result
    finally:
        for server in reversed(owned):
            result["stops"].append(server.stop())
        result["final_sweep"] = [entry for case in CASES for entry in sweep(token(case, tag))]
        for path in sorted(root.rglob("*"), reverse=True):
            if path.is_dir() and not path.is_symlink():
                if not any(path.iterdir()):
                    path.rmdir()
            else:
                path.unlink()
        if root.exists() and not any(root.iterdir()):
            root.rmdir()
        result["root_removed"] = not root.exists()


def main(argv: list[str]) -> int:
    if len(argv) != 5 or not RUN_TAG.match(argv[1]):
        print(__doc__, file=sys.stderr)
        return 64
    result = seam(argv[1], argv[2], argv[3], argv[4])
    sys.stdout.write(json.dumps(result, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
