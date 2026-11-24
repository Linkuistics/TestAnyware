# Acceptance trial plan and checker (k223)

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

Leaf `acceptance-plan-k223` delivers the trial plan and the checker for the
root acceptance inventory. Leaf `acceptance-plan-k242` integrates the review of
leaf `acceptance-plan-k241` into them. No guest booted in either leaf. The ten-clone
acceptance trial stays **0/10, unavailable** until leaf `lifecycle-acceptance-k225`
reports.

The plan and the checker follow the spec
[`guest-command-lifecycle.md`](../specs/guest-command-lifecycle.md), section
"Validation route". They also follow the root brief, sections "Acceptance
inventory and bounds" and "Constraints". Leaf `acceptance-preparation-k224`
runs the corruption controls against the real builds and freezes the plan.
[`exec-reliability-acceptance-preparation.md`](exec-reliability-acceptance-preparation.md)
reports that freeze. A change to the plan after that freeze needs a new freeze.

## Files and tasks

| File | Content |
|---|---|
| `experiments/exec-reliability/acceptance_k223.py` | The trial plan and the trial driver |
| `experiments/exec-reliability/acceptance_k223_cancel.py` | The explicit cancellation helper |
| `experiments/exec-reliability/acceptance_k223_app.py` | The helper that records the answering app |
| `experiments/exec-reliability/acceptance_k223_check.py` | The checker and the corruption controls |
| `experiments/exec-reliability/test_acceptance_k223.py` | 161 controls without a guest |

- `mise exec -- task diagnostic:acceptance-trial OUT=<new> MANIFEST=<k224 manifest>`
  runs the trial.
- `mise exec -- task diagnostic:acceptance-check CLONE=<clone directory>` judges
  one retained clone again.

## The trial

The trial runs five fresh macOS clones, then five fresh Linux clones, one at a
time. One clone is one controller plan with one owned VM. The trial stops after
the first clone that fails its run or its check. Later clones stay unreached.
`OUT/trial.json` gives the count out of 10.

Each clone runs the cases below in the root order. Each `file exec` call names
its own exec id. Each Host CLI call uses JSON mode.

| Step | Calls | Content |
|---|---|---|
| 1 | `app-identity`, `health-before`, `identity`, `bootstrap-dir`, two uploads | The answering app, agent health, the answering agent, and the retained fixture's `guest.sh` and `lesson` |
| 2 | `baseline` | The fixture `inventory` action: no installation and no marker |
| 3 | `install` | The offline installation |
| 4 | `intact` | The lesson itself, so the record evidence holds the stdout and stderr oracle bytes |
| 5 | `ordinary-01` to `ordinary-10` | Health, `printf ordinary-ok` and a process table, in turn |
| 6 | `exit-23` | The issue payload `exit 23` |
| 7 | `timeout` | The child payload with guest timeout 2 s |
| 8 | `after-timeout-01` to `after-timeout-14` | Ten operations, health and `printf ordinary-ok` in turn, and four child checks besides them |
| 9 | `cancel` | The helper: health after 1 s, then `file cancel --wait 5` after 2 s |
| 10 | `after-cancel-01` to `after-cancel-14` | As step 8, for the cancelled child |
| 11 | `interrupt`, `interrupt-child`, `interrupt-health`, `interrupt-fallback` | SIGTERM to the Host CLI after 2 s, a child poll of 4 s at most, health, then `file cancel --wait 5` |
| 12 | `capture`, seven downloads, `download-missing` | The fixture `intact` action, the four oracle files, the three partial files and one typed failure |
| 13 | `cleanup`, `dispose`, `inspect` | The `finally` path: guest cleanup, disposal and the absence inspection |

The macOS clone uses the retained r3 fixture. The Linux clone uses the
retained r5 fixture. The plan checks both fixtures against their frozen
digests before it starts. The online case is not in the root inventory, so the
plan omits it.

The timeout, cancellation and interruption cases use the issue's child payload.
Each case has its own directory under `/tmp/taw-k223`. The payload also prints
`partial-out` to stdout and `partial-err` to stderr, so each record carries
partial evidence.

The controller runs one call at a time. Explicit cancellation needs a second
Host CLI process while `file exec` runs, so a helper is one controller call.
The helper uses only the standard library, because the plan runs it with
`python -I`.

The root asks for "ten after-timeout operations and child checks". The plan
takes the stricter reading. Each after block has ten health and observation
operations, and four child checks besides them: first, after the third and the
sixth operation, and last.

The abrupt interruption is a separate case. The controller sends SIGTERM to the
Host CLI after 2 s. It sends SIGKILL only when the Host CLI is still alive 2 s
later. The Host CLI sends its cancellation with `waitMs: 0`, so its death does
not mean that the agent finished the termination.

`interrupt-child` therefore
polls the known child every 0.1 s, 41 times at most, and stops at the first
absent poll. The 40 waits cover the agent's 2 s grace and 2 s kill interval.
The checker requires absence at the last poll, and the raw stdout keeps each
earlier poll. A health call follows. Then `file cancel --wait 5` is the
fallback for the same exec id. An absent child before that fallback shows
that the cancellation of the Host CLI ended the command, because the guest
deadline is 20 s.

