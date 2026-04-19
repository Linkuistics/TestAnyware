"""HTTP server for testanyware-agent on Linux.

Uses Python's built-in http.server — zero pip dependencies.
"""

import json
from http.server import HTTPServer, BaseHTTPRequestHandler

from testanyware_agent import accessibility
from testanyware_agent import system_endpoints


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
        body = self._read_body()
        if body is None:
            self._send_json(400, {"error": "Invalid JSON in request body"})
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
            "/upload": lambda: system_endpoints.handle_upload(body),
            "/download": lambda: system_endpoints.handle_download(body),
            "/shutdown": lambda: system_endpoints.handle_shutdown(),
        }

        handler = routes.get(self.path)
        if handler is None:
            self._send_json(404, {"error": f"Not found: {self.path}"})
            return

        try:
            status, response_body = handler()  # type: ignore[misc]
            self._send_json(status, response_body)
        except Exception as e:
            self._send_json(500, {"error": str(e)})

    def _read_body(self) -> dict | None:
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0:
            return {}
        raw = self.rfile.read(content_length)
        try:
            return json.loads(raw)
        except (json.JSONDecodeError, ValueError):
            return None

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
