# 8. Golden recovery automation is an in-process observe/act/verify sequence over RFB + OCR, not a blind-sleep shell-out

Date: 2026-06-03

## Status

Accepted

## Context

`provisioner/scripts/vm-create-golden-macos.sh` toggles SIP by booting the setup
VM into macOS Recovery and driving its GUI: navigate the startup picker, open
Terminal via the Utilities menu, run `csrutil disable`/`enable`, and answer the
`y` / username / password prompts. Recovery has no SSH, so this is the one part
of golden creation that cannot go over the `russh` provisioning layer
(ADR-0007) — it must drive the framebuffer directly.

The script's recovery driver (`_recovery_boot_csrutil`, lines 398–594) is a
straight-line sequence of `testanyware` *subprocess* calls separated by blind
fixed sleeps (`sleep 15/10/10/15` around the csrutil prompts; `sleep 0.3/2/5`
around navigation). The sleeps are the script's only synchronization: they
assume each screen transition completes within a guessed wall-clock budget. This
is fragile (a slow boot overruns the budget; a fast one wastes time) and
opaque (a failure leaves no signal about *which* step missed).

The grove node `110-vm-create-golden-macos` decided (node decision Q3, a user
override of the recommended faithful-port) to **re-engineer the recovery
micro-mechanism** while keeping the macro orchestration at parity: the 5-boot
sequence (3 normal + 2 recovery), the disable-SIP → grant-TCC → enable-SIP
order, and the agent-health gate are unchanged; only the in-recovery *how* moves
off blind sleeps. The Rust CLI already owns the parts needed to do this
in-process: `testanyware-rfb` (`RfbConnection`: framebuffer + key/pointer input)
and `testanyware-ocr-client` (`OcrEngine::recognize`, `find_text`) — and
`screen find-text` (`commands/screen.rs`) already composes them into a
poll-frame → OCR → match-on-deadline loop.

The open question (leaf `030` grilling) was how to synchronize the steps that
have no positive on-screen signal — specifically the `csrutil` prompts, where
password entry echoes nothing.

## Decision

**The recovery driver is an in-process, imperative observe/act/verify sequence
over the live RFB framebuffer + OCR.** A `RecoverySession` wraps an
`RfbConnection` + `OcrEngine` and exposes small primitives —
`wait_for_text(query, deadline) -> Located` (pump `request_framebuffer_update`
→ drain `next_message` → `encode_png` → `OcrEngine::recognize` → `find_text`,
retry with backoff), `act(input)` (RFB key/type/pointer), `settle(quiet,
deadline)` (wait until the framebuffer stops changing), and
`verify_transition(predicate, deadline)`. The recovery flow is a straight-line
script of these calls, mirroring the bash structure but with every blind `sleep`
replaced by a signal-driven wait.

It is **not** a generic data-driven state-machine engine: the recovery path is
fixed and linear (no branching, no cycles, no revisited states), so a declarative
FSM would add indirection the path never exercises. "State machine" describes the
*per-step discipline* (observe expected screen → act → verify transition → retry),
not a literal engine.

**Synchronization model:** OCR-the-prompt is primary wherever text is printed
(the startup-picker "Options", the recovery-desktop "Utilities"/"Terminal" menu
items, the csrutil "proceed? [y/n]" prompt, the admin name/password labels, and
the final `System Integrity Protection is …` result line). For the single
genuinely signal-less micro-gap — password entry is masked in the terminal —
`settle` (framebuffer quiesces for a bounded window) is the proxy, not a long
fixed sleep. The csrutil interaction's final step is `wait_for_text` on the
result line; on a miss, the whole interaction is **retried** (a robustness gain
the blind-sleep script lacks). The authoritative correctness gate remains the
existing post-reboot `csrutil status` check **over SSH** (script lines 683/717),
so the in-recovery waits need not be perfect.

`wait_for_text` and `settle` are kept as **independent primitives** (not one
fused call) so that which one is *primary* at a given step can flip during live
verification if OCR proves flaky on small recovery-Terminal monospace.

## Considered Options

- **Faithful transliteration of the blind sleeps** (port `sleep N` 1:1, still
  shelling out to `testanyware` subprocesses or calling the in-process input
  layer with the same fixed delays). Lowest effort, exact parity. Rejected by
  node decision Q3: it carries the fragility forward and gives no failure
  signal — the precise thing the re-engineering exists to remove.
- **Screen-settle as the sole synchronization** (ignore OCR; after each
  keystroke wait for the framebuffer to stabilize, then act). Simpler, no
  dependence on OCR-ing tiny terminal fonts. Rejected as the *primary* mechanism:
  settle can false-trigger on a blinking cursor and gives no semantic
  confirmation of *which* screen is showing, so a wrong-screen miss would go
  undetected until the SSH backstop. Retained as the fallback for the one
  signal-less gap.
- **A generic declarative FSM engine** (`Vec<Step>` with screen + transition
  predicates driven by a runtime). Rejected as over-engineering for a fixed,
  non-branching path (the grove "runaway tree" anti-pattern).

## Consequences

- The recovery driver becomes the codebase's third structured RFB consumer after
  the embedded viewer (ADR-0005) and `screen record` (ADR-0006). Unlike those
  long-lived stream consumers, it is a *bounded, interactive* one: it both reads
  the framebuffer and writes input, over a single short-lived connection per
  recovery boot.
- The frame-refresh + OCR snapshot step is **extracted from `screen find-text`**
  into a shared helper rather than duplicated, so both consumers share one
  capture pipeline.
- This is the **template for Tier-2 linux/win golden recovery flows**: any future
  guest-UI automation that cannot go over SSH reuses the `RecoverySession`
  primitives and the OCR-primary + settle-fallback + out-of-band-backstop model.
- Built and live-VM-iterated in grove leaves `030/010-recovery-driver` (the
  `RecoverySession` + `recovery_boot_csrutil`, fail-fast-verified by running
  `csrutil disable` and confirming `csrutil status` over SSH) and
  `030/020-tcc-and-finalize` (TCC grants, health gate, clean shutdown, clone).
- VNC keysym quirks (memory `cmd-key-tahoe`: Command = `XK_Alt_L`) apply on this
  RFB path, as they do for `input *`.
