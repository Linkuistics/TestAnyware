# Acceptance preparation and freeze (k224)

Leaf `acceptance-preparation-k224` prepares the acceptance trial of leaf
`acceptance-plan-k223` and freezes its inputs. It builds a private Host CLI and
app, creates two new goldens, and writes the freeze manifest. It runs no trial
clone. The ten-clone acceptance trial stays **0/10, unavailable** until leaf
`lifecycle-acceptance-k225` reports.

The trial manifest is
`experiments/exec-reliability/evidence/acceptance-preparation-k244/manifest.json`.
Its SHA-256 is
`1d021e2cac3d46e3f33b4c49f82bb3e4d0c858e770099491d9f44f276bf3010d`. Leaf
`acceptance-preparation-k244` wrote it after review k243, as the section
"Review k243 dispositions (k244)" describes. The trial reads it with
`mise exec -- task diagnostic:acceptance-trial OUT=<new> MANIFEST=<manifest>`.

The k224 freeze manifest is
`experiments/exec-reliability/evidence/acceptance-preparation-k224/manifest.json`,
with the SHA-256
`c95dab85ab3ad208929de49db729c7ef8896df08dc617aaa75f5399dd6002d79`. It stays
unchanged as the record of this leaf. The repaired trial refuses it, because it
has neither the configuration file nor the golden disks. The plan and the checker are in
[`exec-reliability-acceptance-plan.md`](exec-reliability-acceptance-plan.md).

## Files and tasks

| File | Content |
|---|---|
| `experiments/exec-reliability/acceptance_k224.py` | The export, the manifest writer and the preparation clone |
| `experiments/exec-reliability/image_audit_k224.py` | The record and the comparison of the local Tart images |
| `experiments/exec-reliability/test_acceptance_k224.py` | 35 controls without a guest, 11 of them from leaf k244 |

| Task | Action |
|---|---|
| `diagnostic:acceptance-images OUT=<new> HASH="<image> ..."` | Records each local image and hashes each named disk |
| `diagnostic:acceptance-images-compare BEFORE=<record> AFTER=<record>` | Judges each hashed image as unchanged and not booted |
| `diagnostic:acceptance-export OUT=<new> DEST=<new> GOLDENS="<golden> ..."` | Exports each golden once within the export bound |
| `diagnostic:acceptance-manifest OUT=<new> BUILD=<workspace> IMAGES=<record> EXPORTS=<exports.json>` | Writes the freeze manifest |
| `diagnostic:acceptance-refreeze OUT=<new> MANIFEST=<earlier manifest>` | Writes a new manifest after a change to the trial files, from leaf k244 |
| `diagnostic:acceptance-prepare OUT=<new> MANIFEST=<manifest> OS=<macos or linux>` | Runs one preparation clone and checks it |
| `diagnostic:acceptance-prepare-check DIR=<preparation directory>` | Checks one preparation clone again |

The evidence of this leaf is in
`experiments/exec-reliability/evidence/acceptance-preparation-k224/`. The
running log of decisions is in the task file of the leaf.

## The private builds

The builds come from the jj workspace
`/Users/antony/Development/AbstractingComputers/TestAnyware.k224-build`. Its
source is commit `fee86f928e8e923a8449cc3e948fb015209a3ed9` with no workspace
changes. The globally installed CLI and app stay unchanged.

| Build | Path in the workspace | SHA-256, first 12 digits |
|---|---|---|
| Trial Host CLI, release | `cli-rs/target/release/testanyware` | `e5f6bab9e6ee` |
| Golden Host CLI, debug | `cli-rs/target/debug/testanyware` | `03fdf3ef842a` |
| App executable, build 0.0.1 | `cli-rs/target/app/dev/TestAnyware Dev.app/Contents/MacOS/testanyware-app` | `df777bf612b2` |
| macOS agent | `agents/macos/.build/release/testanyware-agent` | `7998bee77cf6` |

The app bundle has the identifier `dev.antony.TestAnyware.dev` and a
Developer ID signature. Tart is `/opt/homebrew/bin/tart`, with the SHA-256
prefix `ba1d1607084f`. The manifest holds each full digest.

