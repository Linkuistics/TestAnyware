"""Tests for writer.py, driven by a test client over pipes, and for completion()."""

from __future__ import annotations

import errno
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import pytest

from retention import completion
from writer import _NOT_REGULAR, LENGTHS, MAX_PAYLOAD, encode_frame
from writer_client import WriterClient, file_pin, healthy_run, pin

PLAN = b'{"run_id": "run"}\n'
FAULTS = Path(__file__).resolve().parent / "writer_faults.py"


def faulty(*faults: str) -> tuple[str, ...]:
    """The command of a writer whose named file calls fail with EIO."""
    return (sys.executable, "-I", str(FAULTS), *faults)


@pytest.fixture
def client():
    started = WriterClient()
    yield started
    started.stop()


def ready(client: WriterClient, root: Path, run: str = "run") -> Path:
    reply = client.setup(root, run, PLAN, [])
    assert reply["status"] == "ok", reply
    return Path(str(reply["run_dir"]))


def lines(path: Path) -> dict[str, object]:
    return json.loads(path.read_text())


def test_healthy_run_is_complete(tmp_path):
    run = healthy_run(tmp_path / "evidence")
    assert run.status == 0
    assert run.after_close == b""
    assert run.run_dir == (tmp_path / "evidence" / "run").resolve()
    assert run.close["status"] == "ok"
    close = run.run_dir / "storage-close.json"
    identity = run.run_dir / "result-identity.json"
    assert run.close["storage_close"] == file_pin(close)
    assert run.close["identity"] == file_pin(identity)
    assert lines(close) == {
        "state": "orderly",
        "last_applied": run.result_seq,
        "refused": 0,
        "failed": 0,
    }
    listed = [entry["path"] for entry in lines(identity)["files"]]
    assert sorted(listed) == sorted(
        [
            "intent-1.json",
            "plan.json",
            "result.json",
            "storage-close.json",
            "subjects-before.json",
            "workload.json",
            "workload.stdout",
        ]
    )
    assert (run.run_dir / "workload.stdout").read_bytes() == b"first line\nsecond line\n"
    report = completion(run.run_dir)
    assert report["reasons"] == []
    assert report["complete"] is True


def test_setup_writes_plan_and_subjects_before(tmp_path, client):
    subject = tmp_path / "subject"
    subject.write_bytes(b"subject")
    reply = client.setup(tmp_path / "root", "run", PLAN, [str(subject), str(tmp_path / "gone")])
    run_dir = Path(str(reply["run_dir"]))
    assert (run_dir / "plan.json").read_bytes() == PLAN
    before = (run_dir / "subjects-before.json").read_bytes()
    assert reply["subjects_before"] == pin(before)
    subjects = json.loads(before)
    assert subjects == reply["subjects"]
    assert subjects[str(subject)] == pin(b"subject")["sha256"]
    assert subjects[str(tmp_path / "gone")].startswith("unavailable: ")


def test_each_frame_is_acknowledged_by_sequence_number(tmp_path, client):
    ready(client, tmp_path)
    sent = [client.send("create", b"x", name=f"file-{n}") for n in range(5)]
    assert [client.ack()["seq"] for _ in sent] == sent


def assert_refused(reply: dict[str, object], op: str, code: int) -> None:
    assert reply["op"] == op
    assert reply["status"] == "refused"
    assert reply["errno"] == code
    assert reply["error"] == os.strerror(code)


@pytest.mark.parametrize(
    "op, name, code",
    [
        ("create", "plan", errno.EPERM),
        ("create", "result.json", errno.EPERM),
        ("create", "storage-close", errno.EPERM),
        ("create", "result-identity.json", errno.EPERM),
        ("create", "subjects-before", errno.EPERM),
        ("create", "result", errno.EPERM),
        ("create", "RESULT.json", errno.EPERM),
        ("create", "Plan", errno.EPERM),
        ("append", "Storage-Close.JSON", errno.EPERM),
        ("append", "plan", errno.EPERM),
        ("append", "storage-close.json", errno.EPERM),
        ("result", "workload.stdout", errno.EINVAL),
        ("create", "nested/name", errno.EINVAL),
        ("create", "../escape", errno.EINVAL),
        ("create", "..", errno.EINVAL),
        ("create", "", errno.EINVAL),
        ("append", "a/b", errno.EINVAL),
    ],
)
def test_refuses_bad_names(tmp_path, client, op, name, code):
    run_dir = ready(client, tmp_path)
    assert_refused(client.request(op, b"data", name=name), op, code)
    # The writer keeps applying frames after a refusal.
    assert client.request("create", b"ok", name="after")["status"] == "ok"
    assert (run_dir / "plan.json").read_bytes() == PLAN
    assert not (tmp_path / "escape").exists()


