# release-without-windows-host-k44

## Goal

Stop shipping the Windows host binary. The release then carries the macOS and Linux Host CLI, the Windows in-VM agent and the cask. This leaf changes build inputs, so it runs before the candidate is rebuilt.

## Context

- The owner said: "let's stop shipping the windows host binary". The reasons the owner gave: driving TestAnyware from a Windows host is future work, and it will use WSL. Windows VMs stay needed as guests.
- What stays: the Windows in-VM agent (`testanyware-agent.exe`, built by `build_windows_agent` in `scripts/release-build.sh`), the Windows goldens, and `vm create-golden --platform windows` on a macOS host.
- What goes: the two Windows host targets `aarch64-pc-windows-gnullvm` and `x86_64-pc-windows-gnu`, and their `.zip` archives.
- Where the Windows host release appears, found by a search of the repo:
  - `scripts/release-build.sh`: `WINDOWS_TARGETS`, `WINDOWS_DLLS`, `build_cli_cross_windows`, the packaging of a `.zip`, and the header comments.
  - `scripts/release-doctor.sh`: `WINDOWS_RUST_TARGETS`, the sysroot case, and the `zip` check.
  - `scripts/tests/release-pipeline-test.sh`: the fixture and the asset list.
  - `scripts/release_candidate.py` and `scripts/templates/`: check whether the manifest or the formula names a Windows archive.
  - `Taskfile.yml`: `release:windows-harness` is already removed by `non-macos-runtime-check-k40`. Check for any other Windows host task or text.
  - `docs/adr/0009-self-hosted-host-cli-verification-harness.md`: it says Windows ships and that the Windows harness leads to distribution. It needs a correction and a new status line.
  - `.grove/_BRIEF.md` and `.grove/20-k21/_app-release.md`: they say that Windows hosts keep the direct path and that the Windows harness runs on the candidate.
  - The README and the other documents: search them as well. Use the enumerate-then-classify method of `references/execute.md`, not a list of patterns.
- `cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs` stays as source. The owner did not ask to remove it, and the code that it tests still builds on a Windows target. State in ADR-0009 that no release runs it.

## Done when

- `scripts/release-build.sh` builds no Windows host target and packages no Windows `.zip`. It still builds and ships the Windows in-VM agent in the shared agents.
- `release-doctor.sh` checks no Windows host prerequisite.
- The pipeline test passes without any Windows archive. It fails if a Windows host archive appears in a candidate.
- The candidate manifest and the formula name no Windows host archive.
- ADR-0009 and the two briefs state that no Windows host binary ships. They keep the x86_64 Linux limit.
- A claim such as "no Windows host archive remains" is verified by the controls of `references/execute.md`.
- The decision log says that the candidate `v2.4.0` is void, as the node brief says, and that a later leaf rebuilds it.

## Notes

- Do not touch the Windows in-VM agent, the Windows goldens or the Windows provisioning. Windows VMs stay.
- The design leaf `app-all-guest-traffic-k42` also voids the candidate. One rebuild covers both. The design must name the leaf that rebuilds the candidate, and that leaf runs after this one.
- The owner asked for focus. Remove the Windows host release and its text, and nothing else.
