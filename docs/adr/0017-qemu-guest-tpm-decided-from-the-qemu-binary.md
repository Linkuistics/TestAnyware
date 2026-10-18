# 17. A QEMU guest's TPM is decided from the host's QEMU binary, with the PPI off under HVF

Date: 2026-10-18

## Status

Accepted (grove `gh-issue-6`, design leaf `windows-tpm-on-hvf-k3`). The
investigation behind it is `docs/research/windows-tpm-on-hvf.md`.

## Context

QEMU 11.1.0 gave `tpm-tis-device`, the only TPM front end Arm's `virt` machine
can take, a 1 KiB RAM-backed PPI (Physical Presence Interface) region, on by
default. Under Hypervisor.framework that region cannot be mapped in whole
16 KiB host pages, QEMU's memory listener calls `hv_vm_unmap` on it, the
framework rejects the unaligned range with `HV_BAD_ARGUMENT`, and QEMU aborts.
So since Homebrew shipped 11.1 (August 2026), no Windows guest could start on
an Apple Silicon host, and neither could Windows golden creation. QEMU 11.0.x,
which the Windows path was verified with in June, has no PPI region and works
on macOS 27. Upstream has not fixed the listener.

The grove's requirements (`plan-k1`) bound the remedy. A Windows guest keeps
its TPM wherever the host can supply one, and goes without one, announced,
only where it cannot (D2); the remedy works with Homebrew's QEMU as shipped
(D3); golden creation takes the same remedy (D4); a host where Windows can
neither get a TPM nor run without one refuses before launch (D6). Only Windows
needs a TPM (D7), and the TPM decision is a pure function every TPM site
consumes (D5).

The investigation found that the PPI property can be switched off
(`ppi=off`), that only QEMU ≥ 11.1 accepts that property, and that the
current Windows golden runs, with or without a TPM, because nothing on it is
sealed to the TPM.

## Decision

**Decide each QEMU guest's TPM from its platform and from what the host's QEMU
binary itself reports; attach the TPM with its PPI off where the accelerator
is HVF.**

The host check runs only for a guest that needs a TPM (Windows). It reads
`<qemu> -machine none -device tpm-tis-device,help`, which takes about 20 ms and
needs no accelerator, VM or swtpm, and checks for swtpm on the path. The TPM
decision then answers, in this order:

| guest and host | answer |
|---|---|
| not Windows | omit: the guest does not need one (no probe runs) |
| the binary lists no `tpm-tis-device` | omit: the host cannot supply one, announced |
| swtpm is not installed | refuse before cloning: `SWTPM_MISSING`, with its install remediation |
| otherwise | attach; under HVF, add `ppi=off` when the binary lists a `ppi` property |

Golden creation takes the same answer. The start reports it in its human and
`--json` outputs: attached, or omitted with the reason.

Outside HVF the device keeps QEMU's default, so a QEMU ≥ 11.1 under KVM keeps
the PPI, which works there and silences the Event ID 15 that tpm.sys otherwise
logs at every boot.

## Considered options

- **Omit the TPM on HVF hosts.** It works (the golden runs without one), but
  it drops the TPM where a remedy that keeps it exists. D2 prefers keeping it.
- **Pin the machine type (`-machine virt-11.0`).** QEMU's compatibility table
  turns the PPI off for that machine, and pinning would also freeze every
  other guest-visible device default across QEMU upgrades. That is a broader
  decision than this defect: it needs a QEMU ≥ 11.0 floor, a bump each time
  QEMU retires old machine types, and it would hold back compat-gated fixes.
  Rejected here; it may be worth its own decision.
- **An unconditional `ppi=off` (or `-global tpm-tis-device.ppi=off`).** QEMU
  older than 11.1 refuses to start: "Property 'tpm-tis-device.ppi' not found".
- **A version rule on `qemu --version`.** It cannot see a distribution that
  backports the PPI to an older version (the patches come from a Red Hat
  engineer), and it has to be maintained. Asking the binary is as cheap and
  cannot drift.
- **A live probe: start a paused guest with the real TPM arguments.** It costs
  1 to 3 seconds and an swtpm per start, and it adds coverage only for
  failures nobody has seen.
- **Omit, rather than refuse, when swtpm is missing.** swtpm is a separately
  installed prerequisite with the same standing as QEMU itself (the Homebrew
  formula declares neither, and `testanyware doctor` checks both). One install
  command keeps the TPM, and omitting would let `vm create-golden` build a
  TPM-less golden on a misinstalled host without anyone choosing that.
- **Wait for an upstream fix, or ship a patched QEMU.** D3 rules out the
  second. The first leaves Windows unusable on every Apple Silicon host until
  Homebrew ships a fix; a QEMU issue is drafted for the human to file.

## Consequences

- Windows guests and Windows golden creation run again on Apple Silicon hosts
  with Homebrew's QEMU 11.1+, with a working TPM. The guest sees the June
  device: a TPM without PPI, and tpm.sys's Event ID 15 at each boot.
- A future reader seeing `ppi=off` should know it is load-bearing under HVF.
  It can come back on HVF once QEMU's listener stops unmapping sections it
  never mapped; turning it on changes the guest's ACPI TPM node, which Windows
  tolerates (it leaves a ghost device entry).
- An x86_64 binary has no `tpm-tis-device`, so a Linux x86_64 host (whose
  `q35` profile also names it) omits the TPM, announced, instead of failing at
  launch. A real TPM there needs a different front end, which is outside this
  decision.
- Each Windows start and golden creation spends about 20 ms on the probe. The
  probe must read the text: QEMU exits 0 even for an unknown device.
- Because the omit answer does not depend on the golden, a golden that was
  sealed to its TPM (none built by `vm create-golden` is: BitLocker is off and
  PCR7 binding is not possible on these guests) would not survive an omit. If
  such goldens ever appear, the decision gains an input and a refusal.
