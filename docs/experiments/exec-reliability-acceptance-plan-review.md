# Acceptance plan review (k241)

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

The acceptance plan and checker in producer commit `b62275b4`
(`acceptance-plan-k223`) need reconciliation before preparation freezes them.
This review finds false failures of permitted cancellation behaviour and gaps
that let evidence pass without establishing the frozen acceptance conditions.
No production or test code changed, no test, build, lint or format command
ran, and no guest booted. Guest acceptance remains **0/10, unavailable**.

The contract is the root brief's acceptance inventory and constraints, the
[lifecycle spec](../specs/guest-command-lifecycle.md), and the producer's
acceptance conditions. Locations below refer to that producer commit. Each
counterexample is derived from source; it is not a newly executed reproducer.

## Findings

### R1 — P1: interruption requires child absence before the allowed termination interval ends

Locations: `experiments/exec-reliability/acceptance_k223.py:392`–395 and
`experiments/exec-reliability/acceptance_k223_check.py:618`–621.

The plan runs `interrupt-child` immediately after the Host CLI dies. The CLI
sends cancellation with `waitMs: 0`, so its death does not mean that the
agent has finished terminating the group. The spec allows up to two seconds
of grace followed by up to two seconds for kill, drain and reap. The checker
nevertheless requires this first snapshot to report `absent`.

For example, the controller sends SIGTERM at host time 2.00 s, the agent
accepts cancellation at 2.05 s, and the CLI dies at 2.10 s. A child snapshot
at 2.20 s can show the terminated `sleep` as a zombie while its reap is still
pending. The group can become empty at 2.80 s and its valid cancelled record
can be published at 3.00 s. All lifecycle and CLI bounds hold, but
`child_state()` returns `zombie` and `judge_exec()` fails the clone. The
later waited fallback cannot cure the earlier judgement.

This is an acceptance-blocking race, not evidence of an over-bound survivor.
The interruption check must establish absence against the permitted
termination deadline, while retaining the early snapshot as an observation.
It must continue to distinguish what the CLI's own cancellation accomplished
from what a later fallback accomplished.

### R2 — P1: the health-overlap inequality rejects healthy, within-bound finalisation

Location: `experiments/exec-reliability/acceptance_k223_check.py:722`–728.

The checker subtracts the guest's spawn-to-outcome duration from the host time
when the waited cancellation reply ends. That result includes finalisation
and reply latency. It is an upper bound on spawn time under compatible clock
rates, not an observation of spawn time. Failing that sufficient test does
not show that health preceded spawn.

Consider these seconds relative to the helper's origin:

| Event | Time |
| --- | ---: |
| Guest spawn | 0.10 |
| Health starts / ends | 1.00 / 1.10 |
| Cancellation starts / reaches the agent | 2.00 / 2.10 |
| Outcome chosen | 2.10 |
| Record published | 3.60 |
| Cancellation reply ends | 3.70 |
| Exec reply collected | 3.80 |

Let the record state `duration_ms: 2000`, `finalised_after_ms: 1500`,
`term_sent_after_ms: 0`, `kill_sent_after_ms: 10`,
`leader_exit_after_ms: 5`, `group_empty_after_ms: 20`, `group_empty: yes`
and `within_bounds: true`. With matching partial bytes and legacy fields,
the record is valid and within the production bounds. Health took 0.10 s
during the running command, and the cancellation reply took 1.70 s.
Nevertheless, `spawned_by` becomes `3.70 - 2.00 = 1.70`; the checker fails
health's actual 1.00 s start.

The report acknowledges this limitation, but the root does not exclude
permitted finalisation latency from acceptance. Preparation needs an
observation that can establish overlap for such runs without narrowing the
production contract. Comparing duration intervals alone does not require
equal host and guest clock origins; uncertainty about their rates is an
additional limitation, not the cause of this counterexample.

The synthetic clean helper in `test_acceptance_k223.py:533`–556 does not
cover this: its record takes 1900 ms to the outcome and 1910 ms more to
finalise, yet the terminal cancellation reply ends at 2.20 s after launch.
Those timestamps cannot describe a command spawned after launch.

### R3 — P1: answering build identities are recorded but never checked

Locations: `experiments/exec-reliability/acceptance_k223.py:136`–143,
`:371`, `:444`–480; `acceptance_k223_check.py:612`–622.

The manifest reader accepts `sha256: {}` and stores no expected guest-agent
identity. The `identity` call receives only the generic exited-zero,
valid-record and timing checks. Neither its digest nor the answering
platform is compared with a frozen expectation, and there is no answering
app identity check in the plan.

A valid `identity` envelope with exact stdout `ok\n`, exit 0 and no build
hash passes this branch. The retained clean-clone test actually uses that
default envelope for `identity` (`test_acceptance_k223.py:515`–516).
Likewise, a different agent build implementing the same record shape can
pass the trial. A failed macOS `shasum` can also be masked by the final
successful `uname`, since the identity command uses semicolons and the
checker does not judge its output.

The root requires proof of which isolated app and agent answer each run.
Names and version-compatible replies do not establish that the frozen
candidate was measured. The freeze contract must require the relevant
identities and the trial must validate the answering identities before
treating the clone as an acceptance sample.

### R4 — P1: freeze digests are not enforced across the ten-clone sequence

Locations: `experiments/exec-reliability/acceptance_k223.py:627`–639;
`acceptance_k223_check.py:999`–1005; reused
`completion_k186_check.py:125`–139.

`main()` compares the manifest's digests once, before entering the clone
loop. The controller subsequently compares each clone's before and after
hashes with each other. Neither `run_clone()` nor `check_clone()` compares
those hashes with the manifest's frozen values.

