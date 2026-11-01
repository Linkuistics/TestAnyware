# diagnostic-foundation-k15

**Reviews:** diagnostic-foundation-k11

<!-- tc: off because the creating session wrote this charter and a review does not rewrite it -->

## Goal
Adversarially review the diagnostic controller, its retained self-checks and
route prerequisite handoff before the causal inquiries rely on them.

## Context
- Read the producer commit for `diagnostic-foundation-k11`, this grove's root
  brief, and `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- Implementation is under `experiments/exec-reliability/`; Taskfile entry points
  are `diagnostic:self-check` and `diagnostic:preflight`.
- Final controller controls are under `evidence/self-check-final/`; the earlier
  `self-check/` and read-only `preflight-20261101-01/` retain their own source
  digests. The separate resource audit is linked in the report.

## Done when
- Review finite declarations, identity pinning before signals/reaping, PID-only
  CLI interruption versus group deadlines, grace/kill/reap bounds, exception
  paths, raw stream retention and file/hash consistency. Check what a blocked
  filesystem or a normally exited leader with surviving helpers actually means
  for the report's limits; do not infer arbitrary descendant containment.
- Check that failed acquisition still reaches exact owned disposal, cleanup and
  observation failures cannot suppress independent disposal, failed inspection
  stays uncertain, and no unrelated resource is targeted. Review the residual
  remedy and the audit separately from the original uncertain receipt.
- Verify that failure controls can reject a broken instrument, source hashes
  describe the run measured, and `ok` distinguishes instrument self-check success,
  raw host exit and guest outcome. Check each final control, not just totals.
- Verify public help/source support for the per-OS installation and teardown
  recipes. Challenge any unavailable route treated as usable, any stale Local
  Network observation treated as fresh health, and any hidden preparation
  shifted into a causal inquiry. No production repair or live VM disposal is
  established by these local mocks.
- Produce findings without fixes. Insert reconciliation ahead of the first live
  consumer only if actionable findings warrant it; otherwise retire. Preserve
  the live causal inquiry chain and the historical 0/10 unavailable trial.

## Notes
Do not invoke `agent:macos:test`: k3 still owns scoping its unsafe process-name
cleanup. The installed app occupies the default socket. Do not run a routing
suite that would replace/quit it, or live guest probes, to obtain this review.
The controller self-check uses owned local subprocesses and fake peers only.

<!-- tc: on -->

## Decisions (running log)

- Inspect only. This review read the producer commit `dfd48c9`, the source, the
  report, every final control receipt, the preflight receipts and the audit. It
  ran no test, build, lint, live probe or routing suite.
- Actionable findings exist. Insert `integrate-review-impl` before
  `macos-completion-k3`, which is the first live consumer of the controller.

## Findings

Severity order is high, medium, then low. Paths are relative to
`experiments/exec-reliability/` unless a path names another root. Line numbers
refer to commit `dfd48c9`.

### F1 (high): an interrupted call always fails the run and stops later calls

`controller.py:225-227` sets `ok` only when `reason == "exit"`. An interrupted
call therefore always has `ok: false`. The loop at `controller.py:279-285` then
stops, and every later declared call becomes unreached. No field declares an
expected interruption or an expected deadline. Receipt `b22264e5…/result.json`
records `ok: false` for the interruption control. That control has no later
call, so the self-check did not show this effect.

Consequence: a plan cannot interrupt the CLI and then observe the same owned VM.
The root acceptance inventory needs ten operations after the abrupt interruption.
`macos-disconnect-availability-k13` and `linux-disconnect-availability-k6` need
the same sequence. A second plan cannot reuse the VM, because the first plan's
`finally` disposes it. The report at
`docs/experiments/exec-reliability-diagnostic-foundation.md:63-64` presents
`interrupt_after` as the match for that comparison, but it does not state this
limit.

### F2 (medium): live routes are "unavailable" without a missing prerequisite

The route table at report lines 167-168 gives one reason for the unchanged
installed macOS and Linux routes: "No owned clone or fresh agent identity/health".
That reason is an omitted probe, not a missing prerequisite. The k11 brief asked
for a predeclared minimal identity/health probe where permitted, or a concrete
missing prerequisite. Thus each causal leaf must establish the route with its
first clone. The root brief forbids that transfer of hidden preparation.

Three facts make that first clone more than routine:

- The installed `vm start --help` says the `--platform` value is "linux or
  windows" and that macOS "is not yet ported to the Rust CLI"
  (`evidence/preflight-20261101-01/e17eacfc…/start-help.stdout`). The same help
  also gives macOS examples. The checkout source does route macOS through tart
  (`cli-rs/crates/testanyware-cli/src/commands/vm.rs:597-639`). But the source of
  the installed 2.7.0 binary is unproven. The report recommends
  `vm start --platform macos` and does not mention this contradiction.
- Report line 210 asks for absence in both the public VM inventory and the
  hypervisor inventory. A `Resource` has one `inspect` call with an exact stdout
  oracle (`controller.py:101-102`). An inventory listing holds other images, so
  its stdout is not a constant. Each consumer must write and hash a checker
  wrapper that the handoff does not supply.
- No boot or slot bound is declared. The help states that a QEMU agent wait can
  take ten to fourteen minutes.

The report's treatment of Local Network is accurate. It calls
`observed_at: 1793431658` (31 October 2026 07:27 UTC) an earlier observation, not
fresh health.

### F3 (medium): an exception or a stall in the controller's own I/O can stop disposal

The `finally` block at `controller.py:286-302` runs cleanup, disposal and
inspection in sequence. A failure that `_call` returns as a receipt does not stop
disposal. An exception that `_call` raises does stop it. That exception also
prevents `result.json`.

One cause comes from the validator itself. `controller.py:105` accepts the call names `plan`, `result`, `subjects-before` and
`intent-1`. `_call` writes `<name>.json` with exclusive creation
(`controller.py:55-59, 234`). Consider a cleanup call named `plan`. It runs, then
raises `FileExistsError` on `plan.json`. The disposal of that resource and of
every earlier-acquired resource is then skipped.

Every receipt write, and the stdout read at `controller.py:228-231`, is unbounded
synchronous I/O in the controller's own thread. A blocked or full evidence
filesystem can delay disposal without bound, or abort it. Report lines 91-94
exclude "disk failure". They do not state that this exclusion also removes the
45-second disposal guarantee.

### F4 (medium): escalation is unconditional, and its receipts can mislead

`controller.py:189-219` always sleeps the full TERM grace. It then always sends
KILL, even when the leader exited at once. The receipt records no time of exit
after TERM. It cannot show whether the CLI met the two-second grace. The brief
says KILL follows "if needed", but the receipt cannot show whether it was needed.

After a reap, the controller does not probe the group, for example with
`killpg(pgid, 0)`. So a group member that survives KILL is not reported. A member
in uninterruptible I/O is one example.

Five final controls record KILL with `"error": "[Errno 1] Operation not
permitted"`. They are `30fe9008…`, `349b1787…` (cleanup and inspection),
`be30cc27…` and `d77bff46…`. On macOS, `killpg` returns EPERM when the group
holds only the zombie leader. The controller records that result as a generic
error, and the report table at lines 117-127 does not mention it. A reader can
mistake it for a failed termination. The controller also cannot tell it apart
from a real permission failure.

