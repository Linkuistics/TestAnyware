"""Run the frozen k5 Linux exec cases through one seam and print the raw evidence.

Usage:
  linux_k5_seam.py RUN inside reference|process|traced
  linux_k5_seam.py RUN host IMAGE APP CLI
  linux_k5_seam.py observe
  linux_k5_seam.py kill PID:TICKS...
  linux_k5_seam.py limit

RUN is the run tag: 12 decimal digits that the runner derives from its run ID.
The script prints one JSON object on stdout. Bytes are hex where the seam gives
bytes. The script judges nothing. `linux_k5_check.py` judges the object.

- `inside` runs in a fresh container as uid 1000. The container mounts the
  unchanged agent at /agent, the desktop stubs at /stub and this directory at
  /seam. `reference` runs each command with `subprocess` and `os.wait4`, and it
  is the oracle for bytes, status and the deadline phases. `process` calls the
  unchanged `system_endpoints.handle_exec` once per case. `traced` does the same
  and records when `Popen` timed out, signalled and reaped, and the partial bytes
  that `TimeoutExpired` held.
- `host` runs on the host. It starts one fresh agent container per case and seam,
  and one test-mode app. It reads each case as raw HTTP bytes and as the receipt
  of the checkout CLI in text and JSON modes.
- `observe` and `kill` run inside an agent container through `docker exec`.
  `observe` lists the processes of the container. `kill` sends SIGKILL to each
  PID whose start time in clock ticks is still the given one. `limit` sets
  `RLIMIT_NPROC` to 1 on the agent, which is PID 1 of its container.

A process belongs to a case when the case container did not list it before the
case. Each container is fresh and carries the run label, so this ownership is
positive. The script observes each case at its return and again at the faithful
bound. Then it kills the owned survivors and observes once more.

Spawn failure uses `RLIMIT_NPROC` of 1. Inside, a forked child sets the limit
before it calls the seam. On the host, the agent container runs as uid 1001
without an init process. The kernel refuses the first `execve` of a container
that starts with the limit, so `limit` sets it after the agent answers. The label
is `instrumentation: rlimit-nproc`.
"""

from __future__ import annotations

import importlib
import json
import os
import re
import resource
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from outcomes_k12_seam import (
    GRACE,
    KILL_WAIT,
    ORACLE_BINARY,
    ORACLE_STDERR,
    ORACLE_STDOUT,
    READY_BOUND,
    Case,
    Owned,
    cli_case,
    decode_status,
    http_case,
    limited_spawn,
    now_ns,
    unix_connect,
    wait_connect,
)

GUEST_TIMEOUT = 2  # the deadline cases ask for this timeout
ORDINARY_TIMEOUT = 10  # every other case asks for this timeout
OBSERVE_BOUND = 30.0  # one ordinary observation
CLEANUP_BOUND = 30.0  # the guest cleanup of one case
DISPOSAL_BOUND = 45.0  # the independent disposal of all containers
AGENT_PORT = 8648
ISSUE_DIRS = ("/tmp/vmfit-k15", "/tmp/vmrel-k16")  # the retained run created these first
CHILD_PID_FILE = "/tmp/vmrel-k16/child.pid"
LABEL = "testanyware.k5.run"
NORMAL_UID = "1000:1000"
LIMITED_UID = "1001:1001"

INSIDE_SEAMS = ("reference", "process", "traced")
HOST_SEAMS = ("http", "cli-text", "cli-json")
SEAMS = INSIDE_SEAMS + HOST_SEAMS
SEAM_DIGIT = {seam: index + 1 for index, seam in enumerate(SEAMS)}
RUN_TAG = re.compile(r"^[0-9]{12}$")

HERE = Path(__file__).resolve().parent
AGENT_DIR = HERE.parent.parent / "agents" / "linux"
STUB_DIR = HERE / "linux_k5_stub"
SELF = "/seam/linux_k5_seam.py"

ISSUE_SLEEP = "sleep 120"
ISSUE_FORK_WAIT = f"sleep 120 & p=$!; echo $p > {CHILD_PID_FILE}; wait"