Take a valid, nonempty manifest that pins the private CLI. After macOS clone
1 finishes, replace that CLI with another compatible build at the same
path and leave it unchanged through clone 2. Both hashes of clone 2 are
equal, its argv still uses the canonical path, and its records can satisfy
every case. The trial can count both builds in one successful condition.
This violates the rule that a revised candidate requires a new freeze and
trial. It remains a gap even when k224 supplies every required digest.

Related subject coverage is also incomplete unless k224 adds it explicitly:
`plan()` omits `completion_k186_check.py`, which the checker imports and
uses after the controller's after-hash. The controller adds its own runtime
modules automatically, but not this checker dependency. The required
subject set and the points at which frozen hashes apply need to include
the checker phase as well as workload execution.

### R5 — P2: adding boot and slot budgets hides a phase overrun

Locations: `experiments/exec-reliability/acceptance_k223.py:535`–539 and
`experiments/exec-reliability/acceptance_k223_check.py:788`–793.

The manifest declares boot and slot as separate bounds, but the only
measurement and limit is total `acquire` time against their sum. For
`boot: 420` and `slot: 60`, a start with no slot wait and a 450-second boot
passes the 480-second acquisition check. The declared boot bound failed.
The `vm start` argv does not even request `--wait`, so the slot allowance
can simply become extra boot time.

The root and spec require declared phase bounds to be checker limits.
Either those phases need their own observations and enforcement, or the
contract needs an explicit reconciliation before the freeze. Merely
checking that the stored bounds equal the same summed plan cannot detect
this overrun.

### R6 — P2: malformed leader-status payloads pass record validity

Location: `experiments/exec-reliability/acceptance_k223_check.py:247`–250.

`termination_failures()` checks the leader-status tag but never checks the
payload of `exited` or `signalled`. Starting from the valid timeout record
in `test_acceptance_k223.py`, replace only
`leader_status: {"kind": "signalled", "signal": 15}` with
`leader_status: {"kind": "signalled"}`. All ordering, legacy, evidence and
bound checks still succeed. The five corruption controls damage other
properties and also continue to reject their copies, so they do not expose
this wrong pass. A string-valued signal has the same problem.

The spec requires a numeric signal in that shape. The real Host CLI reader
also requires it through the `LeaderStatus::Signalled { signal: i64 }`
type in `cli-rs/crates/testanyware-protocol/src/exec_record.rs:159`–162.
The acceptance checker therefore accepts a retained success record that
the production reader cannot produce from valid input. Required payload
fields and their types need the same validation as their tags.

### R7 — P2: the checker compares planned order, not recorded execution order

Locations: `experiments/exec-reliability/acceptance_k223_check.py:997`–1005
and `:1013`–1020; `completion_k186_check.py:152`–154.

The canonical-plan comparison establishes the intended call order.
The actual `result.json` call names are passed to `_run_failures()`,
which rejects duplicates but does not compare their order with the plan.
The checker then retrieves receipts by name and judges them in planned
order, discarding their recorded sequence.

For a clean clone's evidence, swap the `timeout` and first
`after-timeout` entries in `result.json.calls`, leaving their contents
unchanged. The list has the same unique names, the plan comparison still
passes, and every case is judged from the same receipt as before. The
checker can report success for evidence whose execution order contradicts
the acceptance inventory. There is likewise no comparison of each actual
receipt's argv with the corresponding planned argv.

The controller itself executes its plan sequentially; this finding concerns
the acceptance checker's promised judgement of retained evidence. It must
reject a contradictory execution record rather than infer execution from
an unchanged plan file.

## Other inspected obligations

- The ordinary block contains four health calls, three `ordinary-ok`
  commands and three process tables. Each after block contains four child
  checks, three health calls and three `ordinary-ok` commands. Each block
  has ten calls in total. This satisfies a reading where child checks are
  part of the ten after operations; it does not supply ten additional
  operations plus separate child checks. This review does not infer a new
  numerical requirement from the root's ambiguous conjunction. Integration
  should make the intended count explicit before freezing it.
- The termination rules allow leader exit on either side of SIGKILL.
  `unavailable` with `group_empty: no` and `within_bounds: false` is accepted
  as valid, then fails acceptance as intended. A valid incomplete stream is
  also distinguished from a complete acceptance capture.
- The corruption controls parse fresh copies, reject changed items and
  re-read the originals. The inspected implementations do not mutate the
  source evidence. The reset record names only the next same-OS baseline
  and leaves the last clone's reset undemonstrated. No success is claimed
  as detection of a reset after a complete download body.
- `main()` stops the clone loop after a returned failure and retains its
  directory. The reused controller owns guest cleanup, independent disposal
  and inventory inspection in its `finally` path. This review does not
  replace earlier controller reviews or assert a newly measured disposal
  guarantee.
- Retained `self-check-01/verdict.json` reports a pytest timeout at
  120.013 s; `self-check-02/verdict.json` reports a pass at 126.684 s with
  equal before/after subjects. The second run and the increased 300-second
  runner budget have separate retained identities. This supports the
  aggregate-budget adjustment, not a claim that every test has an individual
  stall detector. No new finding is inferred solely from that adjustment.

## Evidence limits and handoff

Graph `index_status` and the coverage request for all inspected paths both
failed with: “CBM CLI could not start because a pre-coordination or unverified
CBM generation is active; close all CBM sessions and commands, then retry”.
No generation or coverage result was obtained. The review used direct reads
of the named source, tests, contracts and retained verdicts. It makes no
exhaustive repository or call-graph claim.

The paired integration step must independently classify each finding against
the acceptance contract, apply accepted repairs and perform the relevant
verification. This review supplies counterexamples and evidence, not fixes
or a revised acceptance claim.
