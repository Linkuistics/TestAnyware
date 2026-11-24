# Acceptance preparation review

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

The freeze of `acceptance-preparation-k224` has six gaps that affect its use
for the ten-clone trial. The image disks and selected runtime tools can change
without a freeze refusal. The preparation checker also accepts some records
that do not establish its promised judgement. Leaf
`acceptance-preparation-k243` reports these findings for independent triage
before `lifecycle-acceptance-k225`.

The reviewed producer is commit `a2c877cb`, whose source remains current.
The contract is the root brief's freeze and evidence requirements, k224's
`Done when`, and the lifecycle spec's bounds and validation route. The
counterexamples below are derived from source; no test, build, lint or format
command ran, and no guest booted during this review.

## R1 The trial does not enforce the golden disk identity

**Priority: high.** In `acceptance_k224.py:88` and `:273`,
`GOLDEN_FILES` includes only `config.json` and `nvram.bin`, so only those image
files enter `sha256`. The disk digest enters the descriptive `images` field
at `:321`. `acceptance_k223.read_manifest` (`acceptance_k223.py:534`)
discards that field. Its freeze refusal (`:723`) and the checker's
`freeze_failures` (`acceptance_k223_check.py:1039`) never check it.

Consider a new golden whose disk changes after preparation while its config
and NVRAM remain equal. Its agent files can remain equal too, for example when
the changed file belongs to the guest interpreter or another installed
package. Every manifest pin can still match, and the trial can clone the
changed disk without a freeze refusal. Equal subject digests before and after
each clone do not help because the disk is not a subject. This permits a
trial under a different guest input from the recorded image. The recorded
export digest does not close the gap: the trial clones the local golden,
not the exported archive.

The manifest's macOS and Linux disk digests are respectively
`ad5baad8becd...` and `18dabb80fdb6...`. They identify the preparation images,
but currently impose no condition on the images consumed by k225.

## R2 The pinned Tart path does not bind the app to that tool

**Priority: high.** `acceptance_k224.py:274` pins
`/opt/homebrew/bin/tart`. Its `answering_app` and `build_manifest` checks use
only the running app's executable and build. The trial's `app_identity_failures`
(`acceptance_k223_check.py:737`) also compares only those two fields.

The app selects lifecycle tools through `AppConfig::tool_search`
(`cli-rs/crates/testanyware-app/src/config.rs:148`). Configured locations
override the built-in search in `ToolSearch::find`
(`cli-rs/crates/testanyware-vm/src/tools.rs:77`), and `run_detached_with`
(`cli-rs/crates/testanyware-vm/src/tart.rs:609`) runs that selection.
The retained `doctor-dev-01.json` records that the private app loaded
`/Users/antony/.config/testanyware/config.rhai`; this input is outside the
manifest pins.

After the required app handover for k225, the same app executable can load a
configuration that names another Tart binary. The original pinned Tart may
remain unchanged, so the freeze and answering-app checks can pass while the
VM lifecycle uses another binary. The independently specified Tart used by
the inventory helper does not prove which Tart booted the VM. The freeze
therefore does not establish the claimed lifecycle tool identity. This also
leaves configuration such as the app's slot allocation outside the freeze.

## R3 The Python pin does not constrain the running interpreter

**Priority: medium.** `build_manifest` pins its own `sys.executable`
(`acceptance_k224.py:276`), but records no interpreter selection that the trial
must use. `run_clone` takes its launch's `sys.executable`
(`acceptance_k223.py:742`) and `plan` adds that path to the subjects (`:655`).
Neither `freeze_refusals` nor `freeze_failures` requires that path to be a
manifest pin. Canonical plan reconstruction takes the interpreter from the
record itself (`acceptance_k223_check.py:994`).

Launch `acceptance_k223.py` with another Python environment while leaving the
pinned environment intact. `pyproject.toml` permits Python 3.12 and later,
and the runner has no condition requiring the frozen interpreter. The
original interpreter's digest still matches, and the new
interpreter's before and after digests can be equal. No check compares that
new digest with a predeclared value. The controller, writer, app helper,
cancellation helper and disposal helper can therefore run under an
interpreter that the freeze never selected. The lockfile pin does not bind
that interpreter. This violates the requirement to freeze the tools before
measurement even when all guest cases happen to pass.

## R4 A preparation recheck does not validate its own source identity

