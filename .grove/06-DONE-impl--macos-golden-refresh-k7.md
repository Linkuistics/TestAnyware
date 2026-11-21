# macos-golden-refresh-k7

## Goal

Make `vm create-golden --platform macos` install the latest release of the
major, as ADR-0027 decides. Then refresh the macOS 27 golden of this Mac to
macOS 27.2.1. Delete the base, the IPSW and the golden that the rule
discards.

## Context

- The decision is
  `docs/adr/0027-each-golden-holds-the-latest-release-of-its-major.md`. The
  running log of `golden-refresh-k3` holds the measurements of 2026-11-21.
- On 2026-11-21, Apple's catalogue listed three builds of 27.2.1. Only
  26B131 lists `VirtualMac2,1`. Today's `select_ipsw(27)` returns 26B8132,
  because `max_by_key` returns the last of equal maxima.
- The catalogue is a property list. The path to a release is
  `MobileDeviceSoftwareVersionsByVersion` → `1` →
  `MobileDeviceSoftwareVersions` → `VirtualMac2,1` → `<build>` → `Restore`.
  `Restore` holds `ProductVersion`, `BuildVersion` and `FirmwareURL`. The
  hardware entries also carry an `Unknown` key. `Cargo.lock` holds no
  property-list parser.
- The code: `ipsw_base.rs` reads the catalogue and builds the base.
  `base_source.rs` names the base. `GoldenPreflight::check` in `golden.rs`
  names the VMs of the hold. Boot 1 in `golden.rs` deletes a golden of the
  same name before it builds the base. `finalize.rs` makes the final clone.
  `commands/vm.rs` prints the dry-run plan, and `main.rs` holds the help
  text.
- `ToolSearch::from_process_path` finds `curl` and `tart` on `PATH`. So a
  contract test can put a fake `curl` beside a fake `tart`, and the product
  needs no new hook.
- `task vm:create-golden-macos` builds a golden with the agent of this
  checkout. Its guard `vm:golden-name-free` refuses a name that exists, to
  protect a retained golden.
- On this Mac: `testanyware-base-macos-27` holds 27.0.1, the cache holds
  `macos-27.0.1-26A434.ipsw` (27 GB), and `testanyware-golden-macos-tahoe`
  holds a macOS 26 image.

## Decisions for this leaf

- **Selection.** Read the property list structurally, by the `plist` crate
  or another parser. Take the entries under `VirtualMac2,1` whose
  `ProductVersion` has the requested major. Order them by version, then by
  build, so that a tie has one answer. If no entry matches, fail with
  `GOLDEN_CREATE_FAILED`. The message names the catalogue URL and the major.
  Never fall back to another build.
- **Base name.** `testanyware-base-macos-<version>-<build>`, for example
  `testanyware-base-macos-27.2.1-26B131`. An older base is each tart VM of
  the same major whose name starts with `testanyware-base-macos-` and is not
  the current name. Read the major from the first number after the prefix.
  The class includes `testanyware-base-macos-27` and a `-building` name of
  another release. It excludes another major, such as 28 or 270.
- **One plan.** A pure function takes the catalogue text, the names that
  `tart list` gives and the major. It returns the release, the base name,
  whether the base exists, and the older bases. `GoldenPreflight::check`
  makes the plan once, and boot 1 builds the base of that plan. So the hold
  names the same `<base>-building` that the build boots.
- **Refresh.** When the base of the plan is ready, delete each older base.
  Then delete each `macos-*.ipsw` and `macos-*.ipsw.part` file in the cache
  of another release. Keep the IPSW of the current release.
- **Replacement.** Delete the earlier golden just before the final clone
  in `finalize.rs`, not in boot 1. A failed build then keeps the earlier
  golden.
- **Dry run.** The plan reads the catalogue and `tart list`, and it fails
  with the error of the real run when either read fails. The JSON object
  gains `release` (`version`, `build`, `url`) and `golden_exists`. Its
  `base` becomes an object (`name`, `exists`, `older`). The text plan names
  the release and says whether the base exists. It names each older base
  that the run deletes, and it says when the run replaces a golden. A sweep
  of `~/Development` on 2026-11-21 found no consumer of the dry-run JSON.
