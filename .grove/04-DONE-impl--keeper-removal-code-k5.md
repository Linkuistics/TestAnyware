# keeper-removal-code-k5

## Goal

Remove the VNC keeper and the app's VNC probe gate, as ADR-0018 decides. Remove
them from the code, the tests, the live scripts and the Taskfile. Update the two
specs that describe that behaviour in the same change.

## Context

- ADR-0018 is the decision:
  `docs/adr/0018-a-macos-host-runs-the-latest-macos-27-and-tart.md`. The
  running log of `keeper-removal-k2` holds the measurements behind it.
- No backward compatibility. No code reads or migrates the keeper state of
  2.9.0. A meta that still holds `keeper_pid` loads, because no struct refuses
  unknown fields. Add no code for either case.
- The sites in `cli-rs/crates/testanyware-vm`:
  - Remove `keeper.rs` and its entry in `lib.rs`.
  - In `lifecycle.rs`, the tart boot observer starts no keeper. `TartBoot`, the
    start track, the meta write, the three stop paths and the cleanup of a
    cancelled start lose their keeper parts.
  - `meta.rs` loses `keeper_pid`, `keeper_identity` and their tests.
    `start_record.rs` loses `keeper`.
  - `process.rs` loses `command_line`, because its only callers check a keeper.
  - The comments in `identity.rs` and `adoption.rs` lose the keeper. The test
    in `legacy_stop.rs` loses its keeper case.
- The sites in `cli-rs/crates/testanyware-app`:
  - `main.rs` loses the `_vnc-keeper` subcommand and `run_keeper`.
  - `adoption.rs` loses `vnc_probe_allowed` and `keeper_holds_vnc`. The app
    probes the VNC banner of each adopted guest that has a VNC endpoint. The
    banner probe keeps its full read, and its comment loses the ADR-0018 reason.
  - `unfinished.rs` loses `Facts.keeper`, the `signal_keeper` flag, the keeper
    argument of `Effects::stop_and_delete` and the keeper tests.
- In `testanyware-app-protocol`, `ProbeAnswer::NotProbed` stays for a guest
  with no endpoint. Its doc says only that.
- In `testanyware-cli`, `main.rs` loses `Command::VncKeeper`, its arm and its
  exemption from `refuse_if_invalid`. `commands/vm.rs` loses `run_vnc_keeper`.
- The tests in `testanyware-cli/tests` and `testanyware-app/tests`:
  - `cli-contract.rs` replaces its three keeper tests with one. `_vnc-keeper`
    exits 2 as an unrecognised subcommand.
  - `app-vm-start-routing.rs` keeps the socket check for `tart run`. The check
    and the identity loop of the meta test lose the keeper.
  - `app-vm-queue.rs` waits until the record names the `tart run` with its
    identity, not the keeper. It no longer waits for the keeper to end.
  - `adoption.rs` turns "a tart guest without a keeper gets no VNC connection"
    into a test that a tart guest gets its banner probe.
  - `app-vm-stop-delete-routing.rs` drops `keeper_pid` from its fixture.
- The specs. In `docs/specs/testanyware-app.md`, change the component view,
  the start sequence, the process table, and the rules of lifecycle and
  adoption. In `docs/specs/admission-queue.md`, change the boot cleanup row,
  the notes on the decision table and the scenarios.
- The live scripts:
  - `scripts/vnc-keeper-live.sh` becomes `scripts/vnc-idle-live.sh`, and
    `task vm:vnc-keeper-live` becomes `task vm:vnc-idle-live`.
  - `scripts/app-adoption-live.sh` and `scripts/upgrade-identity-live.sh` lose
    their keeper lines.
  - The `SCRIPTS` list of `task scripts:check` follows the rename.

## Done when

- The existing suites pass without their keeper cases: `task cli:test`,
  `task app:test` and `task app:routing-test`. No suite gains a compatibility
  case.
- `cli-contract.rs` shows that the command surface holds no `_vnc-keeper`.
- `task vm:vnc-idle-live` passes on the development bundle:
  - A macOS guest that `vm start` starts survives two `screen size` commands
    5 s apart. It then survives 5 minutes with no VNC client.
  - A Linux guest survives 70 s with no VNC client.
  - A `screen size` then works on each guest, and no `_vnc-keeper` process
    exists.
