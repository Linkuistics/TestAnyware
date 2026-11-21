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
- The refresh recipe rebuilds `testanyware-golden-macos-27`. Afterwards
  `testanyware-base-macos-27.2.1-26B131` exists, and
  `testanyware-base-macos-27` and `macos-27.0.1-26A434.ipsw` are gone.
- A clone of the new golden reports `27.2.1` from `sw_vers -productVersion`.
  The live gate in `live-vm-gate.rs` passes on that golden with
  `TESTANYWARE_LIVE_VM=1`. This is the golden-build seam of the root brief.
- After the refresh, the dry run says that the base exists, and it names no
  older base.
- `testanyware-golden-macos-tahoe` is deleted.
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
