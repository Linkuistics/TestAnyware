"""Run the frozen k12 exec cases through one seam and print the raw evidence.

Usage:
  outcomes_k12_seam.py reference
  outcomes_k12_seam.py process PROBE
  outcomes_k12_seam.py traced PROBE
  outcomes_k12_seam.py http-cli AGENT APP CLI

The script prints one JSON object on stdout. Bytes are hex, so nothing is decoded
or trimmed. The script judges nothing. `outcomes_k12_check.py` judges the object.

- `reference` runs each command with `subprocess` and `os.wait4`. It is the
  independent oracle for raw bytes, raw wait status and the deadline phases.
- `process` and `traced` run a probe that calls the unchanged
  `ProcessRunner.runShell` once per case. `traced` also keeps the exit trace.
- `http-cli` starts two loopback agents and a test-mode app. It reads each case
  as raw HTTP bytes and as the receipt of the checkout CLI in text and JSON modes.

Spawn failure uses `RLIMIT_NPROC` of 1 on the process that spawns the shell. The
user already owns more processes than that, so the next fork or spawn fails with
EAGAIN. The label is `instrumentation: rlimit-nproc`.

Each deadline command sleeps for a seam-specific `30.NM` seconds. That token
identifies a leftover `sleep` positively. After each case, the script lists the
processes that carry the token, records them and kills them.
"""

from __future__ import annotations

import http.client
import json
import os
import resource
import signal
import socket
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ORACLE_STDOUT = bytes.fromhex("4578706c61696e2074686973206368616e67652e")
ORACLE_STDERR = bytes.fromhex("7365706172617465207374646572720a")
ORACLE_BINARY = bytes.fromhex("00ff80410a")

GUEST_TIMEOUT = 2  # the deadline cases ask for this timeout
ORDINARY_TIMEOUT = 10  # every other case asks for this timeout
HOST_MARGIN = 20  # a raw host exec gets its timeout plus this bound
CLI_MARGIN = 10  # the CLI adds this to its HTTP timeout, so its bound gets it too
GRACE = 2.0  # the faithful TERM grace
KILL_WAIT = 2.0  # the faithful kill and reap bound
READY_BOUND = 10.0  # an agent or the app must accept a connection within this
STOP_BOUND = 2.0  # a stopped agent or app must exit within this after TERM

SEAMS = ("reference", "process", "traced", "http", "cli-text", "cli-json")
SEAM_DIGIT = {"reference": 1, "process": 2, "traced": 3, "http": 4, "cli-text": 5, "cli-json": 6}
TIMED_OUT_TEXT = b"Process timed out after"


@dataclass(frozen=True)
class Case:
    """One frozen exec case and its faithful outcome.

    `end` is `exit`, `signal`, `deadline` or `spawn-failure`. `code` is the exit
    status for `exit` and the signal number for `signal`. A deadline case keeps the
    bytes that the command wrote before the deadline.
    """

    name: str
    command: str
    timeout: int
    stdout: bytes
    stderr: bytes
    end: str
    code: int | None


def deadline_command(seam: str, index: int, ignore_term: bool) -> str:
    token = f"sleep 30.{SEAM_DIGIT[seam]}{index}"
    if ignore_term:
        return f"trap '' TERM; printf 'ignoring term\\n'; {token}"
    return f"printf 'before deadline\\n'; {token}"


