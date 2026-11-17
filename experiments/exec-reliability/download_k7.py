"""Download consistency cases for leaf download-consistency-k7.

Usage: download_k7.py OUT AGENT [AGENT ...]

Each AGENT is LABEL=PATH. The cases run against each agent binary on the host. A fresh
agent process serves each case on a loopback port. download_k7_shim.c is loaded with
DYLD_INSERT_LIBRARIES. It traces stat, lstat, open, pread and close below the case
directory, pauses one armed call, and binds the agent to loopback.

A calibration download finds the path lookup that supplies the size. It is the last stat
or lstat before the open. The forced cases pause after that lookup or before a pread.
Then they change the file and resume. The raw client records the advertised framing and
the received bytes. The Host CLI records its exit, receipt, destination and temporary
files. The stress cases use natural timing and no pause.

The controller judges nothing. download_k7_check.py classifies the retained evidence.
"""

from __future__ import annotations

import hashlib
import json
import os
import queue
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
SHIM_SOURCE = HERE / "download_k7_shim.c"
CLI = Path(shutil.which("testanyware") or "/opt/homebrew/bin/testanyware").resolve()
HISTORICAL = Path(
    "/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1/"
    "experiments/tutorial-vm-reliability/evidence/macos-candidate-02"
)
HISTORICAL_SHA256 = {
    "after-abrupt-03-initial.json": (
        "b95c9261fb8ff0ca68a9d8b06ebdcb4102a72f60b670c2d748cdf4eca454e7a4"
    ),
    "after-abrupt-03-state-002.json": (
        "ca41852314b967a327b0a4faf0240cac2f2e304387439e5518cf380da4b9a56e"
    ),
    "after-abrupt-03-collect-002.json": (
        "6645b2e4b55f2b89a7e635bfa6bd17074f538c2978606e01852ce049cc3740a3"
    ),
    "after-abrupt-03-launch.json": (
        "eb9c539fccdae4d746627b159d754845a725782efe37d548b61f57de555338b7"
    ),
}
ORACLE = bytes.fromhex("00ff80410a")
CHUNK = 128 * 1024
BIG = 3 * CHUNK + 1000
RAW_TIMEOUT = 20.0
CLI_TIMEOUT = 150.0
PAUSE_TIMEOUT = 20.0
STRESS_SECONDS = 10.0
STRESS_PUBLICATIONS = 200
# Root brief: termination grace at most 2 s, then kill, drain and reap at most 2 s.
GRACE = 2.0
REAP = 2.0
INSPECT_TIMEOUT = 30.0  # ordinary observation ceiling for lsof and ps

Json = dict[str, Any]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def pattern(seed: int, size: int) -> bytes:
    """Distinct non-repeating bytes, so a torn body names the offsets of each version."""
    out = bytearray()
    counter = 0
    while len(out) < size:
        out += hashlib.sha256(f"{seed}:{counter}".encode()).digest()
        counter += 1
    return bytes(out[:size])


def terminal_record() -> bytes:
    """A terminal record in the shape of fixture/r2/operation.py (json.dumps + newline)."""
    record = {
        "state": "terminal",
        "reason": "exit",
        "exit_code": 0,
        "process_returncode": 0,
        "stdout_hex": "6f6b0a",
        "stderr_hex": "",
    }
    return (json.dumps(record) + "\n").encode()


def replace(path: Path, data: bytes) -> None:
    """Write a sibling temporary file and rename it over path, as the guest writer does."""
    tmp = path.with_suffix(".tmp")
    tmp.write_bytes(data)
    tmp.replace(path)


def overwrite_in_place(path: Path, data: bytes) -> None:
    fd = os.open(path, os.O_WRONLY)
    try:
        os.pwrite(fd, data, 0)
    finally:
        os.close(fd)


def append_in_place(path: Path, data: bytes) -> None:
    with path.open("ab") as handle:
        handle.write(data)


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