def test_refuses_oversized_payload_and_keeps_framing(tmp_path, client):
    run_dir = ready(client, tmp_path)
    assert_refused(
        client.request("create", b"x" * (MAX_PAYLOAD + 1), name="big"), "create", errno.EMSGSIZE
    )
    assert not (run_dir / "big").exists()
    assert client.request("create", b"x" * MAX_PAYLOAD, name="limit")["status"] == "ok"
    assert (run_dir / "limit").stat().st_size == MAX_PAYLOAD


def test_refuses_frames_out_of_protocol(tmp_path, client):
    assert_refused(client.request("create", b"x", name="early"), "create", errno.EBADF)
    ready(client, tmp_path)
    assert_refused(
        client.request("setup", PLAN, root=str(tmp_path), run="again", subjects=[]),
        "setup",
        errno.EALREADY,
    )
    assert_refused(client.request("rename", name="x"), "rename", errno.EPROTO)
    assert_refused(client.request("append", b"x", name="never"), "append", errno.ENOENT)
    client.write(encode_frame(client.seq, "create", b"x", name="repeat"))
    assert_refused(client.ack(), "create", errno.EPROTO)


def test_storage_error_ack_carries_the_real_errno(tmp_path, client):
    run_dir = ready(client, tmp_path)
    assert client.request("create", b"one", name="twice")["status"] == "ok"
    reply = client.request("create", b"two", name="twice")
    assert reply["status"] == "failed"
    assert (reply["errno"], reply["call"]) == (errno.EEXIST, "open")
    assert (run_dir / "twice").read_bytes() == b"one"


def test_append_does_not_follow_a_planted_link(tmp_path, client):
    run_dir = ready(client, tmp_path)
    outside = tmp_path / "outside"
    outside.write_bytes(b"outside")
    assert client.request("create", b"inside", name="stream")["status"] == "ok"
    (run_dir / "stream").unlink()
    (run_dir / "stream").symlink_to(outside)
    reply = client.request("append", b"more", name="stream")
    assert (reply["status"], reply["errno"], reply["call"]) == ("failed", errno.ELOOP, "open")
    assert outside.read_bytes() == b"outside"


def test_create_does_not_follow_a_planted_link(tmp_path, client):
    run_dir = ready(client, tmp_path)
    outside = tmp_path / "outside"
    (run_dir / "planted").symlink_to(outside)
    reply = client.request("create", b"data", name="planted")
    assert (reply["status"], reply["errno"]) == ("failed", errno.EEXIST)
    assert not outside.exists()


@pytest.mark.parametrize("target", ["outside", "plan.json"])
def test_append_refuses_a_hard_link_that_replaces_the_record(tmp_path, client, target):
    run_dir = ready(client, tmp_path)
    linked = tmp_path / "outside" if target == "outside" else run_dir / target
    if target == "outside":
        linked.write_bytes(b"outside")
    before = linked.read_bytes()
    assert client.request("create", b"inside", name="stream")["status"] == "ok"
    (run_dir / "stream").unlink()
    os.link(linked, run_dir / "stream")
    assert_refused(client.request("append", b"more", name="stream"), "append", errno.ENOENT)
    assert linked.read_bytes() == before


def test_append_refuses_a_record_with_a_second_link(tmp_path, client):
    run_dir = ready(client, tmp_path)
    assert client.request("create", b"inside", name="stream")["status"] == "ok"
    os.link(run_dir / "stream", tmp_path / "outside")
    assert_refused(client.request("append", b"more", name="stream"), "append", errno.EMLINK)
    assert (tmp_path / "outside").read_bytes() == b"inside"


def test_failed_close_of_a_record_is_acknowledged(tmp_path):
    client = WriterClient(faulty("close:stream"))
    try:
        ready(client, tmp_path)
        reply = client.request("create", b"data", name="stream")
        assert (reply["status"], reply["errno"], reply["call"]) == ("failed", errno.EIO, "close")
        assert client.request("create", b"next", name="next")["status"] == "ok"
    finally:
        client.stop()