def cases(seam: str) -> tuple[Case, ...]:
    """The eight cases. Only the sleep token of a deadline case depends on the seam."""
    return (
        Case(
            "oracle-text",
            "printf 'Explain this change.'; printf 'separate stderr\\n' >&2",
            ORDINARY_TIMEOUT,
            ORACLE_STDOUT,
            ORACLE_STDERR,
            "exit",
            0,
        ),
        Case(
            "binary",
            "printf '\\000\\377\\200A\\n'",
            ORDINARY_TIMEOUT,
            ORACLE_BINARY,
            b"",
            "exit",
            0,
        ),
        Case(
            "exit-23-partial",
            "printf 'partial out\\n'; printf 'partial err\\n' >&2; exit 23",
            ORDINARY_TIMEOUT,
            b"partial out\n",
            b"partial err\n",
            "exit",
            23,
        ),
        Case(
            "signal-kill",
            "printf 'before signal\\n'; kill -KILL $$",
            ORDINARY_TIMEOUT,
            b"before signal\n",
            b"",
            "signal",
            9,
        ),
        Case(
            "exit-9",
            "printf 'before exit\\n'; exit 9",
            ORDINARY_TIMEOUT,
            b"before exit\n",
            b"",
            "exit",
            9,
        ),
        Case(
            "deadline",
            deadline_command(seam, 1, ignore_term=False),
            GUEST_TIMEOUT,
            b"before deadline\n",
            b"",
            "deadline",
            None,
        ),
        Case(
            "deadline-term-ignored",
            deadline_command(seam, 2, ignore_term=True),
            GUEST_TIMEOUT,
            b"ignoring term\n",
            b"",
            "deadline",
            None,
        ),
        Case(
            "spawn-failure",
            "printf 'never runs\\n'",
            ORDINARY_TIMEOUT,
            b"",
            b"",
            "spawn-failure",
            None,
        ),
    )


def now_ns() -> int:
    return time.clock_gettime_ns(time.CLOCK_UPTIME_RAW)


# ---- owned processes ----


def processes() -> list[tuple[int, str, str]]:
    """Each process of this user as (pid, start time, command)."""
    listing = subprocess.run(
        ["/bin/ps", "-x", "-o", "pid=,lstart=,command="],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    ).stdout
    rows: list[tuple[int, str, str]] = []
    for line in listing.splitlines():
        parts = line.split(None, 6)
        if len(parts) == 7 and parts[0].isdigit():
            rows.append((int(parts[0]), " ".join(parts[1:6]), parts[6]))
    return rows


def identity(pid: int) -> str | None:
    for row_pid, start, _ in processes():
        if row_pid == pid:
            return start
    return None


def sweep(token: str) -> list[dict[str, Any]]:
    """Record and kill each process whose command is exactly the sleep token."""
    found: list[dict[str, Any]] = []
    for pid, start, command in processes():
        if command.strip() == token:
            killed = True
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                killed = False
            found.append({"pid": pid, "start": start, "command": command, "killed": killed})
    return found


def token_of(case: Case) -> str | None:
    if case.end != "deadline":
        return None
    return case.command.rsplit("; ", 1)[-1]


def settle(case: Case) -> list[dict[str, Any]]:
    """The residuals of a deadline case, after a bounded wait for them to go."""
    token = token_of(case)
    if token is None:
        return []
    deadline = time.monotonic() + KILL_WAIT
    while time.monotonic() < deadline:
        if not any(command.strip() == token for _, _, command in processes()):
            return []
        time.sleep(0.05)
    return sweep(token)


# ---- reference ----


def wait_until(pid: int, deadline_ns: int) -> tuple[int, int] | None:
    """Poll for the exit of `pid` until `deadline_ns`. Return (status, seen_ns)."""
    while True:
        reaped, status, _ = os.wait4(pid, os.WNOHANG)
        if reaped == pid:
            return status, now_ns()
        if now_ns() >= deadline_ns:
            return None
        time.sleep(0.002)


def decode_status(status: int) -> dict[str, Any]:
    if os.WIFEXITED(status):
        return {"raw": status, "end": "exit", "code": os.WEXITSTATUS(status)}
    if os.WIFSIGNALED(status):
        return {"raw": status, "end": "signal", "code": os.WTERMSIG(status)}
    return {"raw": status, "end": "other", "code": None}


def limited_spawn(case: Case) -> dict[str, Any]:
    """Try the spawn in a forked child whose `RLIMIT_NPROC` is 1.

    macOS refuses to raise the soft limit again, so the controller never lowers its
    own limit. The child reports the spawn error, or the status of an unexpected
    shell, as one JSON line on a pipe.
    """
    read_end, write_end = os.pipe()
    pid = os.fork()
    if pid == 0:  # the child: no return from this branch
        os.close(read_end)
        report: dict[str, Any]
        try:
            _, hard = resource.getrlimit(resource.RLIMIT_NPROC)
            resource.setrlimit(resource.RLIMIT_NPROC, (1, hard))
            shell = os.posix_spawn("/bin/bash", ["/bin/bash", "-c", case.command], {})
            _, status, _ = os.wait4(shell, 0)
            report = {"outcome": "status", "status": decode_status(status)}
        except OSError as error:
            report = {"outcome": "spawn-error", "errno": error.errno, "error": str(error)}
        os.write(write_end, json.dumps(report).encode())
        os._exit(0)
    os.close(write_end)
    chunks: list[bytes] = []
    while chunk := os.read(read_end, 65536):
        chunks.append(chunk)
    os.close(read_end)
    _, status, _ = os.wait4(pid, 0)
    report = json.loads(b"".join(chunks)) if chunks else {"outcome": "no-report"}
    report["helper_status"] = decode_status(status)
    return report


