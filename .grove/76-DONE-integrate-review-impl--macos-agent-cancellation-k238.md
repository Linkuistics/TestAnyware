# macos-agent-cancellation-k238

**Integrates:** macos-agent-cancellation-k237

## Goal

Triage the committed findings of macos-agent-cancellation-k237 against the
macOS implementation and its agreed contract. Reconcile actionable findings
before host-cli-control-k222 consumes this implementation.

## Context

- Read the findings from the review's own commit, not from a restatement here.
- Producer: macos-agent-cancellation-k221, commit `45fb4da1`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0024, ADR-0025,
  ADR-0026, and the producer's completion criteria and notes.
- Retained producer evidence:
  `experiments/exec-reliability/evidence/macos-agent-cancellation-k221`.

## Done when

<!-- tc: off because the review session wrote this charter, and this session does not edit it -->

- Each finding has a source-verified disposition under the integration
  acceptance and material-risk threshold.
- Actionable findings are reconciled in the implementation or contract, and
  appropriate controls and verification establish the resulting behavior.
- The existing cancellation, responsiveness, retention, shutdown, record and
  download contracts remain accounted for.
- Documents and retained evidence state the actual result and its limits.

<!-- tc: on -->

## Notes

This is a flat sibling because it reconciles the same macOS artifact under
the same brief. It sits directly after the review so later implementation
cannot silently move the cited code. Preserve earlier failed evidence. Do not
replace a golden or change the globally installed Host CLI or app.

## Decisions (running log)

- D1. Source check at `1723980577`: the cited code is unchanged since the
  producer commit `45fb4da1`. All four findings reproduce by derivation from
  that source. Each is a real issue that breaks an existing acceptance
  condition. None needs a redesign, so this session repairs each one and adds
  a control that fails on the old code.
- D2. R1: the waiter keeps its long `kevent` wait only while it watches the
  cancellation pipe. Without that source and with a signal, each `kevent` wait
  lasts 10 ms at most, the same interval as the status poll. An internal
  `CancelSignal` initialiser gives a test the two failure branches: no pipe,
  and a pipe that the queue cannot register.
- D3. R2: the download opens with `O_NONBLOCK | O_NOCTTY`, checks `S_IFREG`
  on that handle and then clears `O_NONBLOCK`. A FIFO with no writer then
  gives 404 at once. The early `fileExists` check stays.
- D4. R3: a running entry holds a reference object with the signal and a
  reply slot. `publish` fills the slot. A waiting cancellation reads only its
  own object, as the Linux registry reads its own `_Entry`. A test suspends
  the waiting thread with `thread_suspend`, so the eviction and the reuse
  happen before it wakes, every time.
- D5. R4: a fixture that cannot reach a healthy agent now fails the test. It
  reads the agent log before it removes it. The same class is in
  `ExecCancellationTests`: an admission refusal and a missing PID file were
  skips, and they now fail too.
- D6. The documents state the repaired behaviour. The spec's "Cancel
  endpoint" now says that a waiting cancellation returns only the record of
  the command that it cancelled. The Linux registry already does this.
  `agent-protocol.md` adds the macOS 404 for a non-regular file and the
  cancel-wait rule. Both copies of the component page add the 10 ms poll, the
  own-record rule and the non-blocking open. All four documents were clean
  before the edit, so no conversion commit came first.
- D7. Evidence is in
  `experiments/exec-reliability/evidence/macos-agent-cancellation-k238/`.
  `corrected-pass-02` passed `agent:macos:test` with 83 tests and no skip.
  `repeat-01` to `repeat-03` each passed the 47 tests of the five affected
  classes. `old-code/run-01` put back each producer fragment in a scratch
  copy, and each of the seven cases gave its expected exit. The old R1 owner
  still chose `cancelled` but had a lateness of about 2.7 s. The old fixture
  with `/usr/bin/false` as the agent exited 0 with all 11 transport tests
  skipped. The new fixture exited 1. `ProcessRunner.swift` still compiles
  alone, with and without `EXIT_TRACE`.
- D8. The old-code run and `corrected-pass-01` used one earlier test comment.
  The final source changes that comment only, so `corrected-pass-02` and the
  repeats ran on the final source. Both earlier runs stay as history.
- D9. Limits. The Linux download also opens a FIFO with a blocking `open`.
  Its threading server gives that request its own thread, so health and other
  requests stay free. That is not a material risk to this leaf's acceptance,
  and this leaf does not change the Linux agent. The two new R1 cases use a
  test initialiser of `CancelSignal`. They are not a live descriptor-pressure
  run. No live guest ran. Guest acceptance stays **0/10, unavailable**.
- D10. No new review leaf. Each repair is a bounded change with its own
  old-code control. No redesign happened, and no fresh adversarial challenge
  is needed before host-cli-control-k222. No review names this leaf, so no
  Creator line is written. No node closes, because the root has live leaves.
