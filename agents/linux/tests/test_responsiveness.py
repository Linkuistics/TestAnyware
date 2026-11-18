"""The AT-SPI lock and the responsiveness of the threaded server.

Spec docs/specs/guest-command-lifecycle.md, section "Responsiveness": the
accessibility endpoints share one lock, because AT-SPI is not safe across
threads, and the exec, cancel, health, upload and download handlers never take
it. Row "Responsive agent during a command" of the control table. The test
substitutes the accessibility functions, so no AT-SPI runs here.
"""

from __future__ import annotations

import threading
import time
from collections.abc import Iterator
from pathlib import Path
from urllib.parse import quote

import pytest

from tests.http_agent import (
    get_health,
    in_process_agent,
    post_json,
    post_request,
    raw_exchange,
    timed,
)

BOUND_S = 1.0  # spec, section "Bounds": health during a running command


@pytest.fixture
def port() -> Iterator[int]:
    with in_process_agent() as agent_port:
        yield agent_port


@pytest.fixture
def accessibility():  # type: ignore[no-untyped-def]
    from tests.agent_server import agent_request_handler

    agent_request_handler()
    from testanyware_agent import accessibility as module

    return module


class InsideAtSpi:
    """Counts the calls that are inside the substituted AT-SPI at one time."""

    def __init__(self, hold_s: float) -> None:
        self.hold_s = hold_s
        self._lock = threading.Lock()
        self.inside = 0
        self.most = 0
        self.calls = 0

    def enter(self) -> None:
        with self._lock:
            self.inside += 1
            self.calls += 1
            self.most = max(self.most, self.inside)
        time.sleep(self.hold_s)
        with self._lock:
            self.inside -= 1


def test_health_and_cancel_reply_within_1_s_while_a_request_holds_the_lock(
    port: int, accessibility, monkeypatch: pytest.MonkeyPatch
) -> None:
    entered = threading.Event()
    release = threading.Event()

    def blocking_windows() -> tuple[int, dict]:
        entered.set()
        release.wait(10)
        return 200, {"windows": []}

    monkeypatch.setattr(accessibility, "handle_windows", blocking_windows)
    holder = threading.Thread(target=post_json, args=(port, "/windows", {}), daemon=True)
    holder.start()
    try:
        assert entered.wait(5)
        assert accessibility.AT_SPI_LOCK.locked()
        elapsed, (status, _) = timed(get_health, port, BOUND_S)
        assert status == 200 and elapsed < BOUND_S
        elapsed, (status, reply) = timed(post_json, port, "/exec/cancel", {"id": "none"}, BOUND_S)
        assert (status, reply) == (200, {"state": "unknown_id"})
        assert elapsed < BOUND_S
    finally:
        release.set()
        holder.join(5)


def test_system_endpoints_never_take_the_lock(
    port: int, accessibility, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The test itself holds the lock. Every system endpoint still replies."""
    monkeypatch.setenv("TESTANYWARE_AGENT_CAPTURE_ROOT", str(tmp_path / "captures"))
    payload = tmp_path / "payload.bin"
    payload.write_bytes(b"\x00\xff\x80A\n")
    with accessibility.AT_SPI_LOCK:
        elapsed, (status, _) = timed(get_health, port, BOUND_S)
        assert status == 200 and elapsed < BOUND_S
        elapsed, (status, _) = timed(post_json, port, "/exec/cancel", {"id": "n"}, BOUND_S)
        assert status == 200 and elapsed < BOUND_S
        elapsed, raw = timed(
            raw_exchange, port, post_request(f"/download?path={quote(str(payload))}"), BOUND_S
        )
        assert raw.body == b"\x00\xff\x80A\n" and elapsed < BOUND_S
        target = tmp_path / "uploaded.bin"
        request = post_request(f"/upload?path={quote(str(target))}", b"up")
        elapsed, raw = timed(raw_exchange, port, request, BOUND_S)
        assert raw.status == 200 and target.read_bytes() == b"up" and elapsed < BOUND_S


@pytest.mark.linux_kernel
def test_exec_never_takes_the_lock(
    port: int, accessibility, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("TESTANYWARE_AGENT_CAPTURE_ROOT", str(tmp_path / "captures"))
    with accessibility.AT_SPI_LOCK:
        elapsed, (status, record) = timed(
            post_json, port, "/exec", {"command": "printf ok", "timeout": 5}, BOUND_S
        )
    assert (status, record["stdout"]) == (200, "ok")
    assert elapsed < BOUND_S


def test_two_accessibility_requests_never_run_inside_at_spi_together(
    port: int, accessibility, monkeypatch: pytest.MonkeyPatch
) -> None:
    tracker = InsideAtSpi(hold_s=0.15)

    def windows() -> tuple[int, dict]:
        tracker.enter()
        return 200, {"windows": []}

    def snapshot(body: dict) -> tuple[int, dict]:
        tracker.enter()
        return 200, {"windows": []}

    def enumerate_windows() -> list:
        tracker.enter()
        return []

    def is_accessible() -> bool:
        tracker.enter()
        return True

    monkeypatch.setattr(accessibility, "handle_windows", windows)
    monkeypatch.setattr(accessibility, "handle_snapshot", snapshot)
    monkeypatch.setattr(accessibility, "enumerate_windows", enumerate_windows)
    monkeypatch.setattr(accessibility, "is_accessible", is_accessible)

    requests = [
        (post_json, (port, "/windows", {})),
        (post_json, (port, "/snapshot", {})),
        (post_json, (port, "/windows", {})),
        (post_json, (port, "/wait", {"timeout": 0.6})),
        (get_health, (port, 5.0)),
        (get_health, (port, 5.0)),
    ]
    threads = [threading.Thread(target=call, args=args, daemon=True) for call, args in requests]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(10)
    assert tracker.calls >= len(requests)
    assert tracker.most == 1


def test_health_reports_the_probe_value(port: int, accessibility, monkeypatch) -> None:
    monkeypatch.setattr(accessibility, "is_accessible", lambda: True)
    status, health = get_health(port, BOUND_S)
    assert (status, health) == (200, {"accessible": True, "platform": "linux"})