**Priority: medium.** `trial_files` deliberately excludes
`acceptance_k224.py` (`:132`). `preparation_plan` adds it as a run subject
(`:358`), so its before and after digests are retained. However,
`check_preparation` (`:444`) delegates the current-source comparison to
`checker.freeze_failures`, whose `trial.checker_modules()` list
(`acceptance_k223.py:615`) does not include the preparation checker.

If `acceptance_k224.py` changes after a preparation run, its recorded subject
digests can remain equal and every manifest pin can remain intact. A recheck
then runs the changed judgement without detecting that it differs from the
recorded checker. A changed acceptance condition can reinterpret the saved
sample without a source-identity failure. The full trial explicitly compares
its checker modules with the run's after digests; the preparation recheck
does not provide the same guarantee for its own implementation.

This is a derived drift case. In the current retained samples, the preparation
source still matches its recorded digest.

## R5 The preparation shape accepts altered termination bounds

**Priority: medium.** `shape_failures` (`acceptance_k224.py:408`) compares
the calls and first resource with reconstructed values, then returns. It
omits the checks of `plan.term_grace`, `plan.kill_wait` and the complete
inventory bound table that `checker.plan_failures` performs
(`acceptance_k223_check.py:1011`). `check_preparation` does not call that
function or an equivalent preparation version.

Take a passing preparation record and change `plan.term_grace` from 2 to
100 in both `inventory.json` and the run's `plan.json`, leaving its successful
calls unchanged. `clone.order` still equals `SHAPE`; the calls and resource
still equal their reconstructions; the two plan records still equal each
other; and the receipts, frozen subjects and corruption controls are
unchanged. No remaining preparation check reads that altered field.
The derived result is a pass under a declared 100-second grace. The full
trial's plan check rejects the same alteration because its termination
bounds must be 2 seconds and 2 seconds.

Thus the preparation checker does not establish its advertised equivalence
with the k223 judgement. This counterexample concerns altered records; the
retained passing plans themselves declare the correct bounds.

## R6 The image comparison treats missing content as matching evidence

**Priority: medium.** `file_record` (`image_audit_k224.py:50`) returns
`{"present": false}` for a missing required image file.
`compare` (`:130`) compares `present`, `size` and `sha256` for equality,
without requiring presence or a digest. Its modification-time check uses
the same equality rule for absent times.

Consider two snapshots with `hashed: true`, a stopped Tart entry, and
`disk.img: {"present": false}` in each snapshot. Keep config and NVRAM
equal. The disk comparisons are `false == false` and `None == None`, and
the missing disk modification times are equal too. The derived result is
`ok: true`, although neither snapshot establishes the disk's content or
whether it was modified. Two otherwise equal records with the disk digest
omitted have the same problem. The existing controls check changed hashes,
times, size and state, but no missing-content case.

This violates the root requirement that missing evidence cannot count as
success. The retained snapshots do contain all three required files and
their digests, so this finding does not claim those actual files were absent.

## Retained evidence and review limits

Read-only inspection confirmed that the manifest still has digest
`c95dab85ab3ad208929de49db729c7ef8896df08dc617aaa75f5399dd6002d79`
and every pin matches. The two passing preparation records agree with their
saved rechecks. Their original `intact`, `timeout` and artifact bytes still
match the hashes recorded by the corruption controls. The control functions
construct independent copies and reread the originals. The macOS binary
identity and the Linux listing in those samples agree with the manifest.

`prepare-macos-01` retains exit 7, `VM_BOOT_TIMEOUT`, stage `agent`, and
cause `local_network_denied`; its controller measured about 20.4 seconds.
Its cleanup failed, and its disposal and inspection recorded absence. These
records support the reported route denial and preservation of the failed
sample. A doctor's `not_yet_seen` value alone does not establish the user's
privacy setting. The later passing sample is a separate observation.

The passing preparation VM names use `k224` and index `00`, distinct from
the trial's `k223` names and indices 01 through 05. The boot budget applies
to the whole acquisition call without slot waiting. The declared comparison
ceilings agree with the root and spec; the export calls reserve TERM and
KILL within their 1800-second bound. The retained disposal records support
the report's clone cleanup at those observation times. The named build
workspace, two exports and two new goldens remain resources for k227's audit.

Both graph status and coverage queries failed before serving a result because
an unverified CBM generation was active. Source inspection supplies the
findings; graph freshness and coverage remain unverified. This review
establishes no new measured reliability result. Guest acceptance remains
**0/10, unavailable**.
