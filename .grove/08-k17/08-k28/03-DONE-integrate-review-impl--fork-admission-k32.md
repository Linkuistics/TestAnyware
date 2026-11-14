# fork-admission-k32

**Reviews:** fork-admission-k31

## Goal

Triage the findings of fork-admission-k31 against the fork-admission-k29 commit
and fix the real ones before bounded-inspection-k30 builds on the admission
boundary.

## Context

Read the findings in the commit of fork-admission-k31, not in this body. The
commit lists each finding with its file, line and failure scenario. The summary
below is a pointer only.

- F1 (real): `inspection.fork_admission` reads SIGCHLD through `signal.getsignal`.
  That call reports Python's cached table. A native `signal()` or `sigaction()`
  change is invisible to it, so the check can pass while SIGCHLD is SIG_IGN.
- F2 (prose): two planning documents still say k23-final-01 is the delivered
  condition. The foundation report says k29-final-01 is current.
- F3 (minor): the preflight subject list omits `inspection.py` and
  `test_inspection.py`.
- F4 (minor): the `diagnostic:format` recipe formats the whole directory,
  including frozen evidence scripts.

## Done when

- Reproduce F1 before the fix. Do not rely on the review's probe.
- Read SIGCHLD from the kernel through a native call. Fail closed when the
  native call is unavailable or fails. Keep the Python-visible check.
- Add failing controls first. Use a real native SIG_IGN and a real native
  handler, both installed outside the Python signal module. Restore the
  previous disposition in a `finally` block.
- Triage F2 to F4. Fix each real finding, or record why it is accepted.
- Keep k29 evidence frozen. Freeze new evidence under a new run name, with
  the same independent identity audit as k29-audit-02.
- Keep the k30 obligations unchanged. The k30 text already requires admission
  before every fork.

## Notes

## Decisions (running log)

- Read review commit `190cdc1d` against the committed sources and frozen k29
  receipts. F1 is a concrete admission gap to reproduce independently before
  repair. Keep the Python-visible check and add a read-only native disposition
  query, failing closed on unavailable/error observations.
- F2 and F3 are real omissions: qualify the parent briefs with the delivered
  partial admission slice and include both inspection sources in preflight.
  F4 is real: recursive formatting includes frozen audit scripts. Restrict the
  recipe to top-level diagnostic Python sources. Earlier evidence is immutable.
- Use real native ignore/handler controls with a complete sigaction snapshot
  restored in `finally`. Freeze the red run before implementation, then a new
  final condition and an independent audit adapted from k29-audit-02. k30's
  existing full worker/scheduler obligations stay unchanged.
- Independently reproduced F1 with native ignore and a native callback while
  Python still reported SIG_DFL in k32-red-01. k32-red-02 also demonstrates all
  four native-query failure controls and the missing preflight subjects. Both
  runs have equal before/after input hashes and retained source snapshots.
- Use Darwin's public sigaction layout from the SDK (handler pointer, uint32
  mask, int flags), passing NULL for the new action. Query failures retain a
  native_observation_error receipt; Python and kernel dispositions must both
  be SIG_DFL. This remains initial admission; k30 must repeat it before every
  fork and supply the full worker/scheduler boundary.
- k32-final-01 passes 73 tests plus lint, format and strict types with frozen
  inputs. k32-audit-01 failed before process controls/cleanup because pytest
  had already removed red-01's old session directory. Preserve its script,
  failure and equal before/after input hashes. A fresh k32-audit-02 records
  already-absent scratch explicitly while retaining every recorded PID query;
  it does not attribute earlier pytest cleanup to the auditor.
- k32-audit-02 passed: 1,294 condition artifacts verified, 1,336 audit input
  hashes unchanged, 264 recorded identities absent, 69 resource declarations
  checked, 133 exact scratch directories removed and 27 already absent. The
  retained observations do not prove absence of escaped descendants. All
  earlier evidence and k30's task remain byte-for-byte unchanged.
- All four findings are repaired. The foundation report preserves k29 as a
  historical condition and links the new red/final/audit records; both parent
  briefs now identify k32 as the delivered partial admission slice. Retire
  this integration only: k30 remains live, so k28 and its ancestors stay open.
