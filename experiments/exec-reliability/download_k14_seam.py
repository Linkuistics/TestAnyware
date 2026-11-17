"""In-container seam for leaf linux-download-consistency-k14. Standard library only.

    download_k14_seam.py supervisor CONTROL_PORT CASE_PORT PLAIN_PORT
    download_k14_seam.py agent PORT KIND RUN_DIR
    download_k14_seam.py memory-control

The container mounts the Linux agent from this checkout at /agent, the k5 desktop stubs
at /stub and this directory at /seam. Case files live under /tmp/k14/work, on the container
file system, never on a bind mount.

`supervisor` serves JSON operations on CONTROL_PORT for the host controller. It starts
two agents. The `instrumented` agent on CASE_PORT serves the forced cases. The
`unchanged` agent on PLAIN_PORT runs the unchanged `run_server` with no hook. It
serves the stress reads and the immutable publication control.

The `instrumented` agent runs the unchanged `AgentRequestHandler` and the unchanged
`handle_download` with three observers at module boundaries:

- `system_endpoints.open` records the identity of each opened file and wraps the file.
  Each read of the wrapper records its length, digest, offset and the identity of the
  descriptor and of the path.
- `server.os` forwards every attribute to `os` except `fstat`, which records the size
  that becomes Content-Length.
- A handler subclass records each write to the socket. The first write holds the
  status line and headers. The others hold the body.

An armed hook runs at one event: `after-open`, `after-fstat` or `before-read-N`. It
changes the file at that point inside the agent process, or it pauses until the
host resumes it. The trace goes to RUN_DIR/trace.jsonl, one JSON object a line.

At the start of each request the `instrumented` agent writes 5 to /proc/self/clear_refs.
That resets VmHWM, the peak resident set, to the current resident set. The request line
and the finish line record VmRSS and VmHWM, so the rise in VmHWM belongs to one request.
`ru_maxrss` stays in the trace for comparison only, because it is a lifetime peak.
`memory-control` runs in a fresh process. It shows that the reset hides an earlier peak
and that a later 32 MiB allocation raises VmHWM.
"""

from __future__ import annotations

import hashlib
import importlib
import json
import os
import platform
import resource
import signal
import subprocess
import sys
import threading
import time
from collections.abc import Callable
from http.server import BaseHTTPRequestHandler, HTTPServer, ThreadingHTTPServer
from pathlib import Path
from typing import Any

WORK = Path("/tmp/k14/work")
RUN = Path("/tmp/k14/run")
CHUNK = 1024 * 1024
PAUSE_LIMIT = 60.0  # a pause that the host never resumes ends here and is traced
POLL = 0.002
READY_LIMIT = 20.0
GRACE = 2.0  # root brief: termination grace at most 2 s, then kill and reap at most 2 s
Json = dict[str, Any]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def memory_kib() -> Json:
    """VmRSS and VmHWM of this process, in KiB, from /proc/self/status."""
    values: Json = {}
    for line in Path("/proc/self/status").read_text().splitlines():
        name, _, rest = line.partition(":")
        if name in ("VmRSS", "VmHWM"):
            values[name] = int(rest.split()[0])
    return values


def reset_peak() -> str | None:
    """Reset VmHWM to VmRSS. None means success, else the error."""
    try:
        Path("/proc/self/clear_refs").write_text("5")
    except OSError as failure:
        return repr(failure)
    return None


def memory_control() -> Json:
    """An earlier 96 MiB peak, then the reset, then a 32 MiB allocation. Each step writes
    every byte, so that the pages are resident."""
    earlier = b"\x01" * (96 * 1024 * 1024)
    del earlier
    before = memory_kib()
    error = reset_peak()
    after = memory_kib()
    later = b"\x01" * (32 * 1024 * 1024)
    grown = memory_kib()
    del later
    return {
        "before_reset": before,
        "reset_error": error,
        "after_reset": after,
        "after_32mib": grown,
    }


def identity(stat: os.stat_result) -> Json:
    return {"dev": stat.st_dev, "ino": stat.st_ino, "size": stat.st_size, "nlink": stat.st_nlink}


def path_identity(path: str) -> Json:
    try:
        return identity(os.stat(path))
    except OSError as failure:
        return {"error": repr(failure)}