def deadline_command(seam: str, index: int, ignore_term: bool, run: str) -> str:
    token = f"sleep 30.{SEAM_DIGIT[seam]}{index}{run}"
    if ignore_term:
        return f"trap '' TERM; printf 'ignoring term\\n'; {token}"
    return f"printf 'before deadline\\n'; printf 'deadline err\\n' >&2; {token}"


def cases(seam: str, run: str = "") -> tuple[Case, ...]:
    """The eleven cases. Only the sleep token of a deadline case depends on the seam and run.

    The two issue controls are the exact commands of the retained failure B run.
    Their faithful end is TERM, because the reference ends the group with TERM.
    """
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
            "exit-255",
            "printf 'before exit\\n'; exit 255",
            ORDINARY_TIMEOUT,
            b"before exit\n",
            b"",
            "exit",
            255,
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
            "signal-hup",
            "printf 'before hangup\\n'; kill -HUP $$",
            ORDINARY_TIMEOUT,
            b"before hangup\n",
            b"",
            "signal",
            1,
        ),
        Case(
            "deadline",
            deadline_command(seam, 1, ignore_term=False, run=run),
            GUEST_TIMEOUT,
            b"before deadline\n",
            b"deadline err\n",
            "deadline",
            15,
        ),
        Case(
            "deadline-term-ignored",
            deadline_command(seam, 2, ignore_term=True, run=run),
            GUEST_TIMEOUT,
            b"ignoring term\n",
            b"",
            "deadline",
            9,
        ),
        Case("issue-sleep", ISSUE_SLEEP, GUEST_TIMEOUT, b"", b"", "deadline", 15),
        Case("issue-fork-wait", ISSUE_FORK_WAIT, GUEST_TIMEOUT, b"", b"", "deadline", 15),
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


def mono_ns() -> int:
    return time.monotonic_ns()


def late_ns(case: Case, start_ns: int, end_ns: int) -> int:
    """The faithful bound: by then a faithful seam has reaped every owned process."""
    if case.end == "deadline":
        bound = start_ns + int((case.timeout + GRACE + KILL_WAIT) * 1e9)
        return max(bound, end_ns)
    return end_ns + int(KILL_WAIT * 1e9)


def sleep_until(mark_ns: int, clock: Callable[[], int]) -> None:
    remaining = (mark_ns - clock()) / 1e9
    if remaining > 0:
        time.sleep(remaining)


# ---- processes in one container, read from /proc ----


def proc_rows() -> list[dict[str, Any]]:
    """Each process as pid, state, ppid, pgid, sid, start ticks and argv."""
    rows: list[dict[str, Any]] = []
    for name in os.listdir("/proc"):
        if not name.isdigit():
            continue
        try:
            stat = Path(f"/proc/{name}/stat").read_text()
            cmdline = Path(f"/proc/{name}/cmdline").read_bytes()
        except OSError:
            continue
        fields = stat.rsplit(")", 1)[1].split()
        argv = [part.decode("utf-8", "backslashreplace") for part in cmdline.split(b"\0")[:-1]]
        rows.append(
            {
                "pid": int(name),
                "state": fields[0],
                "ppid": int(fields[1]),
                "pgid": int(fields[2]),
                "sid": int(fields[3]),
                "start_ticks": int(fields[19]),
                "argv": argv,
            }
        )
    return sorted(rows, key=lambda row: int(row["pid"]))


def observe_here() -> dict[str, Any]:
    """The processes of this container, without this observer."""
    me = os.getpid()
    return {"ns": mono_ns(), "rows": [row for row in proc_rows() if row["pid"] != me]}


def kill_here(targets: list[tuple[int, int]]) -> list[dict[str, Any]]:
    """Send SIGKILL to each target whose start ticks are still the given ones."""
    current = {int(row["pid"]): row for row in proc_rows()}
    results: list[dict[str, Any]] = []
    for pid, ticks in targets:
        row = current.get(pid)
        if row is None or row["start_ticks"] != ticks:
            results.append({"pid": pid, "start_ticks": ticks, "killed": False, "why": "gone"})
            continue
        try:
            os.kill(pid, signal.SIGKILL)
            results.append({"pid": pid, "start_ticks": ticks, "killed": True})
        except ProcessLookupError:
            results.append({"pid": pid, "start_ticks": ticks, "killed": False, "why": "raced"})
    return results


