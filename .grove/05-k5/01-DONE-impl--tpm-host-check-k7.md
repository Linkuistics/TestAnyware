# tpm-host-check-k7

## Goal

Make a Windows guest start on Apple Silicon with Homebrew's QEMU 11.1+, keeping
its TPM, by implementing ADR-0017's decision table: observe what the host's
QEMU binary offers, decide attach, omit or refuse from that, and give every
TPM site that one answer. This is the leaf that makes QEMU stop aborting.

## Context

- ADR-0017 is the specification: its table, its probe command and its reasons.
  `docs/research/windows-tpm-on-hvf.md` has the evidence and the recipes.
- `tpm.rs` (crate `testanyware-vm`) holds the decision as `k2` left it:
  `decide_tpm(Platform) -> Result<TpmDecision, VmError>`, `TpmDecision::{Attach,
  Omit(OmitReason)}`, `attaches()`, and `device_args(Option<&Path>)`. Its module
  comment says where the new answers go: *omit, host cannot supply* is another
  `OmitReason`, and *refuse* is the `Err`.
- The consumers of the decision: `VmLifecycle::start` (builds
  `QemuStartOptions.tpm`), `VmLifecycle::dry_run_validate_start`,
  `QemuRunner::start` (which still calls `check_swtpm` itself), and
  `create_golden_windows` with `build_and_provision`. The two builders are
  `build_qemu_args` and `build_install_qemu_args`.
- The profile's accelerator is `QemuProfile.accelerator` (`"hvf"` on macOS).
- A captured probe text for QEMU 11.1.2 is one command away on this Mac:
  `qemu-system-aarch64 -machine none -device tpm-tis-device,help`. It starts
  `tpm-tis-device options:` and lists `ppi=<bool> - on/off (default: on)`.
  For 11.0.1 (a local build that is not in the repo) the list is `irq`,
  `tpm-tis-mmio[0]` and `tpmdev`, with no `ppi`; write that fixture from the
  research note, headed by the same `tpm-tis-device options:` line. Where no
  such device exists QEMU prints `Device 'tpm-tis-device' not found`.

## Done when

