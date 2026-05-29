"""System endpoint handlers: /health, /exec, /upload, /download, /shutdown."""

import os
import subprocess
import tempfile
import threading
from typing import BinaryIO

# Fixed streaming buffer — memory use is bounded by this regardless of
# file size (ADR-0001: no whole-file buffering on either end).
CHUNK_SIZE = 1024 * 1024  # 1 MiB


def handle_health(accessible: bool) -> tuple[int, dict]:
    return 200, {"accessible": accessible, "platform": "linux"}


def handle_exec(body: dict) -> tuple[int, dict]:
    command = body.get("command", "")
    timeout = body.get("timeout", 30)
    detach = body.get("detach", False)

    if detach:
        subprocess.Popen(
            ["/bin/bash", "-c", command],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        return 200, {"exitCode": 0, "stdout": "", "stderr": ""}

    try:
        result = subprocess.run(
            ["/bin/bash", "-c", command],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return 200, {
            "exitCode": result.returncode,
            "stdout": result.stdout.rstrip(),
            "stderr": result.stderr.rstrip(),
        }
    except subprocess.TimeoutExpired:
        return 200, {
            "exitCode": -1,
            "stdout": "",
            "stderr": "Process timed out",
        }
    except Exception as e:
        return 200, {
            "exitCode": -1,
            "stdout": "",
            "stderr": str(e),
        }


def handle_upload(
    path: str, rfile: BinaryIO, content_length: int, chunked: bool = False
) -> tuple[int, dict]:
    """Stream `content_length` bytes from `rfile` into `path` (ADR-0001).

    The body is written to a temp file in the destination's own directory,
    then atomically renamed into place; any error unlinks the temp so the
    destination is never left truncated. Returns an ``ActionResponse`` on
    success or an ``ErrorResponse`` (``upload_failed``) on failure.

    `http.server` cannot decode a chunked request body and reads only
    `Content-Length`, so a chunked upload would silently write a 0-byte file.
    Reject it loudly (411 Length Required) instead — clients must advertise
    the length, as the host CLI does.
    """
    if not path:
        return 400, {"error": "upload_failed", "details": "missing path query parameter"}

    if chunked:
        return 411, {
            "error": "upload_failed",
            "details": "chunked transfer-encoding unsupported; send Content-Length",
        }

    parent = os.path.dirname(path) or "."
    try:
        os.makedirs(parent, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=parent, prefix=".testanyware-upload-")
        try:
            remaining = content_length
            with os.fdopen(fd, "wb") as out:
                while remaining > 0:
                    chunk = rfile.read(min(CHUNK_SIZE, remaining))
                    if not chunk:
                        break
                    out.write(chunk)
                    remaining -= len(chunk)
            if remaining != 0:
                raise OSError(f"incomplete upload: {remaining} bytes not received")
            os.replace(tmp, path)
        except BaseException:
            try:
                os.unlink(tmp)
            except OSError:
                pass
            raise
        return 200, {"success": True, "message": f"Uploaded to {path}"}
    except Exception as e:
        return 400, {"error": "upload_failed", "details": str(e)}


def handle_download(path: str) -> tuple[int, dict | None, BinaryIO | None]:
    """Open `path` for streaming (ADR-0001).

    Returns ``(200, None, file)`` with an open binary file the caller streams
    and closes, or ``(status, ErrorResponse, None)`` (``download_failed``) if
    the file cannot be opened — surfaced before any body bytes are sent.
    """
    if not path:
        return 400, {"error": "download_failed", "details": "missing path query parameter"}, None
    try:
        fileobj = open(path, "rb")
    except Exception as e:
        return 400, {"error": "download_failed", "details": str(e)}, None
    return 200, None, fileobj


def handle_shutdown() -> tuple[int, dict]:
    def do_shutdown():
        subprocess.run(["systemctl", "poweroff"],
                       capture_output=True, timeout=10)

    threading.Timer(0.1, do_shutdown).start()
    return 200, {"success": True, "message": "Shutting down"}
