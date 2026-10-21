# app-all-guest-traffic-increments-k45

## Goal

Cut the increments of the design `app-all-guest-traffic-k42` as `impl` leaves, small and in order. The first increment unblocks `non-macos-runtime-check-k40`. Cut also the leaf that rebuilds the release candidate.

## Context

- The design: `docs/specs/testanyware-app.md`, sections "Guest traffic on a macOS host" and "How the host harnesses reach their guests". ADR-0004, ADR-0009 and ADR-0019, as `app-all-guest-traffic-k42` reworked them.
- The decision log of `app-all-guest-traffic-k42`. It holds the enumeration of the direct dials, the rejected options and the evidence.
- The findings of `app-all-guest-traffic-k43`, the review of the design. When an integration of that review exists, it runs before this leaf.
- The node `app-release-k21` and its brief, in particular "Candidate identity". A change to code, scripts or build inputs after the candidate commit voids the candidate.
- `release-without-windows-host-k44`, which also changes build inputs before the candidate is rebuilt.

## Done when

- One `impl` leaf exists for each increment below, in this order. Each one names what it demonstrates on its own.
- A leaf that rebuilds the candidate exists in `app-release-k21`. It runs after `release-without-windows-host-k44` and after the increments, and before `non-macos-runtime-check-k40`.
- `non-macos-runtime-check-k40` names the new candidate as the one that it runs on.

## Notes

The increments that the design found, in order:

1. **`harness-guest-streams`: the Linux host harness reaches every guest through the app.** It unblocks `non-macos-runtime-check-k40`.
   - The HUT is the Linux golden, from `vm start --platform linux --json` through the Host CLI. The tart clone of the stock image goes, and so do `provision_ssh` and the SSH channel.
   - The provisioning channel is the In-VM agent, as in the Windows harness. Its agent client takes its connections from the app, by the HUT's id.
   - The forward opens a guest stream by the golden's id for each accepted connection. It binds the HUT's gateway address, and it accepts only the HUT's own address, the `src` of `ip route show default`.
   - The harness reads the golden's spec only for the VNC password.
   - The shared layer is `testanyware_cli::app_route`. `agent_client` exists. The harness needs a public function that opens a guest stream for a target and a kind. It also needs an agent client with a timeout that the caller sets, for long uploads and `exec` calls. Do not change a flag, an exit code or a `--json` field.
   - Before staging, a check reads the libraries that the binary and the bundled libraries need. It fails on any library that is neither bundled nor in the C runtime (`libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1`, `ld-linux-aarch64.so.1`). The candidate `v2.4.0` passed this check by `objdump -p`.
   - The address choice, the peer check and the library check are pure functions with unit tests.
   - `linux_dist_install_layout` uses the same HUT, and it changes in the same way.
   - Texts to correct: the module docs of `linux-host-harness.rs`, and `vision/stages/text-ocr/README.md` on the HUT and its channel. The description of the Taskfile task `release:linux-harness` must say that the task needs the Linux golden.
   - Demonstration: the Linux harness passes from an agent session that has no Local Network access of its own. A person allowed the development bundle of the app. The binary is a workspace build for aarch64 Linux.
2. **`remaining-direct-dials`: the last direct dials outside golden creation go through the app.**
   - The Windows host harness: its agent channel takes its connections from the app by the HUT's id. Its forward listens on `127.0.0.1` and opens guest streams by the golden's id. No release runs this harness. The demonstration is the build and the unit tests, and a live run when a person builds a Windows host binary by hand.
   - `scripts/live-start-checks.sh` (the qemu-start check) and `scripts/windows-healthy-start.sh` stop their `curl` to a QEMU agent on `localhost`. Each asks `testanyware agent health --agent localhost:<port> --json`, which the app relays on a macOS host. The check keeps its meaning: the endpoint that the start named answers.
   - The `nc -z` controls of `herdr-acceptance-live.sh` and `app-routing-live.sh` stay. They carry no traffic.
   - Text to correct: the module docs of `windows-host-harness.rs`.
   - Done when an enumeration of the dials finds no direct dial on a macOS host outside golden creation and its live tests. Classify the dials as the design's log does.
3. **The candidate rebuild, in `app-release-k21`.** The candidate `v2.4.0` was never published. Move the unpushed local tag `v2.4.0` to the new commit, as `app-release-bundle-k22` already did once, and build the candidate again with `task release:candidate`. Then `non-macos-runtime-check-k40`, `upgrade-identity-check-k36` and `release-publish-k37` run on it.

The owner asked for focus: small increments and no rabbit holes. The design changes no route of the app and no command of the Host CLI.