def owned(baseline: dict[str, Any], later: dict[str, Any]) -> list[dict[str, Any]]:
    """The rows of `later` that `baseline` did not list. Unknown stays unknown."""
    if "rows" not in baseline or "rows" not in later:
        return []
    before = {(row["pid"], row["start_ticks"]) for row in baseline["rows"]}
    return [row for row in later["rows"] if (row["pid"], row["start_ticks"]) not in before]


def lifecycle(
    case: Case,
    record: dict[str, Any],
    baseline: dict[str, Any],
    observe: Callable[[], dict[str, Any]],
    kill: Callable[[list[tuple[int, int]]], list[dict[str, Any]]],
    clock: Callable[[], int],
    read_child: Callable[[], str | None],
) -> None:
    """Observe at return and at the faithful bound, then clean up the owned survivors."""
    record["baseline"] = baseline
    record["at_return"] = observe()
    if case.name == "issue-fork-wait":
        record["child_pid_file"] = read_child()
    sleep_until(late_ns(case, record["start_ns"], record["end_ns"]), clock)
    record["at_bound"] = observe()
    survivors = owned(baseline, record["at_bound"])
    cleanup: dict[str, Any] = {"targets": [[row["pid"], row["start_ticks"]] for row in survivors]}
    record["cleanup"] = cleanup
    if not survivors:
        return
    started = clock()
    cleanup["killed"] = kill([(row["pid"], row["start_ticks"]) for row in survivors])
    deadline = started + int(CLEANUP_BOUND * 1e9)
    while True:
        after = observe()
        remaining = [row for row in owned(baseline, after) if row["state"] != "Z"]
        if not remaining or clock() >= deadline or "rows" not in after:
            break
        time.sleep(0.1)
    cleanup["after"] = after
    cleanup["seconds"] = (clock() - started) / 1e9


# ---- inside: the reference ----


def wait_until(pid: int, deadline_ns: int) -> tuple[int, int] | None:
    """Poll for the exit of `pid` until `deadline_ns`. Return (status, seen_ns)."""
    while True:
        reaped, status, _ = os.wait4(pid, os.WNOHANG)
        if reaped == pid:
            return status, mono_ns()
        if mono_ns() >= deadline_ns:
            return None
        time.sleep(0.002)


def reference_case(case: Case, work: Path) -> dict[str, Any]:
    record: dict[str, Any] = {"case": case.name, "command": case.command, "timeout": case.timeout}
    if case.end == "spawn-failure":
        record["start_ns"] = mono_ns()
        record.update(limited_spawn(case))
        record["end_ns"] = mono_ns()
        record["instrumentation"] = "rlimit-nproc"
        return record
    out_path, err_path = work / f"{case.name}.out", work / f"{case.name}.err"
    with out_path.open("wb") as out, err_path.open("wb") as err:
        start = mono_ns()
        record["start_ns"] = start
        child = subprocess.Popen(  # noqa: S603 - the frozen case command
            ["/bin/bash", "-c", case.command],
            stdin=subprocess.DEVNULL,
            stdout=out,
            stderr=err,
            start_new_session=True,
        )
        record["shell_pid"] = child.pid
        seen = wait_until(child.pid, start + case.timeout * 1_000_000_000)
        if seen is None:
            record["deadline_ns"] = mono_ns()
            os.killpg(child.pid, signal.SIGTERM)
            record["term_ns"] = mono_ns()
            seen = wait_until(child.pid, record["term_ns"] + int(GRACE * 1e9))
            if seen is None:
                record["grace_expired_ns"] = mono_ns()
                os.killpg(child.pid, signal.SIGKILL)
                record["kill_ns"] = mono_ns()
                _, status, _ = os.wait4(child.pid, 0)
                seen = (status, mono_ns())
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
    record["end_ns"] = mono_ns()
    record["outcome"] = "status"
    record["status"] = decode_status(status)
    record["stdout_hex"] = out_path.read_bytes().hex()
    record["stderr_hex"] = err_path.read_bytes().hex()
    return record