def reference_case(case: Case, work: Path) -> dict[str, Any]:
    out_path = work / f"{case.name}.out"
    err_path = work / f"{case.name}.err"
    record: dict[str, Any] = {"case": case.name, "command": case.command, "timeout": case.timeout}
    if case.end == "spawn-failure":
        record["start_ns"] = now_ns()
        record.update(limited_spawn(case))
        record["end_ns"] = now_ns()
        record["instrumentation"] = "rlimit-nproc"
        return record
    with out_path.open("wb") as out, err_path.open("wb") as err:
        start = now_ns()
        record["start_ns"] = start
        child = subprocess.Popen(  # noqa: S603 - the frozen case command
            ["/bin/bash", "-c", case.command],
            stdin=subprocess.DEVNULL,
            stdout=out,
            stderr=err,
            start_new_session=True,
        )
        due = start + case.timeout * 1_000_000_000
        seen = wait_until(child.pid, due)
        if seen is None:
            record["deadline_ns"] = now_ns()
            os.killpg(child.pid, signal.SIGTERM)
            record["term_ns"] = now_ns()
            seen = wait_until(child.pid, record["term_ns"] + int(GRACE * 1e9))
            if seen is None:
                record["grace_expired_ns"] = now_ns()
                os.killpg(child.pid, signal.SIGKILL)
                record["kill_ns"] = now_ns()
                _, status, _ = os.wait4(child.pid, 0)
                seen = (status, now_ns())
            record["timed_out"] = True
        else:
            record["timed_out"] = False
        status, reaped = seen
        record["reap_ns"] = reaped
        try:
            os.killpg(child.pid, signal.SIGKILL)  # the orphaned members of the group
            record["group_killed"] = True
        except ProcessLookupError:
            record["group_killed"] = False
        child.returncode = 0  # the status is taken; stop Popen from waiting again
    record["outcome"] = "status"
    record["status"] = decode_status(status)
    record["stdout_hex"] = out_path.read_bytes().hex()
    record["stderr_hex"] = err_path.read_bytes().hex()
    record["end_ns"] = now_ns()
    return record


# ---- probes ----


def probe_case(probe: str, case: Case, trace: Path | None) -> dict[str, Any]:
    argv = [probe, case.command, str(case.timeout)]
    instrumentation = None
    if case.end == "spawn-failure":
        argv = ["/bin/sh", "-c", 'ulimit -u 1; exec "$0" "$@"', *argv]
        instrumentation = "rlimit-nproc"
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
    if trace is not None:
        env["TESTANYWARE_EXIT_TRACE"] = str(trace)
    bound = case.timeout + HOST_MARGIN
    record: dict[str, Any] = {
        "case": case.name,
        "command": case.command,
        "timeout": case.timeout,
        "argv": argv,
        "bound_seconds": bound,
        "instrumentation": instrumentation,
        "start_ns": now_ns(),
    }
    child = subprocess.Popen(  # noqa: S603 - the built probe
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        start_new_session=True,
    )
    try:
        stdout, stderr = child.communicate(timeout=bound)
        record["bound_exceeded"] = False
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        stdout, stderr = child.communicate()
        record["bound_exceeded"] = True
    record["end_ns"] = now_ns()
    record["probe_returncode"] = child.returncode
    record["probe_stdout_hex"] = stdout.hex()
    record["probe_stderr_hex"] = stderr.hex()
    try:
        record["result"] = json.loads(stdout)
    except ValueError:
        record["result"] = None
    if trace is not None:
        record["trace"] = read_trace(trace)
    return record


