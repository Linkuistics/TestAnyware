# TestAnyware.gh-issue-6 — brief

## Goal

Make QEMU-backed guests start again on the macOS 27 Apple Silicon host
(Linkuistics/TestAnyware#6), so that a Windows guest reaches a **Healthy
agent** here. A guest gets a TPM only if it needs one. A Windows guest keeps
its TPM wherever the host can supply one, and goes without one, announced,
only where the host cannot.

## Done when

- One `vm start --platform windows` on the macOS 27 host reaches a Healthy
  agent, and its output says whether the TPM was omitted.
- Only guests that need a TPM (today, Windows) get one. That holds at every
  TPM site: the TPM-state copy, the swtpm launch, the swtpm preflight and the
  QEMU arguments, in both argument vectors.
- Where a host cannot supply a TPM, Windows runs without one automatically and
  the start says so. Where a remedy that keeps the TPM exists, it is used
  instead. Either way the remedy works with Homebrew's QEMU.
- On a host where Windows can neither get a TPM nor run without one,
  `vm start --platform windows` refuses before launch, naming the cause and
  what the user can do.
- `vm create-golden --platform windows` takes the same remedy. If the remedy
  depends on a newly built golden, one live golden build has run on this Mac
  and that golden reaches a Healthy agent.
- The root cause is established with evidence, and any upstream report (QEMU,
  Apple) is drafted for the human to file.
- The tests at the agreed seams pass. The CLI design contract, the docs that
  state who needs swtpm or a TPM, and `CONTEXT.md` describe the new behaviour.
  The root `Taskfile.yml` carries the recipes the work runs.

Not this grove's: the issue #3 grove's waiting check
(`start-failure-qemu-k3`, seam check e) runs in that grove once this lands on
`main`. Closing #6 is the human's call once this done line is met.

## Decomposition

1. `tpm-only-when-needed` (impl). Builds the TPM decision with its first
   answers (attach for Windows; omit for guests that do not need one), threads
   it through every TPM site and both argument vectors, and adds the first
   Taskfile. It comes first because it is small, certain and useful on its
   own, and the remedy extends its seam. Windows behaviour does not change.
2. `windows-tpm-on-hvf` (design). Finds the root cause and chooses the remedy
   within the settled decisions, records it, and drafts any upstream report.
   Its last act adds the `planning` leaf for the remedy's implementation,
   because that shape depends on what the investigation finds.

## Pointers

- The settled decisions and their evidence: the retired requirements leaf
  `plan-k1`, its Decisions (D1 to D10) and Facts (1 to 9). Each leaf names the
  decisions it implements.
- Issues: #6 (the human's words). #3, whose grove (jj workspace `gh-issue-3`,
  not on `main`) found #6; its `start-failure-qemu-k3` running log, Q10 and
  Q11, holds the live evidence.
- ADRs: ADR-0009 (the Windows host harness runs its host-under-test under QEMU
  and swtpm, through `vm start`); ADR-0010 (the host-side framebuffer: the
  QEMU VNC endpoint shows a Windows guest's screen before its agent is up).
- Contract: `docs/architecture/cli-design-contract.md` and
  `docs/reference/error-codes.md` (a refusal code, and the start's report of
  an omitted TPM, are recorded there).
- Glossary terms in play: Host CLI, In-VM agent, Healthy agent, Golden image,
  Autounattend provisioning, Linux-on-qcow2, Host-side framebuffer.

## Test seams (agreed with the human)

1. **The TPM decision.** A new, pure decision at the highest point. From the
   guest's platform and what the host check observed, it answers attach; omit,
   with the reason (the guest does not need one, or the host cannot supply
   one); or refuse before launch, with the reason. Every TPM site, and what
   the start reports, consumes that one answer. Unit tests walk its table of
   cases with no QEMU or swtpm.
2. **The two argument builders.** Existing and pure, one for `vm start` and
   one for Windows golden creation. Their unit tests are extended so the TPM
   device appears exactly when the decision says attach.
3. **Live, on this Mac.** One `vm start --platform windows` reaches a Healthy
   agent and reports whether the TPM was omitted. A live golden build is added
   when the remedy depends on a new golden.

Linux-on-qcow2 is unit-tested only. This Mac has no Linux qcow2 golden, and
making one for testing would be test-only image tooling.

## Notes

- **The TPM worked on this Mac in June** (Windows golden, golden creation and
  the Windows harness, 30 May to 8 June 2026). Since then swtpm, QEMU (11.1.2)
  and macOS (27.0.1) have changed, so the abort is a regression and the 16 KiB
  page size is at most a necessary condition.
- **`tpm-tis-device` is the only TPM front end `virt` can take.** QEMU 11.1.2
  also offers `tpm-tis-i2c`, but `virt` has no I2C bus.
- **What this branch lacks.** It starts from `main`, without issue #3's
  honest-start work. A QEMU early exit here says only "QEMU did not remain
  running after launch", and teardown deletes the clone's `qemu.log`. To see
  QEMU's reason, run it directly or keep the log. If a Windows agent never
  answers, `vm start` here warns and exits 0 with no agent: a live check must
  look for a Healthy agent itself.
- **Timing.** A Windows start's agent wait is 10 to 14 minutes (120 probes,
  5 s apart, each up to a 2 s timeout). A Windows golden build is up to about
  75 minutes.
- **Guest control.** Guests are driven only through the In-VM agent and VNC,
  never a host-specific channel. Never modify `testanyware-golden-windows-11`
  itself (experiments run on clones), and do not replace it without asking: it
  is the only Windows golden.
- **Landing.** This grove lands on `main` first, then issue #3's grove rebases
  onto it. Expect conflicts there in `qemu.rs`, `lifecycle.rs`, the Taskfile
  and the contract docs; #3's grove resolves them. Reusing #3's task names
  keeps the Taskfile part small.