# ---- inside: the unchanged agent handler ----

TRACE: list[dict[str, Any]] = []


def install_trace() -> None:
    """Record the Popen phases. Each wrapper calls the original and changes nothing."""
    popen: Any = subprocess.Popen
    communicate, send_signal, wait = popen.communicate, popen.send_signal, popen.wait
    run: Any = subprocess.run

    def traced_communicate(self: Any, *args: Any, **kwargs: Any) -> Any:
        try:
            return communicate(self, *args, **kwargs)
        except subprocess.TimeoutExpired:
            TRACE.append({"event": "communicate-timeout", "ns": mono_ns(), "pid": self.pid})
            raise

    def traced_send_signal(self: Any, sig: int) -> None:
        TRACE.append({"event": "signal", "ns": mono_ns(), "pid": self.pid, "signal": int(sig)})
        send_signal(self, sig)

    def traced_wait(self: Any, *args: Any, **kwargs: Any) -> Any:
        result = wait(self, *args, **kwargs)
        TRACE.append(
            {"event": "wait-return", "ns": mono_ns(), "pid": self.pid, "returncode": result}
        )
        return result

    def traced_run(*args: Any, **kwargs: Any) -> Any:
        try:
            return run(*args, **kwargs)
        except subprocess.TimeoutExpired as expired:
            TRACE.append(
                {
                    "event": "run-timeout",
                    "ns": mono_ns(),
                    "partial_stdout_hex": as_hex(expired.stdout),
                    "partial_stderr_hex": as_hex(expired.stderr),
                }
            )
            raise

    popen.communicate = traced_communicate  # type: ignore[method-assign]
    popen.send_signal = traced_send_signal  # type: ignore[method-assign]
    popen.wait = traced_wait  # type: ignore[method-assign]
    subprocess.run = traced_run  # type: ignore[assignment]


def as_hex(value: object) -> str | None:
    if value is None:
        return None
    if isinstance(value, bytes):
        return value.hex()
    return str(value).encode("utf-8", "surrogateescape").hex()


def call_handler(
    handler: Callable[[dict[str, Any]], tuple[int, dict[str, Any]]], case: Case
) -> Any:
    return handler({"command": case.command, "timeout": case.timeout})


def limited_handler(
    handler: Callable[[dict[str, Any]], tuple[int, dict[str, Any]]], case: Case
) -> dict[str, Any]:
    """Call the handler in a forked child whose `RLIMIT_NPROC` is 1."""
    read_end, write_end = os.pipe()
    pid = os.fork()
    if pid == 0:  # the child: no return from this branch
        os.close(read_end)
        try:
            _, hard = resource.getrlimit(resource.RLIMIT_NPROC)
            resource.setrlimit(resource.RLIMIT_NPROC, (1, hard))
            status, body = call_handler(handler, case)
            report: dict[str, Any] = {"http_status": status, "body": body}
        except Exception as error:  # noqa: BLE001 - the report names it
            report = {"helper_error": f"{type(error).__name__}: {error}"}
        os.write(write_end, json.dumps(report).encode())
        os._exit(0)
    os.close(write_end)
    chunks: list[bytes] = []
    while chunk := os.read(read_end, 65536):
        chunks.append(chunk)
    os.close(read_end)
    _, status, _ = os.wait4(pid, 0)
    report = json.loads(b"".join(chunks)) if chunks else {"helper_error": "no report"}
    report["helper_status"] = decode_status(status)
    return report


def handler_case(
    handler: Callable[[dict[str, Any]], tuple[int, dict[str, Any]]], case: Case
) -> dict[str, Any]:
    record: dict[str, Any] = {"case": case.name, "command": case.command, "timeout": case.timeout}
    first = len(TRACE)
    record["start_ns"] = mono_ns()
    if case.end == "spawn-failure":
        record.update(limited_handler(handler, case))
        record["instrumentation"] = "rlimit-nproc"
    else:
        status, body = call_handler(handler, case)
        record["http_status"] = status
        record["body"] = body
    record["end_ns"] = mono_ns()
    record["trace"] = TRACE[first:]
    return record