The Linux agent identity is the SHA-256 of the sorted `sha256sum` listing of
`testanyware_agent/**/*.py`. Its value is `29ae6a9ab412...`. No agent reports
a version, so the identity and the source commit name each agent.

## The goldens

The human built both goldens from Terminal with the golden Host CLI. Each
recipe used `TESTANYWARE_AGENT_BIN_OVERRIDE` with the new agent. Golden
creation needs direct guest SSH, and Terminal has Local Network access.

| Golden | Built (UTC, 2026-11-18) | `disk.img` SHA-256, first 12 digits |
|---|---|---|
| `testanyware-golden-macos-27-k224` | 12:00:14 to 12:06:44 | `ad5baad8becd` |
| `testanyware-golden-linux-24.04-k224` | 12:07:10 to 12:18:58 | `18dabb80fdb6` |

The image identity of a golden is the SHA-256 of its `config.json`,
`nvram.bin` and `disk.img`. The manifest pins `config.json` and `nvram.bin` of
each new golden. The trial checks those pins before each clone.

`image_audit_k224.py` recorded the images three times:

1. `images-before.json`, before golden creation.
2. `images-after-goldens.json`, after golden creation.
3. `images-final.json`, after both preparation clones.

Each comparison passed. The four retained images stay unchanged and stopped,
and no retained image booted:

- `testanyware-base-macos-27`
- `testanyware-golden-linux-24.04`
- `testanyware-golden-macos-27`
- `testanyware-golden-macos-tahoe`

The macOS recipe clones from `testanyware-base-macos-27`, and that read left
the image unchanged. Both new goldens also stay unchanged and stopped after
their preparation clones.

## Bounds, ceilings and exports

| Bound | Value | Scope |
|---|---|---|
| Boot | 420 s | The whole `vm start`, with the clone, VNC, IP and agent stages |
| Slot | 30 s | Declared. No trial clone waits for a slot. |
| Export | 1800 s | One `tart export` of one new golden |

The manifest field `ceilings` lists every ceiling of the k223 plan and the
export bound. Leaf k227 audits an export resource, so this leaf exported each
new golden once.

| Archive | Bytes | Time | SHA-256, first 12 digits |
|---|---|---|---|
| `testanyware-golden-macos-27-k224.tvm` | 26,552,642,627 | 25.0 s | `23ddd69dd957` |
| `testanyware-golden-linux-24.04-k224.tvm` | 4,935,099,799 | 20.3 s | `48636d7e6d8a` |

The archives are in
`/Users/antony/Development/AbstractingComputers/TestAnyware.k224-exports`,
outside each jj workspace.

## The freeze manifest

The manifest has the fields that `acceptance_k223.read_manifest` reads:

- `cli` and `tart`
- `goldens`, with one macOS golden and one Linux golden
- `bounds`, with `boot`, `slot` and `export`
- `agents`, with the SHA-256 identity of each agent
- `app`, with the build and the executable of the private app
- `sha256`, with 29 pins

The pins cover the trial Host CLI, Tart, the app executable, the macOS agent
and the golden Host CLI. They also cover the golden files, the interpreter,
the trial modules, `pyproject.toml`, `uv.lock`, `Taskfile.yml` and
`scripts/vm_inventory.py`. The manifest also records the source, the builds
with their signatures, the agent sources, the images, the exports and the
ceilings.

The writer refuses in three cases. The answering app is not the private app,
the build workspace has changes, or an export failed. After both preparation
clones, each of the 29 pins still had its manifest digest.

## Verification

`self-check-01` ran in the frozen source state. Lint, format, strict types
and 1,260 tests passed in 128.5 s. The subject digests were equal before and
after the run.

The 24 new controls had a mutation pass. The unmutated copy passed. Each of
12 mutants failed at least one control. `mutations.txt` lists the mutants.

## The preparation clones

The corruption controls need real records from the private builds. One
preparation clone of each new golden supplies them. It runs the k223 cases up
to `download-artifact-bin`, with the k223 argv, and then the cleanup,
disposal and inspection. It writes the trial layout, so the unchanged k223
checker judges it. It is not a trial clone.