def read_trace(path: Path) -> list[Any]:
    if not path.exists():
        return []
    rows: list[Any] = []
    for line in path.read_bytes().splitlines():
        try:
            rows.append(json.loads(line))
        except ValueError:
            rows.append({"unparsable_hex": line.hex()})
    return rows


# ---- loopback agents, the app and the CLI ----


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def wait_connect(connect: Any, child: subprocess.Popen[bytes]) -> bool:
    deadline = time.monotonic() + READY_BOUND
    while time.monotonic() < deadline:
        if child.poll() is not None:
            return False
        try:
            connect()
            return True
        except OSError:
            time.sleep(0.02)
    return False


def tcp_connect(port: int) -> Any:
    def connect() -> None:
        socket.create_connection(("127.0.0.1", port), timeout=1).close()

    return connect


def unix_connect(path: Path) -> Any:
    def connect() -> None:
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(1)
            client.connect(str(path))

    return connect


class Owned:
    """A started server, known by PID and start time."""

    def __init__(self, role: str, argv: list[str], env: dict[str, str]) -> None:
        self.role = role
        self.argv = argv
        self.child = subprocess.Popen(  # noqa: S603 - a built server
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=env,
            start_new_session=True,
        )
        self.start = identity(self.child.pid)

    def stop(self) -> dict[str, Any]:
        record: dict[str, Any] = {"role": self.role, "pid": self.child.pid, "start": self.start}
        if self.child.poll() is not None:
            record["state"] = "exited-before-stop"
            record["returncode"] = self.child.returncode
            return record
        if identity(self.child.pid) != self.start:
            record["state"] = "identity-changed"
            return record
        self.child.terminate()
        try:
            self.child.wait(timeout=STOP_BOUND)
            record["state"] = "terminated"
        except subprocess.TimeoutExpired:
            os.killpg(self.child.pid, signal.SIGKILL)
            self.child.wait(timeout=KILL_WAIT)
            record["state"] = "killed"
        record["returncode"] = self.child.returncode
        try:
            os.killpg(self.child.pid, signal.SIGKILL)
            record["group_members_killed"] = True
        except ProcessLookupError:
            record["group_members_killed"] = False
        return record


def http_case(port: int, case: Case) -> dict[str, Any]:
    body = json.dumps({"command": case.command, "timeout": case.timeout}).encode()
    bound = case.timeout + HOST_MARGIN
    record: dict[str, Any] = {
        "case": case.name,
        "command": case.command,
        "timeout": case.timeout,
        "port": port,
        "request_hex": body.hex(),
        "bound_seconds": bound,
        "start_ns": now_ns(),
    }
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=bound)
    try:
        connection.request("POST", "/exec", body=body, headers={"Content-Type": "application/json"})
        response = connection.getresponse()
        record["status"] = response.status
        record["reason"] = response.reason
        record["headers"] = [[k, v] for k, v in response.getheaders()]
        record["body_hex"] = response.read().hex()
    except OSError as error:
        record["transport_error"] = f"{type(error).__name__}: {error}"
    finally:
        connection.close()
    record["end_ns"] = now_ns()
    return record


def cli_case(cli: str, env: dict[str, str], vm: str, case: Case, json_mode: bool) -> dict[str, Any]:
    argv = [cli, "exec", case.command, "--vm", vm, "--timeout", str(case.timeout)]
    if json_mode:
        argv.append("--json")
    bound = case.timeout + CLI_MARGIN + HOST_MARGIN
    record: dict[str, Any] = {
        "case": case.name,
        "command": case.command,
        "timeout": case.timeout,
        "argv": argv,
        "bound_seconds": bound,
        "start_ns": now_ns(),
    }
    child = subprocess.Popen(  # noqa: S603 - the checkout CLI
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        start_new_session=True,
    )
    try:
        stdout, stderr = child.communicate(timeout=bound)
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


def write_spec(state: Path, vm: str, port: int) -> None:
    spec = {
        "platform": "macos",
        "agent": {"host": "127.0.0.1", "port": port},
        "vnc": {"host": "127.0.0.1", "port": 1},
    }
    (state / "vms" / f"{vm}.json").write_text(json.dumps(spec))