def test_failed_cleanup_keeps_the_primary_write_error(tmp_path):
    client = WriterClient(faulty("write:stream", "close:stream"))
    try:
        ready(client, tmp_path)
        reply = client.request("create", b"data", name="stream")
        assert (reply["status"], reply["errno"], reply["call"]) == ("failed", errno.EIO, "write")
    finally:
        client.stop()


def _plant_symlink(run_dir: Path) -> None:
    (run_dir / "planted").symlink_to(run_dir / "plan.json")


def _plant_fifo(run_dir: Path) -> None:
    os.mkfifo(run_dir / "planted")


def _plant_file(run_dir: Path) -> None:
    (run_dir / "planted").write_bytes(b"planted")


@pytest.mark.parametrize(
    ("faults", "plant", "code", "call"),
    [
        ((), _plant_symlink, errno.ELOOP, "open"),
        ((), _plant_fifo, _NOT_REGULAR, "fstat"),
        (("read:planted",), _plant_file, errno.EIO, "read"),
        (("close:planted",), _plant_file, errno.EIO, "close"),
        (("close:storage-close.json",), None, errno.EIO, "close"),
    ],
    ids=["symlink", "fifo", "read", "pin-close", "record-close"],
)
def test_failed_close_step_writes_no_identity(tmp_path, faults, plant, code, call):
    client = WriterClient(faulty(*faults))
    try:
        run_dir = ready(client, tmp_path)
        if plant is not None:
            plant(run_dir)
        reply = client.request("close")
        assert (reply["status"], reply["errno"], reply["call"]) == ("failed", code, call)
        assert client.remaining_output() == b""
        assert client.process.wait(10) != 0
    finally:
        client.stop()
    assert (run_dir / "storage-close.json").exists()
    assert not (run_dir / "result-identity.json").exists()


def test_files_stay_in_the_run_directory_after_a_rename(tmp_path, client):
    run_dir = ready(client, tmp_path / "root")
    moved = tmp_path / "moved"
    run_dir.rename(moved)
    assert client.request("create", b"data", name="late")["status"] == "ok"
    assert (moved / "late").read_bytes() == b"data"
    assert not run_dir.exists()


def test_refusals_make_the_run_incomplete(tmp_path):
    client = WriterClient()
    try:
        run_dir = ready(client, tmp_path)
        assert client.request("create", b"x", name="a/b")["status"] == "refused"
        result = {"storage": {"sequence": client.seq + 1, "digests": {"plan": pin(PLAN)}}}
        data = json.dumps(result).encode()
        assert client.request("result", data, name="result")["status"] == "ok"
        assert client.request("close")["status"] == "ok"
        assert client.process.wait(10) == 0
    finally:
        client.stop()
    assert lines(run_dir / "storage-close.json")["refused"] == 1
    report = completion(run_dir)
    assert report["complete"] is False
    assert report["reasons"] == ["storage-close reports 1 refused frames"]


def test_end_of_input_without_close(tmp_path, client):
    run_dir = ready(client, tmp_path)
    assert client.request("create", b"kept", name="kept")["status"] == "ok"
    frame = encode_frame(client.next_seq(), "create", b"lost", name="lost")
    client.write(frame[:-2])
    assert client.end_input() == 1
    assert client.remaining_output() == b""
    assert (run_dir / "kept").read_bytes() == b"kept"
    assert not (run_dir / "lost").exists()
    assert not (run_dir / "result-identity.json").exists()
    assert lines(run_dir / "storage-close.json") == {
        "state": "eof_without_close",
        "last_applied": client.seq - 1,
        "refused": 0,
        "failed": 0,
        "discarded_bytes": len(frame) - 2,
    }
    report = completion(run_dir)
    assert report["complete"] is False
    assert "storage-close says 'eof_without_close', not 'orderly'" in report["reasons"]


def test_end_of_input_inside_a_skipped_payload(tmp_path, client):
    run_dir = ready(client, tmp_path)
    header = json.dumps({"seq": client.next_seq(), "op": "create", "name": "big"}).encode()
    client.write(LENGTHS.pack(len(header), MAX_PAYLOAD + 10) + header + b"x" * 100)
    assert_refused(client.ack(), "create", errno.EMSGSIZE)
    assert client.end_input() == 1
    assert lines(run_dir / "storage-close.json")["state"] == "eof_without_close"


def test_writer_exits_after_close(tmp_path, client):
    ready(client, tmp_path)
    assert client.request("close")["status"] == "ok"
    assert client.process.wait(10) == 0
    assert client.remaining_output() == b""


