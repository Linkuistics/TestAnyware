"""Retained-evidence controls: stream pins, result identity and later verification."""

import hashlib
import importlib
import json
import subprocess
import sys
import time
from dataclasses import replace
from pathlib import Path

import pytest

import controller
from test_controller import HERE, call, inspect_pid, mutate_function, run

EXPECTED = bytes.fromhex("00ff80410a")


def retention():
    # Import at use, so a missing module fails each control separately.
    return importlib.import_module("retention")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def stream_states(report):
    return {(item["call"], item["stream"]): item["state"] for item in report["streams"]}


def file_states(report):
    return {item["path"]: item["state"] for item in report["files"]}


def test_review_streams_are_pinned_from_the_bytes_in_memory(tmp_path):
    path, result = run(tmp_path, [call("bytes", "bytes")])
    receipt = result["calls"][0]
    streams = receipt["streams"]
    assert streams["stdout"]["length"] == len(EXPECTED)
    assert streams["stdout"]["sha256"] == sha(EXPECTED)
    assert streams["stderr"]["length"] == len(b"separate stderr\n")
    assert streams["stderr"]["sha256"] == sha(b"separate stderr\n")
    for stream, pin in streams.items():
        assert pin["completeness"] == "eof"
        assert pin["record"] == f"bytes.{stream}"
        assert pin["final_reads"] <= controller.FINAL_READS
    assert (path / "bytes.stdout").read_bytes() == EXPECTED
    stored = json.loads((path / "bytes.json").read_text())
    assert stored["streams"] == streams
    identity = json.loads((path / "result-identity.json").read_text())
    listed = {item["path"] for item in identity["files"]}
    assert {"result.json", "plan.json", "bytes.stdout", "bytes.stderr", "bytes.json"} <= listed
    assert "result-identity.json" not in listed
    assert "result-identity" not in (path / "result.json").read_text()
    entry = next(item for item in identity["files"] if item["path"] == "result.json")
    assert entry["sha256"] == sha((path / "result.json").read_bytes())
    assert identity["started_monotonic"] <= identity["finished_monotonic"]
    report = retention().verify(path)
    assert report["ok"] is True
    assert set(stream_states(report).values()) == {"unchanged"}


def test_review_wrong_stdout_is_judged_from_the_pinned_bytes(tmp_path):
    bad_oracle = replace(call("wrong-absence", "bytes"), expected_stdout=b"absent\n")
    _, result = run(tmp_path, [bad_oracle, call("later", "bytes")])
    receipt = result["calls"][0]
    assert receipt["returncode"] == 0
    assert receipt["stdout_matches"] is False
    assert receipt["streams"]["stdout"]["sha256"] == sha(EXPECTED)
    assert result["ok"] is False
    assert result["unreached"] == ["later"]


def store_with_late_bytes(monkeypatch):
    """Send the writer one more stdout line than the controller read."""
    original = controller._RunStorage.stream

    def stream_with_late_bytes(self, name, chunk, *, create=False):
        if name.endswith(".stdout") and chunk:
            chunk += b"late\n"
        original(self, name, chunk, create=create)

    monkeypatch.setattr(controller._RunStorage, "stream", stream_with_late_bytes)


def test_review_exact_stdout_uses_the_bytes_in_memory(tmp_path, monkeypatch):
    store_with_late_bytes(monkeypatch)
    exact = replace(call("bytes", "bytes"), expected_stdout=EXPECTED)
    path, result = run(tmp_path, [exact])
    receipt = result["calls"][0]
    assert receipt["stdout_matches"] is True
    assert receipt["streams"]["stdout"]["sha256"] == sha(EXPECTED)
    assert (path / "bytes.stdout").read_bytes() == EXPECTED + b"late\n"
    report = retention().verify(path)
    assert stream_states(report)[("bytes", "stdout")] == "changed"
    assert report["ok"] is False


def test_review_mutation_rereading_stdout_loses_the_memory_verdict(tmp_path, monkeypatch):
    store_with_late_bytes(monkeypatch)
    mutate_function(
        monkeypatch,
        controller,
        "_call",
        'bytes(capture.streams["stdout"].data) == call.expected_stdout',
        '(root / f"{call.name}.stdout").read_bytes() == call.expected_stdout',
    )
    exact = replace(call("bytes", "bytes"), expected_stdout=EXPECTED)
    _, result = run(tmp_path, [exact])
    # The readback finds no file, an empty file or the stored bytes. None of
    # them equals the bytes in memory, so the mutated verdict never passes.
    assert result["calls"][0].get("stdout_matches") is not True
    assert result["ok"] is False


