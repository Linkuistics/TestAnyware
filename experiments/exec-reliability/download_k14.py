"""Linux download consistency cases for leaf linux-download-consistency-k14.

Usage: download_k14.py OUT [RUN_ID]

RUN_ID is 12 lower-case hex digits. The wrapper passes its own, so that its cleanup sees
only this run's container. Without RUN_ID, the controller makes one.

The script owns one fresh container of the pinned k5 image. The container runs
download_k14_seam.py as a supervisor with two agents from this checkout: an
instrumented agent for the forced cases and the unchanged `run_server` for the
stress reads and the immutable publication control. The seam docstring holds the
hooks. The host reaches all three ports through loopback ports that Docker publishes.

The order is fixed: file semantics controls without HTTP, then each forced case
with the raw client and the Host CLI, then the natural rename stress, then the
immutable publication control. Each case uses a fresh file. Nothing is retried.

The raw client records the advertised framing and the received bytes. The Host
CLI records its exit, receipt, destination and temporary files. The seam records
the descriptor identity, the size that became Content-Length, each read and each
write to the socket.

The controller judges nothing. download_k14_check.py classifies the retained
evidence. The controller removes the container in its `finally` path.
"""

from __future__ import annotations

import http.client
import json
import signal
import socket
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from download_k7 import (
    CLI,
    CLI_TIMEOUT,
    GRACE,
    REAP,
    cli_download,
    end_process,
    leftover_temporaries,
    pattern,
    raw_download,
    sha256,
    terminal_record,
)
from linux_k5 import IMAGE

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
AGENT = REPO / "agents/linux"
STUB = HERE / "linux_k5_stub"
SEAM = HERE / "download_k14_seam.py"
PENDING = HERE / "evidence/download-consistency-k7/historical/after-abrupt-03-initial.json"
PENDING_SHA256 = "b95c9261fb8ff0ca68a9d8b06ebdcb4102a72f60b670c2d748cdf4eca454e7a4"
ORACLE_HEX = "00ff80410a"
LABEL = "testanyware.k14.run"
CASE_PORT, CONTROL_PORT, PLAIN_PORT = 8648, 8649, 8650
CHUNK = 1024 * 1024  # system_endpoints.CHUNK_SIZE
BIG = 3 * CHUNK + 1000
LARGE = 64 * CHUNK
KEEP_LIMIT = 8 * CHUNK  # bodies up to this size are kept as files, larger ones by digest
# Bounds. The root brief sets the 150 s transfer ceiling, the 45 s disposal ceiling and
# the 30 s cleanup ceiling.
TRANSFER_BOUND = 150.0
RAW_TIMEOUT = 20.0
CONTROL_TIMEOUT = 30.0
READY_BOUND = 30.0
DOCKER_BOUND = 60.0
CLEANUP_BOUND = 30.0
DISPOSE_BOUND = 45.0
ABANDON_AFTER = 64 * 1024
STRESS_SECONDS = 10.0
PUBLICATIONS = 200
PUBLICATION_WAIT = 5.0
MEMORY = "512m"

Json = dict[str, Any]


@dataclass(frozen=True)
class Case:
    name: str
    initial: bytes | None  # None: no file
    seed: tuple[int, int] | None  # a k7 pattern (seed, size) instead of initial bytes
    event: str | None
    action: Json | None
    allowed: tuple[bytes, ...]
    interrupt: str | None = None  # "kill-agent" or "abandon"
    clients: tuple[str, ...] = ("raw", "cli")

    def initial_bytes(self) -> bytes | None:
        return pattern(*self.seed) if self.seed else self.initial


