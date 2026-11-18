# acceptance-plan-k223

## Goal

The acceptance controller has a trial plan and a checker for the root
acceptance inventory. The checker reads the new JSON record. The controller
self-check passes.

## Context

- Controller: `experiments/exec-reliability/controller.py` and its report
  `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- Spec: section "Validation route".
- Root brief: sections "Acceptance inventory and bounds" and "Constraints".
- Host CLI: the reviewed state of host-cli-control-k222, with the repairs of
  host-cli-control-k240. On the app route, a guest reset after a complete
  body reads as success. Bytes that reached the host still fail the framing
  check. The trial must not count that ordering as reset detection.

## Done when

- The trial plan runs every root case on each clone, in the root order. It
  keeps the abrupt CLI interruption separate from explicit cancellation.
- The checker reads JSON mode only. It compares the decoded `evidence` bytes
  with the root byte oracle.
- The checker applies the record validity rules of the spec. It accepts both
  leader orders and `leaderStatus` `unavailable`.
- The checker fails each of the four corruption controls of the spec:
  - an appended LF,
  - a flipped binary byte,
  - a missing terminal outcome,
  - a contradictory termination record.
- Each ceiling and bound of the root brief and the spec is a checker limit.
- The outer ledger disposes of each owned VM in `finally` within 45 s.
- `diagnostic:self-check` passes into a new `OUT` directory.
- No guest boots in this leaf.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

Leaf 68 runs the corruption controls against the real builds and freezes the
plan. A change to the plan after that freeze needs a new freeze.

## Decisions (running log)

- The graph server failed to connect in this session. Direct source reads
  are the evidence for the controller, the Host CLI and the schemas.
- The trial plan is `acceptance_k223.py`, and the checker is
  `acceptance_k223_check.py`. Both live in `experiments/exec-reliability`.
  The self-check therefore lints, type-checks and tests them with the rest.
- One clone is one controller plan with one owned VM. The trial runs five
  macOS plans and then five Linux plans. It stops after the first clone that
  fails its run or its check.
- The macOS clone uses the retained r3 fixture, and the Linux clone uses the
  retained r5 fixture. Both stay unchanged, and their hashes are checked
  before the run. The online case is not in the root inventory, so the plan
  omits it.
- The intact case runs the lesson directly. Its record evidence carries the
  stdout and stderr oracle bytes. A download of `artifact.bin` carries the
  binary oracle, and a download of `status.txt` carries status 0.
- The ten ordinary operations repeat health, `printf ordinary-ok` and one
  process table. Each set of ten after-operations repeats a child check,
  health and `printf ordinary-ok`. It starts and ends with a child check.
- The timeout, cancellation and interruption cases use the issue's child
  payload with one directory for each case. The payload also prints one
  partial line to stdout and one to stderr, so the record carries partial
  evidence.
- Explicit cancellation runs through the helper `acceptance_k223_cancel.py`.
  The helper starts `file exec` and sends `file cancel --wait 5` 2 s later.
  The controller runs one call at a time, so a helper is necessary.
- The abrupt interruption sends SIGTERM to the Host CLI after 2 s. A child
  check and a health call follow. Then `file cancel --wait 5` is the
  fallback. An absent child before that fallback shows that the cancellation
  of the Host CLI ended the command.
- The checker runs the four corruption controls on independent copies of
  each clone's evidence. The contradictory control has two forms, after the
  two examples of the spec.
- The boot and slot bounds come from the freeze manifest of k224. The plan
  refuses to start without them. The export bound belongs to k224.
- A download that succeeds is never evidence of reset detection. The app
  route hides a guest reset after a complete body, so no case claims it.
- The checker rebuilds the canonical plan from the frozen identities. It
  compares each argv, budget and expectation, not only the call names. A
  changed argv passed the first version of the checker, and a control now
  covers it.
- `self-check-01` failed. Its pytest step reached the frozen 120 s budget
  at 90 % of the earlier tests, before the new test file ran. Leaf k19
  recorded the same limit and left the decision to a later leaf. The full
  suite of 1,184 tests took 126 s on this host.
- This leaf raised the pytest budget of `self_check.py` to 300 s. No
  self-check can pass at 120 s on this host. The budget keeps the runner
  finite, and each test keeps its own bounds. No trial ceiling changed.
- `self-check-02` passed: lint, format, strict types and 1,184 tests, with
  equal subject digests before and after. Both runs stay in the evidence.
- The durable report is
  `docs/experiments/exec-reliability-acceptance-plan.md`.
- The environment holds no provider evidence, and this session is not a
  legacy session. The review therefore gets no Creator line.