@dataclass
class Agent:
    label: str
    binary: Path
    shim: Path
    root: Path
    registry: Path
    trace: bool = True
    port: int = 0
    process: subprocess.Popen[bytes] | None = None
    record: Json = field(default_factory=lambda: {})

    @property
    def target(self) -> Path:
        return self.root / "t"

    @property
    def gate(self) -> Path:
        return self.root / "g"

    def start(self) -> None:
        self.target.mkdir(parents=True, exist_ok=True)
        self.gate.mkdir(parents=True, exist_ok=True)
        self.port = free_port()
        env = {k: v for k, v in os.environ.items() if not k.startswith(("DYLD_", "K7_"))}
        env |= {
            "DYLD_INSERT_LIBRARIES": str(self.shim),
            "K7_TARGET_DIR": str(self.target),
            "K7_GATE": str(self.gate),
            "K7_LOOPBACK": "1",
        }
        if self.trace:
            env["K7_TRACE"] = str(self.root / "trace.jsonl")
        log = (self.root / "agent.log").open("wb")
        self.process = subprocess.Popen(
            [str(self.binary), str(self.port)],
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=subprocess.STDOUT,
            env=env,
            start_new_session=True,
        )
        log.close()
        # The wrapper reads this registry to find and end a surviving agent by identity.
        with self.registry.open("a") as handle:
            handle.write(f"{self.process.pid} {self.binary}\n")
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                break
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=0.5):
                    break
            except OSError:
                time.sleep(0.05)
        listen = inspect(
            ["lsof", "-nP", "-a", "-p", str(self.process.pid), "-iTCP", "-sTCP:LISTEN"]
        )[1]
        self.record = {
            "label": self.label,
            "pid": self.process.pid,
            "port": self.port,
            "listen": listen,
            "loopback_only": f"127.0.0.1:{self.port}" in listen
            and "*:" not in listen
            and "0.0.0.0" not in listen,
        }
        if not self.record["loopback_only"]:
            self.stop()
            raise RuntimeError(f"agent did not listen on loopback only: {listen!r}")

    def stop(self, sig: int = signal.SIGTERM) -> None:
        if self.process is None:
            return
        # The first stop names the signal. A later stop of an ended agent keeps it.
        self.record.setdefault("stopped_by", signal.Signals(sig).name)
        if self.process.poll() is None:
            self.record["reaped"] = end_process(self.process, sig)[0]
        self.record["returncode"] = self.process.returncode
        self.record["alive_after_stop"] = pid_alive(self.process.pid)

    def arm(self, event: str, nth: int, generation: str) -> None:
        (self.gate / "arm").write_text(f"{event} {nth} {generation}\n")

    def wait_paused(self) -> str:
        deadline = time.monotonic() + PAUSE_TIMEOUT
        paused = self.gate / "paused"
        while time.monotonic() < deadline:
            if paused.exists():
                return paused.read_text()
            time.sleep(0.01)
        raise TimeoutError("the armed call did not pause")

    def resume(self) -> None:
        (self.gate / "resume").write_text("go\n")


def inspect(argv: list[str]) -> tuple[int | None, str]:
    """Run one read-only inspection command under the observation ceiling. A timeout or
    a start failure gives the exit None, so the caller cannot read it as absence."""
    try:
        done = subprocess.run(argv, capture_output=True, text=True, timeout=INSPECT_TIMEOUT)
    except (OSError, subprocess.TimeoutExpired):
        return None, ""
    return done.returncode, done.stdout


def pid_alive(pid: int) -> bool | None:
    """True or False only when ps answers plainly. Any other answer is unknown (None)."""
    code, out = inspect(["ps", "-p", str(pid), "-o", "pid="])
    if code == 0 and out.strip() == str(pid):
        return True
    if code == 1 and not out.strip():
        return False
    return None


def end_process(
    process: subprocess.Popen[bytes], sig: int = signal.SIGTERM
) -> tuple[str, bytes, bytes]:
    """Send sig, allow GRACE, then SIGKILL and allow REAP. Return how the process ended
    (by the first signal, by SIGKILL, or unreaped inside the bounds) and its streams."""
    process.send_signal(sig)
    try:
        out, err = process.communicate(timeout=GRACE)
        return signal.Signals(sig).name, out or b"", err or b""
    except subprocess.TimeoutExpired:
        pass
    process.kill()
    try:
        out, err = process.communicate(timeout=REAP)
        return "SIGKILL", out or b"", err or b""
    except subprocess.TimeoutExpired:
        return "unreaped", b"", b""