def cases() -> list[Case]:
    pending = PENDING.read_bytes()
    if sha256(pending) != PENDING_SHA256:
        raise SystemExit("the retained pending record changed")
    terminal = terminal_record()
    big = pattern(1, BIG)
    longer = pattern(2, BIG + CHUNK)
    same = pattern(3, BIG)
    half = CHUNK + CHUNK // 2

    def change(op: str, data: bytes | None = None, **extra: Any) -> Json:
        return {"op": op} | ({"data_hex": data.hex()} if data is not None else {}) | extra

    pause: Json = {"op": "pause"}
    return [
        Case("replace-longer-after-open", pending, None, "after-open",
             change("replace", terminal), (pending, terminal)),
        Case("replace-longer-after-fstat", pending, None, "after-fstat",
             change("replace", terminal), (pending, terminal)),
        Case("replace-shorter-after-fstat", terminal, None, "after-fstat",
             change("replace", pending), (terminal, pending)),
        Case("replace-between-reads", None, (1, BIG), "before-read-2",
             change("replace", pattern=[2, BIG + CHUNK]), (big, longer)),
        Case("truncate-after-open", terminal, None, "after-open",
             change("truncate", size=25), (terminal, terminal[:25])),
        Case("truncate-after-fstat", terminal, None, "after-fstat",
             change("truncate", size=25), (terminal, terminal[:25])),
        Case("truncate-between-reads", None, (1, BIG), "before-read-2",
             change("truncate", size=half), (big, big[:half])),
        Case("append-after-fstat", pending, None, "after-fstat",
             change("append", terminal), (pending, pending + terminal)),
        Case("overwrite-longer-after-fstat", pending, None, "after-fstat",
             change("overwrite", terminal), (pending, terminal)),
        Case("overwrite-between-reads", None, (1, BIG), "before-read-2",
             change("overwrite", pattern=[3, BIG]), (big, same)),
        Case("immutable-binary", bytes.fromhex(ORACLE_HEX), None, None, None,
             (bytes.fromhex(ORACLE_HEX),)),
        Case("missing-file", None, None, None, None, ()),
        Case("agent-killed-mid-body", None, (1, BIG), "before-read-2", pause, (big,),
             interrupt="kill-agent"),
        Case("client-abandons-mid-body", None, (1, BIG), "before-read-2", pause, (big,),
             interrupt="abandon"),
        Case("large-immutable", None, (4, LARGE), None, None, (pattern(4, LARGE),)),
    ]  # fmt: skip


def docker(argv: list[str], bound: float = DOCKER_BOUND) -> Json:
    started = time.monotonic()
    try:
        done = subprocess.run(  # noqa: S603 - docker with fixed arguments
            ["docker", *argv], capture_output=True, text=True, timeout=bound, check=False
        )
        code: int | None = done.returncode
        out, err = done.stdout, done.stderr
    except subprocess.TimeoutExpired as expired:
        code = None
        out = str(expired.stdout or "")
        err = str(expired.stderr or "") + f"\nbound {bound} s passed"
    return {
        "argv": ["docker", *argv],
        "returncode": code,
        "stdout": out,
        "stderr": err,
        "seconds": round(time.monotonic() - started, 4),
    }