The fork control `b7edf56a…` records exit -9, not the graceful exit its peer
intends. The handler at `peer.py:25-27` calls `child.wait(timeout=1)` while the
interrupted outer `child.wait()` holds the Popen wait lock. So that control
measured the KILL path. The control still shows that a group deadline reaches a
child in the group.

### F5 (medium): raw streams are not pinned, and the normal exit with surviving helpers is not tested

A receipt records no digest or length of `<name>.stdout` or `<name>.stderr`. The
`result.json` file is not digested either. A leader can exit normally while a
helper in its group survives. That helper keeps the inherited stdout file open,
so it can append bytes after the `stdout_matches` check and after `result.json`.
No later record shows the change.

No control covers this case. The report states
at lines 95-97 that a normal completion does not prove absence of helpers. It
does not state this consequence for the retained evidence or for the absence
oracle.

### F6 (medium): several controls cannot fail, and the self-check verdict is not retained

- No control has exit 0 with wrong stdout. If `stdout_matches` always returned
  true, every test would still pass. The tests check only one direction of the
  absence oracle.
- No control separates PID-only interruption from a group signal. The `ignore-term`
  peer gives the same receipt under both. `signal_scope` at `controller.py:176` is a
  label derived from `interrupt_after`. It does not record what the controller
  did.
