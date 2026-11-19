# Acceptance preparation and freeze (k224)

Leaf `acceptance-preparation-k224` prepares the acceptance trial of leaf
`acceptance-plan-k223` and freezes its inputs. It builds a private Host CLI and
app, creates two new goldens, and writes the freeze manifest. It runs no trial
clone. The ten-clone acceptance trial stays **0/10, unavailable** until leaf
`lifecycle-acceptance-k225` reports.

The freeze manifest is
`experiments/exec-reliability/evidence/acceptance-preparation-k224/manifest.json`.
Its SHA-256 is
`c95dab85ab3ad208929de49db729c7ef8896df08dc617aaa75f5399dd6002d79`. The trial
reads it with `mise exec -- task diagnostic:acceptance-trial OUT=<new>
MANIFEST=<manifest>`. The plan and the checker are in
[`exec-reliability-acceptance-plan.md`](exec-reliability-acceptance-plan.md).

## Files and tasks

| File | Content |
|---|---|
| `experiments/exec-reliability/acceptance_k224.py` | The export, the manifest writer and the preparation clone |
| `experiments/exec-reliability/image_audit_k224.py` | The record and the comparison of the local Tart images |
| `experiments/exec-reliability/test_acceptance_k224.py` | 24 controls without a guest |

| Task | Action |
|---|---|
| `diagnostic:acceptance-images OUT=<new> HASH="<image> ..."` | Records each local image and hashes each named disk |
| `diagnostic:acceptance-images-compare BEFORE=<record> AFTER=<record>` | Judges each hashed image as unchanged and not booted |
| `diagnostic:acceptance-export OUT=<new> DEST=<new> GOLDENS="<golden> ..."` | Exports each golden once within the export bound |
| `diagnostic:acceptance-manifest OUT=<new> BUILD=<workspace> IMAGES=<record> EXPORTS=<exports.json>` | Writes the freeze manifest |
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
- A change to any pinned file needs a new freeze.

## Residual resources for leaf k227

This leaf owns these resources, and leaf k227 audits them:

- The jj workspace `TestAnyware.k224-build` and its builds.
- The archives in `TestAnyware.k224-exports`.
- The goldens `testanyware-golden-macos-27-k224` and
  `testanyware-golden-linux-24.04-k224`.

No clone of this leaf remains.
