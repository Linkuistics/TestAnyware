"""Drive bounded owner turns without acquiring their signal or reap authority."""

import select
import time
from collections.abc import Callable
from dataclasses import dataclass


@dataclass(frozen=True)
class Turn:
    """A bounded callback returns an absolute next wake, or None when terminal."""

    advance: Callable[[], float | None]
    safety: bool = False
    readiness: Callable[[], tuple[tuple[int, ...], tuple[int, ...]]] | None = None


def run_turns(*turns: Turn) -> None:
    """Run ready safety work first, then earliest wake with stable rotation.

    Owners enforce their own immutable cutoffs, including late first turns.
    Exceptions propagate immediately; this driver neither retries nor finalizes
    an owner. A callback must yield after bounded work for other owners to run.
    Readiness interests are sampled while idle and may wake an owner before its
    next timer. Owners must still check cutoffs and tolerate spurious readiness.
    """
    pending: list[tuple[float, int, Turn]] = [
        (float("-inf"), order, turn) for order, turn in enumerate(turns)
    ]
    sequence = len(pending)
    while pending:
        # Newly registered owners are immediately due, even beyond their cutoff.
        now = (
            float("-inf") if all(item[0] == float("-inf") for item in pending) else time.monotonic()
        )
        ready = [item for item in pending if item[0] <= now]
        if not ready:
            wake = min(item[0] for item in pending)
            delay = min(0.005, wake - time.monotonic())
            if delay > 0:
                interests = [
                    (order, turn.readiness())
                    for _, order, turn in pending
                    if turn.readiness is not None
                ]
                readers = sorted({fd for _, (reads, _) in interests for fd in reads})
                writers = sorted({fd for _, (_, writes) in interests for fd in writes})
                if readers or writers:
                    readable, writable, _ = select.select(readers, writers, [], delay)
                    observed = time.monotonic()
                    signaled = {
                        order
                        for order, (reads, writes) in interests
                        if any(fd in readable for fd in reads)
                        or any(fd in writable for fd in writes)
                    }
                    pending = [
                        (min(wake, observed) if order in signaled else wake, order, turn)
                        for wake, order, turn in pending
                    ]
                else:
                    time.sleep(delay)
            continue
        item = min(ready, key=lambda item: (item[0], item[1]))
        pending.remove(item)
        wake = item[2].advance()
        if wake is not None:
            pending.append((wake, sequence, item[2]))
            sequence += 1
