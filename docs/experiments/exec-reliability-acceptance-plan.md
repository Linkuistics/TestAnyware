# Acceptance trial plan and checker (k223)

Leaf `acceptance-plan-k223` delivers the trial plan and the checker for the
root acceptance inventory. No guest booted in this leaf. The ten-clone
acceptance trial stays **0/10, unavailable** until leaf `lifecycle-acceptance-k225`
reports.

The plan and the checker follow the spec
[`guest-command-lifecycle.md`](../specs/guest-command-lifecycle.md), section
"Validation route". They also follow the root brief, sections "Acceptance
inventory and bounds" and "Constraints". Leaf `acceptance-preparation-k224`
runs the corruption controls against the real builds and freezes the plan. A
change to the plan after that freeze needs a new freeze.

## Files and tasks

| File | Content |
|---|---|
| `experiments/exec-reliability/acceptance_k223.py` | The trial plan and the trial driver |
| `experiments/exec-reliability/acceptance_k223_cancel.py` | The explicit cancellation helper |
| `experiments/exec-reliability/acceptance_k223_check.py` | The checker and the corruption controls |
| `experiments/exec-reliability/test_acceptance_k223.py` | 96 controls without a guest |

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
| 1 | `health-before`, `identity`, `bootstrap-dir`, two uploads | Agent health, agent identity, and the retained fixture's `guest.sh` and `lesson` |
| 2 | `baseline` | The fixture `inventory` action: no installation and no marker |
| 3 | `install` | The offline installation |
| 4 | `intact` | The lesson itself, so the record evidence holds the stdout and stderr oracle bytes |
| 5 | `ordinary-01` to `ordinary-10` | Health, `printf ordinary-ok` and a process table, in turn |
| 6 | `exit-23` | The issue payload `exit 23` |
| 7 | `timeout` | The child payload with guest timeout 2 s |
| 8 | `after-timeout-01` to `after-timeout-10` | A child check, health and `printf ordinary-ok`, in turn |
| 9 | `cancel` | The helper: health after 1 s, then `file cancel --wait 5` after 2 s |
| 10 | `after-cancel-01` to `after-cancel-10` | As step 8, for the cancelled child |
| 11 | `interrupt`, `interrupt-child`, `interrupt-health`, `interrupt-fallback` | SIGTERM to the Host CLI after 2 s, a child check, health, then `file cancel --wait 5` |
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

The abrupt interruption is a separate case. The controller sends SIGTERM to the
Host CLI after 2 s. It sends SIGKILL only when the Host CLI is still alive 2 s
later. A child check and a health call follow. Then `file cancel --wait 5` is the
fallback for the same exec id. An absent child before that fallback shows
that the cancellation of the Host CLI ended the command, because the guest
deadline is 20 s.

## The freeze manifest

The trial reads the freeze manifest of leaf k224. The manifest is one JSON
object:

| Field | Content |
|---|---|
| `cli`, `tart` | Absolute paths of the private Host CLI and of Tart |
| `goldens` | The new golden name for `macos` and for `linux` |
| `bounds` | `boot` and `slot` in seconds, each positive and finite |
| `sha256` | Absolute paths with their frozen digests |

The trial refuses to start when a field is missing or when a digest differs.
The acquisition budget is the boot bound plus the slot bound. The export bound
belongs to k224, because no clone exports an image.

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

## Limits

- The checker controls ran on synthetic evidence in this leaf. Leaf k224 runs
  them against the evidence of the real builds.
- No later clone follows the fifth clone of an OS. The reset of that clone
  therefore stays undemonstrated, and `trial.json` says so.
- The helper uses an inequality to prove that health ran during the command.
  The health start must follow a moment: the end of the cancellation reply
  minus the record's `duration_ms`. A slow route can leave this unproven, and the case
  then fails.
- A download that succeeds proves its bytes only. On the app route, a guest
  reset after a complete body reads as success. Bytes that reached the host
  still fail the framing check. No result of the trial claims reset detection.
- A child check that finds a zombie or another command fails the case. That
  choice can fail a clone whose child the kernel has not reaped yet.
