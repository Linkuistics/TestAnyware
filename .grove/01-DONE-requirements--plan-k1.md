# plan-k1

## Goal

Settle what this grove delivers for Linkuistics/TestAnyware#6 (QEMU guests
cannot start on the macOS 27 Apple Silicon host, because `tpm-tis-device`
aborts QEMU with `HV_BAD_ARGUMENT`), in the human's own words. Then write the
root brief and cut the first leaves.

## Context

- The human's words: issue #6 (`gh issue view 6`).
- Where #6 came from: the issue #3 grove (jj workspace `gh-issue-3`, not yet
  on `main`). Its leaf `start-failure-qemu-k3` stays live for one check that
  this grove unblocks. That leaf's running log (Q10 to Q14) holds the live
  evidence behind #6.

## Done when

- The open questions are settled with the human and logged below.
- The root brief states the goal, the done line, the agreed test seams and the
  pointers.
- The first leaves are cut, and the whole is committed.

## Notes

### Facts (looked up, not asked)

1. **The host.** macOS 27.0.1 (26A434) on an Apple M4 Max, `hw.pagesize`
   16384. QEMU 11.1.2 is Homebrew's current stable (installed 2026-09-30);
   swtpm is 0.10.2_1 (installed 2026-09-26).
2. **The TPM worked on this same machine in June.** The Windows golden was
   built on 2026-05-30 with a TPM (its `testanyware-golden-windows-11-tpm`
   state directory). Commits from 2026-06-04 to 06-08 record the Windows
   golden, `vm create-golden --platform windows` and the Windows host harness
   live-verified under QEMU and swtpm. Since then swtpm (Sep 26), QEMU
   (Sep 30) and macOS (Oct 10, 27.0.1) have all changed. The 16 KiB page size
   is fixed by the hardware, so it is at most a necessary condition. The abort
   is a regression in QEMU 11.1.x, in macOS 27's Hypervisor.framework, or (least
   likely, given where it fails) in swtpm.
3. **Two argument vectors add the TPM, both unconditionally.** One is
   `build_qemu_args` (for `vm start`); the other is `build_install_qemu_args`
   (for `vm create-golden --platform windows`). Both are pure and have
   argument-vector unit tests. In the start path, `needs_tpm` gates only the
   swtpm *preflight*. The copy of the golden's TPM state and the swtpm launch
   also run for every guest.
4. **Linux-on-qcow2 is a latent path.** Only Windows golden creation writes a
   `<golden>-tpm` directory, and TestAnyware has no producer of Linux qcow2
   goldens: Linux goldens are tart images, built on a macOS host only. So a
   Linux qcow2 guest is reachable only with a hand-supplied image, and by
   reading of the code its start would fail when it copies the missing TPM
   state, before QEMU runs. On this host the only QEMU golden is Windows, and
   Linux is served by a tart golden, so the Linux half cannot be checked live
   here.
5. **Whether Windows can boot without a TPM is open.** The answer file sets
   `LabConfig` `BypassTPMCheck`, `BypassSecureBootCheck` and `BypassRAMCheck`,
   so Windows Setup does not need a TPM. Nothing in the answer file or the
   post-install scripts disables BitLocker device encryption. So whether the
   existing golden, installed *with* a TPM, is sealed to it is unknown. When
   the issue #3 grove ran Windows with the TPM arguments stripped, the agent
   wait used all 120 probes over 835 s, the last error was `timeout`, and the
   guest never became healthy. Its screen was not captured, so the cause is
   unknown.
6. **This branch has none of issue #3's start work.** The honest-start rule
   for QEMU (`QEMU_FAILED` with `details.qemu_error`), the first root
   `Taskfile.yml` (its `cli:*`, `scripts:check` and `vm:qemu-start` tasks) and
   `scripts/live-start-checks.sh` exist only in the `gh-issue-3` line. On
   `main` a QEMU early exit says only "QEMU did not remain running after
   launch", and teardown deletes the clone's `qemu.log`. The #3 line rewrites
   much of `lifecycle.rs` and some of `qemu.rs`, and it commits its own
   `.grove/` tree.
7. **The Windows host harness inherits `vm start`.** ADR-0009's Windows
   harness launches its host-under-test VM through
   `vm start --platform windows`, so it is blocked here by the same abort.
8. **No upstream report found.** Two web searches on 2026-10-18 found nothing
   linking `tpm-tis-device` to `HV_BAD_ARGUMENT`, and nothing about macOS 27.
   That silence is itself a finding.
9. **`tpm-tis-device` is the only TPM front end the `virt` machine can take.**
   QEMU 11.1.2's device list (`-device help`) offers two TPM front ends:
   `tpm-tis-device` (a system-bus device) and `tpm-tis-i2c`. The second fails
   on `virt` with "No 'i2c-bus' bus found for device 'tpm-tis-i2c'" (checked
   under TCG, paused). No CRB front end is built. So a remedy that keeps the
   TPM has to work through `tpm-tis-device` or through the machine around it.

## Decisions (running log)