def apply(path: str, action: Json) -> Json:
    """Change the file as one writer would. Return what changed."""
    op = action["op"]
    if "pattern" in action:
        data = pattern(int(action["pattern"][0]), int(action["pattern"][1]))
    else:
        data = bytes.fromhex(action.get("data_hex", ""))
    if op == "replace":
        tmp = path + ".tmp"
        with open(tmp, "wb") as handle:
            handle.write(data)
        os.rename(tmp, path)
    elif op == "truncate":
        os.truncate(path, int(action["size"]))
    elif op == "append":
        with open(path, "ab") as handle:
            handle.write(data)
    elif op == "overwrite":
        fd = os.open(path, os.O_WRONLY)
        try:
            os.pwrite(fd, data, 0)
        finally:
            os.close(fd)
    else:
        raise ValueError(f"unknown action {op}")
    return {"op": op, "after": path_identity(path)}


# ---------------------------------------------------------------- agent


class Tracer:
    def __init__(self, run_dir: Path) -> None:
        self.run_dir = run_dir
        self.lock = threading.Lock()
        # A restarted agent continues the numbering, so emitted-N.bin names stay unique.
        numbers = [int(p.stem.split("-")[1]) for p in run_dir.glob("emitted-*.bin")]
        self.request = max(numbers, default=0)

    def emit(self, kind: str, **fields: Any) -> None:
        line = {"t_ns": time.monotonic_ns(), "request": self.request, "kind": kind} | fields
        with self.lock, (self.run_dir / "trace.jsonl").open("a") as handle:
            handle.write(json.dumps(line) + "\n")

    def hook(self, event: str, path: str) -> None:
        """Run the armed action when the event matches. The arm is used once."""
        arm_file = self.run_dir / "arm.json"
        if not arm_file.exists():
            return
        arm = json.loads(arm_file.read_text())
        if arm.get("event") != event:
            return
        arm_file.rename(self.run_dir / f"arm-used-{self.request}.json")
        action: Json = arm["action"]
        if action["op"] == "pause":
            (self.run_dir / "paused").write_text(event)
            self.emit("paused", event=event)
            limit = time.monotonic() + PAUSE_LIMIT
            resume = self.run_dir / "resume"
            while not resume.exists() and time.monotonic() < limit:
                time.sleep(POLL)
            resumed = resume.exists()
            for name in ("paused", "resume"):
                (self.run_dir / name).unlink(missing_ok=True)
            self.emit("resumed" if resumed else "pause-expired", event=event)
            return
        try:
            self.emit("action", event=event, result=apply(path, action))
        except OSError as failure:
            self.emit("action-error", event=event, error=repr(failure))


class TracedFile:
    """The file that handle_download opened, with each read traced."""

    def __init__(self, inner: Any, path: str, tracer: Tracer) -> None:
        self.inner = inner
        self.path = path
        self.tracer = tracer
        self.reads = 0

    def fileno(self) -> int:
        return int(self.inner.fileno())

    def read(self, size: int = -1) -> bytes:
        self.reads += 1
        self.tracer.hook(f"before-read-{self.reads}", self.path)
        offset = self.inner.tell()
        data: bytes = self.inner.read(size)
        self.tracer.emit(
            "read",
            n=self.reads,
            asked=size,
            offset=offset,
            length=len(data),
            sha256=sha256(data),
            descriptor=identity(os.fstat(self.inner.fileno())),
            path=path_identity(self.path),
        )
        return data

    def close(self) -> None:
        self.inner.close()
        self.tracer.emit("close", reads=self.reads)

    def __enter__(self) -> TracedFile:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


class Recorder:
    """The socket writer of one handler. It keeps the body bytes it wrote."""

    def __init__(self, inner: Any, tracer: Tracer, keep: Path) -> None:
        self.inner = inner
        self.tracer = tracer
        self.keep = keep
        self.writes = 0
        self.body = hashlib.sha256()
        self.body_bytes = 0

    def write(self, data: bytes) -> int:
        self.writes += 1
        try:
            written: int = self.inner.write(data)
        except OSError as failure:
            self.tracer.emit("write-error", n=self.writes, length=len(data), error=repr(failure))
            raise
        if self.writes == 1:
            self.tracer.emit("headers", text=bytes(data).decode("latin-1"))
        else:
            self.body.update(data)
            self.body_bytes += len(data)
            if self.body_bytes <= 8 * CHUNK:
                with self.keep.open("ab") as handle:
                    handle.write(data)
            self.tracer.emit(
                "write", n=self.writes, length=len(data), written=written, total=self.body_bytes
            )
        return written

    def flush(self) -> None:
        self.inner.flush()

    @property
    def closed(self) -> bool:  # StreamRequestHandler.finish reads it
        return bool(self.inner.closed)

    def close(self) -> None:  # StreamRequestHandler.finish calls it
        self.inner.close()

    def summary(self) -> Json:
        return {
            "writes": self.writes,
            "body_bytes": self.body_bytes,
            "sha256": self.body.hexdigest(),
        }


