"""Streaming /upload and /download behaviour for the Linux agent (ADR-0001).

Unit tests exercise `system_endpoints.handle_upload` / `handle_download`
directly (no socket); the integration test drives the real
`AgentRequestHandler` over the production server class to prove the `do_POST`
branch-before-`_read_body` routing, query-param path parsing, and raw
octet-stream streaming on both halves.

The download contract tests follow the consistency table of
docs/specs/guest-command-lifecycle.md, section "Evidence transfer", and
ADR-0026. A wrapper of the opened file changes the file after the agent took
its length, before the first read. The old-fail control of the row "Download
never sends extra bytes" is in tests/test_agent_process.py.
"""

from __future__ import annotations

import hashlib
import http.client
import io
import os
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import BinaryIO
from urllib.parse import quote

import pytest

from testanyware_agent import system_endpoints
from tests.http_agent import in_process_agent, post_request, raw_exchange

# ---------------------------------------------------------------------------
# Upload unit tests
# ---------------------------------------------------------------------------

def test_upload_streams_body_to_destination(tmp_path) -> None:
    dest = tmp_path / "nested" / "dir" / "payload.bin"
    data = os.urandom(3 * 1024 * 1024)  # 3 MiB — exercises multi-chunk loop

    status, body = system_endpoints.handle_upload(str(dest), io.BytesIO(data), len(data))

    assert status == 200
    assert body["success"] is True
    assert dest.read_bytes() == data
    # parent directories are created on demand
    assert dest.parent.is_dir()


def test_upload_leaves_no_temp_file_on_success(tmp_path) -> None:
    dest = tmp_path / "payload.bin"
    data = b"hello world"

    system_endpoints.handle_upload(str(dest), io.BytesIO(data), len(data))

    # the destination is the only file in the directory — temp renamed away
    assert [p.name for p in tmp_path.iterdir()] == ["payload.bin"]


def test_upload_truncated_body_fails_without_corrupting_destination(tmp_path) -> None:
    dest = tmp_path / "payload.bin"
    dest.write_bytes(b"ORIGINAL")  # pre-existing file must survive a failed upload
    short = io.BytesIO(b"only-50")  # claims more than it delivers

    status, body = system_endpoints.handle_upload(str(dest), short, content_length=999)

    assert status == 400
    assert body["error"] == "upload_failed"
    assert "details" in body
    assert dest.read_bytes() == b"ORIGINAL"  # never truncated
    # the temp file was unlinked — only the original remains
    assert [p.name for p in tmp_path.iterdir()] == ["payload.bin"]


def test_upload_missing_path_fails(tmp_path) -> None:
    status, body = system_endpoints.handle_upload("", io.BytesIO(b"x"), 1)

    assert status == 400
    assert body["error"] == "upload_failed"


def test_upload_rejects_chunked_transfer_encoding(tmp_path) -> None:
    """http.server cannot decode a chunked request body, so an upload that
    arrives chunked (no usable Content-Length) must fail loudly with
    ``upload_failed`` rather than silently writing a 0-byte file. The
    destination must not be created."""
    dest = tmp_path / "payload.bin"

    status, body = system_endpoints.handle_upload(
        str(dest), io.BytesIO(b"ignored"), content_length=0, chunked=True
    )

    assert status == 411  # Length Required
    assert body["error"] == "upload_failed"
    assert "chunked" in body["details"].lower()
    assert not dest.exists()  # nothing written
    assert list(tmp_path.iterdir()) == []  # not even a temp file


# ---------------------------------------------------------------------------
# Download unit tests
# ---------------------------------------------------------------------------

def test_download_opens_existing_file(tmp_path) -> None:
    src = tmp_path / "payload.bin"
    data = os.urandom(1024)
    src.write_bytes(data)

    status, error, fileobj = system_endpoints.handle_download(str(src))

    assert status == 200
    assert error is None
    assert fileobj is not None
    with fileobj:
        assert fileobj.read() == data


def test_download_missing_file_returns_download_failed(tmp_path) -> None:
    missing = tmp_path / "nope"

    status, error, fileobj = system_endpoints.handle_download(str(missing))

    assert status == 400
    assert error["error"] == "download_failed"
    assert "details" in error
    assert fileobj is None


# ---------------------------------------------------------------------------
# End-to-end integration over a live HTTPServer
# ---------------------------------------------------------------------------

@pytest.fixture
def agent_server() -> Iterator[tuple[str, int]]:
    # tests/agent_server.py supplies AT-SPI stand-ins where pyatspi is missing.
    with in_process_agent() as port:
        yield "127.0.0.1", port


