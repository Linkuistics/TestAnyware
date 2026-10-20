# non-macos-runtime-check-k40

## Goal

Run the Linux and Windows runtime harnesses on the release candidate. The agent client and the configuration changed on every host, so this check proves that the direct path still works.

## Context

- The root brief: Linux and Windows hosts keep the direct path of today.
- ADR-0009, Decision and Consequences. Never ship a binary that the harness has not run green. The aarch64 builds get the full runtime check. The x86_64 builds are build-only, by an accepted limit.
- `app-release-bundle-k22`, which builds the candidate and its manifest.
- `agent-client-hyper-k7` and `config-evaluator-cli-k18`, which changed the code that this leaf proves.
- The harnesses `cli-rs/crates/testanyware-cli/tests/linux-host-harness.rs` and `windows-host-harness.rs`. Both tests are ignored tests, so `task cli:test` does not run them.

## Done when

- A Taskfile task runs the Linux host harness on the aarch64 Linux binary of the candidate archive. It sets `TESTANYWARE_LINUX_HARNESS=1` and `TESTANYWARE_LINUX_BIN`.
- A Taskfile task runs the Windows host harness on the aarch64 Windows binary of the candidate. It sets `TESTANYWARE_WINDOWS_HARNESS=1` and `TESTANYWARE_WINDOWS_BIN`. It also runs the dist zip smoke test with `TESTANYWARE_WINDOWS_DIST_ZIP`.
- Both harnesses pass, and the decision log records the candidate tag, the commit and the binary hashes that they ran.
- The decision log states the x86_64 limit: the x86_64 Linux and Windows builds are build-only, as ADR-0009 accepts.
- When a harness cannot run because a golden or a tool is missing, the leaf stops and names what is missing.
- When a harness fails, the leaf does not change the code. It records the failure and cuts a leaf for the fix. The candidate is then void, as the node brief says.

## Notes

- The check changes no production code and no test code. The tasks only wrap the existing harnesses.
- The check runs on the candidate before the upgrade check. A fix that voids the candidate then costs the least.
- Regressions in the direct path from the early leaves surface here. The owner accepted this cost in order to keep the early herdr unblock small.
