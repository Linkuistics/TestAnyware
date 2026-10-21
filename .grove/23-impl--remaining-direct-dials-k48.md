# remaining-direct-dials-k48

## Goal

The last direct dials of a guest on a macOS host, outside golden creation, go through the app. After this leaf, an enumeration of the dials finds only golden creation and its live tests.

## Context

- The design: `docs/specs/testanyware-app.md`, sections "Guest traffic on a macOS host" and "How the host harnesses reach their guests".
- The decision log of `app-all-guest-traffic-k42` holds the enumeration of the direct dials and their classes. The decision log of `app-all-guest-traffic-k46` holds the owner's two answers of 2026-10-21.
- `harness-guest-streams-k47` built the pieces for the Linux harness: the public `app_route` function that opens a guest stream, and the agent client with a caller timeout. Reuse them.

## Done when

- The Windows host harness (`cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs`) takes its agent connections from the app, by the HUT's id. Its forward listens on `127.0.0.1` and opens guest streams by the golden's id.
- During a manual run, another local user can reach that forward. The owner accepted this as a bounded exception, and the spec states its limits. Do not add a peer check for it.
- No release runs this harness. The demonstration is the build and the unit tests. A live run is a bonus when a person builds a Windows host binary by hand. The decision log says which one happened.
- `scripts/live-start-checks.sh` (the qemu-start check) and `scripts/windows-healthy-start.sh` stop their `curl` to a QEMU agent on `localhost`. Each still checks that the start reported an agent endpoint. Then it runs `testanyware agent health --vm <id> --json`. The app dials the endpoint that the spec of the VM records. Neither script passes `--agent`, because the owner's rule is that TestAnyware's own tests and scripts name a VM only by its id. The check keeps its meaning: the agent that the start reported answers.
- The `nc -z` controls of `herdr-acceptance-live.sh` and `app-routing-live.sh` stay. They carry no traffic.
- The module docs of `windows-host-harness.rs` describe the channel and the forward as they now are.
- An enumeration of the dials finds no direct dial on a macOS host outside golden creation and its live tests. Classify the dials as the design's log does. Enumerate every dial first and classify each one. Do not sweep a list of patterns. Check the instrument with the controls of `references/execute.md` in the grove skill, and record them.

## Notes

- Run the two scripts if the session can. They need a QEMU guest and the app. If it cannot run them, the decision log says which one did not run and why.
- Do not change `Taskfile.yml` task text that is not guest traffic. The description of `app:routing-live` is outside this leaf.
- This leaf changes code and scripts after the candidate commit, so it voids the candidate `v2.4.0`. `candidate-rebuild` in `app-release-k21` rebuilds it.
- The owner asked for focus: small increments and no rabbit holes.