| Sample | Result | `vm start` |
|---|---|---|
| `prepare-macos-01` | Failed: `VM_BOOT_TIMEOUT`, cause `local_network_denied` | 20.4 s, exit 7 |
| `prepare-macos-02` | Passed: 16 of 16 cases | 16.4 s |
| `prepare-linux-01` | Passed: 16 of 16 cases | 20.3 s |

`prepare-macos-01` stays as a failed sample. macOS had not yet given Local
Network access to the new executable of the private app. The cleanup found no
VM, and the disposal and inspection found the clone absent. The grant is not
a pinned input, so the manifest stayed.

In each passing sample, the `identity` case read the manifest identity of the
agent. The checker rejected each damaged copy of the corruption controls:

| Control | Damaged item | Checker reason |
|---|---|---|
| `appended-lf` | `intact` | The stdout bytes differ from the oracle |
| `flipped-byte` | the artifact | The artifact bytes are not the oracle |
| `missing-outcome` | `timeout` | The record has no outcome |
| `contradictory-bounds` | `timeout` | The recorded bound differs from the derived value |
| `contradictory-order` | `timeout` | The termination times are out of order |

The originals stayed unchanged after the controls. The verb
`diagnostic:acceptance-prepare-check` gave the same verdict for both passing
samples.

## The app handover

The human authorised the handover for this leaf only:

1. 2026-11-18T10:24:16Z: the session quit `/Applications/TestAnyware.app`
   and opened the private app.
2. 2026-11-19T05:37:27Z: the session quit only the private app, by its bundle
   identifier, and reopened the installed app.

The installed app answers again as build 2.8.0. `restore.txt` and
`doctor-restored.json` record the restore.

## Notes for leaf k225

- The manifest names the private app as the answering app. Leaf k225 needs
  its own handover to the private app, with the human's authorisation.
- macOS records Local Network access by the code signature and the main
  executable UUID. The private app has the UUID
  `85863282-8EA4-3676-AD50-A94D299B2E66`. It needs the grant before the first
  trial clone.
- The human wants to remove stale Local Network entries. macOS 27.0.1 cannot
  remove an entry. TN3179 says the remove button came in macOS 27.2. A removal
  of the private app entry needs a new grant before k225.
- A change to any pinned file needs a new freeze. Leaf k225 uses the k244
  manifest, not the k224 manifest.
- The k244 manifest pins `/Users/antony/.config/testanyware/config.rhai`.
  The app reads that file only when it starts. An edit to the file refuses
  the next clone, and the private app must start after the last edit.
- Each clone hashes its golden disk twice, about 21 s each time. The ten
  clones add about 7 minutes to the trial. That time is outside every case
  ceiling.

## Review k243 dispositions (k244)

Review `acceptance-preparation-k243` reported six findings in
[`exec-reliability-acceptance-preparation-review.md`](exec-reliability-acceptance-preparation-review.md).
Leaf `acceptance-preparation-k244` checked each one against the source. Each
one reproduced, and each one was a real issue. No finding was noise, an
unclear contract or an accepted trade-off.

| Finding | Disposition | Repair |
|---|---|---|
| R1: the trial did not enforce the golden disk identity | Repaired | The manifest field `golden_disks` names each disk and its digest. Before each clone, the trial hashes the disk of that clone's golden by streaming, and a different digest refuses the clone. It hashes the disk again after the clone and writes both digests to `golden-disk.json`. The checker requires both digests to equal the frozen digest. |
| R2: the pinned Tart did not bind the app to that Tart | Repaired | The manifest pins the configuration file and names it in `app.config`. The `app-identity` case requires the app to have loaded that file, unchanged since its evaluation. It also requires `doctor` to resolve the frozen Tart. |
| R3: the Python pin did not constrain the interpreter | Repaired | The trial and the preparation refuse an interpreter that is not a frozen pin. The checker fails a clone whose plan names such an interpreter. |
| R4: the preparation recheck did not check its own source | Repaired | `check_preparation` requires the current digest of `acceptance_k224.py` to equal the run's subject digest after the run. |
| R5: the preparation shape accepted changed termination bounds | Repaired | `shape_failures` applies the k223 checks of the termination bounds and the complete bound table. |
| R6: the image comparison accepted missing content | Repaired | `compare` requires each file of each hashed image to be present in both snapshots, with a size, a digest and a modification time. |