class Container:
    def __init__(self, run_id: str) -> None:
        self.run_id = run_id
        self.name = f"k14-{run_id}"
        self.ports: dict[int, int] = {}

    def start(self) -> Json:
        argv = ["run", "-d", "--init", "--name", self.name, "--label", f"{LABEL}={self.run_id}"]
        argv += ["-u", "1000:1000", "--memory", MEMORY, "--pids-limit", "128"]
        for port in (CASE_PORT, CONTROL_PORT, PLAIN_PORT):
            argv += ["-p", f"127.0.0.1::{port}"]
        argv += ["-v", f"{AGENT}:/agent:ro", "-v", f"{STUB}:/stub:ro"]
        argv += ["-v", f"{SEAM}:/seam/download_k14_seam.py:ro"]
        argv += [IMAGE, "python3", "-E", "-s", "-B", "/seam/download_k14_seam.py"]
        argv += ["supervisor", str(CONTROL_PORT), str(CASE_PORT), str(PLAIN_PORT)]
        record = {"run": docker(argv)}
        for port in (CASE_PORT, CONTROL_PORT, PLAIN_PORT):
            mapped = docker(["port", self.name, f"{port}/tcp"])
            record[f"port-{port}"] = mapped
            first = str(mapped["stdout"]).splitlines()[0] if mapped["stdout"] else ""
            self.ports[port] = int(first.rsplit(":", 1)[1]) if first else 0
        return record

    def control(
        self, op: str, request: Json | None = None, timeout: float = CONTROL_TIMEOUT
    ) -> Any:
        conn = http.client.HTTPConnection("127.0.0.1", self.ports[CONTROL_PORT], timeout=timeout)
        try:
            body = json.dumps(request or {}).encode()
            conn.request("POST", f"/{op}", body, {"Content-Type": "application/json"})
            response = conn.getresponse()
            payload = response.read()
            if response.getheader("Content-Type") == "application/octet-stream":
                return payload
            return json.loads(payload)
        finally:
            conn.close()

    def wait_control(self) -> Json:
        started = time.monotonic()
        last = ""
        while time.monotonic() - started < READY_BOUND:
            try:
                return self.control("started", timeout=5) | {
                    "seconds": round(time.monotonic() - started, 4)
                }
            except (OSError, http.client.HTTPException, json.JSONDecodeError) as failure:
                last = repr(failure)
                time.sleep(0.1)
        return {"error": f"control not ready in {READY_BOUND} s: {last}"}

    def dispose(self, out: Path) -> Json:
        record: Json = {}
        try:
            record["stop-agents"] = self.control("stop-agents", timeout=10)
        except (OSError, http.client.HTTPException, json.JSONDecodeError) as failure:
            record["stop-agents"] = {"error": repr(failure)}
        copy = docker(["cp", f"{self.name}:/tmp/k14/run", str(out / "agent-run")])
        record["copy-run"] = copy | {"stdout": "", "stderr": copy["stderr"][-2000:]}
        logs = docker(["logs", self.name])
        (out / "container.log").write_text(str(logs["stdout"]) + str(logs["stderr"]))
        record["inspect"] = docker(["container", "inspect", self.name])
        (out / "container-inspect.json").write_text(str(record.pop("inspect")["stdout"]))
        record["cleanup"] = docker(["stop", "-t", "2", self.name], CLEANUP_BOUND)
        record["dispose"] = docker(["rm", "-f", self.name], DISPOSE_BOUND)
        listed = docker(["ps", "-a", "-q", "--filter", f"label={LABEL}={self.run_id}"])
        record["remaining"] = listed
        record["removed"] = listed["returncode"] == 0 and not str(listed["stdout"]).strip()
        return record


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def raw_abandon(port: int, path: str) -> Json:
    """Read the headers and ABANDON_AFTER body bytes, then close the connection."""
    query = "".join(ch if ch.isalnum() or ch in "/-_." else f"%{ord(ch):02X}" for ch in path)
    request = (
        f"POST /download?path={query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n"
        "Content-Length: 0\r\nConnection: close\r\n\r\n"
    ).encode()
    conn = socket.create_connection(("127.0.0.1", port), timeout=RAW_TIMEOUT)
    conn.sendall(request)
    data = b""
    deadline = time.monotonic() + RAW_TIMEOUT
    while time.monotonic() < deadline:
        piece = conn.recv(65536)
        if not piece:
            break
        data += piece
        _, sep, body = data.partition(b"\r\n\r\n")
        if sep and len(body) >= ABANDON_AFTER:
            break
    return {"conn": conn, "received": len(data), "head": data.partition(b"\r\n\r\n")[0]}


def keep(path: Path, data: bytes) -> Json:
    if len(data) <= KEEP_LIMIT:
        path.write_bytes(data)
    return {"bytes": len(data), "sha256": sha256(data), "kept": len(data) <= KEEP_LIMIT}