def test_review_late_writer_cannot_change_the_retained_stream(tmp_path):
    release, declaration = tmp_path / "release", tmp_path / "late.spawned"
    writer = replace(
        call("late-writer", "late-writer", release, declaration), expected_stdout=b"early\n"
    )
    path = None
    child = None
    audit: dict[str, object] = {}
    try:
        path, result = run(tmp_path, [writer, call("later", "bytes")])
        child = int(declaration.read_text())
        receipt = result["calls"][0]
        assert receipt["reason"] == "exit"
        assert receipt["returncode"] == 0
        assert receipt["stdout_matches"] is True
        assert receipt["streams"]["stdout"] == {
            **receipt["streams"]["stdout"],
            "length": 6,
            "sha256": sha(b"early\n"),
            "completeness": "writer_open",
        }
        assert receipt["streams"]["stderr"]["completeness"] == "writer_open"
        # The live group fails termination before any late-write verdict exists.
        assert receipt["termination_ok"] is False
        assert any(item["pid"] == child for item in receipt["residual_members"])
        assert receipt["residual_remedy"]
        assert result["ok"] is False
        assert result["unreached"] == ["later"]
        audit["child_before_release"] = inspect_pid(child)
        assert audit["child_before_release"]["state"] == "live"
        before = retention().verify(path)
        audit["verify_before_release"] = before
        assert before["ok"] is True
        release.write_bytes(b"release")
        end = time.monotonic() + 1
        observations = [inspect_pid(child)]
        while observations[-1]["state"] != "absent" and time.monotonic() < end:
            time.sleep(0.005)
            observations.append(inspect_pid(child))
        audit["child_after_release"] = observations
        assert observations[-1]["state"] == "absent"
        # The controller closed its read end, so the late write meets a
        # broken pipe. The stored stream keeps exactly the bytes in memory.
        assert (path / "late-writer.stdout").read_bytes() == b"early\n"
        after = retention().verify(path)
        audit["verify_after_release"] = after
        assert after["ok"] is True
        assert stream_states(after)[("late-writer", "stdout")] == "unchanged"
        assert file_states(after)["late-writer.stdout"] == "unchanged"
        # The original failed receipt and result stay as they were.
        assert file_states(after)["result.json"] == "unchanged"
        assert file_states(after)["late-writer.json"] == "unchanged"
    finally:
        release.write_bytes(b"release")
        if path is not None:
            if child is not None and "child_after_release" not in audit:
                end = time.monotonic() + 1
                observations = [inspect_pid(child)]
                while observations[-1]["state"] != "absent" and time.monotonic() < end:
                    time.sleep(0.005)
                    observations.append(inspect_pid(child))
                audit["child_cleanup"] = observations
            # Written after the last verification, so it is outside the identity.
            (path.parent / f"{path.name}-late-writer-audit.json").write_text(
                json.dumps(audit, indent=2, default=str)
            )


def change(path: Path) -> None:
    path.write_bytes(path.read_bytes() + b"changed")


MUTATIONS = {
    "result-changed": (lambda p: change(p / "result.json"), "files", "result.json", "changed"),
    "result-missing": (lambda p: (p / "result.json").unlink(), "files", "result.json", "missing"),
    "receipt-changed": (lambda p: change(p / "bytes.json"), "files", "bytes.json", "changed"),
    "stdout-missing": (
        lambda p: (p / "bytes.stdout").unlink(),
        "streams",
        ("bytes", "stdout"),
        "missing",
    ),
    "stderr-changed": (
        lambda p: change(p / "bytes.stderr"),
        "streams",
        ("bytes", "stderr"),
        "changed",
    ),
    "unlisted-added": (lambda p: (p / "extra.json").write_text("{}"), "unlisted", "extra.json", ""),
}


