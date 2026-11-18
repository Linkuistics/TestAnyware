"""HTTP server for testanyware-agent on Linux.

Uses Python's built-in http.server — zero pip dependencies.

Each request runs on its own thread, so a running guest command never blocks
health, observation, cancellation or transfer (spec
docs/specs/guest-command-lifecycle.md, section "Responsiveness"). The
accessibility endpoints share `accessibility.AT_SPI_LOCK`. The system
endpoints never take it.
"""

from __future__ import annotations

import json
import os
import signal
import sys
import threading
import time
from collections.abc import Callable
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from types import FrameType
from urllib.parse import parse_qs, urlsplit

from testanyware_agent import accessibility, exec_registry, system_endpoints

# Health waits this long for a fresh accessibility probe, then replies with the
# newest value. The health bound during a running command is 1 s.
HEALTH_PROBE_WAIT_S = 0.5

# From SIGTERM to the last published record: the root grace bound of 2 s plus
# the root kill, drain and reap bound of 2 s. The owner schedule needs 3.8 s
# after its outcome.
SHUTDOWN_WAIT_S = 4.0


def _log(message: str) -> None:
    print(f"testanyware-agent: {message}", file=sys.stderr, flush=True)


class AccessibilityProbe:
    """The `accessible` value of health, which never takes the AT-SPI lock.

    One thread calls the check under the lock when health asks for a fresh
    value. Health waits a bounded time for it and then replies with the newest
    value. Before the first probe finishes, that value is False.
    """

    def __init__(self, check: Callable[[], bool], at_spi_lock: threading.Lock) -> None:
        self._check = check
        self._at_spi_lock = at_spi_lock
        self._changed = threading.Condition()
        self._value = False
        self._generation = 0
        self._requested = False
        threading.Thread(target=self._run, name="accessibility-probe", daemon=True).start()

    def current(self, wait_s: float) -> bool:
        with self._changed:
            generation = self._generation
            self._requested = True
            self._changed.notify_all()
            self._changed.wait_for(lambda: self._generation != generation, wait_s)
            return self._value

    def _run(self) -> None:
        while True:
            with self._changed:
                self._changed.wait_for(lambda: self._requested)
                self._requested = False
            with self._at_spi_lock:
                try:
                    value = self._check()
                except Exception:  # a failed check must not end the probe thread
                    value = False
            with self._changed:
                self._value = value
                self._generation += 1
                self._changed.notify_all()


def _new_probe() -> AccessibilityProbe:
    # The lambda reads the module attribute at each call, so a test can replace it.
    return AccessibilityProbe(lambda: accessibility.is_accessible(), accessibility.AT_SPI_LOCK)


_fallback_probe: AccessibilityProbe | None = None
_fallback_lock = threading.Lock()


def _probe_of(server: object) -> AccessibilityProbe:
    """The probe of an `AgentServer`, or one shared probe for another server."""
    probe = getattr(server, "accessibility_probe", None)
    if probe is not None:
        return probe
    global _fallback_probe
    with _fallback_lock:
        if _fallback_probe is None:
            _fallback_probe = _new_probe()
        return _fallback_probe