The app identity is a helper call, because `doctor` exits 1 when any host
check fails, for example the install path of a private Host CLI. The controller
stops a plan at its first failed call. The helper runs `doctor --json` within
the observation ceiling and reports its exit, stdout and stderr.

## The freeze manifest

The trial reads the freeze manifest of leaf k224, as leaf k244 reconciled
it after review k243. The manifest is one JSON object:

| Field | Content |
|---|---|
| `cli`, `tart` | Absolute paths of the private Host CLI and of Tart |
| `goldens` | The new golden name for `macos` and for `linux` |
| `bounds` | `boot` and `slot` in seconds, each positive and finite |
| `agents` | The frozen agent identity of `macos` and of `linux`, each a SHA-256 |
| `app` | The `build` and the absolute `executable` of the private app, and the absolute `config` file that the app and the Host CLI read |
| `golden_disks` | For `macos` and for `linux`, the absolute `path` of the golden's `disk.img` and its frozen `sha256` |
| `sha256` | Absolute paths with their frozen digests. It must pin `cli`, `tart`, the app executable and the app configuration file |

The macOS agent identity is the SHA-256 of `/usr/local/bin/testanyware-agent`.
The Linux agent identity is the SHA-256 of the sorted `sha256sum` listing of the
`*.py` files under `/opt/testanyware`, each line with its line feed. That is
the listing that the `identity` case prints first. The app identity is the
`build` and the `executable` that `doctor --json` reports for the running app.

The trial refuses to start when a field is missing or malformed, or when a
digest differs. It checks the manifest digest, every frozen digest and the
fixture digests again before each clone. It also refuses an interpreter that
is not a frozen path. The refused clone counts as failed, and the later clones
stay unreached. A changed subject is a new candidate, so it needs a new freeze
and a new trial.

Before each clone, the trial hashes the golden disk of that
clone by streaming. A difference stops the trial before that clone starts.
The trial hashes the disk again after the clone and writes both digests to
`golden-disk.json`. A 50 GB disk takes about 21 s to hash. It cannot be a
controller subject, because the writer hashes its subjects within the 5 s
readiness ceiling.

The boot bound limits the whole `vm start`: the clone, the boot and the agent
health. `vm start` runs without `--wait`, so a full slot fails the acquisition
at once with `VM_LIMIT_REACHED`. No clone waits for a slot. The manifest still
declares the slot bound, and each inventory records that no clone exercises
it. The export bound belongs to k224, because no clone exports an image.

## The checker

The checker reads JSON mode only. It compares the decoded `evidence` bytes
with the byte oracle, because text mode keeps the trimmed legacy streams.

The checker keeps two questions apart:

- Validity applies the spec's validity rules to each `file exec` envelope and
  to each `file cancel` record. It accepts both leader orders. It also accepts
  a record with `leaderStatus` `unavailable`.
- Acceptance applies the bounds to a valid record. A valid record whose group
  did not end within the bounds fails acceptance.

Every ceiling and bound of the root brief and the spec is a checker limit. The
docstring of `acceptance_k223_check.py` lists each limit with its observation.
The checker also rebuilds the canonical plan from the frozen identities. It
compares each argv, budget and expectation with the plan that the controller
ran.

The checker judges three properties of the run itself:

- Identity: the `identity` output gives the frozen agent identity, and the
  `app-identity` helper shows the frozen app build and executable. The app
  loaded the frozen configuration file, and that file has not changed since
  the app evaluated it. The `doctor` report resolves the frozen Tart. The
  configuration file can name another Tart, so a pinned Tart path alone does
  not show which Tart the app runs.
- Freeze: the manifest still has its frozen digest and gives the inventory's
  digests and identities. Each frozen path, the manifest and each fixture file
  had its frozen digest before and after the run. Each checker module has,
  when the checker runs, the digest that the run's subjects recorded. The
  interpreter of the plan is a frozen path. The golden disk had its frozen
  digest before and after the run.
- Execution: the recorded calls have the plan's names in the plan's order, each
  with its planned argv. Their start times never decrease.

The termination check also checks the payload of each leader status: an
integer `exit_code` for `exited`, an integer `signal` for `signalled`, and no
other field. The Host CLI writes exactly these shapes.

The checker runs the corruption controls on independent in-memory copies of
each clone's evidence:

| Control | Damaged copy |
|---|---|
| `appended-lf` | The `intact` record with a line feed after stdout. Its length and digest change with it, so only the byte oracle rejects it. |
| `flipped-byte` | The downloaded `artifact.bin` with one changed bit |
| `missing-outcome` | The `timeout` record without `outcome` |
| `contradictory-bounds` | The `timeout` record with `groupEmpty` `no` and `withinBounds` true |
| `contradictory-order` | The `timeout` record with `groupEmptyAfterMs` before `killSentAfterMs` |

