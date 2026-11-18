"""Retention and shutdown rules of the exec registry.

Spec docs/specs/guest-command-lifecycle.md, sections "Retention" and "Agent
shutdown". New behaviour. The registry is pure Python, so these tests run on
any host. A test clock moves the retention time without a wait.
"""

from __future__ import annotations

import threading
import time

from testanyware_agent import exec_registry
from testanyware_agent.exec_registry import ExecRegistry
from testanyware_agent.models import ExecRecord


class Clock:
    def __init__(self) -> None:
        self.now = 1000.0

    def __call__(self) -> float:
        return self.now


def record() -> ExecRecord:
    # The registry stores a record and returns it. It never reads its fields.
    return object()  # type: ignore[return-value]


def finish(registry: ExecRegistry, exec_id: str) -> ExecRecord:
    assert registry.admit(exec_id).kind == "start"
    published = record()
    registry.publish(exec_id, published)
    return published


def test_the_spec_values_are_300_s_and_64() -> None:
    assert (exec_registry.RETENTION_S, exec_registry.RETENTION_LIMIT) == (300.0, 64)


def test_a_terminal_record_is_retained_for_300_s() -> None:
    clock = Clock()
    registry = ExecRegistry(clock=clock)
    published = finish(registry, "a")
    clock.now += 299.9
    assert registry.cancel("a", 0) == ("terminal", published)
    assert registry.admit("a").kind == "in_use"
    clock.now += 0.1
    assert registry.state("a") is None
    assert registry.cancel("a", 0) == ("unknown_id", None)


def test_a_tombstone_is_retained_for_300_s_from_its_first_cancellation() -> None:
    clock = Clock()
    registry = ExecRegistry(clock=clock)
    assert registry.cancel("t", 0) == ("unknown_id", None)
    clock.now += 200
    assert registry.cancel("t", 0) == ("unknown_id", None)  # does not renew it
    clock.now += 100
    assert registry.admit("t").kind == "start"


def test_the_oldest_retained_entry_goes_first_beyond_64() -> None:
    clock = Clock()
    registry = ExecRegistry(clock=clock)
    for n in range(64):
        finish(registry, f"r{n}")
        clock.now += 1
    registry.cancel("tomb", 0)
    assert registry.state("r0") is None
    assert registry.state("r1") == "terminal"
    assert registry.state("tomb") == "tombstone"
    finish(registry, "late")
    assert registry.state("r1") is None
    assert registry.state("tomb") == "tombstone"


def test_a_running_command_is_never_evicted() -> None:
    clock = Clock()
    registry = ExecRegistry(clock=clock)
    assert registry.admit("long").kind == "start"
    for n in range(100):
        finish(registry, f"r{n}")
    clock.now += 10_000
    finish(registry, "after")
    assert registry.state("long") == "running"


def test_the_cancel_signal_closes_at_publication() -> None:
    registry = ExecRegistry()
    admission = registry.admit("s")
    assert admission.signal is not None
    registry.publish("s", record())
    # A later request writes nothing to the closed pipe and changes nothing.
    admission.signal.request(time.monotonic())
    assert admission.signal.requested_at is None


def test_close_stops_admission_and_cancels_every_running_command() -> None:
    registry = ExecRegistry()
    first = registry.admit("one")
    second = registry.admit("two")
    finish(registry, "done")
    assert sorted(registry.close()) == ["one", "two"]
    assert first.signal is not None and first.signal.requested_at is not None
    assert second.signal is not None and second.signal.requested_at is not None
    assert registry.admit("new").kind == "closed"


def test_wait_published_returns_at_its_bound_with_the_pending_ids() -> None:
    registry = ExecRegistry()
    registry.admit("slow")
    registry.admit("fast")
    running = registry.close()
    threading.Timer(0.1, registry.publish, args=("fast", record())).start()
    started = time.monotonic()
    pending = registry.wait_published(running, time.monotonic() + 0.5)
    elapsed = time.monotonic() - started
    assert pending == ["slow"]
    assert 0.45 <= elapsed < 1.0


def test_an_abandoned_command_does_not_delay_the_shutdown() -> None:
    registry = ExecRegistry()
    registry.admit("broken")
    running = registry.close()
    registry.abandon("broken")
    assert registry.wait_published(running, time.monotonic() + 5) == []
