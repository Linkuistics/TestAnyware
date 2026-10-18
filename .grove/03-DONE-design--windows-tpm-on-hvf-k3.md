# windows-tpm-on-hvf-k3

## Goal

Establish why `tpm-tis-device` aborts QEMU 11.1.2 with `HV_BAD_ARGUMENT` under
Hypervisor.framework on the macOS 27 Apple Silicon host. Then choose the remedy
that gets a Windows guest to a Healthy agent here, within D2, D3, D4 and D6.
Record the remedy, draft any upstream report, and add the `planning` leaf that
cuts its implementation.

## Context

- Decisions in the retired `plan-k1`:
  - D1: the bar.
  - D2: omit the TPM only where the host cannot supply one, announced; a remedy
    that keeps the TPM wins.
  - D3: Homebrew's QEMU only; local builds are investigation tools.
  - D4: golden creation, and when a live build is needed.
  - D6: refuse before launch when nothing works.
  - D9: drafts, not filings.
  - Facts 1, 2, 5, 6, 8 and 9.
- Issue #6, and the #3 grove's `start-failure-qemu-k3` running log (Q10, Q11):
  - the hand reproduction: the host profile's arguments stay up, and adding
    only `tpm-tis-device` aborts;
  - the TPM-less Windows run: 835 s, last error `timeout`, never healthy,
    screen not captured.
- ADR-0010: the QEMU VNC endpoint shows a Windows guest's screen before its
  agent is up.
- `tpm-only-when-needed-k2` (before this leaf) built the TPM decision. The
  remedy extends it with the host's observation and the *omit, because the host
  cannot supply one* and *refuse* answers.

## Done when

