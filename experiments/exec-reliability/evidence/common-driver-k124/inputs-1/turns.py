"""Drive bounded owner turns without acquiring their signal or reap authority."""

import time
from collections.abc import Callable
from dataclasses import dataclass


@dataclass(frozen=True)
class Turn:
    """A bounded callback returns an absolute next wake, or None when terminal."""

    advance: Callable[[], float | None]
    safety: bool = False


def run_turns(*turns: Turn) -> None:
    """Run ready safety work first, then earliest wake with stable rotation.

    Owners enforce their own immutable cutoffs, including late first turns.
    Exceptions propagate immediately; this driver neither retries nor finalizes
    an owner. A callback must yield after bounded work for other owners to run.
    """
    pending = [(float("-inf"), order, turn) for order, turn in enumerate(turns)]
    sequence = len(pending)
    while pending:
        now = time.monotonic()
        ready = [item for item in pending if item[0] <= now]
        if not ready:
            wake = min(item[0] for item in pending)
            delay = min(0.005, wake - time.monotonic())
            if delay > 0:
                time.sleep(delay)
            continue
        item = min(ready, key=lambda item: (not item[2].safety, item[0], item[1]))
        pending.remove(item)
        wake = item[2].advance()
        if wake is not None:
            pending.append((wake, sequence, item[2]))
            sequence += 1