def _sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def test_round_trip_large_unicode_path(agent_server, tmp_path) -> None:
    host, port = agent_server
    dest = tmp_path / "файл スペース.bin"  # Unicode + space in the guest path
    data = os.urandom(5 * 1024 * 1024)  # 5 MiB — well past the old macOS cap

    # --- upload ---
    conn = http.client.HTTPConnection(host, port)
    conn.request(
        "POST",
        f"/upload?path={quote(str(dest))}",
        body=data,
        headers={"Content-Type": "application/octet-stream",
                 "Content-Length": str(len(data))},
    )
    resp = conn.getresponse()
    assert resp.status == 200
    resp.read()
    conn.close()
    assert dest.read_bytes() == data

    # --- download ---
    conn = http.client.HTTPConnection(host, port)
    conn.request("POST", f"/download?path={quote(str(dest))}")
    resp = conn.getresponse()
    assert resp.status == 200
    assert resp.getheader("Content-Type") == "application/octet-stream"
    received = resp.read()
    conn.close()
    assert _sha(received) == _sha(data)


def test_download_missing_file_over_http(agent_server, tmp_path) -> None:
    host, port = agent_server
    missing = tmp_path / "does-not-exist"

    conn = http.client.HTTPConnection(host, port)
    conn.request("POST", f"/download?path={quote(str(missing))}")
    resp = conn.getresponse()
    assert resp.status == 400
    assert resp.getheader("Content-Type") == "application/json"
    import json
    body = json.loads(resp.read())
    conn.close()
    assert body["error"] == "download_failed"


# ---------------------------------------------------------------------------
# Download contract (ADR-0026): one handle, exactly the declared length
# ---------------------------------------------------------------------------


class ChangingFile:
    """The file that `handle_download` opened. Before the first read it runs
    one change of the file, after the agent took the length from `fstat`."""

    def __init__(self, inner: BinaryIO, change: Callable[[], None]) -> None:
        self._inner = inner
        self._change: Callable[[], None] | None = change

    def fileno(self) -> int:
        return self._inner.fileno()

    def read(self, n: int = -1) -> bytes:
        if self._change is not None:
            change, self._change = self._change, None
            change()
        return self._inner.read(n)

    def close(self) -> None:
        self._inner.close()

    def __enter__(self) -> ChangingFile:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


def download_with_change(
    port: int, path: Path, change: Callable[[], None], monkeypatch: pytest.MonkeyPatch
):  # type: ignore[no-untyped-def]
    real_open = open

    def changing_open(file: str, mode: str = "r", *args, **kwargs):  # type: ignore[no-untyped-def]
        return ChangingFile(real_open(file, mode, *args, **kwargs), change)

    monkeypatch.setattr(system_endpoints, "open", changing_open, raising=False)
    try:
        return raw_exchange(port, post_request(f"/download?path={quote(str(path))}"))
    finally:
        monkeypatch.undo()


VERSION_1 = b"version one of the evidence\n" * 1000
VERSION_2 = b"VERSION TWO, A DIFFERENT LENGTH\n" * 1500


def test_unchanged_file_gives_the_exact_bytes(agent_server, tmp_path) -> None:
    _, port = agent_server
    path = tmp_path / "evidence.txt"
    path.write_bytes(VERSION_1)
    raw = raw_exchange(port, post_request(f"/download?path={quote(str(path))}"))
    assert (raw.declared, raw.body, raw.extra) == (len(VERSION_1), VERSION_1, b"")


def test_append_during_the_read_gives_the_first_l_bytes(
    agent_server, tmp_path, monkeypatch
) -> None:
    _, port = agent_server
    path = tmp_path / "evidence.txt"
    path.write_bytes(VERSION_1)

    def append() -> None:
        with open(path, "ab") as f:
            f.write(b"appended after the length was taken\n" * 100)

    raw = download_with_change(port, path, append, monkeypatch)
    assert raw.declared == len(VERSION_1)
    assert raw.body == VERSION_1
    assert raw.extra == b"", f"{len(raw.extra)} bytes followed the declared length"


def test_rename_replacement_gives_the_version_open_at_the_start(
    agent_server, tmp_path, monkeypatch
) -> None:
    _, port = agent_server
    path = tmp_path / "evidence.txt"
    path.write_bytes(VERSION_1)
    fresh = tmp_path / "evidence.txt.new"
    fresh.write_bytes(VERSION_2)

    raw = download_with_change(port, path, lambda: os.replace(fresh, path), monkeypatch)
    assert (raw.declared, raw.body, raw.extra) == (len(VERSION_1), VERSION_1, b"")
    assert path.read_bytes() == VERSION_2


def test_truncation_during_the_read_gives_a_short_body(
    agent_server, tmp_path, monkeypatch
) -> None:
    """The agent closes the connection, so the client sees fewer bytes than the
    declared length and detects the failure (`CONNECTION_DROPPED`)."""
    _, port = agent_server
    path = tmp_path / "evidence.txt"
    path.write_bytes(VERSION_1)

    def truncate() -> None:
        os.truncate(path, 100)

    raw = download_with_change(port, path, truncate, monkeypatch)
    assert raw.declared == len(VERSION_1)
    assert raw.body == VERSION_1[:100]
    assert len(raw.body) < raw.declared
    assert raw.extra == b""