- `task vm:tart-vnc-idle-repro`, unchanged, passes without `HOLD=1`.
- `task scripts:check` passes.
- A sweep of `keeper` over `cli-rs/`, `scripts/`, `Taskfile.yml` and
  `docs/specs/` finds only two kinds of hit. One kind asserts that no keeper
  exists, and the other is the word "Gatekeeper". The sweep has a positive
  control, and it skips `experiments/exec-reliability/evidence/`.

## Notes

- `vm start` runs in the app, and the app spawned the keeper. So run each live
  check against the development bundle, not the installed 2.9.0 app. Follow
  `scripts/input-live.sh`: take the bundle as an argument, and refuse an app on
  the socket that is not that bundle.
- The two `screen size` commands 5 s apart test the reset trigger through the
  app. The app resets each guest connection when its client leaves. On 27.0.1 a
  reset crashed tart at the next connection, and the keeper hid that.
- On 27.0.1 the first VNC connection after a boot once saw a 1280x720 frame.
  From 2.2.0 on, the keeper was always that first client. A fresh clone on
  27.2.1 served 1024x768 on its first connection. Watch the first
  `screen size` of the live check.
- `confirm_input_read`, `task input:live` and `task app:routing-test` stay as
  they are. The fault of issue #12 is still present on the latest host.
- In the specs, rewrite the text that states current behaviour. Leave the
  record of a past measurement as it was measured.

## Decisions (running log)

- 2026-11-21, code. The keeper, its meta and record fields, `command_line`, the
  hidden commands of the CLI and the app, and the probe gate are gone. The
  survivor of an adoption no longer needs its meta or a process probe, so
  `survivor` takes the id, the legacy flag and the paths only.
- 2026-11-21, tests. `cli-contract.rs` has one test for `_vnc-keeper`, which
  exits 2 as an unrecognised subcommand. The unit test of the app's VNC gate
  and the two unit tests of a keeper verdict are gone. The run-level test of a
  matching keeper beside a gone `tart run` now tests the second row alone,
  where `tart delete` runs by name with no signal. The record test names a swtpm in place of the
  keeper. The queue tests wait for a record that names the `tart run` with its
  identity. The replaced record of `retry-c` names no `tart run`, because its
  clone step waits for tart.
- 2026-11-21, live script. `scripts/vnc-idle-live.sh` takes the bundle as its
  argument, as `input-live.sh` does. It has no negative control, because the
  host has no fault to show. Its instrument for a `_vnc-keeper` process first
  finds a decoy with that command line, so a clean read is evidence.
- 2026-11-21, specs. The two specs had five hard findings of the
  controlled-English checker. A preparatory change fixed them. The upgrade
  samples of 2.4.0 and 2.5.0 in the app spec now say that the guest kept each
  of its processes. That is still what the run measured, and the keeper sweep
  then finds no past-tense hit.
- 2026-11-21, sweep. `grep -rniI keeper` over `cli-rs`, `scripts`,
  `Taskfile.yml` and `docs/specs`, without `target` and `evidence`, finds only
  hits that assert no keeper exists, and the word "Gatekeeper". The same
  command finds 101 hits in a scratch copy of the old `keeper.rs`, and 18 in
  ADR-0018 and the glossary. Without the filter, it finds the Gatekeeper hits.
- 2026-11-21, suites. `task cli:test` passed 1716 tests with none failed.
  `task app:test` passed 487, and `task app:routing-test` passed 158.
  `task scripts:check` passed. `task vm:tart-vnc-idle-repro` passed without
  `HOLD=1`. Both of its connections saw 1024x768, and tart did not crash.
- 2026-11-22, live run. The first run of `task vm:vnc-idle-live` failed at
  the agent wait with `No route to host`. The development bundle had no Local
  Network grant. The owner allowed it, and the second run passed all three
  checks. No `_vnc-keeper` process existed. The installed 2.9.0 app was quit
  for the run. A failed start now prints its output.
- 2026-11-22, first frame. The macOS guest of the live run switched to
  1920x1080, but its first `screen size` reported 1280x720. A second guest
  reported 1920x1080 at every read. The keeper had always been the first
  client, so its removal can expose this stale size. The finding needs its
  own measurement and decision. So `first-frame-size-k9`, a design leaf, now
  sits ahead of the release leaf. The live script prints the second size too.
- 2026-11-22, final live run. A third run used the final script and
  Taskfile, with digests checked before and after. It passed all three
  checks. Its first `screen size` reported 1280x720, and its second
  1920x1080.
- 2026-11-22, review. This session cut no `review-impl` leaf. The change
  removes code, and the suites and the live seams test each removed path.
  No doubt remains that needs an independent read.
