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
  - `.grove/_BRIEF.md` and the brief of `app-release-k21` (`_app-release.md`): they say that Windows hosts keep the direct path and that the Windows harness runs on the candidate.
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

## Decisions (running log)

- **The candidate `v2.4.0` is void.** This leaf changes build inputs (`scripts/release-build.sh`, `scripts/release-doctor.sh`, `scripts/release_candidate.py`), and the node brief says that a change to scripts after the candidate commit voids the candidate. `candidate-rebuild-k53` runs after this leaf and rebuilds it. The design leaf `app-all-guest-traffic-k42` voids the same candidate, so that one rebuild covers both. Nothing was built or published here.
- **The Windows host release is removed from the build, not hidden.** Deleted: `WINDOWS_TARGETS`, `WINDOWS_DLLS`, `build_cli_cross_windows`, `stage_ffmpeg_dlls`, `windows_bindgen_target`, the Windows branch of `package_bundle` and `assemble_bundle`, the two Windows sysroot cases, and the header text. Kept: `build_windows_agent` and the agent in the shared payload, because Windows VMs stay as guests.
- **The doctor drops the `zip` check.** The only user of `zip` in the pipeline was the Windows archive. The app zip is made by `ditto` in `notarize-app.sh`. The doctor also checks only the two Linux Rust targets and their sysroots now.
- **The guard lives in `release_candidate.py`.** `write_manifest` and `verify_candidate` both refuse a file named `testanyware-v*-pc-windows-*`. One rule then covers the build (a candidate is never written with one) and the publish (a candidate is never verified with one). The pipeline test shows `release-publish.sh` pushes nothing for such a candidate. No other test seam was needed.
- **The `targets:` line of the build was split across lines.** `IFS` is a newline in the script, so `${LINUX_TARGETS[*]}` joined with newlines. I fixed the line while I edited it, because the new test reads it. This is the only change outside the Windows removal.
- **The Windows harness stays as source.** `windows-host-harness.rs` is untouched, as the leaf says. Its `windows_dist_zip_smoke` now tests an archive that no script builds. ADR-0009 says so, and says that no release runs the harness. I did not edit the comments inside the Rust file.
- **Documents.** ADR-0009 has a new status line, a statement in the Decision, a rejected option, a three-triple matrix and the correction of the Windows-first-class sentence. ADR-0011, ADR-0005, `docs/architecture/video-codec-licensing.md` and the glossary entry for the verification harness lost their claim that a Windows zip ships. The root brief and the brief of `app-release-k21` state that no Windows host binary ships and keep the x86_64 Linux limit. The spec already said it, so I left it.
- **Left alone on purpose.** `docs/research/*` and `docs/superpowers/*` are dated records of earlier work. The OCR engine facts in ADR-0011 and `testanyware-ocr-client/Cargo.toml` (it cross-builds for `aarch64-pc-windows-gnullvm`) are about source that still builds. The `.grove/` task files are history.
- **Verification of "no Windows host archive remains".** I extracted every hit of `pc-windows|gnullvm|WINDOWS_TARGETS|WINDOWS_DLLS|cross_windows|ffmpeg_dlls|windows_bindgen|aarch64-windows|x86_64-windows|taw-dist|windows_dist_zip|…\.zip` over the whole tree, dotdirs included, and classified each one. The build scripts, the Taskfile and the templates have none. A positive control found 149 hits in a copy of the previous commit. The new tests also failed against the previous scripts: four failures in the pipeline test and two in the unit tests. They pass on the new ones. The `taw-dist` hits in `linux-host-harness.rs` are the Linux prefix, a false match.
- **Not run:** the cross builds, the notarisation and the publication. They need the toolchains and the owner. The pipeline test and the unit tests run without them, and they passed.