**D1. The done bar is a Windows guest that works on this host.** The grove is
done when one `vm start --platform windows` on this macOS 27 Apple Silicon host
reaches a Healthy agent. The grove owns the root-cause investigation and
whatever remedy it needs. That also unblocks the issue #3 grove's waiting check
(`start-failure-qemu-k3`, seam check e), the Windows host harness and Windows
golden creation on this Mac. Rejected: correct failures only (leaves Windows
unusable here and #3 blocked); diagnose first and set the bar later (one more
round with the human, and no commitment). The human chose this over both.

**D2. A Windows guest goes without a TPM only where the host cannot supply
one.** Suppose a host cannot give a Windows guest a working TPM, but Windows
reaches a Healthy agent without one. Then `vm start` omits the TPM on that host
automatically, and its output says so. Everywhere the TPM works, Windows keeps
it. Telling such hosts apart reliably is the investigation's to design. The
recommendation stated a preference, which the human accepted with it: where a
remedy that *keeps* the TPM exists, it wins over omitting it. Rejected: never
(Windows would stay unusable here whenever the TPM is broken); dropping the TPM
everywhere (an unsupported configuration even where the TPM works today); a
start flag (every unattended script would need it on this Mac only).

**D3. The shipped remedy works with Homebrew's QEMU.** TestAnyware keeps
depending on Homebrew's `qemu` formula. Building or pinning QEMU locally is
allowed as an investigation tool (to bisect, or to prove a patch), never as a
requirement on users. A fix that lives in QEMU's source becomes a drafted
upstream report and is adopted when Homebrew ships it; meanwhile D2's fallback
covers the host. Rejected: a pinned or patched QEMU shipped as a requirement (a
hand-maintained build for every affected user); a local build meeting the done
bar on this Mac only (the check would verify a configuration users never get).

**D4. Golden creation gets the same remedy; a live build only if the remedy
needs one.** `vm create-golden --platform windows` builds its own QEMU argument
vector with the same TPM device, so it takes the same remedy, pinned by its
argument-vector unit test. A live golden build on this Mac (up to about 75
minutes of unattended install) is part of done only if the remedy depends on a
newly built golden. The expected case is the current golden refusing to boot
without the TPM it was installed with: then D2's fallback needs a golden built
without one. Rejected: always running a live build (long, and it proves little
when the existing golden meets the bar); leaving golden creation alone (it
stays blocked on this Mac).

**D5. The test seams (agreed with the human).**
1. *The TPM decision* (new, pure, at the highest point): from the guest's
   platform and what the host check observed, it answers attach; omit, with
   the reason (the guest does not need one, or the host cannot supply one); or
   refuse before launch, with the reason. Every TPM site consumes that one
   answer: both argument builders, the TPM-state copy, the swtpm launch, the
   preflight, and what the start reports. Unit tests walk the table of cases
   with no QEMU or swtpm.
2. *The two argument builders* (existing, pure): their tests are extended so
   the TPM device appears exactly when the decision says attach, for
   `vm start` and for golden creation.
3. *Live, on this Mac*: one `vm start --platform windows` reaches a Healthy
   agent and reports whether the TPM was omitted (D1). A live golden build is
   added when D4's condition holds.

Linux-on-qcow2 is unit-tested only: this Mac has no Linux qcow2 golden, and
making one for testing would be test-only image tooling. Rejected: a live Linux
qcow2 check (needs a hand-made golden); no new seam (the omit and refuse logic
would be tested only by the live run).

D6 to D10 were stated as settled ground (taken from the issue, or forced by
the repository's structure) and confirmed by the human together with the
summary of D1 to D5.

**D6. When nothing works, `vm start` refuses before launch.** This is the
issue's direction 3 ("instead of letting it fail at launch"). Take a host where
a Windows guest can neither get a working TPM nor run without one. There,
`vm start --platform windows` refuses before cloning anything, with an error
that names the cause and what the user can do. D5's TPM decision carries this
as its refuse answer. Reaching that case also means this grove cannot meet D1,
so it stops and asks the human rather than lowering the bar.

**D7. Only guests that need a TPM get one.** This is the issue's direction 2.
Today only Windows needs one. The rule covers all four TPM sites together (the
TPM-state copy, the swtpm launch, the swtpm preflight and the QEMU arguments),
in both argument vectors. So a Linux-on-qcow2 guest gets no TPM device,
launches no swtpm and needs no `<golden>-tpm` state.

**D8. This grove's code starts from `main`.** It does not start from issue
#3's unmerged line: stacking on it would merge #3's committed `.grove/` tree
into this one. The cost is that on this branch a QEMU start failure still says
only "QEMU did not remain running after launch", so the investigation reads
QEMU's own output directly (running QEMU by hand, as issue #6 did). Once this
grove lands on `main`, the #3 grove rebases onto it and runs its waiting check
(`start-failure-qemu-k3`, seam check e) through its honest-start path.

**D9. Upstream reports are drafted, not filed.** If the fault is in QEMU or in
macOS, the grove drafts the report (a QEMU issue, an Apple Feedback) with its
evidence. The human decides whether to file it. No session posts anything
outward-facing.

**D10. A root `Taskfile.yml`, carried forward from issue #3's choice.** `main`
has none. This grove adds one with only the tasks its own work runs. Wherever
they overlap, it uses #3's task names and recipes (`cli:test`, `cli:build`,
and so on), so the later merge is a small mechanical conflict rather than two
designs.
