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