def instrumented(port: int, run_dir: Path) -> None:
    sys.path[:0] = ["/stub", "/agent"]
    server: Any = importlib.import_module("testanyware_agent.server")
    endpoints: Any = importlib.import_module("testanyware_agent.system_endpoints")
    tracer = Tracer(run_dir)
    real_open = open
    real_fstat = os.fstat

    def traced_open(path: str, mode: str = "r", *args: Any, **kwargs: Any) -> Any:
        handle = real_open(path, mode, *args, **kwargs)
        tracer.emit("open", path=path, mode=mode, descriptor=identity(real_fstat(handle.fileno())))
        tracer.hook("after-open", path)
        return TracedFile(handle, path, tracer)

    class OsView:
        def __getattr__(self, name: str) -> Any:
            return getattr(os, name)

        def fstat(self, fd: int) -> os.stat_result:
            stat = real_fstat(fd)
            tracer.emit("fstat", descriptor=identity(stat))
            path = tracer_paths.get(fd, "")
            tracer.hook("after-fstat", path)
            return stat

    tracer_paths: dict[int, str] = {}

    def remembering_open(path: str, mode: str = "r", *args: Any, **kwargs: Any) -> Any:
        wrapped = traced_open(path, mode, *args, **kwargs)
        tracer_paths[wrapped.fileno()] = path
        return wrapped

    endpoints.open = remembering_open  # shadows the builtin in that module only
    server.os = OsView()

    handler: type[BaseHTTPRequestHandler] = server.AgentRequestHandler

    class Recorded(handler):  # the unchanged handler with a recording writer
        def setup(self) -> None:
            super().setup()
            tracer.request += 1
            reset_error = reset_peak()
            tracer.emit(
                "request",
                maxrss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                peak_reset_error=reset_error,
                memory_kib=memory_kib(),
            )
            keep = run_dir / f"emitted-{tracer.request}.bin"
            self.recorder = Recorder(self.wfile, tracer, keep)
            self.wfile = self.recorder  # type: ignore[assignment]

        def finish(self) -> None:
            try:
                super().finish()
            finally:
                tracer.emit(
                    "finish",
                    emitted=self.recorder.summary(),
                    maxrss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                    memory_kib=memory_kib(),
                )

    class Server(HTTPServer):
        def handle_error(self, request: Any, client_address: Any) -> None:
            tracer.emit("handler-error", error=repr(sys.exc_info()[1]))

    Server(("0.0.0.0", port), Recorded).serve_forever()


def unchanged(port: int) -> None:
    sys.path[:0] = ["/stub", "/agent"]
    server: Any = importlib.import_module("testanyware_agent.server")
    server.run_server(port)  # the unchanged entry: a plain HTTPServer


# ---------------------------------------------------------------- supervisor


class Agents:
    def __init__(self, case_port: int, plain_port: int) -> None:
        self.ports = {"instrumented": case_port, "unchanged": plain_port}
        self.children: dict[str, subprocess.Popen[bytes]] = {}
        self.starts = 0

    def run_dir(self, kind: str) -> Path:
        return RUN / kind

    def start(self, kind: str) -> Json:
        self.starts += 1
        run_dir = self.run_dir(kind)
        run_dir.mkdir(parents=True, exist_ok=True)
        log = (run_dir / f"agent-{self.starts}.log").open("ab")
        argv = [sys.executable, "-E", "-s", "-B", __file__, "agent", str(self.ports[kind]), kind]
        argv.append(str(run_dir))
        child = subprocess.Popen(argv, stdout=log, stderr=log, start_new_session=True)
        self.children[kind] = child
        return {"kind": kind, "pid": child.pid, "ready": wait_ready(self.ports[kind])}

    def kill(self, kind: str) -> Json:
        child = self.children[kind]
        child.kill()
        try:
            child.wait(timeout=GRACE)
            return {"kind": kind, "pid": child.pid, "returncode": child.returncode}
        except subprocess.TimeoutExpired:
            return {"kind": kind, "pid": child.pid, "returncode": None, "unreaped": True}

    def stop_all(self) -> list[Json]:
        ended: list[Json] = []
        for kind, child in self.children.items():
            child.terminate()
            try:
                child.wait(timeout=GRACE)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=GRACE)
            ended.append({"kind": kind, "pid": child.pid, "returncode": child.returncode})
        return ended