def run_raw(box: Container, case: Case, path: str, directory: Path) -> Json:
    port = box.ports[CASE_PORT]
    timeout = TRANSFER_BOUND if case.name == "large-immutable" else RAW_TIMEOUT
    record: Json = {}
    if case.interrupt is None:
        result, body = raw_download(port, Path(path), timeout)
        record["result"] = result | {"body": keep(directory / "received.bin", body)}
        return record
    if case.interrupt == "abandon":
        opened = raw_abandon(port, path)
        record["paused"] = box.control("wait-paused")
        conn: socket.socket = opened.pop("conn")
        conn.close()
        record["abandoned"] = {"received": opened["received"], "head": opened["head"].decode()}
        record["resume"] = box.control("resume")
        time.sleep(0.2)  # allow the resumed handler to reach its failed write
        return record
    outcome: list[tuple[Json, bytes] | BaseException] = []

    def client() -> None:
        try:
            outcome.append(raw_download(port, Path(path), timeout))
        except OSError as failure:
            outcome.append(failure)

    import threading

    worker = threading.Thread(target=client, daemon=True)
    worker.start()
    record["paused"] = box.control("wait-paused")
    record["kill"] = box.control("kill-agent")
    worker.join(timeout=timeout + 5)
    record["restart"] = box.control("start-agent")
    if outcome and isinstance(outcome[0], tuple):
        result, body = outcome[0]
        record["result"] = result | {"body": keep(directory / "received.bin", body)}
    else:
        record["error"] = repr(outcome[0]) if outcome else "raw client alive at the bound"
    return record


def run_cli(box: Container, case: Case, path: str, directory: Path) -> Json:
    port = box.ports[CASE_PORT]
    local = directory / "destination" / "file"
    local.parent.mkdir()
    if case.interrupt is None:
        result = cli_download(port, Path(path), local)
        if local.exists() and local.stat().st_size > KEEP_LIMIT:
            local.unlink()
            result["destination_removed_after_digest"] = True
        return {"result": result}
    argv = [str(CLI), "file", "download", "--agent", f"127.0.0.1:{port}", path, str(local)]
    argv.append("--json")
    started = time.monotonic()
    cli = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)  # noqa: S603
    record: Json = {"argv": argv}
    try:
        record["paused"] = box.control("wait-paused")
        record["temporaries_while_paused"] = leftover_temporaries(local.parent)
        if case.interrupt == "kill-agent":
            record["kill"] = box.control("kill-agent")
            try:
                out, err = cli.communicate(timeout=CLI_TIMEOUT)
                ended = "exit"
            except subprocess.TimeoutExpired:
                ended, out, err = end_process(cli, signal.SIGKILL)
            record["restart"] = box.control("start-agent")
        else:
            ended, out, err = end_process(cli)
            record["resume"] = box.control("resume")
            time.sleep(0.2)
        record["result"] = {
            "ended_by": ended,
            "exit": cli.returncode,
            "seconds": round(time.monotonic() - started, 4),
            "stdout": out.decode(errors="replace"),
            "stderr": err.decode(errors="replace"),
            "destination_exists": local.exists(),
            "temporaries": leftover_temporaries(local.parent),
        }
    finally:
        if cli.poll() is None:
            record["cli_cleanup"] = end_process(cli, signal.SIGKILL)[0]
    return record


