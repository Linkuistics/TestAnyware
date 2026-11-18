"""Shared test configuration for the Linux agent.

The lifecycle controls need a Linux kernel: a pidfd, `waitid` with `P_PIDFD`
and /proc. Each such test carries the `linux_kernel` marker. On another host
the test skips. `task agent:linux:test` also runs the suite in a pinned Linux
container with `TESTANYWARE_LINUX_CONTROLS=required`, and there a missing
kernel feature fails the test instead of skipping it.
"""

from __future__ import annotations

import os
import sys

import pytest

REQUIRED = os.environ.get("TESTANYWARE_LINUX_CONTROLS") == "required"


def pytest_configure(config: pytest.Config) -> None:
    config.addinivalue_line("markers", "linux_kernel: needs a Linux kernel with pidfd support")


def _linux_kernel_problem() -> str | None:
    if not sys.platform.startswith("linux"):
        return f"the host is {sys.platform}, not Linux"
    if not hasattr(os, "pidfd_open") or not hasattr(os, "P_PIDFD"):
        return "this Python has no pidfd support"
    return None


def pytest_runtest_setup(item: pytest.Item) -> None:
    if item.get_closest_marker("linux_kernel") is None:
        return
    problem = _linux_kernel_problem()
    if problem is None:
        return
    if REQUIRED:
        pytest.fail(f"Linux controls are required, but {problem}")
    pytest.skip(f"Linux lifecycle control: {problem}")


@pytest.fixture(autouse=True)
def fresh_exec_registry(monkeypatch: pytest.MonkeyPatch) -> None:
    """Each test gets its own registry, so retained ids never cross tests.

    An agent before leaf k220 has no registry. The old-fail controls of
    tests/test_agent_process.py run unchanged against such an agent.
    """
    try:
        from testanyware_agent import exec_registry
    except ImportError:
        return
    monkeypatch.setattr(exec_registry, "REGISTRY", exec_registry.ExecRegistry())