def wait_ready(port: int) -> Json:
    import http.client

    started = time.monotonic()
    while time.monotonic() - started < READY_LIMIT:
        try:
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
            conn.request("GET", "/health")
            status = conn.getresponse().status
            conn.close()
            return {"status": status, "seconds": round(time.monotonic() - started, 4)}
        except OSError:
            time.sleep(0.05)
    return {"status": None, "seconds": round(time.monotonic() - started, 4)}


def controls() -> Json:
    """Linux file semantics without HTTP, on the container file system."""
    base = WORK / "controls"
    base.mkdir(parents=True)
    result: Json = {}

    def fresh(name: str, data: bytes) -> str:
        path = base / name
        path.write_bytes(data)
        return str(path)

    old, new = b"A" * 25, b"B" * 123
    path = fresh("rename", old)
    with open(path, "rb") as handle:
        apply(path, {"op": "replace", "data_hex": new.hex()})
        body = handle.read()
    result["rename-after-open"] = {"descriptor_reads_old": body == old, "length": len(body)}
    path = fresh("truncate", new)
    with open(path, "rb") as handle:
        size = os.fstat(handle.fileno()).st_size
        os.truncate(path, 25)
        body = handle.read()
    result["truncate-after-fstat"] = {"fstat": size, "length": len(body)}
    path = fresh("append", old)
    with open(path, "rb") as handle:
        size = os.fstat(handle.fileno()).st_size
        apply(path, {"op": "append", "data_hex": new.hex()})
        body = handle.read()
    result["append-after-fstat"] = {"fstat": size, "length": len(body)}
    first, second = b"C" * 8, b"D" * 16
    path = fresh("torn", first + first)
    with open(path, "rb", buffering=0) as handle:
        head = handle.read(8)
        apply(path, {"op": "overwrite", "data_hex": second.hex()})
        tail = handle.read(8)
    result["overwrite-between-reads"] = {"torn": head + tail == first + b"D" * 8}
    argv = [sys.executable, "-E", "-s", "-B", __file__, "memory-control"]
    done = subprocess.run(argv, capture_output=True, timeout=30, check=False)
    try:
        result["peak-memory"] = json.loads(done.stdout)
    except json.JSONDecodeError:
        result["peak-memory"] = {"exit": done.returncode, "stderr": done.stderr.decode()[-500:]}
    mounts = Path("/proc/mounts").read_text().splitlines()
    result["environment"] = {
        "uname": platform.uname()._asdict(),
        "python": sys.version,
        "uid": os.getuid(),
        "root_mount": [m for m in mounts if m.split()[1] == "/"],
        "work_device": os.stat(WORK).st_dev,
    }
    return result


class Stress:
    """A writer that replaces one file by rename, as the guest writer does."""

    def __init__(self) -> None:
        self.thread: threading.Thread | None = None
        self.stop_flag = threading.Event()
        self.writes = 0
        self.error: str | None = None

    def start(self, path: str, versions: list[bytes]) -> None:
        def loop() -> None:
            try:
                while not self.stop_flag.is_set():
                    data = versions[self.writes % len(versions)]
                    apply(path, {"op": "replace", "data_hex": data.hex()})
                    self.writes += 1
            except OSError as failure:
                self.error = repr(failure)

        Path(path).write_bytes(versions[0])
        self.thread = threading.Thread(target=loop, daemon=True)
        self.thread.start()

    def stop(self) -> Json:
        self.stop_flag.set()
        stopped = False
        if self.thread is not None:
            self.thread.join(timeout=GRACE)
            stopped = not self.thread.is_alive()
        return {"writes": self.writes, "stopped": stopped, "error": self.error}