When `killSentAfterMs` is 0, the order control puts `leaderExitAfterMs` after
`finalisedAfterMs` instead. Afterwards the checker reads the originals again.
Their digests must not change, and each original must still pass.

## Verification

All verification ran on the owner's macOS host without a guest.

The verification of leaf k223 follows.

- `self-check-01` failed. Its pytest step reached the frozen 120 s budget, at
  90 % of the earlier tests. Leaf k19 recorded this limit: the suite took
  124 s and 127 s. This leaf raised the pytest budget of the self-check to
  300 s. The budget keeps the runner finite, and each test keeps its own bounds.
- `self-check-02` passed. Lint, format, strict types and 1,184 tests passed in
  126.5 s. The subject digests before and after the run are equal.
- The 96 new controls build a clean clone for each OS and break one property
  at a time. Mutation controls show that each corruption control fails when its
  damage changes nothing. They also show failures when the checker loses its
  termination rules or its byte oracle, and when a control changes an original.
- `diagnostic:acceptance-check` exits 0 on a clean synthetic clone. It fails on
  the same clone after one changed bit in `artifact.bin`.
- `diagnostic:acceptance-trial` refuses a manifest without a slot bound. It
  also refuses a manifest with a wrong digest. Neither refusal creates `OUT`.

The evidence is in `experiments/exec-reliability/evidence/acceptance-plan-k223/`.

## Review integration

Leaf k242 read the source again for each finding of the review in
[`exec-reliability-acceptance-plan-review.md`](exec-reliability-acceptance-plan-review.md).
Each counterexample reproduces from the source. The table gives each
disposition.

| Finding | Class | Repair |
|---|---|---|
| R1: the child check after the interruption could see a permitted zombie | Real issue | A bounded poll of 4 s, with absence required at the last poll |
| R2: the health inequality rejected a valid slow finalisation | Real issue | Subtract `duration_ms` and `finalised_after_ms` from the earlier reply end |
| R3: the trial did not compare the answering agent and app with the freeze | Real issue | Frozen agent and app identities, the `identity` judgement and the `app-identity` helper |
| R4: the frozen digests applied only once, before the first clone | Real issue | A freeze check before each clone, and a freeze judgement in the checker |
| R5: the boot bound could use the slot allowance | Real issue | The boot bound alone limits the acquisition |
| R6: a leader status without its payload passed | Real issue, small risk | The payload keys and their integer types |
| R7: the checker judged the planned order, not the recorded order | Real issue | The recorded names, argv and start times |
| The count of the after blocks was ambiguous | Contract stated unclearly | The stricter reading: ten operations, and child checks besides them |

The new controls cover each repair. A mutation run reverted each repair in a
scratch copy, one at a time. Each mutant failed between two and six controls.
The verification of leaf k242 ran on the owner's macOS host without a guest:

- `self-check-01` passed. Lint, format, strict types and 1,236 tests passed in
  129.7 s. The subject digests before and after the run are equal.
- The 148 controls pass. `mutations.txt` records the mutation run.
- `diagnostic:acceptance-trial` refuses a manifest without `agents`, and it
  creates no `OUT` directory.

The evidence is in `experiments/exec-reliability/evidence/acceptance-plan-k242/`.

The R2 control uses the review's timeline: a 1.5 s finalisation, a reply that
ends at 3.70 s, and health at 1.00 s. That clone now passes. Health at 0.15 s
still fails, because the spawn came by 0.20 s at the latest.

## Limits

- The checker controls ran on synthetic evidence in this leaf. Leaf k224 runs
  them against the evidence of the real builds.
- No later clone follows the fifth clone of an OS. The reset of that clone
  therefore stays undemonstrated, and `trial.json` says so.
- The helper uses an inequality to prove that health ran during the command.
  The health start must follow the latest possible spawn: the earlier end of
  the two replies minus `duration_ms` and `finalised_after_ms`. The reply
  latency after publication stays inside that moment. A reply that takes
  longer than the gap between the spawn and the health start leaves the overlap
  unproven, and the case then fails. The derivation also takes the guest and
  host clock rates as equal.
- The app identity is the answer of `GET /v1/status`, as `doctor` reports it.
  It names the app that answered `doctor`, once in each clone. It does not
  prove that the same app answered every later call of the clone.
- The interruption poll starts after the death of the Host CLI. When the CLI
  received the reply to its cancellation, the outcome came before that death,
  so the poll covers the agent's whole termination interval. When the reply
  did not arrive within 1.5 s, the outcome time is unknown, and a late absence
  can then fail the case.
- The slot bound has no observation in this trial, because no clone waits for
  a slot.
- A download that succeeds proves its bytes only. On the app route, a guest
  reset after a complete body reads as success. Bytes that reached the host
  still fail the framing check. No result of the trial claims reset detection.
- A child check in an after block that finds a zombie or another command fails
  the case. Each after block starts after a published terminal record, so a
  within-bounds termination has already emptied the group.