def read_child_here() -> str | None:
    try:
        return Path(CHILD_PID_FILE).read_text()
    except OSError:
        return None


def inside(run: str, seam: str) -> dict[str, Any]:
    for directory in ISSUE_DIRS:
        os.makedirs(directory, exist_ok=True)
    result: dict[str, Any] = {"seam": seam, "uid": os.getuid(), "python": sys.version}
    handler: Callable[[dict[str, Any]], tuple[int, dict[str, Any]]] | None = None
    if seam != "reference":
        sys.path[:0] = ["/stub", "/agent"]  # -E ignores PYTHONPATH, so the path is explicit
        if seam == "traced":
            install_trace()
        endpoints: Any = importlib.import_module("testanyware_agent.system_endpoints")
        handler = endpoints.handle_exec
        result["handler_file"] = endpoints.__file__
    work = Path(tempfile.mkdtemp(prefix=f"k5-{seam}-"))
    records: list[dict[str, Any]] = []
    for case in cases(seam, run):
        baseline = observe_here()
        if handler is None:
            record = reference_case(case, work)
        else:
            record = handler_case(handler, case)
        lifecycle(case, record, baseline, observe_here, kill_here, mono_ns, read_child_here)
        records.append(record)
    for path in work.iterdir():
        path.unlink()
    work.rmdir()
    result[seam] = records
    result["work_removed"] = not work.exists()
    result["final"] = observe_here()
    return result


# ---- host: agent containers, the app and the CLI ----


def docker(argv: list[str], bound: float) -> dict[str, Any]:
    """Run one docker command within `bound` seconds and keep its whole receipt."""
    started = time.monotonic()
    try:
        done = subprocess.run(  # noqa: S603 - docker with fixed arguments
            ["docker", *argv], capture_output=True, timeout=bound, check=False
        )
        receipt: dict[str, Any] = {
            "returncode": done.returncode,
            "stdout": done.stdout.decode("utf-8", "backslashreplace"),
            "stderr": done.stderr.decode("utf-8", "backslashreplace"),
        }
    except subprocess.TimeoutExpired:
        receipt = {"returncode": None, "bound_exceeded": True}
    receipt["argv"] = argv
    receipt["seconds"] = time.monotonic() - started
    return receipt


def container_argv(image: str, run: str, name: str, limited: bool) -> list[str]:
    argv = ["run", "-d", "--rm", "--name", name, "--label", f"{LABEL}={run}"]
    if limited:
        argv += ["-u", LIMITED_UID]
    else:
        argv += ["--init", "-u", NORMAL_UID]
    return [
        *argv,
        "-p",
        f"127.0.0.1::{AGENT_PORT}",
        "-v",
        f"{AGENT_DIR}:/agent:ro",
        "-v",
        f"{STUB_DIR}:/stub:ro",
        "-v",
        f"{HERE}:/seam:ro",
        "-e",
        "PYTHONPATH=/stub:/agent",
        "-e",
        "PYTHONDONTWRITEBYTECODE=1",
        image,
        "python3",
        "-m",
        "testanyware_agent",
        "--port",
        str(AGENT_PORT),
    ]


def observe_in(name: str) -> dict[str, Any]:
    receipt = docker(
        ["exec", "-u", "0", name, "python3", "-E", "-s", "-B", SELF, "observe"],
        OBSERVE_BOUND,
    )
    try:
        return dict(json.loads(receipt["stdout"])) | {"host_ns": now_ns()}
    except (KeyError, ValueError):
        return {"unknown": receipt}