def test_close_before_setup_exits_without_a_directory(tmp_path, client):
    assert_refused(client.request("close"), "close", errno.EBADF)
    assert client.process.wait(10) == 1
    assert list(tmp_path.iterdir()) == []


def test_usage_error(tmp_path):
    from writer_client import WRITER

    done = subprocess.run((*WRITER, "extra"), capture_output=True, timeout=10)
    assert done.returncode == 2


def writer_descriptors(pid: int) -> list[str]:
    done = subprocess.run(
        ["lsof", "-a", "-p", str(pid), "-d", "0-1023", "-F", "ft"],
        capture_output=True,
        text=True,
        timeout=10,
        check=True,
    )
    kinds: list[str] = []
    for line in done.stdout.splitlines():
        if line.startswith("f"):
            kinds.append(line[1:])
        elif line.startswith("t") and kinds:
            kinds[-1] += ":" + line[1:]
    return sorted(kinds, key=lambda kind: int(kind.split(":")[0]))


@pytest.mark.skipif(shutil.which("lsof") is None, reason="lsof reports another process's fds")
def test_writer_holds_only_standard_and_run_directory_descriptors(tmp_path, client):
    run_dir = ready(client, tmp_path)
    assert client.request("create", b"x", name="file")["status"] == "ok"
    assert client.request("subjects")["status"] == "ok"
    time.sleep(0.05)
    kinds = writer_descriptors(client.process.pid)
    # Setup closes the evidence-root descriptor after it opens the run
    # directory, so the run directory need not be the lowest free number.
    assert [kind.split(":")[0] for kind in kinds[:3]] == ["0", "1", "2"], kinds
    assert len(kinds) == 4, kinds
    assert kinds[3].endswith(":DIR"), kinds
    assert run_dir.is_dir()


# completion() on copies of a writer-produced directory ---------------------


@pytest.fixture
def produced(tmp_path):
    run = healthy_run(tmp_path / "evidence")
    assert completion(run.run_dir)["complete"] is True
    return run


def copy_of(run_dir: Path, tmp_path: Path) -> Path:
    copy = tmp_path / "copy"
    shutil.copytree(run_dir, copy)
    return copy


def test_completion_rejects_a_missing_storage_close(produced, tmp_path):
    copy = copy_of(produced.run_dir, tmp_path)
    (copy / "storage-close.json").unlink()
    reasons = completion(copy)["reasons"]
    assert "storage-close.json is missing" in reasons
    assert "storage-close record missing" in reasons


def test_completion_rejects_a_changed_storage_close(produced, tmp_path):
    copy = copy_of(produced.run_dir, tmp_path)
    (copy / "storage-close.json").write_text(
        json.dumps({"state": "orderly", "last_applied": 99, "refused": 0, "failed": 0})
    )
    assert completion(copy)["reasons"] == ["storage-close.json is changed"]


def test_completion_rejects_a_missing_identity(produced, tmp_path):
    copy = copy_of(produced.run_dir, tmp_path)
    (copy / "result-identity.json").unlink()
    assert completion(copy)["reasons"][0] == "identity record missing"


def test_completion_rejects_an_unlisted_file(produced, tmp_path):
    copy = copy_of(produced.run_dir, tmp_path)
    (copy / "extra").write_bytes(b"extra")
    assert completion(copy)["reasons"] == ["extra is not in the identity listing"]


def test_completion_rejects_a_changed_digest_record(produced, tmp_path):
    copy = copy_of(produced.run_dir, tmp_path)
    (copy / "workload.stdout").write_bytes(b"changed\n")
    assert completion(copy)["reasons"] == [
        "workload.stdout is changed",
        "digest entry workload.stdout is changed",
    ]


def test_completion_rejects_a_result_after_the_last_applied_frame(tmp_path):
    client = WriterClient()
    try:
        run_dir = ready(client, tmp_path)
        claimed = client.seq + 5
        result = {"storage": {"sequence": claimed, "digests": {}}}
        reply = client.request("result", json.dumps(result).encode(), name="result")
        assert reply["status"] == "ok"
        applied = client.seq
        assert client.request("close")["status"] == "ok"
    finally:
        client.stop()
    assert completion(run_dir)["reasons"] == [
        f"storage-close last applied {applied} precedes result {claimed}"
    ]


def test_writer_and_retention_agree_on_record_files():
    import retention
    import writer

    assert writer.RESERVED_FILES == retention.RESERVED_FILES
    assert writer.IDENTITY == retention.IDENTITY
    assert writer.STORAGE_CLOSE == retention.STORAGE_CLOSE
