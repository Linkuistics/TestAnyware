# Acceptance trial (k225)

Leaf `lifecycle-acceptance-k225` ran the frozen acceptance trial of leaf
`acceptance-plan-k223`. The trial used the freeze manifest of leaf
`acceptance-preparation-k244`. Five fresh macOS clones and then five fresh
Linux clones ran one at a time through the private app.

**Result: 10/10, passed.** Each clone passed every root case, the byte oracle,
every ceiling and the four corruption controls. No clone failed, and no clone
stayed unreached. The historical tutorial trial stays on record as **0/10,
unavailable**. This result does not replace it.

## Inputs

| Input | Identity |
|---|---|
| Freeze manifest | `experiments/exec-reliability/evidence/acceptance-preparation-k244/manifest.json`, SHA-256 `1d021e2cac3d46e3f33b4c49f82bb3e4d0c858e770099491d9f44f276bf3010d` |
| Trial Host CLI | `TestAnyware.k224-build/cli-rs/target/release/testanyware`, SHA-256 prefix `e5f6bab9e6ee` |
| Private app | `TestAnyware Dev.app`, build 0.0.1, executable SHA-256 prefix `df777bf612b2` |
| Goldens | `testanyware-golden-macos-27-k224` and `testanyware-golden-linux-24.04-k224` |
| Source of the builds | Commit `fee86f928e8e923a8449cc3e948fb015209a3ed9`, with no workspace changes |

The section "The private builds" of
[`exec-reliability-acceptance-preparation.md`](exec-reliability-acceptance-preparation.md)
gives each full identity. The trial ran this command:

```bash
mise exec -- task diagnostic:acceptance-trial \
  OUT=experiments/exec-reliability/evidence/lifecycle-acceptance-k225/trial-01 \
  MANIFEST=experiments/exec-reliability/evidence/acceptance-preparation-k244/manifest.json
```

Before the trial, each of the 30 pins of the manifest had its frozen digest.
After the trial, each pin still had its frozen digest. The trial also checked
every pin before each clone.

## The app handover

The human authorised the handover for this leaf only, on the terms of leaf
k224.

1. At 2026-11-19T09:22:29Z, the session quit `/Applications/TestAnyware.app` by
   its bundle identifier. It then opened the private app.
2. `doctor` showed build 0.0.1. The app loaded the pinned `config.rhai` at
   its start, and the file did not change after that load. `doctor` resolved
   `/opt/homebrew/bin/tart`.
3. The trial ran from 09:22:55Z to 09:34:53Z, which is 12 minutes.
4. At 09:37:38Z, the session quit only the private app, by its bundle
   identifier. It then opened the installed app again.
5. `doctor` showed build 2.8.0 again.

After the restore, `doctor` exits 1 because the app does not start at login.
The k224 restore has the same failure, so this leaf did not cause it.

## Results by clone

Each clone ran 67 checked cases in the plan's order. The table gives the
measured times in seconds. The interval runs from the start of a call to the
exit of its process.

| Clone | `vm start` | Timeout | Cancel | Interrupt | Cleanup | Disposal | Slowest ordinary call |
|---|---|---|---|---|---|---|---|
| macos-01 | 17.0 | 2.1 | 2.1 | 2.0 | 0.1 | 2.5 | 2.47 |
| macos-02 | 19.2 | 2.1 | 2.1 | 2.0 | 0.1 | 3.4 | 0.11 |
| macos-03 | 15.9 | 2.1 | 2.1 | 2.0 | 0.1 | 2.0 | 0.14 |
| macos-04 | 13.9 | 2.1 | 2.1 | 2.0 | 0.1 | 3.0 | 0.39 |
| macos-05 | 14.9 | 2.1 | 2.1 | 2.0 | 0.1 | 1.0 | 0.09 |
| linux-01 | 19.4 | 2.1 | 2.1 | 2.0 | 0.1 | 0.6 | 0.08 |
| linux-02 | 19.4 | 2.1 | 2.1 | 2.1 | 0.1 | 0.6 | 0.08 |
| linux-03 | 19.4 | 2.1 | 2.1 | 2.0 | 0.1 | 0.6 | 0.08 |
| linux-04 | 19.4 | 2.1 | 2.1 | 2.0 | 0.0 | 0.5 | 0.08 |
| linux-05 | 19.4 | 2.1 | 2.1 | 2.0 | 0.1 | 0.5 | 0.08 |

The ceilings are 420 s for `vm start`, 30 s for an observation and for the
cleanup, and 45 s for the disposal. In every clone, each download took 0.07 s
or less. Each call after the timeout and after the cancellation took 0.08 s or
less. `durations.tsv` holds these values. The checker applies each ceiling and
each termination bound to the records themselves.

The slowest ordinary call of macos-01 took 2.47 s, within its ceiling of 30 s.
The other ordinary calls took 0.39 s or less.

## Independent checks

The session checked the result in five more ways after the trial:

- **Recheck.** `diagnostic:acceptance-check` judged each of the ten clones again.
  Each one exited 0, with no failure in its 67 cases. In each clone the checker
  rejected each damaged copy, and the originals stayed unchanged.
- **Negative control.** A scratch copy of macos-01 had one bit flipped in
  `downloads/artifact.bin`. The recheck of that copy failed, because its
  corruption-control stage found a changed original.
- **Golden disks.** Each clone hashed its golden disk before and after it ran.
  Each digest equalled the frozen digest. Each hash took 18.6 s to 22.6 s.
- **VMs.** After the trial, `tart list` showed no VM with the prefix `k223-`, and
  no running VM. In each clone the ledger state was `disposed`, with no
  residual.
- **Pins.** Each of the 30 pins had its frozen digest after the trial.

## Reset

Each later clone of an OS started from the golden in a fresh state. The
`baseline` case found no installation and no marker in each clone. So clones 2
to 5 of each OS show the reset of the clone before them. The fifth clone of
each OS has no later clone, so its reset stays undemonstrated. `trial.json`
records this for macos-05 and linux-05.

## Limits

- This result is for the frozen builds only. A changed build, agent, golden or
  checker is a new candidate. It needs a new freeze and a new trial.
- The trial ran on one macOS host. It measures nothing about Windows.
- A pass does not promise zero future failures. Ten clones cannot show a rare
  timing defect.
- The app identity names the app that answered `doctor` once in each clone. It
  does not prove that the same app answered every later call.
- The limits of the plan still apply. The section "Limits" of
  [`exec-reliability-acceptance-plan.md`](exec-reliability-acceptance-plan.md)
  gives them.
- No result claims the detection of a guest reset after a complete download
  body.

## Evidence

The evidence is in
`experiments/exec-reliability/evidence/lifecycle-acceptance-k225/`:

| File | Content |
|---|---|
| `trial-01/trial.json` | The count out of 10, each run id and the reset record |
| `trial-01/<clone>/` | Each clone's inventory, raw receipts, downloads, check and golden disk record |
| `trial-01.stdout`, `trial-01.stderr`, `trial-01.exit`, `trial-01.started` | The trial's own streams, exit status and times |
| `recheck-<clone>.json` | The independent recheck of each clone |
| `durations.tsv` | The measured times of the table above |
| `handover.txt`, `doctor-private.json` | The handover to the private app |
| `restore.txt`, `doctor-restored.json` | The restore of the installed app |
| `tart-list-after.txt` | The local VM list after the trial |

## Residual resources

This leaf created no lasting resource. The ledger disposed of each trial VM. Leaf k227
audits the resources of leaf k224: the build workspace, the export archives and
the two goldens.