def kill_in(name: str) -> Callable[[list[tuple[int, int]]], list[dict[str, Any]]]:
    def kill(targets: list[tuple[int, int]]) -> list[dict[str, Any]]:
        argv = [
            "exec",
            "-u",
            "0",
            name,
            "python3",
            "-E",
            "-s",
            "-B",
            SELF,
            "kill",
        ]
        receipt = docker(argv + [f"{pid}:{ticks}" for pid, ticks in targets], CLEANUP_BOUND)
        try:
            return list(json.loads(receipt["stdout"]))
        except (KeyError, ValueError):
            return [{"unknown": receipt}]

    return kill


def read_child_in(name: str) -> Callable[[], str | None]:
    def read() -> str | None:
        receipt = docker(["exec", "-u", "0", name, "cat", CHILD_PID_FILE], OBSERVE_BOUND)
        return receipt["stdout"] if receipt["returncode"] == 0 else None

    return read


def wait_health(port: int) -> bool:
    """The agent answers GET /health. The docker proxy accepts TCP before that."""
    deadline = time.monotonic() + READY_BOUND
    while time.monotonic() < deadline:
        receipt = subprocess.run(  # noqa: S603 - curl with fixed arguments
            ["curl", "-s", "-m", "1", f"http://127.0.0.1:{port}/health"],
            capture_output=True,
            check=False,
        )
        if receipt.returncode == 0 and b'"platform": "linux"' in receipt.stdout:
            return True
        time.sleep(0.1)
    return False


def limit_here() -> dict[str, Any]:
    """Set `RLIMIT_NPROC` to 1 on PID 1 when PID 1 is the agent and has this uid."""
    row = next((row for row in proc_rows() if row["pid"] == 1), None)
    report: dict[str, Any] = {"pid1": row}
    if row is None or row["argv"][1:3] != ["-m", "testanyware_agent"]:
        report["applied"] = False
        return report
    prlimit: Any = getattr(resource, "prlimit")  # noqa: B009 - Linux only; pyright reads macOS
    report["before"] = list(prlimit(1, resource.RLIMIT_NPROC, (1, 1)))
    report["after"] = list(prlimit(1, resource.RLIMIT_NPROC))
    report["applied"] = report["after"] == [1, 1]
    return report


def write_spec(state: Path, vm: str, port: int) -> None:
    spec = {
        "platform": "linux",
        "agent": {"host": "127.0.0.1", "port": port},
        "vnc": {"host": "127.0.0.1", "port": 1},
    }
    (state / "vms" / f"{vm}.json").write_text(json.dumps(spec))


def start_agents(image: str, run: str, result: dict[str, Any]) -> dict[str, dict[str, Any]]:
    agents: dict[str, dict[str, Any]] = {}
    for seam in HOST_SEAMS:
        for index, case in enumerate(cases(seam, run)):
            name = f"k5-{run}-{seam}-{index}"
            limited = case.end == "spawn-failure"
            agent: dict[str, Any] = {"name": name, "seam": seam, "case": case.name}
            agent["run"] = docker(container_argv(image, run, name, limited), OBSERVE_BOUND)
            agents[name] = agent
            result["agents"].append(agent)
            if agent["run"]["returncode"] != 0:
                return agents
            port_receipt = docker(["port", name, str(AGENT_PORT)], OBSERVE_BOUND)
            agent["port_receipt"] = port_receipt
            ports = port_receipt.get("stdout", "").split()
            if not ports:
                return agents
            agent["port"] = int(ports[0].rsplit(":", 1)[1])
            agent["ready"] = wait_health(agent["port"])
            if agent["ready"] and limited:
                receipt = docker(
                    ["exec", "-u", LIMITED_UID, name, "python3", "-E", "-s", "-B", SELF, "limit"],
                    OBSERVE_BOUND,
                )
                agent["limit"] = receipt
                agent["ready"] = '"applied": true' in receipt.get("stdout", "")
            if not agent["ready"]:
                return agents
    return agents