A 50 GB disk takes about 21 s to hash. The controller writer hashes its
subjects within its 5 s readiness ceiling, so a disk cannot be a controller
subject. The trial therefore hashes each disk by itself.

### The new manifest

The k244 manifest needs no answering app. The verb `refreeze` derives it from
the k224 manifest. It refuses unless each carried pin still has its k224
digest and each golden disk still has its k224 image digest. It then pins
the interpreter, the trial files and the configuration file again. The
answering app is still checked in every clone.

- The nine carried pins are the trial and golden Host CLIs, Tart, the app
  executable, the macOS agent and the `config.json` and `nvram.bin` of each
  new golden. Each one had its k224 digest.
- Both golden disks had their k224 digests, in 20.6 s and 20.5 s.
- Four pins changed: `acceptance_k223.py`, `acceptance_k223_check.py`,
  `Taskfile.yml`, and the new pin of the configuration file. The field
  `refreeze.changed_pins` gives each earlier and current digest.
- The builds, the app identity, the images, the exports, the bounds and the
  ceilings are those of k224. The field `supersedes` names the k224
  manifest and its digest.
- `preflight.txt` records that the trial's freeze and interpreter checks
  accept the k244 manifest, and that the repaired reader refuses the k224
  manifest.

### Verification

All six repairs have controls without a guest: 13 new controls in
`test_acceptance_k223.py` and 11 in `test_acceptance_k224.py`. In a mutation
pass, each disabled repair failed at least one control, and the unmutated
copy passed. The first R6 mutant removed only the presence test, and it
survived because the digest test still caught each case. A second mutant
disabled the whole evidence test and failed three controls.

The new app check also ran against the real `app-identity` records of
`prepare-macos-02` and `prepare-linux-01`. Each record passes. Each one fails
when the frozen Tart is another path.

The evidence is in
`experiments/exec-reliability/evidence/acceptance-preparation-k244/`:

| Sample | Result |
|---|---|
| `self-check-01` | Failed. Lint, format and types passed. 1,283 of 1,284 tests passed. `test_close_acknowledgement_read_at_p_is_late[exited]` failed. |
| `storage-control-repeat-01` | That control alone, 20 times: 17 passed, and the `[exited]` case failed 3 times. |
| `self-check-02` | Passed. Lint, format, types and 1,284 tests passed in 128.3 s. |

Both self-checks measured the same source state, and each run had equal
subject digests before and after. The failed control is the storage timing
finding that leaf k19 recorded as open. It imports only `controller`,
`worker_capture` and `retention`, and k244 changed none of their modules. The
failing assertion gives the publication `killed` instead of `incomplete`.
Neither publication is `complete`, so this failure cannot make a trial clone
pass. It stays an open limit for a later leaf.

### Remaining limits

- The preparation samples of k224 ran under the k224 checker. The repaired
  checker refuses to recheck them, because its checker modules differ from
  their recorded digests and they have no golden disk record. Their saved
  `check.json` and `.recheck.json` stay the verdicts of record. Each k225
  clone runs the four corruption controls with the repaired checker.
- The app reports `changed_since_evaluation` from a file fingerprint, not
  from the content. An edit that keeps that fingerprint is not detected by
  this field. The pinned digest still refuses a clone after such an edit.
- No guest ran in k244. Guest acceptance stays **0/10, unavailable**.

## Residual resources for leaf k227

This leaf owns these resources, and leaf k227 audits them:

- The jj workspace `TestAnyware.k224-build` and its builds.
- The archives in `TestAnyware.k224-exports`.
- The goldens `testanyware-golden-macos-27-k224` and
  `testanyware-golden-linux-24.04-k224`.

No clone of this leaf remains.