def run_case(box: Container, case: Case, client: str, root: Path) -> Json:
    directory = root / case.name / client
    directory.mkdir(parents=True)
    for index, version in enumerate(case.allowed):
        if len(version) <= KEEP_LIMIT:
            (directory / f"allowed-{index}.bin").write_bytes(version)
    record: Json = {
        "case": case.name,
        "client": client,
        "event": case.event,
        "action": {k: v for k, v in (case.action or {}).items() if k != "data_hex"},
        "allowed": [{"bytes": len(v), "sha256": sha256(v)} for v in case.allowed],
        "interrupt": case.interrupt,
    }
    initial = case.initial_bytes()
    request: Json = {"name": f"{case.name}-{client}"}
    if case.seed:
        request["pattern"] = list(case.seed)
    elif initial is not None:
        request["initial_hex"] = initial.hex()
    if case.event and case.action:
        request["arm"] = {"event": case.event, "action": case.action}
    try:
        prepared = box.control("prepare", request)
        record["prepared"] = prepared
        if client == "raw":
            record |= run_raw(box, case, prepared["path"], directory)
        else:
            record |= run_cli(box, case, prepared["path"], directory)
        finished = box.control("finish", {"path": prepared["path"], "mark": prepared["mark"]})
        record["agent"] = finished
        requests = sorted({line["request"] for line in finished["trace"]})
        emitted: list[Json] = []
        record["emitted"] = emitted
        for number in requests:
            blob = box.control("blob", {"request": number})
            (directory / f"emitted-{number}.bin").write_bytes(blob)
            emitted.append({"request": number, "bytes": len(blob), "sha256": sha256(blob)})
        if case.interrupt:
            record["health_after"] = box.control("health")
    except (OSError, http.client.HTTPException, json.JSONDecodeError, KeyError) as failure:
        record["error"] = repr(failure)
    write_json(directory / "record.json", record)
    return record


class Receipts:
    """One JSON object a line, written as each item ends. Each body is kept once, by digest,
    so that the checker can compare the received bytes again."""

    def __init__(self, out: Path, name: str) -> None:
        self.path = out / f"{name}.jsonl"
        self.bodies = out / f"{name}-bodies"
        self.bodies.mkdir()
        self.handle = self.path.open("w")
        self.count = 0

    def add(self, item: Json, body: bytes | None = None) -> None:
        if body is not None:
            kept = self.bodies / f"{sha256(body)}.bin"
            if not kept.exists():
                kept.write_bytes(body)
        self.handle.write(json.dumps(item, sort_keys=True) + "\n")
        self.count += 1

    def close(self) -> None:
        self.handle.close()


def timed_read(box: Container, path: Path) -> tuple[Json, bytes]:
    """One raw read with its monotonic start and end."""
    started = time.monotonic_ns()
    result, body = raw_download(box.ports[PLAIN_PORT], path)
    return result | {"started_ns": started, "ended_ns": time.monotonic_ns()}, body


def stress(box: Container, out: Path) -> Json:
    """Natural timing on the unchanged agent: a writer swaps the pending and terminal
    records by rename while the raw client reads the path again and again. Each read
    gets one receipt in stress-reads.jsonl. The controller does not group reads."""
    pending, terminal = PENDING.read_bytes(), terminal_record()
    started = box.control("stress-start", {"versions_hex": [pending.hex(), terminal.hex()]})
    receipts = Receipts(out, "stress-reads")
    error: str | None = None
    began = time.monotonic_ns()
    deadline = time.monotonic() + STRESS_SECONDS
    try:
        while time.monotonic() < deadline:
            index = receipts.count
            try:
                result, body = timed_read(box, Path(started["path"]))
            except OSError as failure:
                receipts.add({"index": index, "error": repr(failure)})
                raise
            receipts.add({"index": index} | result, body)
    except OSError as failure:
        error = repr(failure)
    finally:
        receipts.close()
    stopped = box.control("stress-stop")
    return {
        "configured_seconds": STRESS_SECONDS,
        "started_ns": began,
        "ended_ns": time.monotonic_ns(),
        "reads": receipts.count,
        "receipts": receipts.path.name,
        "writer": stopped,
        "versions": {sha256(pending): "pending", sha256(terminal): "terminal"},
        "error": error,
    }