def host_case(
    seam: str, case: Case, agent: dict[str, Any], cli: str, cli_env: dict[str, str]
) -> dict[str, Any]:
    name = agent["name"]
    limited = case.end == "spawn-failure"
    if not limited:
        agent["setup"] = docker(["exec", "-u", NORMAL_UID, name, "mkdir", "-p", *ISSUE_DIRS], 30)
    baseline = observe_in(name)
    if seam == "http":
        record = http_case(agent["port"], case)
    else:
        record = cli_case(cli, cli_env, f"{seam}-{agent['index']}", case, seam == "cli-json")
    record["container"] = name
    record["instrumentation"] = "rlimit-nproc" if limited else None
    lifecycle(
        case,
        record,
        baseline,
        lambda: observe_in(name),
        kill_in(name),
        now_ns,  # the clock of http_case and cli_case
        read_child_in(name),
    )
    return record


def host(run: str, image: str, app: str, cli: str) -> dict[str, Any]:
    """Run the host seams. Every container and the root go, whatever happens."""
    result: dict[str, Any] = {"seam": "host", "image": image, "agents": [], "stops": []}
    servers: list[Owned] = []
    agents: dict[str, dict[str, Any]] = {}
    root = Path(tempfile.mkdtemp(prefix="k5", dir="/tmp"))  # a socket path has 103 bytes at most
    result["root"] = str(root)
    try:
        agents = start_agents(image, run, result)
        if not all(agent.get("ready") for agent in agents.values()) or len(agents) != 33:
            result["error"] = "an agent container did not become ready"
            return result
        state, data = root / "xdg-state" / "testanyware", root / "xdg-data" / "testanyware"
        tools, tmp, sock_dir = root / "tools", root / "tmp", root / "run"
        for directory in (state / "vms", data, tools, tmp, sock_dir):
            directory.mkdir(parents=True)
        sock_dir.chmod(0o700)
        sock = sock_dir / "testanyware.sock"
        for agent in agents.values():
            agent["index"] = int(agent["name"].rsplit("-", 1)[1])
            if agent["seam"] != "http":
                write_spec(state, f"{agent['seam']}-{agent['index']}", agent["port"])
        base = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
        app_argv = [app, "--test-mode", "--socket", str(sock), "--state-dir", str(state)]
        app_argv += ["--data-dir", str(data), "--tool-dir", str(tools)]
        server = Owned("app", app_argv, {**base, "TMPDIR": str(tmp)})
        servers.append(server)
        ready = wait_connect(unix_connect(sock), server.child)
        result["app"] = {"argv": app_argv, "pid": server.child.pid, "start": server.start}
        result["app"]["ready"] = ready
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
        for seam in HOST_SEAMS:
            records: list[dict[str, Any]] = []
            for index, case in enumerate(cases(seam, run)):
                agent = agents[f"k5-{run}-{seam}-{index}"]
                records.append(host_case(seam, case, agent, cli, cli_env))
            result[seam] = records
        return result
    finally:
        for server in reversed(servers):
            result["stops"].append(server.stop())
        names = [agent["name"] for agent in agents.values()]
        if names:
            result["disposal"] = docker(["rm", "-f", *names], DISPOSAL_BOUND)
        listing = docker(["ps", "-a", "-q", "--filter", f"label={LABEL}={run}"], OBSERVE_BOUND)
        result["containers_after"] = listing
        shutil.rmtree(root, ignore_errors=True)
        result["root_removed"] = not root.exists()


def main(argv: list[str]) -> int:
    if argv[1:2] == ["observe"] and len(argv) == 2:
        result: Any = observe_here()
    elif argv[1:2] == ["kill"]:
        targets = [tuple(int(part) for part in item.split(":")) for item in argv[2:]]
        result = kill_here([(pid, ticks) for pid, ticks in targets])
    elif argv[1:] == ["limit"]:
        result = limit_here()
    elif (
        len(argv) == 4
        and RUN_TAG.match(argv[1])
        and argv[2] == "inside"
        and argv[3] in INSIDE_SEAMS
    ):
        result = inside(argv[1], argv[3])
    elif len(argv) == 6 and RUN_TAG.match(argv[1]) and argv[2] == "host":
        result = host(argv[1], argv[3], argv[4], argv[5])
    else:
        print(__doc__, file=sys.stderr)
        return 64
    sys.stdout.write(json.dumps(result, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