def raw_download(port: int, path: Path, timeout: float = RAW_TIMEOUT) -> tuple[Json, bytes]:
    """POST /download with one connection. Read the body to Content-Length, then wait
    briefly for extra bytes or the close. Nothing is retried. The timeout bounds the
    whole download, not each receive, so a peer that trickles bytes cannot extend it."""
    query = "path=" + "".join(
        ch if ch.isalnum() or ch in "/-_." else f"%{ord(ch):02X}" for ch in str(path)
    )
    request = (
        f"POST /download?{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n"
        "Content-Length: 0\r\nConnection: close\r\n\r\n"
    ).encode()
    started = time.monotonic()
    deadline = started + timeout
    data = b""
    end = "complete"
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as conn:

        def receive() -> bytes:
            left = deadline - time.monotonic()
            if left <= 0:
                raise TimeoutError("download deadline")
            conn.settimeout(left)
            return conn.recv(65536)

        conn.sendall(request)
        while b"\r\n\r\n" not in data:
            piece = receive()
            if not piece:
                break
            data += piece
        head, _, body = data.partition(b"\r\n\r\n")
        lines = head.decode("latin-1").split("\r\n")
        status = int(lines[0].split()[1]) if lines and len(lines[0].split()) > 1 else 0
        headers = {
            k.strip().lower(): v.strip() for k, _, v in (x.partition(":") for x in lines[1:])
        }
        length = int(headers["content-length"]) if "content-length" in headers else None
        try:
            while length is None or len(body) < length:
                piece = receive()
                if not piece:
                    end = "eof-before-length" if length is not None else "eof"
                    break
                body += piece
        except TimeoutError:
            end = "timeout-before-length"
        if end == "complete":
            conn.settimeout(min(0.5, max(deadline - time.monotonic(), 0.001)))
            try:
                extra = conn.recv(65536)
                end = "complete-extra-bytes" if extra else "complete"
                body += extra
            except TimeoutError:
                end = "complete-open"
        # Bytes past Content-Length can arrive in the same segment as the headers.
        if length is not None and len(body) > length:
            end = "complete-extra-bytes"
    result: Json = {
        "status": status,
        "headers": headers,
        "content_length": length,
        "received": len(body),
        "sha256": sha256(body),
        "end": end,
        "seconds": round(time.monotonic() - started, 4),
    }
    return result, body


def leftover_temporaries(directory: Path) -> list[str]:
    return sorted(p.name for p in directory.iterdir() if ".testanyware-" in p.name)


def cli_download(port: int, remote: Path, local: Path) -> Json:
    env = {k: v for k, v in os.environ.items() if k != "TESTANYWARE_APP_SOCKET"}
    argv = [str(CLI), "file", "download", "--agent", f"127.0.0.1:{port}"]
    argv += [str(remote), str(local), "--json"]
    started = time.monotonic()
    try:
        done = subprocess.run(argv, capture_output=True, timeout=CLI_TIMEOUT, env=env)
        code: int | None = done.returncode
        out, err, killed = done.stdout, done.stderr, False
    except subprocess.TimeoutExpired as expired:
        code, killed = None, True
        out = expired.stdout or b""
        err = expired.stderr or b""
    result: Json = {
        "argv": argv,
        "exit": code,
        "killed_at_bound": killed,
        "seconds": round(time.monotonic() - started, 4),
        "stdout": out.decode(errors="replace"),
        "stderr": err.decode(errors="replace"),
        "destination_exists": local.exists(),
        "temporaries": leftover_temporaries(local.parent),
    }
    if local.exists():
        body = local.read_bytes()
        result |= {"destination_bytes": len(body), "destination_sha256": sha256(body)}
    return result


