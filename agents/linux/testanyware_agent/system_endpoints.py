"""System endpoint handlers: /health, /exec, /exec/cancel, /upload, /download, /shutdown.

None of them takes the AT-SPI lock (spec, section "Responsiveness").
"""

import math
import os
import re
import subprocess
import tempfile
import threading
import time
from collections.abc import Callable
from typing import BinaryIO

from testanyware_agent import exec_owner, exec_registry

# Fixed streaming buffer — memory use is bounded by this regardless of
# file size (ADR-0001: no whole-file buffering on either end).
CHUNK_SIZE = 1024 * 1024  # 1 MiB


def prepare_exec_captures() -> None:
    """Remove the capture directories of an earlier agent run."""
    exec_owner.prepare_capture_root()


def handle_health(accessible: bool) -> tuple[int, dict]:
    return 200, {"accessible": accessible, "platform": "linux"}


EXEC_ID_PATTERN = re.compile(r"[A-Za-z0-9._-]{1,64}")


def handle_exec(body: dict) -> tuple[int, dict]:
    request_start = time.monotonic()
    command = body.get("command", "")
    timeout = body.get("timeout", 30)
    detach = body.get("detach", False)

    if detach:
        # A detached command has no owner and no record (spec "Limits").
        subprocess.Popen(
            ["/bin/bash", "-c", command],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        return 200, {"exitCode": 0, "stdout": "", "stderr": ""}

    if not isinstance(command, str):
        return 400, {"error": "exec_failed", "details": "command must be a string"}
    if (
        isinstance(timeout, bool)
        or not isinstance(timeout, int | float)
        # Python's JSON decoder accepts NaN and Infinity, and the record cannot carry them.
        or (isinstance(timeout, float) and not math.isfinite(timeout))
    ):
        return 400, {"error": "exec_failed", "details": "timeout must be a number of seconds"}
    exec_id = body.get("id")
    if exec_id is None:
        exec_id = exec_owner.new_exec_id()
    elif not isinstance(exec_id, str) or not EXEC_ID_PATTERN.fullmatch(exec_id):
        return 400, {
            "error": "exec_failed",
            "details": "id must have 1 to 64 characters from A-Z, a-z, 0-9, '.', '_' and '-'",
        }

    registry = exec_registry.REGISTRY
    admission = registry.admit(exec_id)
    if admission.kind == "in_use":
        return 409, {
            "error": "exec_id_in_use",
            "details": f"exec id {exec_id} names a running or retained command",
        }
    if admission.kind == "closed":
        return 503, {"error": "exec_failed", "details": "the agent is shutting down"}
    try:
        if admission.kind == "tombstoned":
            # The tombstone was the pending event, so the owner was not late.
            record = exec_owner.cancelled_before_start(exec_id, request_start, 0)
        else:
            record = exec_owner.run_command(
                command, timeout, exec_id, request_start, cancel=admission.signal
            )
    except BaseException:
        registry.abandon(exec_id)
        raise
    registry.publish(exec_id, record)
    return 200, record.to_dict()


def handle_cancel(body: object) -> tuple[int, dict]:
    """`POST /exec/cancel` (spec, section "Cancel endpoint", ADR-0025)."""
    if not isinstance(body, dict):
        return 400, {"error": "exec_failed", "details": "the body must be a JSON object"}
    exec_id = body.get("id")
    if not isinstance(exec_id, str) or not EXEC_ID_PATTERN.fullmatch(exec_id):
        return 400, {
            "error": "exec_failed",
            "details": "id must have 1 to 64 characters from A-Z, a-z, 0-9, '.', '_' and '-'",
        }
    wait_ms = body.get("waitMs", 0)
    if (
        isinstance(wait_ms, bool)
        or not isinstance(wait_ms, int)
        or not 0 <= wait_ms <= exec_registry.CANCEL_WAIT_MAX_MS
    ):
        return 400, {
            "error": "exec_failed",
            "details": f"waitMs must be an integer from 0 to {exec_registry.CANCEL_WAIT_MAX_MS}",
        }
    state, record = exec_registry.REGISTRY.cancel(exec_id, wait_ms / 1000)
    reply: dict = {"state": state}
    if record is not None:
        reply["record"] = record.to_dict()
    return 200, reply


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
    """Open `path` once for streaming (ADR-0001, ADR-0026).

    Returns ``(200, None, file)`` with an open binary file the caller streams
    and closes, or ``(status, ErrorResponse, None)`` (``download_failed``) if
    the file cannot be opened — surfaced before any body bytes are sent. The
    caller takes the length from `fstat` of this handle and sends exactly that
    many bytes from it (`send_exact`).
    """
    if not path:
        return 400, {"error": "download_failed", "details": "missing path query parameter"}, None
    try:
        fileobj = open(path, "rb")
    except Exception as e:
        return 400, {"error": "download_failed", "details": str(e)}, None
    return 200, None, fileobj


def send_exact(fileobj: BinaryIO, length: int, write: Callable[[bytes], object]) -> bool:
    """Send exactly `length` bytes of `fileobj` (spec, "Agent download contract").

    Never sends a byte past `length`, so growth after the `fstat` stays out of
    the body. Returns False when the handle ends first, for example after a
    truncation. The caller must then close the connection, so the body is
    short of its declared length and the client detects it.
    """
    remaining = length
    while remaining > 0:
        chunk = fileobj.read(min(CHUNK_SIZE, remaining))
        if not chunk:
            return False
        write(chunk)
        remaining -= len(chunk)
    return True


def handle_shutdown() -> tuple[int, dict]:
    def do_shutdown():
        subprocess.run(["systemctl", "poweroff"],
                       capture_output=True, timeout=10)

    threading.Timer(0.1, do_shutdown).start()
    return 200, {"success": True, "message": "Shutting down"}
