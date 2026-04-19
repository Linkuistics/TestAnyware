"""System endpoint handlers: /health, /exec, /upload, /download, /shutdown."""

import base64
import os
import subprocess
import threading


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


def handle_upload(body: dict) -> tuple[int, dict]:
    path = body.get("path", "")
    content = body.get("content", "")

    try:
        data = base64.b64decode(content)
        parent = os.path.dirname(path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(path, "wb") as f:
            f.write(data)
        return 200, {"success": True, "message": f"Uploaded to {path}"}
    except Exception as e:
        return 200, {"success": False, "message": f"Upload failed: {e}"}


def handle_download(body: dict) -> tuple[int, dict]:
    path = body.get("path", "")

    try:
        with open(path, "rb") as f:
            data = f.read()
        return 200, {"content": base64.b64encode(data).decode("ascii")}
    except Exception as e:
        return 400, {"error": f"Download failed: {e}"}


def handle_shutdown() -> tuple[int, dict]:
    def do_shutdown():
        subprocess.run(["systemctl", "poweroff"],
                       capture_output=True, timeout=10)

    threading.Timer(0.1, do_shutdown).start()
    return 200, {"success": True, "message": "Shutting down"}