def http_cli(agent: str, app: str, cli: str) -> dict[str, Any]:
    result: dict[str, Any] = {"seam": "http-cli", "servers": [], "stops": []}
    owned: list[Owned] = []
    root = Path(tempfile.mkdtemp(prefix="k12", dir="/tmp"))  # a socket path has 103 bytes at most
    result["root"] = str(root)
    try:
        state = root / "xdg-state" / "testanyware"
        data = root / "xdg-data" / "testanyware"
        tools = root / "tools"
        tmp = root / "tmp"
        run = root / "run"
        for directory in (state / "vms", data, tools, tmp, run):
            directory.mkdir(parents=True)
        run.chmod(0o700)
        sock = run / "testanyware.sock"
        normal_port, limited_port = free_port(), free_port()
        base = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
        servers = (
            ("agent-normal", [agent, str(normal_port)], tcp_connect(normal_port)),
            (
                "agent-rlimit-nproc",
                ["/bin/sh", "-c", 'ulimit -u 1; exec "$0" "$@"', agent, str(limited_port)],
                tcp_connect(limited_port),
            ),
        )
        for role, argv, connect in servers:
            server = Owned(role, argv, base)
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
        write_spec(state, "normal", normal_port)
        write_spec(state, "nproc", limited_port)
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
        server = Owned("app", app_argv, {**base, "TMPDIR": str(tmp)})
        owned.append(server)
        ready = wait_connect(unix_connect(sock), server.child)
        result["servers"].append(
            {
                "role": "app",
                "argv": app_argv,
                "pid": server.child.pid,
                "start": server.start,
                "ready": ready,
            }
        )
        if not ready:
            result["error"] = "the app did not accept a connection"
            return result
        cli_env = {
            **base,
            "HOME": str(root),
            "TMPDIR": str(tmp),
            "XDG_STATE_HOME": str(root / "xdg-state"),
            "XDG_DATA_HOME": str(root / "xdg-data"),
            "TESTANYWARE_APP_SOCKET": str(sock),
        }
        for seam in ("http", "cli-text", "cli-json"):
            records: list[dict[str, Any]] = []
            for case in cases(seam):
                limited = case.end == "spawn-failure"
                if seam == "http":
                    record = http_case(limited_port if limited else normal_port, case)
                else:
                    vm = "nproc" if limited else "normal"
                    record = cli_case(cli, cli_env, vm, case, json_mode=seam == "cli-json")
                record["instrumentation"] = "rlimit-nproc" if limited else None
                record["residuals"] = settle(case)
                records.append(record)
            result[seam] = records
        return result
    finally:
        for server in reversed(owned):
            result["stops"].append(server.stop())
        for path in sorted(root.rglob("*"), reverse=True):
            if path.is_dir() and not path.is_symlink():
                path.rmdir() if not any(path.iterdir()) else None
            else:
                path.unlink()
        root.rmdir() if root.exists() and not any(root.iterdir()) else None
        result["root_removed"] = not root.exists()


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 64
    seam = argv[1]
    result: dict[str, Any]
    if seam == "reference" and len(argv) == 2:
        work = Path(tempfile.mkdtemp(prefix="k12-reference-"))
        records: list[dict[str, Any]] = []
        for case in cases(seam):
            record = reference_case(case, work)
            record["residuals"] = settle(case)
            records.append(record)
        for path in work.iterdir():
            path.unlink()
        work.rmdir()
        result = {"seam": seam, "reference": records, "work_removed": not work.exists()}
    elif seam in ("process", "traced") and len(argv) == 3:
        work = Path(tempfile.mkdtemp(prefix=f"k12-{seam}-"))
        records = []
        for index, case in enumerate(cases(seam)):
            trace = work / f"{index}-{case.name}.jsonl" if seam == "traced" else None
            record = probe_case(argv[2], case, trace)
            record["residuals"] = settle(case)
            records.append(record)
        for path in work.iterdir():
            path.unlink()
        work.rmdir()
        result = {"seam": seam, seam: records, "work_removed": not work.exists()}
    elif seam == "http-cli" and len(argv) == 5:
        result = http_cli(argv[2], argv[3], argv[4])
    else:
        print(__doc__, file=sys.stderr)
        return 64
    sys.stdout.write(json.dumps(result, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