The findings answer each of these with evidence (commands and their output,
and citations into QEMU 11.1.2's source):

1. Which Hypervisor.framework call fails, which of the TPM device's memory
   regions it rejects, and why: alignment to the 16 KiB host page, or a newer
   Hypervisor.framework restriction.
2. Whether this is a QEMU regression or a macOS one. Does the QEMU that worked
   here in June (11.0.x, built locally per D3) abort the same way on macOS 27?
3. Whether any TestAnyware-side configuration gives a Windows guest a working
   TPM with Homebrew's QEMU: device properties, placement or machine options.
   `tpm-tis-device` is the only front end `virt` can take (fact 9).
4. Whether the current Windows golden reaches a Healthy agent with no TPM. If
   not: what its screen shows (BitLocker recovery, a hang, something else), and
   whether a golden built without a TPM would succeed (D4's condition).
5. How `vm start` can tell a host that cannot supply a TPM from one that can,
   reliably and cheaply (a version rule, or a probe), and what it reports in
   the omit and refuse cases.

And:

- The remedy is chosen within the decisions above and recorded where it binds.
  That means an ADR if it clears the ADR test, which a remedy that omits the
  TPM on some hosts very likely does.
- Any upstream report is drafted in this leaf's Notes, for the human (D9).
- A `planning` leaf is added after this one. Its body carries the remedy and
  what its implementation must verify: D1, D4's live build if triggered, and
  the root brief's seams.

## Notes

- `hvf-all.c:123` should be the call site of the failing Hypervisor.framework
  call, since QEMU's `assert_hvf_ok` reports where it was called. Read that
  line in the 11.1.2 source first.
- The agent forward is QEMU's user-mode network on localhost, so #3's macOS
  Local Network finding (errno 65 to `192.168.64.x`) should not apply. A
  `timeout` there means the guest never answered on its forwarded port.
- Watch a TPM-less boot through VNC (`testanyware screen capture` against the
  clone's endpoint) rather than waiting out the agent window blind.
- Run experiments on clones, never on `testanyware-golden-windows-11` itself.
  Do not replace that golden without asking: it is the only Windows golden.
- If nothing works within D2 to D4, stop and ask (D6) rather than widening the
  remedy space.
- One optional idea, not a requirement: once the host check exists, whether
  `testanyware doctor` should report it is a small, separable question for the
  human.

### Upstream report draft (D9: for the human to file, at gitlab.com/qemu-project/qemu/-/issues)

**Title:** hvf: `tpm-tis-device` aborts with HV_BAD_ARGUMENT on Apple Silicon
since 11.1.0 (unaligned `hv_vm_unmap` of the PPI region)

**Host.** macOS 27.0.1 (26A434), Apple M4 Max, 16 KiB host pages. QEMU 11.1.2
(Homebrew). Not run on other versions, but 11.1.0's source has the same
`ppi` default and master (2026-10-18) has the same `ppi` default and the same
`hvf_set_phys_mem`.

**Reproduce.**

```
swtpm socket --tpmstate dir=/tmp/st --ctrl type=unixio,path=/tmp/sock --tpm2 --flags not-need-init &
qemu-system-aarch64 -machine virt -accel hvf -cpu host -m 512 -S -display none \
  -chardev socket,id=chrtpm,path=/tmp/sock -tpmdev emulator,id=tpm0,chardev=chrtpm \
  -device tpm-tis-device,tpmdev=tpm0 -trace 'hvf_vm_*'
```

**Result.**

```
hvf_vm_unmap paddr:0x000000000c005000 size:0x00000400
qemu-system-aarch64: -device tpm-tis-device,tpmdev=tpm0: Error: ret = HV_BAD_ARGUMENT (0xfae94003, at ../accel/hvf/hvf-all.c:123)
```

With `ppi=off` on the device, or with `-machine virt-11.0`, QEMU starts. QEMU
11.0.x starts with the same arguments (no PPI region).

**Analysis.** Commit 46cd2c10 ("hw/tpm: add PPI support to tpm-tis-device
for ARM64 virt") gives `tpm-tis-device` a 0x400-byte `ram_device` region as
sysbus MMIO 1; on `virt` it lands at 0x0c005000. In `hvf_set_phys_mem`, a RAM
section not aligned to `qemu_real_host_page_size()` sets `add = false` and
calls `hv_vm_unmap(gpa, size)` on the unaligned range. Hypervisor.framework
requires a page-aligned address and a size that is a multiple of the page size
(`hv_vm.h`), so it returns `HV_BAD_ARGUMENT` and `assert_hvf_ok` aborts. A
standalone probe on the same host: `hv_vm_unmap` of a never-mapped,
16 KiB-aligned range returns `HV_SUCCESS`; any misaligned address or size
returns `HV_BAD_ARGUMENT`. The listener path is unchanged since at least
11.0.1, so the regression is the new region meeting it: any HVF user of
`tpm-tis-device` (for example a Windows 11 Arm64 guest, which needs a TPM)
cannot start on 11.1 without `ppi=off`.

**Possible fix (for the maintainers to judge).** An unaligned section was never
mapped by this listener, so the `!add` path could skip `hv_vm_unmap` for it
(or align the section inward, as KVM's `kvm_align_section` does, and skip it
when empty). Separately, `tpm-tis-device` could default `ppi` off under HVF,
since a sub-page RAM region cannot be mapped there anyway.

## Decisions (running log)

**F1 (question 1) — the failing call is `hv_vm_unmap(0x0c005000, 0x400)`, the
TPM's new PPI region, rejected for misalignment.** QEMU 11.1.2's
`accel/hvf/hvf-all.c:123` is the `hv_vm_unmap` in `hvf_set_phys_mem`'s `!add`
branch, which a RAM region takes when its address or size is not aligned to
`qemu_real_host_page_size()` (16 KiB here). In 11.1, `tpm-tis-device` gained a
`ppi` property (default `true`) that registers a second sysbus MMIO: a 1 KiB
(`TPM_PPI_ADDR_SIZE` = 0x400) `ram_device` region named `tpm-ppi`
(`hw/tpm/tpm_tis_sysbus.c`, realize). On `virt` it lands at platform-bus base
0x0c000000 + 0x5000 (after the 0x5000 TIS MMIO). Trace evidence
(`-trace 'hvf_vm_*'`, Homebrew 11.1.2, paused `virt` guest): the last call
before the abort is `hvf_vm_unmap paddr:0x000000000c005000 size:0x00000400`.
With `ppi=off` QEMU stays up. A direct Hypervisor.framework probe on this host
(macOS 27.0.1): `hv_vm_unmap` returns `HV_BAD_ARGUMENT` (0xfae94003) for
(0x0c005000, 0x400), (0x0c004000, 0x400) and (0x0c005000, 0x4000), and
`HV_SUCCESS` for 16 KiB-aligned never-mapped ranges (0x0c004000, 0x4000) and
(0x0c000000, 0x8000). The SDK header (`hv_vm.h`, `API_AVAILABLE(macos(11.0))`)
documents that contract: page-aligned `ipa`, size a multiple of the page size.
So it is **alignment to the 16 KiB host page, not a newer Hypervisor.framework
restriction**: the framework rejects exactly what its contract has always
excluded, and accepts an unmap of a range that was never mapped.

**F2 (question 2) — a QEMU regression, introduced in 11.1.0; macOS 27 is not
the cause.** Homebrew's formula history (`gh api` on homebrew-core's
`Formula/q/qemu.rb`): 11.0.1 shipped 2026-05-28 and was current through the
June verification (30 May to 8 June); 11.0.2 came on 06-28, 11.1.0 on 08-13,
11.1.2 on 09-29. In the source tarballs, `tpm_tis_sysbus.c` has no `ppi`
property in 11.0.1 or 11.0.5 and has `DEFINE_PROP_BOOL("ppi", …, true)` in
11.1.0 and 11.1.2, while `hvf_set_phys_mem` is identical from 11.0.1 to
11.1.2. QEMU 11.0.1 built locally here (`--target-list=aarch64-softmmu
--disable-pvg`; pvg must be off to compile against the macOS 27 SDK, as
Homebrew's 2026-09-11 formula change also does) **stays up on macOS 27.0.1**
with the identical TPM arguments, and its `-device tpm-tis-device,help` lists
`irq`, `tpm-tis-mmio[0]` and `tpmdev`, no `ppi`. So the HVF listener's
unaligned-unmap path is old and latent; 11.1.0 exposed it by giving the device
its first sub-page RAM region. The same `,help` query on 11.1.2 lists
`ppi=<bool> - on/off (default: on)`, takes about 20 ms with no accelerator,
VM or swtpm, and exits 0 even for an unknown device (so a probe reads the
property list, not the exit status).

**F3 (question 3) — yes: turning the PPI off keeps a working TPM device with
Homebrew's QEMU, but only QEMU ≥ 11.1 accepts the property.** Upstream
history (GitLab API on `hw/tpm/tpm_tis_sysbus.c`): `46cd2c10` "hw/tpm: add PPI
support to tpm-tis-device for ARM64 virt" (authored 2026-03-27, committed
04-24; its message: so Windows 11 ARM64 guests "no longer log Event ID 15
errors from tpm.sys on every boot"), then `5f0348bf` "gate PPI support on
tpm-tis-device behind a device property" (default true; `ppi=off` never
registers the RAM block). QEMU 11.1.2's `hw_compat_11_0` sets
`{ "tpm-tis-device", "ppi", "off" }`. QEMU master (fetched 2026-10-18) still
defaults `ppi` on and has the same `hvf_set_phys_mem`, so upstream has not
fixed it. Paused-guest checks, each spelling against both binaries:
`-device …,ppi=off` and `-global tpm-tis-device.ppi=off` stay up on 11.1.2 and
**fail on 11.0.1** ("Property 'tpm-tis-device.ppi' not found");
`-machine virt-11.0,…` stays up on both. So the cost of PPI off to the guest is
the Event ID 15 log entry per boot, which is the June configuration exactly.
Placement cannot help (the platform bus allocates the PPI right after the
0x5000 TIS window, and its 0x400 size is sub-page wherever it lands), and
`tpm-tis-i2c` has no bus on `virt` (fact 9).

**F4 (live E1) — with `ppi=off`, Homebrew QEMU 11.1.2 runs the current golden
with a working TPM; a second, unrelated blocker stops the agent: the golden's
`admin` password has expired.** E1: this branch's debug CLI, `vm start
--platform windows --id ta-e1-ppioff`, with a PATH shim in front of Homebrew's
`qemu-system-aarch64` that only appends `,ppi=off` to the `tpm-tis-device`
argument (argv logged; swtpm on the clone's copy of the golden's TPM state).
QEMU stayed up; Windows booted to the lock screen in about 2 minutes and sat
there. Over VNC: the sign-in screen for `admin`; typing the answer file's
password gave "Your password has expired and must be changed." The answer file
sets `<AutoLogon>` with no `LogonCount` (in-guest: `AutoAdminLogon` = 1, no
`AutoLogonCount`), and `net accounts` shows **Maximum password age 42 days**.
The golden was built 2026-05-30 and the guest clock reads 2026-10-18 (141
days), so auto-logon fails on every boot, on every host, TPM or not. The agent
is a logon-triggered scheduled task (`SetupComplete.cmd`, `schtasks … /sc
onlogon /ru admin`), so no logon means no agent. After changing the password
by hand on the clone, the agent answered `/health` in 8 s
(`{"accessible":true,"platform":"windows"}`) and `vm start` returned with its
agent. In-guest (unelevated, via `file exec`): `tpmtool getdeviceinformation`
reports TPM present, 2.0, manufacturer IBM (swtpm), initialized, ready for
storage, PPI version empty, "Bitlocker PCR7 Binding State: Binding Not
Possible"; `Get-PnpDevice` shows `ACPI\MSFT0101\1` OK beside a ghost
`ACPI\MSFT0101\0` (Unknown), so 11.1's ACPI TPM node differs from June's in
its instance id; `System.Volume.BitLockerProtection` on `C:` is 2 (off);
tpm.sys logs Event ID 15 at boot, the noise upstream's PPI was added to
remove. This also reinterprets the #3 grove's TPM-less run (Q10): its 835 s
agent timeout is what an expired password produces, so it said nothing about
the TPM.

**F5 (question 4) — the current golden reaches a Healthy agent with no TPM;
no TPM-less golden is needed.** E2b: same golden, a fresh clone, the shim
dropping the whole `-chardev`/`-tpmdev`/`-device` triple (0 TPM arguments in
the logged argv). Windows booted to the lock screen exactly as in E1 (no
BitLocker recovery, no hang); the identical keystroke sequence over VNC gave
"password has expired" → change → "Your password has been changed"; the agent
answered `/health` 10 s after logon. In-guest, `tpmtool getdeviceinformation`
fails with 0x800710df ("The device is not ready for use") and only the ghost
`ACPI\MSFT0101\0` remains, so the guest really ran TPM-less. This is
consistent with F4's BitLocker-off finding: nothing on the golden is sealed to
its TPM. (An earlier E2 attempt hit "Access is denied" after a mis-aimed click
put the old password in the wrong field; the clean E2b replay shows that was
the input, not the TPM.) So D4's expected trigger (a golden that refuses to
boot without the TPM it was installed with) does not occur, and D2's omit
answer is viable for this golden.

**F6 (live E3) — the remedy repeats, and the golden has no licence clock.**
E3 repeated E1's configuration (shim `ppi=off`, TPM attached): lock screen
136 s after QEMU started, the same expired-password logon, Healthy agent 9 s
after logon (`accessible` false at first, true moments later; the glossary
counts both as healthy). In-guest: `slmgr /dli` reports Windows Professional,
VOLUME_KMSCLIENT channel, License Status "Notification" (0xC004F056, no KMS
host) and `/xpr` "Notification mode"; `Win32_OperatingSystem` reports
Microsoft Windows 11 Pro 10.0.26200, installed 2026-05-29. The answer file
selects `/IMAGE/NAME` "Windows 11 Pro" with the generic KMS client key, not the
time-limited Enterprise Evaluation edition, so the password is the golden's
only age-dependent failure found.

**F7 (question 5) — the host check is the QEMU binary's own property list.**
`<qemu> -machine none -device tpm-tis-device,help` (about 20 ms, no
accelerator, VM or swtpm; F2) answers both things the decision needs: whether
the binary has the `virt` TPM front end at all (absent: "Device
'tpm-tis-device' not found", exit 0, so parse the text), and whether it has the
`ppi` property (11.1+: a `ppi=<bool>` line). `tpm-tis-device` is only built
where a machine implies `TPM_TIS_SYSBUS` (`hw/arm/Kconfig`, `virt`), so an
x86_64 binary reports it absent. Rejected: a version rule on `--version`
(blind to distro backports, and the PPI patch's author is at Red Hat); a live
paused-guest probe of the real TPM arguments (1 to 3 s and an swtpm per start,
and it only adds coverage for failures nobody has seen); an unconditional
`ppi=off` or `-global` (F3: fatal on QEMU < 11.1).

**R1 — the remedy for the abort: attach the TPM with its PPI off where the
accelerator is HVF and the QEMU binary offers `ppi`.** Everywhere else the
device keeps QEMU's default (PPI on wherever a QEMU ≥ 11.1 runs under KVM,
where it works and silences Event ID 15; absent on older QEMU). It keeps the
TPM, so it wins over omitting it (D2); it is Homebrew QEMU as shipped (D3); it
reproduces the June guest exactly (F2, F3). Rejected: omitting the TPM on
HVF hosts (D2's preference); pinning `-machine virt-11.0` (freezes every
compat-gated device default, needs a QEMU ≥ 11.0 floor and a bump as machine
types retire; a broader decision than this defect); an unconditional `ppi=off`
or `-global` (fatal on QEMU < 11.1, F3); a version rule or a live probe (F7);
waiting for an upstream fix or a patched QEMU (D3). Revisit when QEMU's HVF
listener stops unmapping sections it never mapped: PPI could then come back
on HVF too.

**R2 — the TPM decision's table.** Inputs: the guest platform, and only for a
platform that needs a TPM, the host check (F7's front-end probe, swtpm on the
path, and the profile's accelerator). Answers, in order: not Windows → *omit,
the guest does not need one* (no probe runs); the binary has no
`tpm-tis-device` → *omit, the host cannot supply one*, announced (Windows runs
without one, F5); swtpm missing → *refuse* before cloning with the existing
`SWTPM_MISSING` and its install remediation; otherwise *attach*, with PPI off
under HVF when `ppi` is offered (R1). A missing swtpm refuses rather than
omits because swtpm, like QEMU itself, is a separately installed prerequisite
(the Homebrew formula declares neither; `doctor` checks both), the one-line
install keeps the TPM (D2's preference), and omitting would let `vm
create-golden` build a TPM-less golden on a misinstalled host without anyone
choosing that. This is the one place D2's letter can be read the other way;
it is flagged for the human. The *omit, host cannot supply* answer's real
trigger today is an x86_64 binary (the Linux x86_64 profile's `q35` has no
`tpm-tis-device`; the proper front end there is a separate, pre-existing gap,
not this grove's), or a QEMU built without TPM support. The start reports the
decision in both outputs: attached, or omitted with its reason.

**R3 — the second blocker: golden creation makes `admin`'s password never
expire, and a new golden is built (D4's live build is triggered, for this
reason rather than the TPM).** Build it under a new `--name`, verify the D1
bar with `vm start --platform windows --base <new>`, then ask the human
before swapping it into `testanyware-golden-windows-11` (the brief: never
replace the only Windows golden without asking). The ISO and the VirtIO ISO
are cached here (`~/.local/share/testanyware/cache/`), and 2.2 TiB is free.
Rejected: repairing the current golden in place (modifies the only golden,
needs elevation the unelevated agent lacks, and leaves a golden no
`create-golden` run reproduces); driving the password change over VNC on
every start (runtime automation papering over an image defect); pinning the
guest clock with `-rtc base=` (breaks time-dependent guests and is undone by
time sync); an agent that needs no logon (the agent drives the interactive
desktop, so it needs the logged-on session).

**R4 — upstream.** A QEMU issue is drafted below (D9). No Apple Feedback:
Hypervisor.framework behaves exactly as its header documents (F1).

**R5 — artifacts.** The root cause and its evidence go to
`docs/research/windows-tpm-on-hvf.md`; R1 and R2 clear the ADR test (hard to
reverse once goldens are built against a guest-visible TPM; `ppi=off` reads as
a removable oddity without context; real rejected alternatives) and go to one
ADR. R3 does not (no real trade-off; it is recorded where it lands). The ADR
takes the repository's numbered `NNNN-slug.md` form with a Date and Status,
which every existing ADR (including the recent `hidpi-vision` grove's) uses,
rather than Grove's slug-only default. No spec: the seams were agreed in
`plan-k1` D5 and nothing new needs sign-off before decomposition. The term
*TPM decision* goes into `CONTEXT.md`.

**R6 — one recipe kept: `task vm:qemu-tpm-check`.** The paused-guest check
(`scripts/qemu-tpm-check.sh`, shellcheck and `shfmt -i 2 -ci` clean) runs the
TPM device as ADR-0017 attaches it and, for information, with QEMU's default
PPI. It is how anyone tells, in seconds after a QEMU upgrade, whether
ADR-0017's revisit condition holds. Observed: Homebrew 11.1.2, PASS with
`ppi=off` and INFO "aborts, as ADR-0017 expects" with the original
`hvf-all.c:123` error, exit 0; the local 11.0.1 build, PASS with no `ppi`
(attached as is), exit 0; `QEMU=qemu-system-x86_64`, FAIL "no
tpm-tis-device", exit 1 (which also observes F7's x86_64 claim directly).
The investigation shim, the local QEMU build and the Hypervisor.framework
probe stay out of the repository: they were one-off instruments, and the
research note records how to rebuild them.

**Done-when check.** Questions 1 to 5: F1, F2, F3, F4 to F6, F7, each with
commands, output and QEMU 11.1.2 source citations. The remedy: R1 and R2 in
ADR-0017 (cited both ways with `docs/research/windows-tpm-on-hvf.md`); R3, the
second blocker, in the planning leaf. Upstream: the QEMU draft in Notes; no
Apple Feedback (R4). The `planning` leaf `windows-remedy-k4` follows this one
and carries the remedy, D1, D4's now-triggered live build (with the human's
approval before the golden swap) and the root brief's seams. No in-session
reviewer was spent and no `review-design` leaf was cut: the central claims
were observed live three times and in controlled paused-guest runs, and the
one judgement call (R2's refusal on a missing swtpm) is the human's to weigh.
