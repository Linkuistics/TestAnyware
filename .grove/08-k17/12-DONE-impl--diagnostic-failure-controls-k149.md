# diagnostic-failure-controls-k149


## Goal

Resolve the retained process-control failures that still prevent k17's whole
contract from passing, and attempt its two missing real controls with positively
owned fixtures and any required separately authorised owner.



## Context

The reviewed stream/result identity work is complete in retained-evidence-k21
and retained-evidence-k148. This task owns the remaining acceptance attempt;
it is not another scheduler map, prerequisite review or restatement of k17.

Start from the retained-failures section of
`experiments/exec-reliability/evidence/worker-overlap-k146/report.md` and its
`unavailable-controls.json`, then follow the concrete original receipts cited
there and in the current diagnostic handoff. The k145 interrupted live-group
case retains `assert -9 == 0`, PID TERM followed by PID KILL and a reaped leader
with return code -9. It lacks a child-readiness timestamp. The frozen peer's
stopping-flag handling differs from current k143 code; that comparison alone
does not establish causation. Earlier timing, setup, unsupported table-state
and ps-timeout failures also remain unresolved. Read the k28 brief's retained
failure inventory before choosing the bounded controls.

The real controls require (1) an identified, owned, unreaped actual `/bin/ps`
with an observed live timing window for KILL and bounded reap, and (2) a
separately authorised owner for a fixture whose real and effective UIDs differ
from the sender, including denied signal, missing-table classification and
owner cleanup. Installed setuid metadata, injected errno and substitute helpers
are insufficient.

## Done when

- Attempt the historical failing conditions with controlled readiness and
  timing evidence, preserving original receipts. Establish which failures
  demonstrate runtime defects, fixture defects or unavoidable missed scheduling
  bounds, with per-observation evidence; later passing samples alone are not a
  causal explanation.
- Repair demonstrated runtime or fixture defects inline and demonstrate the
  failing condition against the broken version and the corrected behavior
  against frozen final inputs. Preserve failed timing, uncertain inspection and
  separate worker/helper/workload ownership whenever a bound is missed.
- Attempt the actual setuid-`ps` KILL control safely with an identified owned
  child. Establish real UID denial and permission-unknown using the separately
  authorised fixture owner. Do not acquire privilege or create foreign-UID
  fixtures on an inferred authorisation.
- When an external prerequisite prevents either real control, record the exact
  missing action and ask the human for that action using Grove's required-input
  handoff. Keep this leaf live while awaiting it. Do not retire on an unavailable
  receipt, weaken the agreed bar or cut another unchanged continuation.
- Retain per-case verdicts, frozen before/after subject hashes, exact child and
  resource cleanup audits and untouched earlier evidence in fresh directories;
  use the repository Taskfile for reusable verification.
- Reconcile each remaining k17 done-when condition against the resulting
  evidence and update the durable diagnostic handoff. Any material code change
  needing independent challenge receives a specific review before consumption.
  k17 closes only when its full contract is met. k18 retains stalled storage;
  no live guest, app quit or ten-clone acceptance claim follows.

## Notes

This is a flat sibling under k17 because it directly resolves that node's
remaining process-control acceptance conditions and needs no new shared context
boundary. Run the bounded acceptance attempts rather than delegating the same
aggregate charter again. Privileged input can require a human handoff with this
leaf and session still open.

## Decisions (running log)

- Each historical failure group has one class, and its controls run on frozen
  inputs in fresh directories under
  `experiments/exec-reliability/evidence/failure-controls-k149`. A later pass
  never counts as the cause. Per-case verdicts are in `verdicts.json` there.
- The k110 `?E` table is a runtime defect. Apple `ps` prints `E` for `P_WEXIT`,
  and `?` when it reads no thread state. `controller.py` now accepts `?` only
  with `E`, and it counts each `E` member as non-executable. The `ps` source
  is https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/print.c.
  The kernel source is
  https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exit.c.
- The admission-timing failures are fixture defects. The tests timestamped
  syscall entry, not runtime admission. The tests now assert admission before
  the cutoff and a gap below 50 ms from admission to syscall. Four runtime
  mutants still fail.
- The late-created identity failures are fixture defects. The worker wrote its
  witness inside the KILL window. It now records TERM state in an inherited
  anonymous `mmap`, and the parent publishes the witness.
- Missed scheduling bounds keep their bounds. No bound was raised, and the
  runtime failed closed in each case.
- Control 1 runs the real setuid `/bin/ps` capture path. The runtime kills its
  own unreaped child. This uses no privilege acquisition.
- Control 2 needs a foreign-UID owner, so it was a required-input handoff. The
  human ran `foreign_owner.py` with `sudo` and answered "ready". The sender ran
  as UID 501 against the committed inputs and passed 16 of 16 checks.
- One full-suite run failed 5 bootstrap timing tests during a load average of
  21 from another session's `jj` process. That run stays as evidence. The rerun
  and the committed-input run each passed 975 tests.
- The internal subagent review does not replace the grove review. A specific
  `review-impl` leaf reviews the `controller.py` and test changes before k18
  consumes them, so k17 stays open until that review retires.
