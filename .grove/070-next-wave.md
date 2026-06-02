# 070-next-wave

**Kind:** planning

## Goal

Materialize the **next (and likely final) wave** of the port grove. The first
waves (agent parity, doctor, screen find-text, server-retire, show-menu, macOS
Vision OCR, RFB encodings, live-VM gate, egui viewer) are all built & retired.
This planning task grills the remaining root-brief arc into ordered work
leaves/nodes and decides their sequencing. The grove's "Done when" (full
contract parity + distribution + `cli/` deleted) is reached when this wave's
leaves are all retired.

## Context

Verified against `cli-rs` HEAD (2026-06-02), the genuinely-pending arc items
(code-confirmed, not just stale checkboxes) are:

1. **`screen record`** — still `unimplemented!("screen record")` in `main.rs`
   (canonical + `record` alias). Root brief mandates **embedded libav
   (`ffmpeg-next`), not a subprocess.** Largest single unknown: codec/container
   choice, the `screen-record` schema already declared in `surface.rs`, and how
   it drives the long-lived RFB stream (cf. the viewer — another continuous
   consumer; reconcile against ADR-0004 short-lived-vs-long-lived).
2. **Windows-host support** — `backlog task 14`, stubbed in
   `qemu_profile.rs` / `process.rs` (usable defaults, not real support). The
   cross-platform pass: process spawning, paths, and the `#[cfg]` facility
   seams already established for OCR.
3. **`vm create-golden --platform <p>` subcommand** — not in `surface.rs`.
   Retires the external `vm-create-golden-*.sh` scripts. NOTE: `scripts/` shows
   **no golden scripts present now** — reconcile where golden creation currently
   lives (were they already removed? is it manual?) before porting. See memory
   [[project_golden_creation_in_cli]].
4. **Distribution** — Homebrew (macOS + Linux) + Windows zip. Releases run
   **locally from `scripts/` on an arm64 Mac, no CI** (memory
   [[local_release_no_ci]]). Must revisit the `eframe`/`wgpu` binary-size &
   cross-compile cost flagged by ADR-0005. Linux cross-check via `zig cc`
   (memory [[reference_linux_crosscheck_zig]]).
5. **Final parity verification → delete `cli/`** (100 Swift files still present)
   + de-transition `CONTEXT.md` (drop the "in transition" framing). The grove's
   terminal step; gated on the `cli-contract.rs` full surface passing and a
   parity sweep.

Carried-over follow-up (not a wave item, but must not be lost): **leaf 030
viewer live GUI verification** on the macOS host (window opens; bounce VM
reconnects; `vm start --viewer`). Promoted into the root brief.

## Done when

- Each pending item above is either a materialized work leaf/node with a clear
  brief, or an explicit decision to defer/drop it (recorded in the root brief).
- Sequencing is decided. Recommended ordering to grill against:
  **(a)** `screen record` (last contract-parity stub; closes "no
  `unimplemented()`"), **(b)** `vm create-golden` (depends on golden-layout
  reconciliation), **(c)** Windows-host pass, **(d)** distribution (depends on
  everything compiling cross-platform), **(e)** final parity + delete `cli/`
  (strictly last). Grill whether Windows-host and distribution interleave.
- Open scope questions resolved with the user: is `screen record`'s embedded-
  libav still required, or is that decision revisitable? Is Windows-host in
  scope for *this* grove or a sibling? Does `vm create-golden` cover all three
  platforms or just the ones with kept-built goldens?
- The tree is grown (leaves/nodes added via `grove-llm leaf-add`/`leaf-insert`);
  ADRs raised only where a decision is hard-to-reverse/surprising (e.g.
  `screen record` codec/container; distribution packaging).

## Decisions (running log)

**Q1 — Windows-host scope: KEEP IN THIS GROVE.** The root brief already
committed Windows-host to full-retirement scope; the user confirmed that holds
even after the parity reframing below. Reframing recorded so it isn't lost:
the Swift CLI is **macOS-host-only** (`cli/Package.swift`:
`platforms: [.macOS(.v14)]`); every "Windows" reference in `cli/` is a Windows
*guest* profile, not a Windows *host*. So Windows-host is **net-new capability
beyond parity**, and it is **unverifiable in this environment** (no Windows
host, no kept-built Windows goldens — the live-VM gate is macOS/tart only).
Consequence: the grove's "done" for this item is **compiles cross-platform +
best-effort smoke**, not live-VM verified; live Windows-host verification is an
explicit known gap to record, not a blocker. The `#[cfg]` facility seams
(OCR, `qemu_profile.rs`, `process.rs`) are the established hand-off points.

**Q2 — `screen record` encoder strategy: PER-PLATFORM SEAM.** A `VideoEncoder`
seam mirroring `OcrEngine`: native **AVFoundation/VideoToolbox on macOS via
objc2** (true parity — Swift used `AVAssetWriter`, hardware-accelerated, and
keeps ffmpeg *out* of the primary locally-built macOS bundle), **ffmpeg-next on
Linux/Windows**. Build the macOS-native encoder **first** (only verifiable
target); the Linux/Windows encoder couples to the Windows-host pass. This
**revises the root brief's "embedded libav (ffmpeg-next), not a subprocess"
line** — that was never parity (Swift = AVFoundation) and conflicts with the
established conditional-facilities direction. Two discoveries grounding this:
(a) Swift's recorder encodes via `AVAssetWriter`
(`cli/Sources/TestAnywareDriver/Capture/StreamingCapture.swift`); (b) it ran
**inside the Shared-VNC `_server`** that ADR-0004 deletes — so the Rust record
must **own the RFB stream itself**, becoming the **second long-lived RFB
consumer** after the viewer (reuse ADR-0005's dedicated-RFB-thread pattern,
bounded by `--duration`, non-interactive). **→ ADR to raise** (codec/container +
per-platform encoder dependency strategy; cite ADR-0002/0003 for the seam
precedent and ADR-0004/0005 for the RFB-lifecycle reconciliation).