class AgentRequestHandler(BaseHTTPRequestHandler):
    """Route HTTP requests to the appropriate handler."""

    def do_GET(self) -> None:
        if self.path == "/health":
            accessible = _probe_of(self.server).current(HEALTH_PROBE_WAIT_S)
            status, body = system_endpoints.handle_health(accessible)
            self._send_json(status, body)
        else:
            self._send_json(404, {"error": f"Not found: {self.path}"})

    def do_POST(self) -> None:
        parsed = urlsplit(self.path)
        route = parsed.path

        # /upload and /download stream raw octet-stream bodies (ADR-0001):
        # branch *before* _read_body() consumes self.rfile as JSON.
        if route == "/upload":
            self._handle_upload(parsed.query)
            return
        if route == "/download":
            self._handle_download(parsed.query)
            return

        body, parse_error = self._read_body()
        if body is None:
            self._send_json(400, {"error": "invalid_json", "details": parse_error})
            return

        # Each of these calls AT-SPI and runs under the AT-SPI lock.
        serialised: dict[str, Callable[[], tuple[int, dict]]] = {
            "/windows": lambda: accessibility.handle_windows(),
            "/snapshot": lambda: accessibility.handle_snapshot(body),
            "/inspect": lambda: accessibility.handle_inspect(body),
            "/press": lambda: accessibility.handle_action(body, "press"),
            "/focus": lambda: accessibility.handle_action(body, "focus"),
            "/show-menu": lambda: accessibility.handle_action(body, "show-menu"),
            "/set-value": lambda: accessibility.handle_set_value(body),
            "/window-focus": lambda: accessibility.handle_window_action(body, "window-focus"),
            "/window-resize": lambda: accessibility.handle_window_resize(body),
            "/window-move": lambda: accessibility.handle_window_move(body),
            "/window-close": lambda: accessibility.handle_window_action(body, "window-close"),
            "/window-minimize": lambda: accessibility.handle_window_action(
                body, "window-minimize"
            ),
        }
        # These never take the AT-SPI lock. `/wait` takes it for each poll.
        unlocked: dict[str, Callable[[], tuple[int, dict]]] = {
            "/wait": lambda: accessibility.handle_wait(body),
            "/exec": lambda: system_endpoints.handle_exec(body),
            "/exec/cancel": lambda: system_endpoints.handle_cancel(body),
            "/shutdown": lambda: system_endpoints.handle_shutdown(),
        }

        try:
            if route in serialised:
                with accessibility.AT_SPI_LOCK:
                    status, response_body = serialised[route]()
            elif route in unlocked:
                status, response_body = unlocked[route]()
            else:
                self._send_json(404, {"error": f"Not found: {self.path}"})
                return
            self._send_json(status, response_body)
        except Exception as e:
            self._send_json(500, {"error": str(e)})

    def _query_path(self, query: str) -> str:
        return parse_qs(query).get("path", [""])[0]

    def _handle_upload(self, query: str) -> None:
        path = self._query_path(query)
        content_length = int(self.headers.get("Content-Length", 0))
        chunked = "chunked" in self.headers.get("Transfer-Encoding", "").lower()
        status, body = system_endpoints.handle_upload(
            path, self.rfile, content_length, chunked=chunked
        )
        self._send_json(status, body)

    def _handle_download(self, query: str) -> None:
        path = self._query_path(query)
        status, error, fileobj = system_endpoints.handle_download(path)
        if fileobj is None:
            self._send_json(status, error)
            return
        with fileobj:
            # One handle gives the length and the bytes (ADR-0026).
            size = os.fstat(fileobj.fileno()).st_size
            self.send_response(status)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(size))
            self.end_headers()
            if not system_endpoints.send_exact(fileobj, size, self.wfile.write):
                # The handle ended before the declared length. Closing the
                # connection leaves the body short, so the client detects it.
                self.close_connection = True

    def _read_body(self) -> tuple[dict | None, str]:
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0:
            return {}, ""
        raw = self.rfile.read(content_length)
        try:
            return json.loads(raw), ""
        except (json.JSONDecodeError, ValueError) as e:
            return None, str(e)

    def _send_json(self, status: int, body: dict) -> None:
        payload = json.dumps(body, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format: str, *args: object) -> None:
        # Suppress default stderr logging for cleaner output
        pass


class AgentServer(ThreadingHTTPServer):
    """One thread per request. The threads are daemons, so a running command
    never delays `server_close`.

    Source: https://docs.python.org/3.12/library/http.server.html#http.server.ThreadingHTTPServer
    and CPython 3.12 Lib/socketserver.py (`ThreadingMixIn.server_close` joins
    only non-daemon threads).
    """

    daemon_threads = True

    def __init__(self, address: tuple[str, int]) -> None:
        super().__init__(address, AgentRequestHandler)
        self.accessibility_probe = _new_probe()


def make_server(host: str, port: int) -> AgentServer:
    return AgentServer((host, port))


class _Terminate(BaseException):
    """Raised by the SIGTERM handler to end `serve_forever` in the main thread."""


def shut_down(server: AgentServer, until: float) -> list[str]:
    """Stop the agent (spec, section "Agent shutdown").

    Stop admission, cancel every running command, close the listening socket
    and wait until each record is published or `until` (monotonic) passes.
    A leader whose status stays unavailable does not delay this, because its
    owner publishes on schedule. Returns the ids still unpublished.
    """
    running = exec_registry.REGISTRY.close()
    server.server_close()
    pending = exec_registry.REGISTRY.wait_published(running, until)
    if pending:
        _log(f"exiting before these records were published: {', '.join(pending)}")
    return pending


def run_server(port: int = 8648) -> None:
    system_endpoints.prepare_exec_captures()
    server = make_server("0.0.0.0", port)

    def on_sigterm(signum: int, frame: FrameType | None) -> None:
        raise _Terminate

    signal.signal(signal.SIGTERM, on_sigterm)
    try:
        server.serve_forever()
    except (_Terminate, KeyboardInterrupt):
        pass
    finally:
        until = time.monotonic() + SHUTDOWN_WAIT_S
        # A second SIGTERM during the bounded wait ends the agent at once.
        signal.signal(signal.SIGTERM, signal.SIG_DFL)
        shut_down(server, until)
