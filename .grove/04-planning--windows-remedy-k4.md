# windows-remedy-k4

## Goal

Cut the implementation of the remedy `windows-tpm-on-hvf-k3` chose into the
smallest independently useful increments, so that the root brief's done line
is met: one `vm start --platform windows` on this macOS 27 host reaches a
Healthy agent and says whether its TPM was omitted.

The remedy has two independent parts, because the investigation found two
independent faults:

1. **The TPM abort (ADR-0017).** QEMU ≥ 11.1's `tpm-tis-device` has a 1 KiB
   PPI region that Hypervisor.framework cannot map, so QEMU aborts. Attach the
   TPM with `ppi=off` under HVF when the QEMU binary offers the property. The
   host check is the binary's own `-machine none -device tpm-tis-device,help`
   output, plus swtpm on the path. The TPM decision (built by
   `tpm-only-when-needed-k2` in `cli-rs/crates/testanyware-vm/src/tpm.rs`)
   grows the answers in ADR-0017's table: *omit, the host cannot supply one*
   when the binary has no `tpm-tis-device`; *refuse* (`SWTPM_MISSING`) when
   swtpm is missing; *attach* otherwise, with PPI off under HVF.
2. **The expired password (k3's R3).** The Windows golden's `admin` password
   expired 42 days after the golden was built, so auto-logon fails and the
   logon-triggered agent never starts, TPM or not. Golden creation must make
   that password never expire, and a new golden must be built.

## Context

- `windows-tpm-on-hvf-k3` (retired): findings F1 to F7 and decisions R1 to R5
  in its running log, and its Notes' upstream QEMU report draft.
- `docs/research/windows-tpm-on-hvf.md`: the evidence, the live runs (E1, E2b,
  E3) and the reproduction recipes.
- `docs/adr/0017-qemu-guest-tpm-decided-from-the-qemu-binary.md`: the decision
  table and why.
- `CONTEXT.md`: *TPM decision* (new), *Healthy agent*, *Autounattend
  provisioning*.
- Retired `plan-k1`: D1 (the bar), D4 (golden creation and the live build),
  D5 (the seams), D10 (the Taskfile; reuse issue #3's task names, notably
  `vm:qemu-start`).
- `task vm:qemu-tpm-check` (`scripts/qemu-tpm-check.sh`, added by k3): in
  seconds, whether this host's QEMU runs the TPM device as ADR-0017 attaches
  it, and whether QEMU's default PPI still aborts. It reads the property list
  the same way the host check must.

## Done when

- The plan is written as leaves under the root, each a vertical slice that can
  be verified on its own, and each names what it verifies and how.
- Between them, the leaves cover everything below.
  - **Seam 1.** The TPM decision's table is unit-tested with no QEMU or swtpm,
    including the host observation's cases: front end absent; present with
    and without `ppi`; swtpm missing; HVF and not.
  - **The host check's parser** is unit-tested on captured `,help` texts:
    11.1.2's (lists `ppi`), 11.0.1's (no `ppi`; `irq`, `tpm-tis-mmio[0]`,
    `tpmdev`), and "Device 'tpm-tis-device' not found". QEMU exits 0 for an
    unknown device, so the check reads the text.
  - **Seam 2.** Both argument builders (`build_qemu_args`,
    `build_install_qemu_args`) emit `ppi=off` exactly when the decision says
    so, and the TPM triple exactly when it says attach.
  - **The start reports** attached or omitted-with-reason, in both its human
    and `--json` outputs; the contract (`docs/architecture/cli-design-contract.md`,
    `docs/reference/error-codes.md`, the `vm start` schema) records it.
  - **Golden creation** takes the same decision (D4) and sets `admin`'s
    password to never expire. The check for that is in-guest:
    `net user admin` shows "Password expires  Never".
  - **D4's live build is triggered** (by the password, not the TPM). Build the
    new golden under a new `--name`. Verify the D1 bar against it with
    `vm start --platform windows --base <new>`, with no manual logon. **Ask
    the human before** it replaces `testanyware-golden-windows-11`, which is
    the only Windows golden. Once it is swapped in (or the human decides
    otherwise), run the final D1 check with the default golden.
  - **Seam 3, live.** On this Mac, one `vm start --platform windows` reaches a
    Healthy agent, with no help over VNC, and its output says the TPM was
    attached. This branch lacks issue #3's honest start: `vm start` warns and
    exits 0 if the agent never answers, so the check must look for the agent
    itself.
  - **Docs.** The docs that state who needs swtpm or a TPM, and `CONTEXT.md`'s
    *Autounattend provisioning* (the never-expiring password), describe the
    new behaviour. Grep the claim, not a file list: `website/docs/` holds
    hand-synced copies (for example `reference/error-codes.md`,
    `architecture/vm-lifecycle.md`, `user/golden-images.md`).
  - **The Taskfile** carries the recipes the leaves run, under issue #3's
    names where they overlap.

## Notes

- **Expected cost.** A Windows golden build is up to about 75 minutes,
  unattended. A Windows start reaches the lock screen in about 2 to 2.5
  minutes on this Mac, and the agent answers about 10 s after logon. Both ISOs
  are cached in `~/.local/share/testanyware/cache/`, and about 2.2 TiB is
  free.
- **Before the new golden exists**, a live check of part 1 can only reach the
  logon screen unaided: every clone of the current golden stops at "Your
  password has expired". k3 got past it by changing the password over VNC on
  the clone (OK, then new password `admin` twice), which is good evidence for
  part 1 but is not the D1 bar.
- **An edge for planning to settle.** If a golden has no `<golden>-tpm` state
  (one created where the decision omitted the TPM) and a later start on the
  same host attaches, the state copy fails. k3's recommendation: start swtpm
  on a fresh, empty state directory. Nothing on these goldens is sealed to the
  TPM (BitLocker is off and PCR7 binding is not possible), so a fresh TPM is
  safe.
- **Where to make the password never expire.** Either `net accounts
  /maxpwage:unlimited` (machine-wide) or `Set-LocalUser -Name admin
  -PasswordNeverExpires $true` (one account), run elevated during setup:
  `SetupComplete.cmd` runs as SYSTEM. Avoid `wmic`, which Microsoft has
  deprecated (not checked in this guest). k3 knows of no never-expires setting
  in the answer file's `<LocalAccount>` schema, so it has to be a command.
- **For the human, separable (k3's leaf noted it as optional):** whether
  `testanyware doctor` should report the host check (the front-end probe and
  the PPI rule). k3 recommends yes: it is the same 20 ms call, and it would
  have pointed straight at this fault.
- **Issue #3's grove** (`start-failure-qemu-k3`, seam check e) starts the
  default Windows golden, so its waiting check needs the swapped-in golden as
  well as this grove's code on `main`.
- The upstream QEMU report is a draft in k3's Notes. Filing it is the human's
  call (D9).