- **Refresh recipe.** Add a Taskfile entry point that refreshes the golden
  of the default name with the agent of this checkout. It skips the name
  guard, because the rule replaces that golden.

## Done when

- Unit tests read a trimmed copy of the catalogue of 2026-11-21. The copy
  keeps the real structure, the `VirtualMac2,1` entry and one entry for each
  of 26B7131 and 26B8132. The plan selects 27.2.1 (26B131). A catalogue
  with no `VirtualMac2,1` entry for the major fails, and the error names the
  catalogue URL.
- Unit tests classify the older bases as the decision above describes.
- `cli-contract.rs` runs the dry run with a fake `curl` and a fake `tart` on
  `PATH`. The JSON names 27.2.1 (26B131). With `testanyware-base-macos-27`
  listed, `base.exists` is false, and `base.older` names that base. With the
  current base listed, `base.exists` is true, and `base.older` is empty. The
  text plan names the release and the older base.
- On this Mac, before the refresh, `vm create-golden --platform macos
  --dry-run` names macOS 27.2.1 (26B131) and the older base
  `testanyware-base-macos-27`. This is the golden-plan seam of the root
  brief.
- The glossary changed in `golden-refresh-k3`. These pages describe the
  new base name and the refresh: `README.md`, `docs/user/golden-images.md`,
  `docs/user/quick-start.md`, `docs/components/provisioner.md`, their
  mirrors under `website/docs/`, the schema
  `docs/reference/cli-schemas/vm-create-golden.json`, `LLM_INSTRUCTIONS.md`,
  the help text in `main.rs` and the descriptions of `vm:ipsw-base-live` and
  `vm:create-golden-macos`. The user guide states the rule for each
  platform, including the Windows limit in ADR-0027.
- The existing suites pass, and no suite gains a compatibility case. Each
  changed prose page passes the controlled-English checker, or it carries a
  marked departure.

### Handed over on 2026-11-22

`macos-golden-refresh-run-k11` owns these items, after the fault fix of
`setup-assistant-zrle-fault-k10`:

- The refresh recipe rebuilds `testanyware-golden-macos-27`. Afterwards
  `testanyware-base-macos-27.2.1-26B131` exists, and
  `testanyware-base-macos-27` and `macos-27.0.1-26A434.ipsw` are gone.
- A clone of the new golden reports `27.2.1` from `sw_vers -productVersion`.
  The live gate in `live-vm-gate.rs` passes on that golden with
  `TESTANYWARE_LIVE_VM=1`. This is the golden-build seam of the root brief.
- After the refresh, the dry run says that the base exists, and it names no
  older base.
- `testanyware-golden-macos-tahoe` is deleted.

## Notes

- A base build downloads about 25 GB. The install takes about 4 minutes,
  and Setup Assistant about 5 more. The volume had 1.6 TiB free.
- Setup Assistant of 27.2.1 can differ from that of 27.0.1. If the base
  build stops there, `task vm:setup-assistant-live` iterates on a clone of
  the installed VM with no reinstall (ADR-0008).
- Golden creation asks the TestAnyware app for a slot. Keep both macOS slots
  free, and allow the Local Network alert if it appears.
- Keep the build logs in scratch, not in the repository.
- `linux-golden-refresh-k8` changes the Linux build. The release leaf
  follows both golden leaves.

## Decisions (running log)

- 2026-11-22, parser. The read uses `plist::Value` of the `plist` crate
  1.10, with the serde feature off. Cargo locked 1.10.4, which needs Rust
  1.88. The workspace `rust-version` 1.81 binds nothing today: russh 0.61.1
  already needs 1.85. docs.rs gives the same `Value::from_reader_xml`,
  `Value::as_dictionary` and `Dictionary::get` for 1.8.0 and 1.10.4. The
  catalogue of 2026-11-22 still lists 27.2.1 as on 2026-11-21.
