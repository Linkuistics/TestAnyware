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

## Decisions (running log)

**P1. Two ordered increments, one per fault, each a node.** The remedy's two
parts are independent faults with different evidence and different checks, and
each leaves the product working. Stage 1 (`windows-start-on-hvf-k5`) restores
Windows starts on HVF: a live start keeps QEMU up and reaches the lock screen,
which is checkable without the golden being fixed. Stage 2
(`windows-golden-password-k6`) makes the golden survive its 42-day password
age and meets the root bar. A user with a fresh golden needs only stage 1.
Stage 2 follows stage 1 because golden creation aborts at the TPM until the
host check lands (no stage-2 live build can run before it). Rejected: one flat
list of four leaves (it hides that the stages are separately useful and
verified); three stages with the report as its own (the report is two small
outputs over an answer that already exists, so it rides in stage 1 as its own
leaf, which keeps the host check's session about the decision).

**P2. Stage 1 is two leaves, cut by what each can be verified on.**
`tpm-host-check-k7` holds everything that is one decision and its consumers
(observation, parser, full table, `ppi=off` in both builders, wired into
start, dry-run and golden creation, the fresh-state edge, the TPM docs sweep).
Splitting the pure table from its wiring would leave a leaf dead until its
sibling landed, so they stay together. `start-reports-tpm-k9` is a separate
vertical slice (decision to result to text and JSON, schema, contract) that
the first leaf's new answers feed, verifiable on its own by the schema test
and a live `--json` start. Rejected: report first (it would report answers
that do not exist yet).

**P3. Stage 2 is two leaves, split at the human's decision.**
`admin-password-never-expires-k8` is the fix plus the live build and its proof,
ending on a committed, verified new golden under its own name.
`golden-swap-in-k10` is the human-gated replacement of the only Windows golden
plus the final bar with the default golden. It is its own leaf because the
decision is the human's (the brief forbids replacing the only golden without
asking) and because the 75-minute build session should not end mid-question.

**P4. The edge the leaf was asked to settle: a golden with no TPM state.**
When a golden has no `<golden>-tpm` directory (built where the decision
omitted the TPM) and a later start attaches, start swtpm on a fresh, empty
state directory rather than failing the copy. k3's evidence supports it:
nothing on these goldens is sealed to the TPM (BitLocker off, PCR7 binding
impossible). It sits in k7, with the sites it touches.

**P5. Where the password is made never to expire: `SetupComplete.cmd`, with an
in-guest check that fails the build.** k3 found no answer-file setting for it.
The leaf prefers `Set-LocalUser -PasswordNeverExpires` (one account) and
allows `net accounts /maxpwage:unlimited`. The addition that matters is the
check: after provisioning, `net user admin` through the agent must show
"Password expires Never" or the build fails with the output. Without it a
regression would be invisible for 42 days, because a fresh golden reaches a
Healthy agent either way. For the same reason k8 says the fix is proved by
`net user admin`, not by the Healthy agent, and offers a shifted-RTC clone
as an optional behavioural proof.

**P6. The Taskfile recipe is `vm:windows-healthy-start`, not `vm:qemu-start`.**
Issue #3's `vm:qemu-start` runs a different recipe through
`scripts/live-start-checks.sh`, which is not on `main`. A distinct name keeps
the later merge a union of the two files. The recipe is first needed by k8 and
reused by k10; it must read the agent itself because this branch's start
exits 0 with a warning when the agent never answers.

**P7. No review chain; the `doctor` question goes to the human with the swap.**
The plan is four leaves, all decisions recorded in ADR-0017, and every leaf
has executable checks, so a `review-planning` leaf would not earn its place
(`references/decompose.md`: a load-bearing artifact earns one; size and
preference do not). No in-session reviewer was spent. Whether `doctor` should
report the host TPM check is the human's call and not work this grove has been
asked for, so it is no leaf yet: `golden-swap-in` asks it while the human is
there and cuts the leaf on a yes. It is also in the root brief's Notes.

**Check.** `grove-llm pick` returns `tpm-host-check-k7`; its brief chain is
root, `windows-start-on-hvf`. Every item of this leaf's Done when has a home:
seam 1, the parser and seam 2 (k7); the start's report and its contract (k9);
golden creation's decision (k7) and password (k8); D4's live build (k8); the
swap and the final bar (k10); docs (k7, k8, k9); the Taskfile (k8).