@dataclass
class Case:
    name: str
    initial: bytes
    allowed: list[bytes]
    pause: str  # "size", "pread:N" or "none"
    change: Callable[[Path], None] | None
    kill_agent: bool = False
    kill_cli: bool = False


def cases() -> list[Case]:
    pending = (HISTORICAL / "after-abrupt-03-initial.json").read_bytes()
    terminal = terminal_record()
    big_a, big_b = pattern(1, BIG), pattern(2, BIG)
    return [
        Case(
            "replace-longer", pending, [pending, terminal], "size", lambda p: replace(p, terminal)
        ),
        Case(
            "replace-shorter", terminal, [terminal, pending], "size", lambda p: replace(p, pending)
        ),
        Case(
            "truncate-during-read",
            big_a,
            [big_a, big_a[: CHUNK // 2]],
            "pread:2",
            lambda p: os.truncate(p, CHUNK // 2),
        ),
        Case(
            "overwrite-during-read",
            big_a,
            [big_a, big_b],
            "pread:2",
            lambda p: overwrite_in_place(p, big_b),
        ),
        Case(
            "append-during-read",
            big_a,
            [big_a, big_a + big_b[:CHUNK]],
            "pread:2",
            lambda p: append_in_place(p, big_b[:CHUNK]),
        ),
        Case("immutable-binary", ORACLE, [ORACLE], "none", None),
        Case("agent-killed-mid-body", big_a, [big_a], "pread:2", None, kill_agent=True),
        Case("cli-killed-mid-body", big_a, [big_a], "pread:2", None, kill_cli=True),
    ]


def calibrate(agent: Agent) -> Json:
    """Find the path lookup that supplies Content-Length: the last stat or lstat before
    the open, with its ordinal among calls of the same kind."""
    path = agent.target / "calibrate.bin"
    path.write_bytes(b"calibrate")
    result, _ = raw_download(agent.port, path)
    events = [json.loads(x) for x in (agent.root / "trace.jsonl").read_text().splitlines()]
    events = [e for e in events if e.get("path") == str(path)]
    names = [str(e["event"]) for e in events]
    opened = names.index("open")
    size_event = next(n for n in reversed(names[:opened]) if n in ("stat", "lstat"))
    ordinal = names[:opened].count(size_event)
    return {"events": names, "size_event": size_event, "ordinal": ordinal, "raw": result}


def run_case(agent: Agent, case: Case, client: str, calibration: Json) -> Json:
    path = agent.target / f"{case.name}.bin"
    path.write_bytes(case.initial)
    (agent.root / "initial.bin").write_bytes(case.initial)
    for index, version in enumerate(case.allowed):
        (agent.root / f"allowed-{index}.bin").write_bytes(version)
    record: Json = {"case": case.name, "client": client, "pause": case.pause}
    if case.pause == "size":
        agent.arm(str(calibration["size_event"]), int(calibration["ordinal"]), case.name)
    elif case.pause.startswith("pread:"):
        agent.arm("pread", int(case.pause.split(":")[1]), case.name)
    results: queue.Queue[Json] = queue.Queue()
    local = agent.root / "local" / f"{case.name}.bin"
    local.parent.mkdir(exist_ok=True)

    def raw_client() -> None:
        try:
            result, body = raw_download(agent.port, path)
            (agent.root / "received.bin").write_bytes(body)
            results.put(result)
        except OSError as error:
            results.put({"error": repr(error)})

    cli_process: subprocess.Popen[bytes] | None = None
    if client == "raw":
        worker = threading.Thread(target=raw_client, daemon=True)
        worker.start()
    else:
        worker = threading.Thread(
            target=lambda: results.put(cli_download(agent.port, path, local)), daemon=True
        )
        if case.kill_cli:
            env = {k: v for k, v in os.environ.items() if k != "TESTANYWARE_APP_SOCKET"}
            argv = [str(CLI), "file", "download", "--agent", f"127.0.0.1:{agent.port}"]
            cli_process = subprocess.Popen(
                [*argv, str(path), str(local), "--json"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=env,
            )
        else:
            worker.start()
    try:
        if case.pause != "none":
            record["paused"] = agent.wait_paused()
            if case.change is not None:
                case.change(path)
                record["changed_sha256"] = sha256(path.read_bytes())
            if case.kill_agent:
                agent.stop(signal.SIGKILL)
            if cli_process is not None:
                temporaries = leftover_temporaries(local.parent)
                ended, out, err = end_process(cli_process)
                record["cli_killed"] = {
                    "signal": "SIGTERM",
                    "ended_by": ended,
                    "temporaries_while_paused": temporaries,
                    "exit": cli_process.returncode,
                    "stdout": out.decode(errors="replace"),
                    "stderr": err.decode(errors="replace"),
                    "destination_exists": local.exists(),
                    "temporaries_after": leftover_temporaries(local.parent),
                }
            if not case.kill_agent:
                agent.resume()
    finally:
        # A failure before the planned interruption must not leave the CLI running.
        if cli_process is not None and cli_process.poll() is None:
            record["cli_cleanup"] = end_process(cli_process, signal.SIGKILL)[0]
    if cli_process is None:
        worker.join(timeout=CLI_TIMEOUT + 30)
        try:
            record["result"] = results.get_nowait()
        except queue.Empty:
            record["result"] = {"error": "worker alive at the bound"}
    record["file_after_sha256"] = sha256(path.read_bytes()) if path.exists() else None
    return record


def stress_mutable(agent: Agent, seconds: float) -> Json:
    """Natural timing: a writer swaps the pending and terminal records by rename while the
    raw client reads the path again and again. Every response is retained by digest."""
    pending = (HISTORICAL / "after-abrupt-03-initial.json").read_bytes()
    terminal = terminal_record()
    path = agent.target / "stress.json"
    path.write_bytes(pending)
    stop = threading.Event()
    swaps = 0

    def writer() -> None:
        nonlocal swaps
        while not stop.is_set():
            replace(path, terminal if swaps % 2 == 0 else pending)
            swaps += 1

    thread = threading.Thread(target=writer, daemon=True)
    thread.start()
    outcomes: dict[str, Json] = {}
    deadline = time.monotonic() + seconds
    reads = 0
    error: str | None = None
    try:
        while time.monotonic() < deadline:
            result, body = raw_download(agent.port, path)
            reads += 1
            key = f"{result['status']}:{result['content_length']}:{result['end']}:{sha256(body)}"
            entry = outcomes.setdefault(key, {"count": 0, "body_hex": body.hex()[:400]} | result)
            entry["count"] += 1
    except OSError as failure:
        error = repr(failure)
    finally:
        stop.set()
        thread.join(timeout=GRACE)
    record: Json = {
        "reads": reads,
        "swaps": swaps,
        "versions": {sha256(pending): "pending", sha256(terminal): "terminal"},
        "outcomes": outcomes,
        "writer_alive": thread.is_alive(),
    }
    if error is not None:
        record["error"] = error
    return record


def stress_immutable(agent: Agent, publications: int) -> Json:
    """Natural timing for a publication that never rebinds a name: each record is written
    to a temporary file and renamed to a fresh name once. The raw client polls the next
    name until it answers 200. A 404 before publication is expected."""
    names = agent.target / "published"
    names.mkdir()
    outcomes: dict[str, int] = {}
    mismatches: list[Json] = []
    for index in range(publications):
        data = terminal_record() + f"{index}\n".encode()
        final = names / f"record-{index:04}.json"
        timer = threading.Timer(0.002 * (index % 5), replace, args=(final, data))
        timer.daemon = True
        timer.start()
        deadline = time.monotonic() + 5
        try:
            while time.monotonic() < deadline:
                result, body = raw_download(agent.port, final)
                if result["status"] == 200:
                    good = body == data and result["end"] == "complete"
                    kind = "complete" if good else "mismatch"
                    outcomes[kind] = outcomes.get(kind, 0) + 1
                    if kind == "mismatch":
                        mismatches.append(result | {"body_hex": body.hex()[:400]})
                    break
                outcomes[str(result["status"])] = outcomes.get(str(result["status"]), 0) + 1
        except OSError as failure:
            timer.join(timeout=GRACE)
            return {
                "publications": publications,
                "outcomes": outcomes,
                "mismatches": mismatches,
                "error": f"publication {index}: {failure!r}",
            }
        timer.join(timeout=GRACE)
    return {"publications": publications, "outcomes": outcomes, "mismatches": mismatches}


def historical_digests() -> dict[str, str | None]:
    return {
        name: sha256((HISTORICAL / name).read_bytes()) if (HISTORICAL / name).exists() else None
        for name in HISTORICAL_SHA256
    }


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 64
    out = Path(argv[1])
    out.mkdir(parents=True)
    agents = {label: Path(path) for label, _, path in (a.partition("=") for a in argv[2:])}
    registry = out / "agents.pids"
    registry.touch()
    shim = out / "shim.dylib"
    subprocess.run(
        ["clang", "-Wall", "-Wextra", "-O1", "-dynamiclib", "-o", str(shim), str(SHIM_SOURCE)],
        check=True,
    )
    meta: Json = {
        "started_unix": time.time(),
        "cli": {"path": str(CLI), "sha256": sha256(CLI.read_bytes())},
        "shim": {
            "source_sha256": sha256(SHIM_SOURCE.read_bytes()),
            "sha256": sha256(shim.read_bytes()),
        },
        "agents": {
            k: {"path": str(v), "sha256": sha256(v.read_bytes())} for k, v in agents.items()
        },
        "historical_before": historical_digests(),
        "bounds": {
            "raw_s": RAW_TIMEOUT,
            "cli_s": CLI_TIMEOUT,
            "pause_s": PAUSE_TIMEOUT,
            "grace_s": GRACE,
            "reap_s": REAP,
            "inspect_s": INSPECT_TIMEOUT,
        },
    }
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    records: list[Json] = []
    for label, binary in agents.items():
        root = out / label
        probe = Agent(label, binary, shim, root / "calibration", registry)
        probe.start()
        try:
            calibration = calibrate(probe)
        finally:
            probe.stop()
        calibration["agent"] = probe.record
        (root / "calibration" / "calibration.json").write_text(json.dumps(calibration, indent=2))
        for case in cases():
            for client in ("raw", "cli"):
                if case.kill_cli and client == "raw":
                    continue
                agent = Agent(label, binary, shim, root / f"{case.name}-{client}", registry)
                agent.start()
                record: Json
                try:
                    record = run_case(agent, case, client, calibration)
                except Exception as error:  # retained as evidence, never retried
                    record = {"case": case.name, "client": client, "error": repr(error)}
                finally:
                    agent.stop()
                record["agent"] = agent.record
                (agent.root / "record.json").write_text(json.dumps(record, indent=2))
                records.append(record)
                print(label, case.name, client, json.dumps(record.get("result", record))[:300])
        stress: list[tuple[str, Callable[[Agent], Json]]] = [
            ("stress-mutable", lambda a: stress_mutable(a, STRESS_SECONDS)),
            ("stress-immutable", lambda a: stress_immutable(a, STRESS_PUBLICATIONS)),
        ]
        for name, run in stress:
            agent = Agent(label, binary, shim, root / name, registry, trace=False)
            agent.start()
            try:
                record = {"case": name} | run(agent)
            except Exception as error:  # retained as evidence, never retried
                record = {"case": name, "error": repr(error)}
            finally:
                agent.stop()
            record["agent"] = agent.record
            (agent.root / "record.json").write_text(json.dumps(record, indent=2))
            print(label, name, json.dumps(record)[:300])
    meta["historical_after"] = historical_digests()
    # The controller compiled the shim and read these executables. Hash them again, so
    # the checker can compare each with its first digest.
    meta["after"] = {
        "cli_sha256": sha256(CLI.read_bytes()),
        "shim_sha256": sha256(shim.read_bytes()),
        "agents_sha256": {k: sha256(v.read_bytes()) for k, v in agents.items()},
    }
    meta["finished_unix"] = time.time()
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
