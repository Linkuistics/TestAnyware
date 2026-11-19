# acceptance-preparation-k244

**Integrates:** acceptance-preparation-k243

## Goal

Triage the findings of acceptance-preparation-k243 against the agreed
acceptance bar. Reconcile the findings that meet that bar before k225 uses
the preparation freeze.

## Context

- Read the findings from the review's own commit, named by its stable handle
  `acceptance-preparation-k243`.
- Producer: `acceptance-preparation-k224`.
- Root brief: the acceptance, constraints and inventory bounds.
- Spec: `docs/specs/guest-command-lifecycle.md`, especially the bounds and
  validation route.

## Done when

- Each finding has an independently verified classification under the
  integration procedure's acceptance and material-risk threshold.
- Necessary repairs and their verification are complete, and the durable
  preparation report records the dispositions and remaining limits.
- Any changed frozen input has a reconciled freeze before a trial clone uses
  it. Failed samples remain intact and guest acceptance is reported honestly.

## Notes

This step precedes `lifecycle-acceptance-k225`. Preserve the root's resource
and public-interface constraints. The earlier app handover authorisation was
limited to k224. Do not infer a new interval from that session.

## Decisions (running log)

- Read the six findings from review commit `8eb40a3f` and checked each one
  against the current source. Each one reproduces from the code. Each one is
  a real issue, and none is noise, an unclear contract or an accepted
  trade-off.
- R1 needs a repair. The trial consumes each golden disk, but no check
  compares it with the frozen disk digest. The root freezes every image
  input. The writer hashes its subjects inside the 5 s readiness ceiling.
  One 50 GB disk takes about 21 s, so it cannot be a controller subject. The
  trial therefore hashes the golden disk of each clone by
  streaming, before and after the clone. A difference before the clone
  refuses it. The checker requires both digests to equal the frozen digest.
- R2 needs a repair. The app and the Host CLI read one `config.rhai`, which
  can name another Tart and the slot count. The freeze pins that file. The
  `app-identity` case requires the app to have loaded the pinned file,
  unchanged since its evaluation. It also requires the resolved Tart of
  `doctor` to be the frozen Tart.
- R3 needs a repair. The trial and the preparation refuse an interpreter that
  is not a frozen pin. The checker requires the recorded interpreter to be a
  frozen pin.
- R4 and R5 need a repair. They affect only the preparation recheck, not the
  k225 judgement. That recheck is the documented way to check the k224
  corruption-control evidence again. A recheck that accepts a changed checker
  or changed bounds weakens that evidence. Both repairs are small,
  and `acceptance_k224.py` is not a trial pin.
- R6 needs a repair. The image comparison is the evidence that no retained
  golden was booted or changed. Under the root, missing evidence cannot count
  as success.
- The repairs change pinned trial files, so k225 needs a new freeze. The
  original manifest writer needs the private app to answer, and the app
  handover authorisation was limited to k224. A new `refreeze` verb derives
  the new manifest from the k224 manifest instead. It refuses unless each
  build, Tart and golden pin still has its k224 digest and each golden disk
  still has its k224 digest. It then pins the trial files, the interpreter
  and the configuration file again. The answering app is still checked in
  every clone. The k224 manifest stays unchanged.
- Every repair has controls without a guest: 13 new controls in the k223
  tests and 11 in the k224 tests. In a mutation pass, each disabled repair
  failed at least one control. The first R6 mutant removed only the presence
  test and survived, because the digest test still caught each case. A
  mutant that disabled the whole test failed three controls.
- The new app check passes on the real `app-identity` records of
  `prepare-macos-02` and `prepare-linux-01`. Each record fails when the
  frozen Tart is another path.
- `self-check-01` failed: 1,283 of 1,284 tests passed. The failed control is
  `test_close_acknowledgement_read_at_p_is_late[exited]`, the storage timing
  finding that k19 left open. Alone, it failed 3 of 20 runs
  (`storage-control-repeat-01`). It uses no module that k244 changed. Its
  failure gives `killed` instead of `incomplete`, and neither is a complete
  publication, so it cannot make a clone pass. It stays an open limit, and
  this leaf does not repair it. `self-check-02` passed on the same source
  state. Both samples stay.
- `refreeze` wrote the k244 manifest, SHA-256 `1d021e2cac3d...`, with 30
  pins. The nine carried pins and both golden disks had their k224 digests.
  Four pins changed: the two trial modules, the Taskfile and the new
  configuration pin. The k224 manifest keeps its digest `c95dab85ab3a...`.
  `preflight.txt` records that the trial accepts the k244 manifest and
  refuses the k224 manifest.
- Updated the body of lifecycle-acceptance-k225 to name the k244 manifest.
  No guest ran, and guest acceptance stays 0/10, unavailable.
- No new review follows. Each repair is a small check inside the reviewed
  design, and in-session controls and the mutation pass verify it. No repair
  rethinks the trial. The root still has live trial, release and handoff
  leaves, so no node closes with this leaf.
