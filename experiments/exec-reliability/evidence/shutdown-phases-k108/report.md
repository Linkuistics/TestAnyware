# Review of declared worker shutdown phases (complete-worker-scheduler-k108)

This review covers complete-worker-scheduler-k104, commit `53fbcd1`. Line numbers refer to that commit. No commit since then touches `worker_capture.py`, `controller.py`, `test_shutdown.py` or `test_worker_capture.py`.

## Outcome

The review found no defect in the shutdown safety behaviour. It found three low-severity findings, all about proof strength or receipt meaning. Each finding is actionable by a test or receipt change.

## Method and limits

The review ran no test, build, lint, format or mutation command. It ran read-only hash, byte-comparison, diff and JUnit-parsing commands. The code graph predates commit `53fbcd1`. The review read the source directly and makes no graph completeness claim.

## Verified

- The frozen `inputs-final-2` copies of the four source files equal the committed bytes.
- Each of the five mutants differs from `inputs-final-2` by one changed line in `worker_capture.py`. The only other difference is `preparation.json`, which records hashes.
- JUnit counts match the report. `full-final-2` has 679 tests with no failure. The mutants fail 6, 2, 4, 3 and 2 of 53 selected tests, as the report states.
- `prior-before.json` equals the `snapshot` in `prior-after.json`. Both hold 151,643 paths with no change.
- `close()` rejects invalid bounds at line 438, before it sets `closed` or sends a signal. The expired-cutoff mode sends no signal and starts no reap.
- Healthy, failure and bootstrap close each select their own declared grace and reap allowance at lines 460 to 467.

The review did not recompute the artifact digests or the real-child audit receipts.

## Findings

### F1: no control makes the earlier-cutoff reserve binding

Line 485 computes `kill_at` from three terms. The term `max(shutdown_started, old_end - reap_budget)` is the only one that reserves the reap allowance inside an earlier cutoff. Line 476 takes the earlier TERM time as the start of the grace clock.

Every finite-cutoff control produces `kill_at == shutdown_started`:

- In `test_shutdown.py`, the modes `finite`, `prior-term`, `denied-term`, `prior-kill` and `denied-kill` set a cutoff 0.1 to 0.11 seconds ahead. The declared reap allowance is 0.16 seconds. The retained receipts show `term_deadline_monotonic - started_monotonic` equal to zero for all five.
- In `test_worker_capture.py`, lines 2286 and 2313 set a cutoff 0.4 seconds ahead against the default two-second reap allowance. The clamp wins there too.

The assertion at `test_shutdown.py:196` is `kill_at <= seeded TERM time + term_grace`. It passes for any `kill_at` that is not late. Three single-line mutations would survive:

1. Drop `- reap_budget` from line 489.
2. Start the grace clock at `shutdown_started` and ignore the earlier TERM time at line 476.
3. Grant a full fresh grace after an earlier TERM.

The five retained mutants target none of these. The specification forbids a fresh grace after a prior attempt and requires the reap allowance inside the earlier cutoff. The retained evidence does not enforce either claim.

### F2: the grace-phase `overrun` is true on every ordinary escalation

Line 557 sets `overrun` to `finish > cutoff`. For an escalated grace phase, `finish` is the KILL time and `cutoff` is `kill_at`. The loop sends KILL only when `kill_at <= now`, so `finish` exceeds `cutoff` by microseconds or more. All 6 escalated grace phases in `full-final-2` report `overrun: true`. The shutdown-level `overrun` is false for them, and the worker records no failure.

A receipt reader cannot tell an on-time escalation from a late one by this field. The numeric `lateness_seconds` is present but has no bound for the grace phase. The assertion at `test_shutdown.py:225` restates the formula, so it cannot catch a wrong truth value. Only the 0.04-second checks at lines 183 and 192 bound the KILL delay, and only for two modes. k106 resolved the same ambiguity for bootstrap by adding outcomes and lateness.

### F3: the retained evidence samples each mode once

`full-final-2` holds one real-child receipt for each of the 12 modes. The tests check a KILL window of 40 milliseconds in two modes. That window is wide beside the 70 and 40 millisecond grace values. A mutation that adds a few milliseconds to every grace would pass. The five mutants include no timing-offset mutant for TERM grace or KILL delay.

## Observations kept out of the findings

- The `denied-*` modes seed signal history directly. The report discloses this. No control shows a kernel denial.
- Orderly mode sends no SIGTERM, which matches the specification text.
- The grace clock for a first TERM starts at `shutdown_started`, not at the TERM syscall. The difference is microseconds.

## Scope

The shared earliest-deadline scheduler, overlap with controller teardown and the final k28 review stay with complete-worker-scheduler-k107. This review does not claim guest acceptance. The parent aggregate bounded-inspection-k28 and k17 stay open.