- `test_controller.py:82` asserts `os.getpid() == controller_pid`. That assertion
  cannot fail while the test runs.
- No pytest, Ruff or Pyright output is retained. Most final `result.json` files
  correctly say `ok: false`. Only the report prose maps each one to "the test
  passed". The `a03dc090…` row cites a run with `ok: true` and no calls. Its
  rejected foreign identity and its rejected reuse left no receipt. So the
  evidence cannot separate instrument success from a command result. The
  `self-check-index.json` file has no generating recipe.

### F7 (low): the audit closes one of six uncertain ledger entries

The self-check receipts leave six resources `uncertain`: three under `self-check/`
and three under `self-check-final/`. The audit
`evidence/resource-audit/6440c5d9…` removed and inspected only
`diag-444a6cccc6b546599ef421539c5a01a9-vm`. Report line 243 calls it "the
remaining fake-VM marker". The marker `diag-623a274a31dc4b2db83a1ac2aaf6eb03-vm`
had the same blocked disposal, but nothing inspected it. Pytest has since pruned
both scratch roots, so the remedy argv now names missing paths.

The audit's PID
list matches all 49 PIDs in the receipts, including both fork children. But its
oracle, exit 1 with empty stdout, also accepts a `ps` usage error. Only the empty
`known-process-inspection.stderr` file excludes that case.

### F8 (low): disposal of an absent resource always records "uncertain"

`vm stop` exits 3 with `VM_NOT_FOUND` for an absent VM (stop help). According to
the start help, a failed `vm start` removes its own clone. A failed acquisition
is then followed by a disposal that exits 3. The disposal has one
`expected_exit`, so the entry stays `uncertain` even when inspection proves
absence. Its remedy argv repeats the failing command.

### F9 (low): small inconsistencies

- In `Taskfile.yml:17`, the default `RUN` is `self-check`. A run without `RUN`
  writes new run directories into the earlier 12-test condition's directory.
- `preflight.py:88` silently removes a missing subject. Report line 48 says a
  missing subject prevents cases.
- A skipped resource acquisition does not appear in `unreached` or in the ledger
  (`controller.py:271-273`).
- The module docstring says "POSIX hosts only". `_validate` accepts macOS only
  (`controller.py:1, 82-83`).
- The `vm:create-golden-*` recipes check only that `NAME` is not empty. They do
  not refuse an existing golden name. The rule against replacing a retained
  golden depends on prose.

### Checked with no finding

- Pinning: the controller sends no signal after a reap, and an unreaped leader
  holds the group ID. The deadline path sends group signals, and the
  interruption path signals only the PID.
- A failed acquisition still reaches cleanup, disposal and inspection, because
  intent is written before acquisition. A failed inspection leaves the entry
  `uncertain`.
- Every resource action must name the run-unique identity as an argv token. The
  controller signals only its own child PIDs and groups.
- The source digests in each final receipt match the committed files. The
  controller digest is `907f7319…`. The preflight receipts record the earlier
  controller `0086e83f…`, as the report states.
- The report keeps the historical 0/10 unavailable trial and claims no repair or
  live disposal.
