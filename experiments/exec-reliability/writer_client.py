"""A blocking test client for writer.py over pipes.

The tests and the close record control use it. The controller's own client,
with nonblocking pipes and a bounded outbox, belongs to storage-client-k165.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

from writer import encode_frame

HERE = Path(__file__).resolve().parent
WRITER = (sys.executable, "-I", str(HERE / "writer.py"))
TIMEOUT = 10.0


def pin(data: bytes) -> dict[str, object]:
    return {"length": len(data), "sha256": hashlib.sha256(data).hexdigest()}


@dataclass
class WriterClient:
    """Spawn one writer and exchange frames and acknowledgements with it."""

    command: tuple[str, ...] = WRITER
    seq: int = 0
    process: subprocess.Popen[bytes] = field(init=False)

    def __post_init__(self) -> None:
        # The spawn shape that the spec gives the writer: its own session,
        # the filesystem root as working directory and a null standard error.
        self.process = subprocess.Popen(
            self.command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd="/",
            start_new_session=True,
            close_fds=True,
        )

    def next_seq(self) -> int:
        self.seq += 1
        return self.seq

    def write(self, data: bytes) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(data)
        self.process.stdin.flush()

    def send(self, op: str, payload: bytes = b"", **fields: object) -> int:
        seq = self.next_seq()
        self.write(encode_frame(seq, op, payload, **fields))
        return seq

    def ack(self) -> dict[str, object]:
        assert self.process.stdout is not None
        line = self.process.stdout.readline()
        assert line, "the writer closed its acknowledgement pipe"
        return json.loads(line)

    def request(self, op: str, payload: bytes = b"", **fields: object) -> dict[str, object]:
        seq = self.send(op, payload, **fields)
        reply = self.ack()
        assert reply["seq"] == seq, reply
        return reply

    def setup(self, root: Path, run: str, plan: bytes, subjects: list[str]) -> dict[str, object]:
        return self.request("setup", plan, root=str(root), run=run, subjects=subjects)

    def end_input(self) -> int:
        assert self.process.stdin is not None
        self.process.stdin.close()
        return self.process.wait(TIMEOUT)

    def remaining_output(self) -> bytes:
        assert self.process.stdout is not None
        return self.process.stdout.read()

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait(TIMEOUT)
        for stream in (self.process.stdin, self.process.stdout):
            if stream is not None and not stream.closed:
                stream.close()


@dataclass
class HealthyRun:
    run_dir: Path
    readiness: dict[str, object]
    close: dict[str, object]
    status: int
    after_close: bytes
    result_seq: int


def healthy_run(root: Path, run: str = "run", command: tuple[str, ...] = WRITER) -> HealthyRun:
    """Drive one writer through the frames of a small healthy run.

    The result storage section has the shape that the completion check reads.
    It holds the result's own sequence number and one digest entry for each
    record that the client serialised before the result.
    """
    client = WriterClient(command)
    try:
        plan = json.dumps({"run_id": run}, indent=2).encode() + b"\n"
        subject = str(HERE / "writer.py")
        readiness = client.setup(root, run, plan, [subject])
        assert readiness["status"] == "ok", readiness
        digests: dict[str, object] = {"plan": pin(plan)}
        intent = json.dumps({"identity": f"vm-{run}"}).encode()
        assert client.request("create", intent, name="intent-1.json")["status"] == "ok"
        digests["intent-1.json"] = pin(intent)
        stdout = [b"first line\n", b"second line\n"]
        assert client.request("create", stdout[0], name="workload.stdout")["status"] == "ok"
        assert client.request("append", stdout[1], name="workload.stdout")["status"] == "ok"
        digests["workload.stdout"] = pin(b"".join(stdout))
        receipt = json.dumps({"name": "workload", "ok": True}).encode()
        assert client.request("create", receipt, name="workload.json")["status"] == "ok"
        digests["workload.json"] = pin(receipt)
        after = client.request("subjects")
        assert after["status"] == "ok", after
        result_seq = client.next_seq()
        result = {
            "run_id": run,
            "ok": True,
            "subjects_before": readiness["subjects"],
            "subjects_after": after["subjects"],
            "storage": {
                "sequence": result_seq,
                "publication": "pending",
                "digests": digests,
                "writer_reported": {"subjects-before": readiness["subjects_before"]},
            },
        }
        data = json.dumps(result, indent=2).encode() + b"\n"
        client.write(encode_frame(result_seq, "result", data, name="result"))
        reply = client.ack()
        assert reply == {"seq": result_seq, "op": "result", "status": "ok"}, reply
        close = client.request("close")
        after_close = client.remaining_output()
        status = client.process.wait(TIMEOUT)
        return HealthyRun(
            run_dir=Path(str(readiness["run_dir"])),
            readiness=readiness,
            close=close,
            status=status,
            after_close=after_close,
            result_seq=result_seq,
        )
    finally:
        client.stop()


def file_pin(path: Path) -> dict[str, object]:
    return pin(path.read_bytes())


def open_descriptors() -> set[int]:
    """Descriptors that this process has open, for leak checks."""
    return {int(name) for name in os.listdir("/dev/fd")}