**Q3 — `vm create-golden` mechanism: FULL RUST PORT** (user overrode my
façade-over-scripts rec; recorded faithfully, cost accepted: it is ~the
largest item in the wave and brittle around boot-timing). The three external
`vm-create-golden-{macos,linux,windows}.sh` scripts are **reimplemented in Rust
and deleted**, satisfying [[golden-creation-in-cli]] maximally. Structural
consequences (execution refinement, not relitigation):
- Large enough to be a **node decomposed per-platform**, not one leaf.
- Builds on existing `testanyware-vm` infra: `tart.rs` (tart runner from the
  retired `050-live-vm-gate`), `qemu.rs`, `lifecycle.rs`, `health.rs`,
  `process.rs`.
- **Net-new work** = SSH provisioning orchestration + boot-wait/recovery-mode
  sequencing + (macOS) SIP cycle & TCC `sqlite` grants. No ssh/provisioning
  helper exists in `cli-rs` yet — needs an ssh approach (ssh2 crate vs
  `Command::new("ssh")`); decide in the macOS leaf.
- **macOS/tart is the only verifiable target** here (kept-built tart goldens);
  linux/windows golden ports are compile + best-effort, same gap as Windows-host
  (Q1). Sequence macOS golden first.
- Must satisfy the CLI design contract (schema `vm-create-golden`? not yet in
  `surface.rs`; add it), `--platform <p>`, `--json`, `--dry-run`.

**Q4 — Distribution build strategy: CROSS-COMPILE via `zig cc`, PROVEN OUT BY AN
EARLY SPIKE.** User wants to prove the single-Mac cross-build path
(memory [[reference_linux_crosscheck_zig]]) rather than build-on-target. Because
feasibility is uncertain and load-bearing, front-load a **cross-compile
feasibility spike leaf** before the full distribution design:
- The spike can run **now, on current HEAD**, which already links the two
  hardest native deps — `wgpu` (viewer, ADR-0005) and `ring` (known to break
  `cargo check` cross-builds). A successful linux + windows *release link* of
  today's binary is the strong signal; `ffmpeg-next` (Q2) is added to the spike
  once the encoder lands.
- **Fail-fast value:** if the spike disproves the zig cc path, fall back to
  **build-on-target via VMs** (TestAnyware's own goldens as build hosts) —
  documented fallback, not the plan of record.
- macOS arm64 distribution (Homebrew formula) is native & easy regardless;
  ships as its own (Tier-1) leaf. Current `release-build.sh` ships a
  **macOS-only Swift host CLI** — Linux/Windows host binaries are net-new.

**Q5 — Sequencing & delete-cli/ timing: DELETE cli/ AFTER macOS PARITY
(mid-grove).** Key insight: `cli/` is **macOS-only Swift**, so "delete cli/"
needs only **macOS parity**, not the Linux/Windows additive work. The wave
splits into two tiers:
- **Tier 1 (macOS, verifiable, = parity → delete cli/):** screen-record
  macOS-native encoder → `vm create-golden` macOS → macOS Homebrew
  distribution → `cli-contract.rs` full macOS surface passes → **delete cli/ +
  de-transition CONTEXT.md**.
- **Tier 2 (Linux/Windows, beyond-parity, unverifiable here):** Windows-host
  pass, ffmpeg-next encoders, linux/win golden, cross-compile distribution —
  proceeds on the clean post-cli/ tree.
- Plus the **cross-compile feasibility spike** front-loaded to fail-fast
  (Q4), and the carried-over **viewer live-GUI verification** (root brief).

### Resulting tree (this planning task grows)

- `080-crosscompile-spike` — de-risk zig cc cross-build on current HEAD
  (wgpu+ring) → linux+windows. Fail-fast for Tier-2 distribution.
- `090-viewer-live-verify` — carried-over macOS-host GUI check (window opens;
  bounce reconnects; `vm start --viewer`).
- `100-screen-record-encoder-macos` — `VideoEncoder` seam +
  AVFoundation/VideoToolbox via objc2 + RFB long-lived driving. → **ADR-0006**.
- `110-vm-create-golden-macos` — surface spec + ssh provisioning + macOS golden
  port (full Rust).
- `120-macos-distribution` — Rust Homebrew arm64 formula; revise `scripts/`.
- `130-macos-parity-and-delete-cli` — Tier-1 terminal: contract sweep, delete
  `cli/`, de-transition `CONTEXT.md`.
- `140-tier2-plan` — planning leaf: re-grill & decompose Tier 2 (shaped by spike
  outcome + macOS encoder/golden seams). Lazy; Tier 2 stays in root checklist
  until then.

ADR raised: **ADR-0006** (screen record per-platform encoder strategy). No PRD
(decisions captured here + root brief + ADR; a PRD would duplicate).

## Notes

- This is a **planning** task: open with grilling (`grilling.md`), one question
  at a time, recommended answer per question; update `CONTEXT.md` inline as
  terms resolve; write a PRD only at a genuine agreement point.
- The acceptance gate for every resulting work leaf is the **CLI design
  contract** (`docs/architecture/cli-design-contract.md`).
- Best done in a **fresh session** (`grove do port-swift-cli-to-rust`) for a
  clean grilling context — this leaf is the durable agenda either way.