- 2026-11-22, placement. The catalogue read, the base names and the plan
  are pure, so they move to `base_source.rs`, which builds on every host.
  `ipsw_base.rs` keeps the `curl` and `tart` reads, the build and the
  refresh.
- 2026-11-22, plan input. The plan also takes the golden name and returns
  `golden_exists`. So the dry run reads `tart list` once, and the hold, the
  build and the dry run share one answer.
- 2026-11-22, other hosts. The real run fails on a host that is not macOS
  with `BACKEND_UNSUPPORTED`. The dry run now fails the same way, because
  it reads `tart list`. The contract tests of the plan are macOS-only.
- 2026-11-22, fakes. The shared `fake-tart` lists
  `testanyware-base-macos-27.2.1-26B131` in place of
  `testanyware-base-macos-27`. A test that runs the fast checks also puts a
  fake `curl` on `PATH`, which prints the trimmed catalogue. No test reads
  the network.
- 2026-11-22, refresh failures. A refresh that cannot delete an older base
  or a cached IPSW warns and goes on. The base is ready by then, and the
  next golden creation tries the deletion again.
- 2026-11-22, a base that went away. Boot 1 builds the base of the plan
  when `tart list` no longer names it, even if the plan said that it
  existed. The hold then lacks `<base>-building`, but the build still makes
  the planned release.
- 2026-11-22, controls. A mutation of `VIRTUAL_MAC` to `Mac17,10` failed 4
  unit tests and 3 contract tests, and the restored file passed them. The
  repository sweep for `testanyware-base-macos-27` found only the
  deliberate older-base references and the frozen evidence under
  `experiments/`. The same pattern found ADR-0027.
- 2026-11-22, docs. The golden images guide gained "Keeping goldens
  current", with the rule for each platform. Its Linux item says only that
  the build installs the current Ubuntu updates, because
  `linux-golden-refresh-k8` owns the new Linux mechanism. The admission-queue
  spec names the plan as a fast check, and its diagram deletes the older
  bases and the earlier golden. The provisioner page names `helpers/`, the
  directory that exists, in place of `autounattend/`.
- 2026-11-22, format. The changed Rust files were not rustfmt-clean before
  this leaf. Only `base_source.rs`, which this leaf rewrote, is formatted.
- 2026-11-22, golden-plan seam. Before the refresh, the dry run on this
  Mac named macOS 27.2.1 (26B131), the missing base
  `testanyware-base-macos-27.2.1-26B131`, the older base
  `testanyware-base-macos-27` and the existing golden. The seam passed.
- 2026-11-22, suites. `task cli:test` passed: 62 suites, 1724 tests, 0
  failed, 12 live tests ignored. The changed files did not move during the
  run. After the run, a clippy fix in `base_source.rs` passed the 380 unit
  tests of `testanyware-vm` again.
- 2026-11-22, refresh attempt. `task vm:refresh-golden-macos` passed the
  fast checks, got a slot, downloaded the IPSW of 27.2.1 and installed it.
  The boot to Setup Assistant then failed with
  `RFB error: protocol violation: ZRLE: tile stream ended mid-pixel`. The
  build deleted its `-building` VM. The earlier golden and the older base
  stayed, as the replacement rule requires.
- 2026-11-22, split. A root-cause hunt in the RFB client does not fit this
  session, and the question is precise. So the work splits into flat
  siblings, which share the context of the root. `setup-assistant-zrle-fault-k10`
  owns the fault. `macos-golden-refresh-run-k11` owns the refresh and the
  live criteria that need it. This leaf retires against the other criteria.
- 2026-11-22, tahoe golden. The auto-mode permission layer refused
  `testanyware vm delete testanyware-golden-macos-tahoe`. The deletion moves
  to `macos-golden-refresh-run-k11`, which asks the owner first.
- 2026-11-22, review. This leaf cut no `review-impl` leaf. The plist calls
  carry a docs.rs citation, the mutation control failed the new tests, and
  the live refresh of `macos-golden-refresh-run-k11` tests the plan, the
  build and the refresh on the real catalogue.