def supervisor(control_port: int, case_port: int, plain_port: int) -> None:
    agents = Agents(case_port, plain_port)
    stress = Stress()
    started = [agents.start("instrumented"), agents.start("unchanged")]

    def op_prepare(req: Json) -> Json:
        directory = WORK / req["name"]
        directory.mkdir(parents=True)
        path = directory / "file"
        if req.get("initial_hex") is not None:
            path.write_bytes(bytes.fromhex(req["initial_hex"]))
        elif req.get("pattern") is not None:
            seed, size = req["pattern"]
            path.write_bytes(pattern(int(seed), int(size)))
        run_dir = agents.run_dir("instrumented")
        trace = run_dir / "trace.jsonl"
        mark = trace.stat().st_size if trace.exists() else 0
        # A killed agent leaves its pause marker. Remove it, so wait-paused sees only this case.
        stale = [n for n in ("paused", "resume", "arm.json") if (run_dir / n).exists()]
        for name in stale:
            (run_dir / name).unlink()
        if req.get("arm"):
            (run_dir / "arm.json").write_text(json.dumps(req["arm"]))
        return {
            "path": str(path),
            "mark": mark,
            "identity": path_identity(str(path)),
            "stale": stale,
        }

    def op_finish(req: Json) -> Json:
        run_dir = agents.run_dir("instrumented")
        trace = run_dir / "trace.jsonl"
        lines: list[Json] = []
        if trace.exists():
            with trace.open("rb") as handle:
                handle.seek(int(req["mark"]))
                lines = [json.loads(x) for x in handle.read().splitlines()]
        path = Path(req["path"])
        leftover_arm = (run_dir / "arm.json").exists()
        (run_dir / "arm.json").unlink(missing_ok=True)
        final: Json = {"identity": path_identity(str(path))}
        if path.exists():
            data = path.read_bytes()
            final |= {"length": len(data), "sha256": sha256(data)}
        return {
            "trace": lines,
            "final": final,
            "directory": sorted(p.name for p in path.parent.iterdir()),
            "arm_unused": leftover_arm,
        }

    def op_wait_paused(req: Json) -> Json:
        paused = agents.run_dir("instrumented") / "paused"
        limit = time.monotonic() + float(req.get("timeout", 20))
        while time.monotonic() < limit:
            if paused.exists():
                return {"paused": paused.read_text()}
            time.sleep(POLL)
        return {"paused": None}

    def op_resume(req: Json) -> Json:
        (agents.run_dir("instrumented") / "resume").write_text("go")
        return {"resumed": True}

    def op_publish(req: Json) -> Json:
        directory = WORK / "published"
        directory.mkdir(exist_ok=True)
        final = str(directory / req["name"])
        data = bytes.fromhex(req["data_hex"])
        action = {"op": "replace", "data_hex": data.hex()}
        timer = threading.Timer(float(req["delay"]), apply, args=(final, action))
        timer.daemon = True
        timer.start()
        return {"path": final}

    def op_stress_start(req: Json) -> Json:
        directory = WORK / "stress"
        directory.mkdir()
        path = str(directory / "file")
        stress.start(path, [bytes.fromhex(v) for v in req["versions_hex"]])
        return {"path": path}

    def op_blob(req: Json) -> bytes:
        path = agents.run_dir("instrumented") / f"emitted-{int(req['request'])}.bin"
        return path.read_bytes() if path.exists() else b""

    operations: dict[str, Callable[[Json], Any]] = {
        "started": lambda req: {"agents": started},
        "controls": lambda req: controls(),
        "prepare": op_prepare,
        "finish": op_finish,
        "wait-paused": op_wait_paused,
        "resume": op_resume,
        "kill-agent": lambda req: agents.kill("instrumented"),
        "start-agent": lambda req: agents.start("instrumented"),
        "health": lambda req: {k: wait_ready(p) for k, p in agents.ports.items()},
        "publish": op_publish,
        "stress-start": op_stress_start,
        "stress-stop": lambda req: stress.stop(),
        "stop-agents": lambda req: {"ended": agents.stop_all()},
    }

    class Control(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
            req: Json = json.loads(body or b"{}")
            name = self.path.strip("/")
            try:
                if name == "blob":
                    payload, kind = op_blob(req), "application/octet-stream"
                else:
                    payload = json.dumps(operations[name](req)).encode()
                    kind = "application/json"
                status = 200
            except Exception as failure:  # noqa: BLE001 - reported to the host
                payload = json.dumps({"error": repr(failure)}).encode()
                kind, status = "application/json", 500
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, format: str, *args: object) -> None:
            pass

    def on_term(signum: int, frame: object) -> None:
        agents.stop_all()
        raise SystemExit(0)

    signal.signal(signal.SIGTERM, on_term)
    ThreadingHTTPServer(("0.0.0.0", control_port), Control).serve_forever()


def pattern(seed: int, size: int) -> bytes:
    """The k7 pattern: distinct non-repeating bytes, so a torn body names its offsets."""
    out = bytearray()
    counter = 0
    while len(out) < size:
        out += hashlib.sha256(f"{seed}:{counter}".encode()).digest()
        counter += 1
    return bytes(out[:size])


def main(argv: list[str]) -> int:
    if argv[1] == "supervisor":
        supervisor(int(argv[2]), int(argv[3]), int(argv[4]))
    elif argv[1] == "agent" and argv[3] == "instrumented":
        instrumented(int(argv[2]), Path(argv[4]))
    elif argv[1] == "agent" and argv[3] == "unchanged":
        unchanged(int(argv[2]))
    elif argv[1] == "memory-control":
        print(json.dumps(memory_control()))
    else:
        raise SystemExit(__doc__)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
