# complete-worker-scheduler-k82

**Reviews:** complete-worker-scheduler-k78

## Goal

Independently contest the bounded repair of fork admission and deadline checks
after preparation, reading the producer's committed diff against current source.
This is the fresh-context adversarial review of this artifact, not the eventual
aggregate scheduler review. Derive the claims from the narrowed producer task
and the inspection spec; do not accept its report as a verdict.

## Context

The producer added a second admission snapshot and fixed-cutoff check after
pipes/signal masking at both actual fork sites. Its original persistent-worker
and single-scheduler charter remains wholly owned by
complete-worker-scheduler-k81; no ancestor is closed. Read the bounded-inspection
brief chain and producer diff, experiments/exec-reliability/evidence/fork-boundary-k78/report.md,
and frozen condition declarations/verdicts. Preserve all historical evidence.

Inspect at least these concrete doubts:

- Can prepared descriptors, signal state or the native/Python thread/SIGCHLD
  state escape refusal or be sampled at the wrong boundary? Does elapsed final
  observation prevent fork without renewing the original deadline?
- Does helper refusal remain outside the real os.fork error-publication handler,
  with no manufactured child/PID/owner or kernel-error authority? Are earlier
  successful creation, actual error, late-return and reap paths still correct?
- Do cleanup/finally paths restore the previous mask and close all prepared
  descriptors even when final native observation fails? Do the tests distinguish
  descriptor-set equality from descriptor identity strongly enough for this change?
- Do the seven conditions at both boundaries genuinely mutate after preparation,
  and are the thread/native signal conditions real? Does the fork tripwire prove
  refusal without unsafe real child creation? Does the repaired late-kernel-return
  fixture still test a real child and fixed allowance?
- Can the retained RED fail for unrelated reasons, or can the auditor hide one?
  Reconcile 14+14 attempted-fork failures and the full-01 fixture failure against
  the final 415-case green run, immutable subject digests, errno portability,
  exact PID controls and resource observations. Later absence is no historical
  reap/unpublished-helper proof. Independently check actual waits/signals relevant
  to this bounded change without mistaking it for aggregate acceptance.

## Done when

The committed artifact has an independent adversarial assessment with specific
path/line evidence, validation appropriate to each material claim, and explicit
limits. Unavailable privileged controls remain unavailable; no guests or
installed app mutation is authorized. Ten-clone acceptance remains 0/10
unavailable. If findings earn integration, insert an integrate-review-impl step
before the next live sibling complete-worker-scheduler-k81 using this producer's
bare stem and this review's stable handle; its charter refers to this review
without transcribing the finding list. If there are no actionable findings,
retire with no empty integration. The eventual aggregate review with
Reviews: bounded-inspection-k28 before k21 is still the final aggregate producer's
responsibility, not satisfied by this review.
