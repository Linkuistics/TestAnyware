"""Seam module for an agent process: every leader status stays unavailable.

Importing it replaces the leader-wait source of the owner (spec, section "Test
seams and required controls", the unavailable-status control). The waiter
still wakes on a cancellation, but it never reports a leader exit. The leader
is real and receives the real signals. The seam is not a second backend.
"""

from __future__ import annotations

import select
import time

from testanyware_agent import exec_owner
from testanyware_agent.models import LeaderStatus


class NeverExitWaiter(exec_owner.PidfdLeaderWaiter):
    def wait_exit(self, timeout: float | None, wake: int | None = None) -> bool:
        limit = exec_owner.MAX_KERNEL_WAIT_S if timeout is None else max(0.0, timeout)
        end = time.monotonic() + limit
        while True:
            left = min(end - time.monotonic(), exec_owner.MAX_KERNEL_WAIT_S)
            if left <= 0:
                return False
            if wake is not None:
                if select.select([wake], [], [], left)[0]:
                    return False
            else:
                time.sleep(left)

    def peek(self) -> LeaderStatus | None:
        return None


exec_owner.DEFAULT_SEAMS.leader_waiter = NeverExitWaiter
