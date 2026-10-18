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