def immutable(box: Container, out: Path) -> Json:
    """The k7 control on the unchanged agent: each record is written to a temporary file
    and renamed to a fresh name once. The raw client polls the name until it answers 200.
    publication-reads.jsonl holds one receipt for each publish and for each poll."""
    receipts = Receipts(out, "publication-reads")
    record: Json = {"publications": PUBLICATIONS, "receipts": receipts.path.name}
    try:
        for index in range(PUBLICATIONS):
            data = terminal_record() + f"{index}\n".encode()
            name = f"record-{index:04}.json"
            asked = time.monotonic_ns()
            final = box.control(
                "publish", {"name": name, "data_hex": data.hex(), "delay": 0.002 * (index % 5)}
            )["path"]
            receipts.add({"kind": "publish", "publication": index, "path": final,
                          "expected_sha256": sha256(data), "expected_bytes": len(data),
                          "started_ns": asked, "ended_ns": time.monotonic_ns()})  # fmt: skip
            deadline = time.monotonic() + PUBLICATION_WAIT
            attempt = 0
            while time.monotonic() < deadline:
                result, body = timed_read(box, Path(final))
                answered = result["status"] == 200
                item = {"kind": "read", "publication": index, "attempt": attempt} | result
                receipts.add(item, body if answered else None)
                attempt += 1
                if answered:
                    break
    except (OSError, http.client.HTTPException, json.JSONDecodeError, KeyError) as failure:
        record["error"] = repr(failure)
    finally:
        receipts.close()
    return record


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        raise SystemExit(__doc__)
    run_id = argv[2] if len(argv) == 3 else uuid.uuid4().hex[:12]
    if len(run_id) != 12 or any(ch not in "0123456789abcdef" for ch in run_id):
        raise SystemExit(f"bad RUN_ID {run_id!r}")
    out = Path(argv[1])
    out.mkdir(parents=True)
    plan = cases()
    inventory: Json = {
        "leaf": "linux-download-consistency-k14",
        "run_id": run_id,
        "image": IMAGE,
        "label": f"{LABEL}={run_id}",
        "cli": str(CLI),
        "cases": [
            {
                "name": c.name,
                "event": c.event,
                "action": c.action and c.action["op"],
                "interrupt": c.interrupt,
                "clients": list(c.clients),
                "allowed_sha256": [sha256(v) for v in c.allowed],
            }
            for c in plan
        ],  # fmt: skip
        "bounds": {
            "transfer": TRANSFER_BOUND,
            "raw": RAW_TIMEOUT,
            "control": CONTROL_TIMEOUT,
            "ready": READY_BOUND,
            "docker": DOCKER_BOUND,
            "cleanup": CLEANUP_BOUND,
            "dispose": DISPOSE_BOUND,
            "grace": GRACE,
            "reap": REAP,
            "memory": MEMORY,
        },  # fmt: skip
        "stress_seconds": STRESS_SECONDS,
        "publications": PUBLICATIONS,
    }
    write_json(out / "inventory.json", inventory)
    box = Container(run_id)
    summary: Json = {"run_id": run_id, "started_ns": time.monotonic_ns()}
    try:
        summary["container"] = box.start()
        summary["ready"] = box.wait_control()
        if "error" in summary["ready"]:
            raise RuntimeError(summary["ready"]["error"])
        write_json(out / "controls.json", box.control("controls"))
        root = out / "cases"
        ran: list[Json] = []
        summary["cases"] = ran
        for case in plan:
            for client in case.clients:
                record = run_case(box, case, client, root)
                ran.append({"case": case.name, "client": client, "error": record.get("error")})
        write_json(out / "stress.json", stress(box, out))
        write_json(out / "immutable.json", immutable(box, out))
    except (OSError, RuntimeError, http.client.HTTPException, json.JSONDecodeError) as failure:
        summary["error"] = repr(failure)
    finally:
        summary["disposal"] = box.dispose(out)
        summary["ended_ns"] = time.monotonic_ns()
        write_json(out / "summary.json", summary)
    complete = "error" not in summary and summary["disposal"]["removed"]
    print(f"complete {complete} run {run_id}")
    return 0 if complete else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
