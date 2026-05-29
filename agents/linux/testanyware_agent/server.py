"""HTTP server for testanyware-agent on Linux.

Uses Python's built-in http.server — zero pip dependencies.
"""

import json
import os
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlsplit

from testanyware_agent import accessibility, system_endpoints


class AgentRequestHandler(BaseHTTPRequestHandler):
    """Route HTTP requests to the appropriate handler."""

    def do_GET(self) -> None:
        if self.path == "/health":
            accessible = accessibility.is_accessible()
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

        routes: dict[str, object] = {
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
            "/window-minimize": lambda: accessibility.handle_window_action(body, "window-minimize"),
            "/wait": lambda: accessibility.handle_wait(body),
            "/exec": lambda: system_endpoints.handle_exec(body),
            "/shutdown": lambda: system_endpoints.handle_shutdown(),
        }

        handler = routes.get(route)
        if handler is None:
            self._send_json(404, {"error": f"Not found: {self.path}"})
            return

        try:
            status, response_body = handler()  # type: ignore[misc]
            self._send_json(status, response_body)
        except Exception as e:
            self._send_json(500, {"error": str(e)})

    def _query_path(self, query: str) -> str:
        return parse_qs(query).get("path", [""])[0]

    def _handle_upload(self, query: str) -> None:
        path = self._query_path(query)
        content_length = int(self.headers.get("Content-Length", 0))
        status, body = system_endpoints.handle_upload(path, self.rfile, content_length)
        self._send_json(status, body)

    def _handle_download(self, query: str) -> None:
        path = self._query_path(query)
        status, error, fileobj = system_endpoints.handle_download(path)
        if fileobj is None:
            self._send_json(status, error)
            return
        with fileobj:
            size = os.fstat(fileobj.fileno()).st_size
            self.send_response(status)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(size))
            self.end_headers()
            while chunk := fileobj.read(system_endpoints.CHUNK_SIZE):
                self.wfile.write(chunk)

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


def run_server(port: int = 8648) -> None:
    server = HTTPServer(("0.0.0.0", port), AgentRequestHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
