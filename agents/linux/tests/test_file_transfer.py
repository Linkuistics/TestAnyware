"""Streaming /upload and /download behaviour for the Linux agent (ADR-0001).

Unit tests exercise `system_endpoints.handle_upload` / `handle_download`
directly (no socket); the integration test drives the real
`AgentRequestHandler` over a live `HTTPServer` to prove the `do_POST`
branch-before-`_read_body` routing, query-param path parsing, and raw
octet-stream streaming on both halves.
"""

from __future__ import annotations

import hashlib
import http.client
import io
import os
import threading
from http.server import HTTPServer
from urllib.parse import quote

import pytest

from testanyware_agent import system_endpoints

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
def agent_server():
    # server.py imports the accessibility module, which needs pyatspi
    # (Linux-only). Skip the live-socket tests where it is unavailable;
    # they run on the Linux agent and in CI.
    pytest.importorskip("pyatspi")
    from testanyware_agent.server import AgentRequestHandler

    server = HTTPServer(("127.0.0.1", 0), AgentRequestHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_address  # (host, port)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


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
