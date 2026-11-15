# Incremental workload-leader reap turns (k119)

`_LeaderReap.advance()` performs at most one exact positive-PID WNOHANG attempt
and yields an absolute next wake without sleeping. `_reap_leader` consumes those
turns synchronously, with sleeping outside the owner. This exposes a scheduler
prerequisite while preserving the existing synchronous controller behavior.

The owner freezes the reap cutoff and closes signal/probe authority when it is
constructed. One first reap attempt remains mandatory even at or after that
cutoff; later turns admitted at/after it perform no wait. A successful exact
reap updates the Popen return code and releases the strong workload owner.
Errors and exhausted not-ready outcomes retain the strong owner and explicit
residual evidence. Repeated terminal advances do nothing. A late first reap
may remove the child, but its receipt still fails the process bound. No signal
or probe was added to the reap path.

## Retained comparison

| Condition | Pytest result | Instrument |
| --- | --- | --- |
| red | 4 failed | Old controller lacks `_LeaderReap` |
| green-controls | 22 passed | New turns plus existing first-reap/caller controls |
| late-poll | 1 failed, 3 passed | Removing later-poll admission guard permits wait at cutoff |
| sleep-in-turn | 3 failed, 1 passed | Sleeping in a yielding turn is rejected |
| full | 860 passed in 79.51s | Complete diagnostic suite |

The four new controls each create a real exited direct child and independently
reap it with the original waitpid or bounded Popen wait, then inspect its PID for
absence even when assertions fail. A fake clock and the owner-facing waitpid
result control distinguish ready, ECHILD, withheld and late-return outcomes.
The expected wake values, syscall times/counts, terminal idempotence and retained
owner membership are literal oracles. The injected sleep is not reached by the
late-return case, which terminates before yielding; its passing result is
expected. All twenty new-control child receipts across the five conditions
record final absence. These are finite controls, not a real-time latency proof.

Full Ruff, format and strict Pyright checks passed. The outer full pytest checker
completed in 80.465s inside its 120s bound; focused checkers used 60s, static
checkers 30s. Each has separate 2s TERM and 2s KILL/reap limits. Every checker was
reaped without timeout, and its termination-fixture audit passed.

Each condition retains declarations, raw stdout/stderr, JUnit, receipts,
fixture journals, before/after subject hashes and artifact hashes. Frozen
`inputs-old`, `inputs-final` and two single-site mutants retain the runtime,
tests, manifest, lock and Taskfile. The final source/config set equals the frozen
candidate. `condition-audit.json` validates the expected failure sets, exact
child absence, checker/fixture outcomes and all measured subject/artifact hashes.
`prior-before.json` and `prior-after.json` retain identical inventories of
191643 prior evidence paths, excluding this new root and inherited cache paths.

## Tooling corrections retained

`tools-1` retains the initial strict-typing error in the auditor's untyped summary
dictionary. `audit.initial.py.txt` preserves that version. `tools-2`, `tools-3`
and `tools-4` pass. Collection established 860 full cases, correcting the initial
859 expectation; the sleep mutation fails 3 cases, correcting the anticipated 4.
`audit.red.py.txt` and `audit.focused.py.txt` preserve the exact measured auditor
versions. The final auditor binds those original conditions to their measured
bytes; it does not pretend its corrected bytes ran earlier. No runtime/test
edit followed the green-controls freeze. The original comparison-plan and
observed-deviations subjects remain unchanged; this section records the tooling
corrections separately. A preservation invocation using PHASE instead of MODE
was rejected before running; the corrected MODE=after check passed.

## Scope and continuation

The original aggregate charter and complete historical handoffs moved verbatim
to complete-worker-scheduler-k122. It owns actual shared scheduling, helper
transport integration, teardown overlap, the retained timing failures, the
parent/candidate interleaved repeats and final review of bounded-inspection-k28.
The worker capture/close paths and controller resource teardown still run
synchronously. This slice proves neither aggregate completion nor repeatability;
one passing full sample does not cancel any previous failed sample. The bounded
ownership seam is to receive a separate review before further scheduler changes.
Ancestors remain open and ten-clone acceptance stays 0/10, unavailable. No guest,
privilege, installed configuration or unrelated process was touched.
