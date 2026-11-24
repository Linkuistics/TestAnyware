> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Native SIGCHLD auto-reap admission (k64)

The Darwin observation previously checked the SIGCHLD handler but ignored
`sa_flags`. A native caller can install `SIG_DFL | SA_NOCLDWAIT` while Python's
cached disposition still reports `SIG_DFL`. Darwin can then discard child wait
status, defeating the exact-child ownership precondition.

`inspection._native_sigchld` now reports `SA_NOCLDWAIT` when the native handler
is default and flag `0x0020` is set. Existing admission rejects that state before
controller acquisition and both existing worker/helper fork boundaries. Existing
nondefault handler classifications stay intact; Darwin also sets the auto-reap
bit for `SIG_IGN`. The worker decoder accepts a coherent refusal carrying the
new state, and rejects a contradictory ready claim. The source cites Apple's
[public signal definitions](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/signal.h).

## Controls and fixed bounds

[plan.json](plan.json) declares both native flag conditions (`0x0020`, `0x0028`),
one-second capture allowances, 30-second lint/format/type checks, a 60-second
pytest ceiling and checker timeout TERM/KILL-reap bounds of two seconds each.
No ceiling was raised. Each sample freezes 14 subjects, including the interpreter
and `/bin/ps`; the 12 nonbinary inputs have matching copies in
[sources-red01](sources-red01), [sources-green01](sources-green01) and
[sources-green02](sources-green02). No measured input changed during a run.

The ten added cases use these boundaries:

- Six actual libc `sigaction` controls install both flag conditions before
  acquisition, worker capture and direct helper capture. They verify the native
  default handler, unchanged Python cache and exact native-state restoration.
  A safety assertion rejects RED before an unsafe child could be created.
- Two controls install actual native state inside a real worker after reset,
  immediately before the original helper capture. Only that fixture wrapper is
  injected: worker fork, reset, framing, transport, decoder and reap are real.
  GREEN reports a known precreation refusal, no helper/output, and a reaped worker.
- Two synthetic helper publications travel through a real worker/channel. They
  test the decoder's contradictory ready claim and coherent native-state refusal.
  They create no helper and do not prove helper identity or kernel behavior.

## Retained outcomes

| Sample | Result |
| --- | --- |
| [red01](red01/verdict.json) | 259 passed, ten added cases failed. Lint, formatting and strict types pass. The safety guard prevents unsafe helper creation; both real workers are reaped, while their receipts conservatively retain unknown creation. |
| [green01](green01/verdict.json) | 268 passed, one existing native `SIG_IGN` classification case failed. The first repair checked the flag before the handler and mislabeled ignore states. All ten new cases pass; lint, formatting and types pass. |
| [green02](green02/verdict.json) | All 269 tests, lint, formatting and strict types pass after preserving existing nondefault handler classifications. Pytest takes 31.35 seconds within its original 60-second ceiling. |
| [audit01](audit01/failure.json) | Audit stops on an incorrect count assumption: green01 has four auto-reap-labeled acquisition refusals, including two existing ignore controls. Original auditor and inputs remain retained. |
| [audit02](audit02/verification.json) | Corrected independent audit passes. All three samples have the same 269 individual case identities, with exactly the expected failures and no skips/errors. All source, subject and declared artifact digests verify. |

The [second frozen auditor](verify-evidence-v2.py) checks 3,388 stable input
paths. All 57,834 earlier evidence files retain their original membership and
bytes, verified against [prior-evidence.json](prior-evidence.json). It itemizes
the four direct precreation receipts, two real-worker refusals and two controller
acquisition refusals in the final GREEN sample. GREEN workers exit normally without
shutdown signals and have no unknown helper creation. The independent
[identity audit](audit02/identity-audit.json) queries only 18 recorded PIDs
(12 checkers and six native-state fixture workers), with a two-second `/bin/ps`
bound. All are absent. It sends no signal and touches no unrelated process.

## Limits and remaining work

This is admission-state detection and transport validation. It is a snapshot,
not synchronization against a host changing thread or signal state afterward.
These new controls intentionally create no helper; later PID absence is not
helper reap acknowledgement and cannot resolve RED's unknown-creation receipt.
The audit does not claim a complete resource census for every existing test.

Graph coverage has matching metadata and no recorded issue for the changed
runtime/tests and runner at generation `2026-11-14T13:09:18Z`; this is a best-effort
signal only. Changed documentation and the untracked auditor use direct source
reads rather than graph completeness claims.

The full original scheduler charter is preserved in flat sibling
`complete-worker-scheduler-k67`, verified against the parent commit apart from
its header and appended decomposition note. Persistent pre-resource bootstrap,
gated helper publication, independent ownership, the shared immutable scheduler,
the complete fault inventory and the final aggregate review before k21 remain
open. A separate review of k64 precedes that remaining implementation. All
aggregate ancestors remain open. No guest, installed-app, privilege, stalled
storage or ten-clone acceptance claim changes: **0/10, unavailable**.