@pytest.mark.parametrize("mutation", sorted(MUTATIONS))
def test_review_verify_rejects_changed_missing_and_added_files(tmp_path, mutation):
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    assert retention().verify(path)["ok"] is True
    apply, section, key, state = MUTATIONS[mutation]
    apply(path)
    report = retention().verify(path)
    assert report["ok"] is False
    if section == "files":
        assert file_states(report)[key] == state
    elif section == "streams":
        assert stream_states(report)[key] == state
    else:
        assert key in report["unlisted"]


@pytest.mark.parametrize("corruption", ["digest", "syntax", "missing", "no-result"])
def test_review_verify_rejects_corrupted_result_identity(tmp_path, corruption):
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    identity_path = path / "result-identity.json"
    identity = json.loads(identity_path.read_text())
    if corruption == "digest":
        entry = next(item for item in identity["files"] if item["path"] == "result.json")
        entry["sha256"] = "0" * 64
        identity_path.write_text(json.dumps(identity))
    elif corruption == "syntax":
        identity_path.write_text("{")
    elif corruption == "missing":
        identity_path.unlink()
    else:
        identity["files"] = [item for item in identity["files"] if item["path"] != "result.json"]
        identity_path.write_text(json.dumps(identity))
    report = retention().verify(path)
    assert report["ok"] is False
    if corruption == "digest":
        assert file_states(report)["result.json"] == "changed"
    elif corruption == "missing":
        assert report["identity"] == "missing"
    else:
        assert report["identity"].startswith("invalid")


def test_review_verify_cannot_see_a_change_reverted_before_the_check(tmp_path):
    # Limit control: a digest compares two snapshots, not the interval between them.
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    original = (path / "bytes.stdout").read_bytes()
    (path / "bytes.stdout").write_bytes(b"intervening")
    (path / "bytes.stdout").write_bytes(original)
    assert retention().verify(path)["ok"] is True


def test_review_late_write_is_caught_without_the_stream_comparison(tmp_path, monkeypatch):
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    module = retention()
    # Rewrite the identity after the late write: the receipt pin and the
    # storage digest entry still differ. Each one detects the change alone.
    change(path / "bytes.stdout")
    (path / "result-identity.json").unlink()
    module.write_identity(path)
    report = module.verify(path)
    assert report["ok"] is False
    assert stream_states(report)[("bytes", "stdout")] == "changed"
    assert report["completion"]["complete"] is False
    mutate_function(monkeypatch, module, "verify", "and streams_ok", "")
    assert module.verify(path)["ok"] is False
    monkeypatch.undo()
    mutate_function(monkeypatch, module, "verify", "and streams_ok and complete_ok", "")
    assert module.verify(path)["ok"] is True


def test_verify_refuses_a_run_directory_from_before_the_writer(tmp_path):
    # A run directory from before the writer has no storage-close record and no
    # storage section. Its own identity, files and streams still verify.
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    module = retention()
    result = json.loads((path / "result.json").read_text())
    del result["storage"]
    for name in ("result.json", "storage-close.json", "result-identity.json"):
        (path / name).unlink()
    (path / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    module.write_identity(path)
    report = module.verify(path)
    assert report["identity"] == "ok"
    assert all(state == "unchanged" for state in file_states(report).values())
    assert all(state == "unchanged" for state in stream_states(report).values())
    assert report["ok"] is False
    assert report["completion"]["complete"] is False
    assert any("storage-close" in reason for reason in report["completion"]["reasons"])


def test_review_verify_command_reports_and_exits_on_the_verdict(tmp_path):
    path, _ = run(tmp_path, [call("bytes", "bytes")])
    script = HERE / "retention.py"
    first = tmp_path / "verify-1.json"
    passed = subprocess.run(
        [sys.executable, str(script), "verify", str(path), "--out", str(first)],
        capture_output=True,
        timeout=10,
    )
    assert passed.returncode == 0, passed.stderr
    assert json.loads(first.read_text())["ok"] is True
    change(path / "result.json")
    second = tmp_path / "verify-2.json"
    failed = subprocess.run(
        [sys.executable, str(script), "verify", str(path), "--out", str(second)],
        capture_output=True,
        timeout=10,
    )
    assert failed.returncode == 1
    assert json.loads(second.read_text())["ok"] is False
    reused = subprocess.run(
        [sys.executable, str(script), "verify", str(path), "--out", str(second)],
        capture_output=True,
        timeout=10,
    )
    assert reused.returncode == 2
