# vm-broker-increments-k39

**Integrates:** vm-broker-increments-k38

## Goal

Triage the five findings of `vm-broker-increments-k38` and correct the planned
subtree where they hold, before the first implementation increment runs.

## Context

- Read the committed review record of `vm-broker-increments-k38`. Its findings
  cite the producer commit `15985c4de06b9d3611c57ec7452a8b7dac587d1c` and paths
  before this integration was inserted. Resolve the stable handles for current
  paths; the insertion renumbered sibling entries without changing their bodies.
- The producer is `vm-broker-increments-k4`. The artifact is the root brief and
  the planned subtree from `agent-client-hyper-k7` to `release-publish-k37`.
- The agreed contract is `docs/specs/testanyware-app.md`, ADRs 0004, 0019,
  0020 and 0021, and ADR-0009 for non-macOS runtime verification.
- Findings concern safe legacy QEMU stop before signal guards, configuration
  activation across both processes, the invalid-config doctor path, preparation
  of release candidates before tag approval, and non-macOS runtime evidence.

## Done when

- Every finding has an evidenced disposition: apply, reject, or record an
  explicit accepted trade-off. Proposed corrections in the review are inputs
  to triage, not new requirements.
- Each revised leaf has a complete behaviour and an assigned check that can
  pass without waiting on a later sibling. The early herdr unblock stays early.
- Release preparation, acceptance checks, owner approval and publication have
  an executable dependency order, with candidate identity preserved.
- The root brief and affected node briefs agree with the revised ordering and
  ownership. The spec's requirements and existing compatibility obligations
  have assigned implementation and verification work.

## Notes

This is planning integration. Change task boundaries and their briefs, not
production or test code. The owner asked for small increments and no rabbit
holes. The review ran no executable checks, and its graph-index limitation is
recorded in its inspection limits.


## Decisions (running log)

**All five findings are real issues, and this session applies each one.**

- I read the findings from the commit of `vm-broker-increments-k38`.
- I checked each claim against the source and the spec, not against the review text.
- No finding is noise. One trade-off is accepted, under F5.

**F1 is applied. The safe stop of an older QEMU guest now lands before the signal guard.**

- Evidence: `lifecycle.rs:519` calls `QemuRunner::stop`. `qemu.rs:437` signals QEMU and swtpm, and it deletes the clone and session directories.
- The stop of `app-vm-stop-delete-k28` uses this path. So the guard of `app-adoption-k30` would leave an older guest with an unsafe stop or with none.
- Change: `qemu-legacy-stop-k31` and `app-adoption-k30` swap positions. The legacy stop needs only the absence of identity fields from `vm-process-identity-k15`. It does not need the guard.
- `qemu-legacy-stop-k31` routes an older QEMU meta away from the shared teardown. It leaves the path of a meta with identity unchanged.
- `app-adoption-k30` refuses a mismatch and deletes nothing. It tests that an older QEMU meta reaches the legacy stop.
- The node brief gains an ordering rule.

**F2 is applied with an expand step. A macOS host ignores the file until the app reads it.**

- Evidence: `config-evaluator-cli-k18` made the CLI honour the file on every host. The app evaluates it only in `config-in-app-k33`.
- The spec section "Golden creation keeps its direct path" closes the same gap on the creation side.
- Change: k18 adds one activation switch. It is on for Linux and Windows hosts and off for a macOS host.
- `golden-directory-check-k34` turns the switch on for macOS, at the same step as the golden check. It adds harness tests with the real CLI and the app, with a non-default `state_dir` and `data_dir`.
- The slug of k34 stays, because a handle does not change. Its goal states the wider scope.
- Rejected alternative: more leaves. The switch gives the same safety with no new leaf.

**F3 is applied. The `doctor` exception moves into `config-evaluator-cli-k18`.**

- Evidence: the done-when of k18 said every command except `doctor` refuses. Its note gave the exception to k20.
- The spec requirement "doctor reports the failures that it diagnoses" needs the `config` check as soon as a file can block commands.
- Change: k18 owns the `config` check, the fallback values, the exit 1 and the schema entry.
- `doctor-app-check-k20` keeps the `app` check. It tests that the `app` check still reports the running app when the file fails.
- The tests of k18 are pure-function tests, because the macOS switch is off on this Mac. The real-binary test of a failing file lands in k34.

**F4 is applied. A release candidate is a build of a local tag, with a manifest.**

- Evidence: `release-build.sh:130` requires a clean, exactly tagged tree. `main` derives the version from the nearest tag.
- `release-publish.sh` reads the version with `git describe --abbrev=0`. `gh release create` makes a missing tag from the default branch, not from the built commit.
- The build deletes `target/dist` on each run, so a second candidate would delete the first.
- Change: `app-release-bundle-k22` owns candidate preparation. It has an explicit tag input, a directory for each candidate, and a manifest of the tag, the commit and the artifact hashes. Its publish script pushes the existing tag.
- `upgrade-identity-check-k36` uses the candidate as N+1 and a throwaway N, and it deletes the throwaway.
- `release-publish-k37` checks the identity and asks the owner with the tag, the commit and the hashes. It publishes only the recorded candidate.
- The node brief gains a "Candidate identity" rule. A change to code or build inputs after the candidate commit voids the candidate. A change to `.grove/` or to documents does not.

**F5 is applied with one new leaf. One trade-off is accepted.**

- Evidence: both harnesses carry `#[ignore]`. `task cli:test` runs `cargo test --workspace`, so neither runs.
- ADR-0009 says to never ship a binary that the harness has not run green.
- The harnesses read the binary from `TESTANYWARE_LINUX_BIN` and `TESTANYWARE_WINDOWS_BIN`. So the candidate can be tested directly, and no seam is missing.
- Change: the new leaf `non-macos-runtime-check-k40` sits in `app-release`, after `app-release-bundle-k22` and before the upgrade check.
- It runs both harnesses and the Windows dist zip smoke test on the candidate. It records the x86_64 build-only limit.
- A failing harness cuts a leaf for the fix and voids the candidate. The check runs before the upgrade check, so a fix wastes the least work.
- Accepted trade-off: the harness does not run early. A regression from `agent-client-hyper-k7` or from k18 on Linux or Windows shows at release.
- I rejected a harness run in k7. It would delay the early herdr unblock, and the owner asked for that unblock first. An implementer may still run the harness at any time.

**The checks for the done-when of this leaf.**

- Each revised leaf has an assigned check that needs no later sibling.
- The early herdr unblock is unchanged. `agent-client-hyper-k7` to `herdr-acceptance-run-k29` are not touched.
- Each spec requirement heading still has an owner. The new leaf owns the non-macOS direct path.
- The sizing doubts of the review about k27 and k22 stay with their implementers. k22 grew by the candidate work, and it is the leaf most likely to decompose.
- No in-session reviewer ran. No code, test or build ran, because this session changes task files only.
- The checker `ste-lint.py` reports no hard finding in the files that I changed, except the two lines of the original task text above.