- **Seam 1.** `decide_tpm` takes the platform and a host observation and
  returns ADR-0017's answers in ADR-0017's order. Its unit tests walk the
  table with no QEMU or swtpm: not Windows (and the observation is not even
  made); front end absent; front end present with and without `ppi`, under HVF
  and under another accelerator; swtpm missing (an `Err` that
  `code()`s as `SWTPM_MISSING`); swtpm missing *and* front end absent (the
  table's order decides; pin it).
- The probe's parser is unit-tested on the three captured texts above. It
  reads text, not the exit status: QEMU exits 0 for an unknown device.
- **Seam 2.** `build_qemu_args` and `build_install_qemu_args` add `ppi=off` to
  the `-device tpm-tis-device,tpmdev=tpm0` argument exactly when the decision
  says so, and emit the six-argument TPM triple exactly when it says attach.
  A caller cannot write `ppi=off` without a TPM.
- `vm start`, `vm start --dry-run` and `vm create-golden --platform windows`
  each ask the decision once, before cloning or booting anything. `--dry-run`
  reports a refusal. There is one swtpm rule, in the decision. If
  `check_swtpm` or its helper survives, it is only the observation's input,
  not a second rule at `QemuRunner::start`.
- A golden with no `<golden>-tpm` directory starts on a fresh, empty swtpm
  state instead of failing the state copy (k3's recommendation: nothing on
  these goldens is sealed to the TPM). This is a small pure choice (copy the
  golden's state, or start empty) with a unit test. A golden created where the
  decision omitted the TPM, started later where it attaches, is the case it
  serves.
- `task cli:test` passes (550 tests before this leaf; yours add to that),
  `task cli:clippy` reports nothing new in the files touched, and
  `task vm:qemu-tpm-check` still passes.
- **Live, on this Mac, on a clone** (never the golden): one
  `vm start --platform windows` with a fresh `--id` keeps QEMU up. QEMU's
  argv, from `ps`, contains `tpm-tis-device,tpmdev=tpm0,ppi=off`, and a VNC
  capture shows the Windows lock screen. With the current golden the screen
  says "Your password has expired", which is stage 2's fault and is expected.
  Stop and delete the clone afterwards.
- The docs that state who needs swtpm or a TPM, or how a QEMU guest is
  started, describe the host check and the refusal. Grep the claim (every
  TPM/swtpm mention in every tracked non-Rust file, then classify each, as
  `k2` did); `website/docs/` holds hand-synced copies (for example
  `reference/error-codes.md`, `architecture/vm-lifecycle.md`,
  `user/golden-images.md`). The `SWTPM_MISSING` row in `error-codes.md` and
  the contract's error table say when it is raised, now before cloning and
  from the decision. Rust help text in the CLI that mentions swtpm or the TPM
  (`main.rs`, `commands/vm.rs`) belongs to the sweep. If the implementation
  departs from ADR-0017, edit the ADR.

## Notes

- **Make the observation lazy.** ADR-0017 says no probe runs for a guest that
  does not need a TPM. The shape that follows from that is a decision that
  receives a way to observe, such as an `FnOnce() -> HostTpm`, so that a test
  passes a value and a non-Windows call never invokes it. Settle the exact
  shape yourself; keep `decide_tpm(..)?` and `.attaches()` working for callers
  that need only the old question.
- **Recommended: carry the wiring as one value.** The decision's `Attach` can
  carry the PPI choice, and the builders can take one `Option<TpmWiring>`
  (swtpm socket plus the PPI choice) in place of today's `Option<PathBuf>`, so
  that "PPI off, no TPM" is not expressible. This is a recommendation, not a
  requirement.
- **Probe failure.** Resolve the QEMU binary the way the launch sites do
  (`which`, then the bare name). If the probe cannot be spawned at all, return
  the same `QemuFailed` the launch would, rather than reading the failure as
  "the host cannot supply a TPM": omit is for a QEMU that ran and has no
  `tpm-tis-device`, not for a QEMU that is not there.
- **The omit that happens today** is an x86_64 binary (the Linux x86_64
  profile's `q35` has no `tpm-tis-device`), or a QEMU built without TPM
  support. Neither can be run live on this Mac; the table's unit tests are the
  evidence, plus `QEMU=qemu-system-x86_64 task vm:qemu-tpm-check`, which fails
  with "no tpm-tis-device" and so observes the same fact (that binary is
  installed here: `/opt/homebrew/bin/qemu-system-x86_64`).
- **Golden creation cannot be run live in this leaf** (a build is up to about
  75 minutes, and its result would be an unusable golden until stage 2's
  password fix). `build_install_qemu_args`'s unit test pins it; stage 2's
  `admin-password-never-expires` runs the live build.
- Issue #3's line also edits `QemuRunner::start` and `lifecycle.rs`. Keep
  edits there narrow so its later rebase stays small (plan-k1 D8).
- Teardown and golden deletion are already idempotent about a missing
  `-tpm` directory (`k2`'s Notes); neither needs to change.

## Decisions (running log)

**The decision is lazy, and a closure carries the observation.**
`decide_tpm(platform, accelerator, observe: impl FnOnce() -> Result<HostTpm,
VmError>)`. A non-Windows guest returns before calling `observe`, and a test
pins that with a `Cell` flag. `decide_tpm_on_host(platform, &profile)` is the
entry point the three consumers call, with the real probe.

**The table's order is code order.** Not Windows → omit; front end absent →
omit (`OmitReason::HostCannotSupplyOne`); swtpm missing → `Err(SwtpmMissing)`;
otherwise `Attach { ppi_off }` with `ppi_off = accelerator == "hvf" && ppi
listed`. Front end absent *and* swtpm missing omits, as the table orders it; a
test pins it.

**One value carries the wiring.** `TpmDecision::Attach { ppi_off }` plus
`TpmDecision::wiring(socket) -> Option<TpmWiring>`; both builders now take
`tpm: Option<TpmWiring>` in place of `tpm_socket: Option<PathBuf>`, so "PPI off,
no TPM" cannot be written.

**There is one swtpm rule.** `preflight::check_swtpm` and `swtpm_result` are
gone, and `QemuRunner::start` no longer preflights swtpm. The observation reads
`which("swtpm").is_some()`, and the decision turns that into the refusal.

**Probe failure is `QemuFailed`, not omit.** The probe spawns the resolved QEMU
(`which`, then the bare name). A spawn failure, or output that is neither a
property list nor "Device 'tpm-tis-device' not found", is `QemuFailed` (the
second one quotes the text). The parser reads stdout and stderr together: the
list goes to stdout, the "not found" complaint to stderr, and the exit status is
0 either way.

**Host gate before the decision in `start`.** `check_host_supports_local_qemu()`
now runs just before the decision, so a Windows host still reports
`VM_HOST_UNSUPPORTED` rather than a probe failure. `QemuRunner::start` keeps its
own call; it is cheap and idempotent.

**A golden without TPM state starts on a fresh one.** `tpm_state_source(
golden_state_exists) -> {CopyGolden, Fresh}`; `Fresh` creates the clone's empty
`-tpm` directory (swtpm needs it to exist). Pure, unit-tested.

**ADR-0017 needed no edit.** Implementation matches it.

## Result

**Verification.** `task cli:test`: 562 passed, 0 failed (550 before). `task
cli:clippy`: nothing in the files touched (the warnings in `finalize.rs`,
`golden_linux.rs` and `rfb` were there before). `task vm:qemu-tpm-check` passes
with Homebrew's QEMU 11.1.2, and fails with "no tpm-tis-device" under
`QEMU=qemu-system-x86_64`, as the leaf expected.

**Live, on a clone.** `vm start --platform windows --id k7-live`: QEMU stayed up
and its argv, from `ps`, held `tpm-tis-device,tpmdev=tpm0,ppi=off`. A VNC capture
of the clone showed the Windows lock screen (the clock and date), with no
"password expired" text visible at that point. The start was killed before it
wrote a spec (the agent wait is 10 to 14 minutes), so I stopped QEMU and swtpm by
pid and removed the clone and session directories by hand. The golden was
untouched. `vm start --dry-run` for Windows passes.

**Docs sweep.** Every TPM/swtpm mention in every tracked non-Rust file, then
classified. Edited: the QEMU start flow in `docs/architecture/vm-lifecycle.md`
and its `website/docs/` copy (host check, refusal, `ppi=off`, fresh state); the
`SWTPM_MISSING` row in `docs/reference/error-codes.md`, its website copy, and the
contract's error table (raised before cloning, by the decision, also under
`--dry-run`); the "UEFI + TPM 2.0" line in `docs/user/golden-images.md` and its
copy (a golden built where the host cannot supply a TPM has none); the Rust help
text for the install plan in `commands/vm.rs`. Left as true: the `QEMU+swtpm`
mentions in READMEs, the quick-start prerequisites and the overview diagrams.
`CONTEXT.md` already defines *TPM decision* (written by the design leaf).

**For `start-reports-tpm-k9`.** `TpmDecision` is `Attach { ppi_off }` or
`Omit(GuestDoesNotNeedOne | HostCannotSupplyOne)`. `VmLifecycle::start` makes the
decision at the top of the QEMU path and `dry_run_validate_start` discards it, so
the dry-run plan has to take it from there.
