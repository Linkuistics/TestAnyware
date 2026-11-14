# complete-worker-scheduler-k64

## Goal

Reject native SIGCHLD auto-reaping before resource acquisition and at the existing worker/helper fork admissions. Darwin SA_NOCLDWAIT must not masquerade as a safe SIG_DFL handler while exact-child wait ownership is required.

## Context

The complete original charter and decisions are preserved in complete-worker-scheduler-k67. All ancestor contracts remain open. The current native observation checks the SIGCHLD handler but ignores sa_flags; Python's cached disposition cannot reveal SA_NOCLDWAIT installed through libc.

## Done when

- Observe and reject SA_NOCLDWAIT even when the native handler and Python cache are SIG_DFL. Preserve a distinct native-state receipt value and accept that rejected state in the strict worker receipt decoder without allowing contradictory ready claims.
- Real Darwin sigaction controls demonstrate rejection before resource acquisition, worker fork and helper fork, with exact restoration of prior signal state. Retain RED and GREEN results, source/tool hashes and all failures; run the complete diagnostic checks. Preserve every prior evidence file byte-for-byte.
- Record the shipped behavior and its limitations. Schedule a separate adversarial review of this admission boundary before the remaining implementation relies on it. No aggregate scheduler requirement is claimed complete.

## Decisions (running log)

- 2026-11-15: Chose flat sibling decomposition because this admission defect is independently testable in the live controller and both capture engines; it adds no new shared context. complete-worker-scheduler-k67 retains the complete original contract and every outstanding control. All aggregate ancestors remain open.
- 2026-11-15: Closed-standard-stream handling was considered and rejected as a change: reset_after_fork already preserves allowed descriptors and real subprocess controls cover all seven nonempty closure combinations.
- 2026-11-15: The claim to challenge is that native auto-reap cannot pass existing admission and lose exact-child status. Real sigaction controls are the immediate instrument; a scheduled review-impl will contest the committed boundary rather than duplicate that review in this session.
- 2026-11-15: Retained native-auto-reap-k64/red01: lint, format and strict types pass; 259 tests pass and all ten added cases fail. Six real native-state controls stop at the safety guard before unsafe child creation; two real-worker helper controls reap their workers after the guard fails; the two decoder cases reject the new state as invalid rather than recognize a refusal or a contradictory ready claim. Implement a bitwise Darwin SA_NOCLDWAIT observation and extend the decoder's rejected native states; keep reason derivation unchanged so ready contradictions still fail.
- 2026-11-15: green01 retains 268 passes and one existing native SIG_IGN classification failure: Darwin's kernel sets SA_NOCLDWAIT with SIG_IGN. Preserve existing nondefault handler classifications (already rejected); report SA_NOCLDWAIT for a default handler with that bit. Do not erase the failed condition or change any ceiling. During the scope audit, restore k67's full original charter directly from the parent commit and verify it byte-for-byte apart from the header and appended decomposition note.
- 2026-11-15: green02 passes all 269 cases, lint, format and strict types under the original bounds. The first independent audit retained its failure on an incorrect green01 count (four flag-labeled refusals, including existing ignore controls); audit02 corrects only that expectation and passes. Its 3,388 input paths are stable, all 57,834 prior evidence files are unchanged, and all 18 exact recorded checker/fixture-worker PIDs are absent. The report separates real native state and worker transport from synthetic decoder bodies and preserves unknown creation in RED. No helper identity/reap or aggregate scheduler claim follows. The spec clarifies that SIG_DFL excludes native SA_NOCLDWAIT; the report points to k67's complete preserved remainder.
- 2026-11-15: Final producer handoff inserts review complete-worker-scheduler-k68 before k67. No provider evidence is present in this launch, so the review has no Creator line. k67 and k68 keep k48 and all aggregate ancestors live; no parent close is claimed. No new ADR is needed: this repairs the existing exact-child admission precondition rather than changing the architectural decisions.
