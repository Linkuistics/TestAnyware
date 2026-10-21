# app-all-guest-traffic-increments-k45

## Goal

Cut the increments of the design `app-all-guest-traffic-k42` as `impl` leaves, small and in order. The first increment unblocks `non-macos-runtime-check-k40`. Cut also the leaf that rebuilds the release candidate.

## Context

- The design: `docs/specs/testanyware-app.md`, sections "Guest traffic on a macOS host" and "How the host harnesses reach their guests". ADR-0004, ADR-0009 and ADR-0019, as `app-all-guest-traffic-k42` reworked them.
- The decision log of `app-all-guest-traffic-k42`. It holds the enumeration of the direct dials, the rejected options and the evidence.
- The findings of `app-all-guest-traffic-k43`, the review of the design. `app-all-guest-traffic-k46` integrated them, and the notes below include its corrections. Its decision log holds each disposition and the owner's two answers of 2026-10-21.
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
   - The HUT must be a tart guest. A same-named qcow2 makes `vm start` choose QEMU (`wants_tart` in `testanyware-vm/src/lifecycle.rs`). After the start, the harness reads the HUT's `backend` from `vm list --json`. When it is not `tart`, the harness stops the HUT, opens no forward and fails. Its message names the qcow2. Do not change how `vm start` chooses a backend.
   - The provisioning channel is the In-VM agent, as in the Windows harness. Its agent client takes its connections from the app, by the HUT's id.
   - The forward opens a guest stream by the golden's id for each accepted connection. It binds the HUT's gateway address, and it accepts only the HUT's own address, the `src` of `ip route show default`.
   - The harness reads the golden's spec only for the VNC password.
   - The shared layer is `testanyware_cli::app_route`. `agent_client` exists. The harness needs a public function that opens a guest stream for a target and a kind. It also needs an agent client with a timeout that the caller sets, for long uploads and `exec` calls. Do not change a flag, an exit code or a `--json` field.
   - Before staging, a check reads the libraries that the binary and the bundled libraries need. It fails on any library that is neither bundled nor in the C runtime (`libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1`, `ld-linux-aarch64.so.1`). The candidate `v2.4.0` passed this check by `objdump -p`.
   - A second check runs after the harness provisions the OCR venv in the HUT. It reads the libraries that each native library of the venv needs at load time. It fails on any library that resolves outside the venv and is neither in the C runtime nor `libstdc++.so.6`. The stock server image proved this boundary for the OCR band, and the desktop golden cannot prove it by a run.
   - The address choice, the peer check, the backend check and the two library checks are pure functions with unit tests.
   - `linux_dist_install_layout` uses the same HUT, and it changes in the same way.
   - Texts to correct: the module docs of `linux-host-harness.rs`, and `vision/stages/text-ocr/README.md` on the HUT and its channel. The description of the Taskfile task `release:linux-harness` must say that the task needs the Linux golden.
   - Demonstration: the Linux harness passes from an agent session that has no Local Network access of its own. A person allowed the development bundle of the app. The binary is a workspace build for aarch64 Linux.
2. **`remaining-direct-dials`: the last direct dials outside golden creation go through the app.**
   - The Windows host harness: its agent channel takes its connections from the app by the HUT's id. Its forward listens on `127.0.0.1` and opens guest streams by the golden's id. During a run, another local user can reach the forward. The owner accepted that as a bounded exception, and the spec states its limits. Do not add a peer check for it. No release runs this harness. The demonstration is the build and the unit tests, and a live run when a person builds a Windows host binary by hand.
   - `scripts/live-start-checks.sh` (the qemu-start check) and `scripts/windows-healthy-start.sh` stop their `curl` to a QEMU agent on `localhost`. Each still checks that the start reported an agent endpoint. Then it asks `testanyware agent health --vm <id> --json`, and the app dials the endpoint that the VM's spec records. The owner's rule is that TestAnyware's own tests and scripts name a VM only by its id. So neither script passes `--agent`. The check keeps its meaning: the agent that the start reported answers.
   - The `nc -z` controls of `herdr-acceptance-live.sh` and `app-routing-live.sh` stay. They carry no traffic.
   - Text to correct: the module docs of `windows-host-harness.rs`.
   - Done when an enumeration of the dials finds no direct dial on a macOS host outside golden creation and its live tests. Classify the dials as the design's log does.
3. **The candidate rebuild, in `app-release-k21`.** The candidate `v2.4.0` was never published. Move the unpushed local tag `v2.4.0` to the new commit, as `app-release-bundle-k22` already did once, and build the candidate again with `task release:candidate`. Then `non-macos-runtime-check-k40`, `upgrade-identity-check-k36` and `release-publish-k37` run on it.

The owner asked for focus: small increments and no rabbit holes. The design changes no route of the app and no command of the Host CLI.

## Decisions (running log)

- **Increment 1 is a node of four leaves, not one leaf.** The Linux harness file has 1,931 lines and three separable concerns. A leaf that did all three would not fit one session. Each leaf leaves the harness green when a person runs it from Terminal, and each demonstrates one new fact. (1) `harness-golden-hut`: the HUT is the Linux golden and the agent channel provisions it. It carries the new `app_route` functions, because nothing else calls them. It also carries the backend check and the binary library check, because the desktop golden makes both necessary. (2) `harness-ocr-library-check`: the OCR venv library check, apart from the forward. (3) `harness-golden-forward`: the forward opens guest streams. After it, `linux_host_harness` passes from an agent session. This is the demonstration of the increment. (4) `dist-layout-golden-hut`: the install-layout test moves too.
- **The unblock does not wait for leaf 4.** `task release:linux-harness` filters on `linux_host_harness`, and that name does not match `linux_dist_install_layout`. Leaf 4 stays in the increment because the design put it there, and it runs last.
- **The new `app_route` functions stay in leaf 1.** They are a public surface with no caller until the harness uses them. A leaf of their own would be a horizontal layer with nothing to demonstrate.
- **Increment 2 is one leaf.** `remaining-direct-dials` holds the Windows harness, two scripts and the enumeration. It has no seam that earns a split. The leaf may decompose itself if the Windows harness proves large.
- **The rebuild leaf is `candidate-rebuild` in `app-release-k21`.** It sits after `release-without-windows-host-k44` and before `non-macos-runtime-check-k40`. The two increments sit at the grove root, before `app-release-k21`, so `pick` reaches them first. The leaf keeps the tag `v2.4.0` and moves it, as the design log decided.
- **`non-macos-runtime-check-k40` now names the new candidate.** I edited its Goal and Context, and I appended a log entry that voids the commit `8a7ad3f3` and the hashes. I changed one phrase in `upgrade-identity-check-k36` the same way. I updated the decomposition of the node brief of `app-release-k21` and the root brief.
- **No review chain.** The increments carry the design that `app-all-guest-traffic-k43` already reviewed. This session cut no `review-planning` leaf.
- **Seen, and left alone.** The test seam of the new `app_route` functions is the existing protocol seam: the app in test mode with a fake In-VM agent. Leaf 1 decides whether it needs a new test there.
