# port-swift-cli-to-rust — brief

## Goal

Complete the replacement of the legacy **Swift CLI** (`cli/`) with the
**Rust CLI** (`cli-rs/`) and **delete `cli/`**. The grove finishes when every
command in the canonical surface satisfies the **CLI design contract**
(`docs/architecture/cli-design-contract.md`) at parity, the platform/distribution
work below is done, and the Swift tree is removed from the repo.

Scope decision (this grove): **full retirement** — functional parity *plus*
distribution, Windows-host support, the tart/macOS-guest runner, and the
golden-image subcommand are all in scope, not split into sibling groves.

## Done when

- Every command in `cli-rs/.../surface.rs::CANONICAL_COMMANDS` is implemented
  (no `unimplemented()` stubs) and satisfies the contract: stable error codes,
  `--json` for data-producing commands, `--dry-run` for mutating commands,
  help-text template, schema discovery.
- The `cli-contract.rs` integration test passes for the full surface.
- Platform/distribution backlog (below) is complete.
- `cli/` is deleted; `CONTEXT.md` is updated to drop the "in transition" framing.

## Decomposition

Leaves are clustered by **shared capability dependency**, not by noun, and
ordered lazily — only the first wave is materialized (grove constraint 4). The
root checklist below is the full arc; downstream items become leaves when their
turn comes.

First wave (materialized now):
- `010-agent-action-parity` — the 8 pure-HTTP agent action stubs.
- `020-port-doctor` — `testanyware doctor` preflight checks.
- `030-screen-find-text` — OCR-backed `screen find-text`.

Second (final) wave — planned by `070-next-wave`, materialized as `080`–`140`.
**Two-tier structure** (key insight: `cli/` is macOS-only Swift, so deleting it
needs only **macOS parity**, not the Linux/Windows additive capability):

- **Tier 1 — macOS parity → delete `cli/`:** `100-screen-record-encoder-macos`,
  `110-vm-create-golden-macos`, `120-macos-distribution`,
  `130-macos-parity-and-delete-cli`. Reaching these + `cli-contract.rs` passing
  on macOS *is* the parity bar; `cli/` is deleted here, mid-grove.
- **Tier 2 — Linux/Windows additive (beyond-parity, unverifiable in this env):**
  Windows-host pass, `ffmpeg-next` encoders, linux/win golden, cross-compile
  distribution. Re-grilled & decomposed lazily by `140-tier2-plan` after Tier 1.
- **Cross-cutting:** `080-crosscompile-spike` (front-loaded fail-fast for the
  zig-cc cross-build path) and `090-viewer-live-verify` (carried-over follow-up).

## Pointers

- ADRs a session here must read: `docs/adr/0001-streaming-file-transfer.md`.
- Contract: `docs/architecture/cli-design-contract.md` (the "CLI design contract"
  glossary term) — the acceptance gate for every command leaf.
- Canonical surface: `cli-rs/crates/testanyware-cli/src/surface.rs`.
- Glossary terms in play: Host CLI, Swift CLI, Rust CLI, Command surface,
  CLI design contract, In-VM agent, Golden image (see `CONTEXT.md`).
- Per-platform-facilities direction: use the best native facility per platform
  via `#[cfg(target_os = ...)]` (macOS Apple Vision OCR; EasyOCR/other on
  Linux/Windows). Reverses the old "EasyOCR everywhere" decision — reconcile
  `git show a062072:LLM_STATE/core/decisions.md` framing when it surfaces.
- Old backlog task descriptions (a stale snapshot, descriptions only — NOT
  status): `git show a062072:LLM_STATE/core/backlog.yaml`.

## Remaining-work checklist (verified against `cli-rs/` HEAD, 2026-05-30)

**Done** (wired in the Rust CLI; not stubs):
- `vm start/stop/list/delete` (QEMU lifecycle).
- `agent health/snapshot/inspect/windows/press`.
- all `input *` (key, type, click, drag, scroll, …).
- `screen capture` + `screen size` (RFB: handshake, Raw, CopyRect).
- all `file *` (exec, upload, download — incl. ADR-0001 streaming).
- `capabilities`, `schema`, `llm-instructions`.

**Command-parity gaps** (currently `unimplemented()` stubs):
- [ ] `agent` HTTP actions: `set-value`, `focus`, `wait`,
      `window-{focus,resize,move,close,minimize}` → **010** (pure HTTP, no VNC).
- [x] `doctor` → **020**.
- [x] `screen find-text` (OCR) → **030** (daemon path; per-platform `OcrEngine`
      seam + ADR-0002; native macOS Vision deferred to **040**).
- [ ] `agent show-menu` → **030**. NOT blocked on a missing VNC-input layer
      (that layer already exists: `testanyware-rfb::input` powers `input *`).
      Real work is porting Swift's `MenuBarLocator` orchestration over the
      existing RFB `click()` + agent snapshot.
- [ ] `screen record` — **per-platform `VideoEncoder` seam (ADR-0006):** native
      AVFoundation/VideoToolbox via objc2 on macOS (parity; macOS first →
      `100`), `ffmpeg-next` on Linux/Windows (Tier 2). Becomes the **second
      long-lived RFB consumer** (reuses ADR-0005's pattern, bounded by
      `--duration`, non-interactive). Revises the root "embedded libav
      everywhere" line.
- [ ] `server` → **020 (retire, not port)**. The `server` stub is the Rust
      shadow of the Swift `_server` **Shared-VNC server** (a persistent VNC
      multiplexer), which the Rust CLI deliberately drops — NOT the OCR daemon.
      Earlier framing conflated the two; they are structurally distinct (see
      CONTEXT.md *Shared-VNC server* vs *OCR daemon*). `OcrChildBridge` stays.

**Platform / facilities** (not started; materialize as leaves later):
- [ ] Native macOS **Apple Vision** OCR engine at the `OcrEngine` seam → **040**
      (ADR-0002; FFI strategy — objc2 vs Swift shim — decided in that leaf).
- [x] VNC viewer with `egui` to replace the AppleScript launcher (`--viewer`).
      Node `060-egui-viewer` (ADR-0005), leaves `010-render-loop`,
      `020-input-forwarding`, `030-reconnect-and-start-sugar` — all built &
      committed; node retired. **Live macOS-host GUI verification of leaf 030
      (window opens; bounce stop/start reconnects; `vm start --viewer`) is the
      one pending follow-up** — record it once done (cf. leaf 020's verify log
      in `done/060-egui-viewer/BRIEF.md`).
- [x] ZRLE + Tight encodings for the RFB client crate (node 040; live
      verification rolls up into leaf 050).
- [ ] tart runner for the macOS-host-macOS-guest path (`#[cfg(target_os=macos)]`).
      **Pulled into the `050-live-vm-gate` node** as leaf `010-tart-runner` — the
      gate needs it to reach the cheap kept-built tart goldens. Owned there now.
- [ ] Windows-host support (cross-platform pass). **Decision (070): IN SCOPE,
      Tier 2.** Reframing for the record — the Swift CLI is **macOS-host-only**
      (`cli/Package.swift`: `platforms: [.macOS(.v14)]`), so this is **net-new
      capability beyond parity** and **unverifiable in this env** (no Windows
      host, no kept-built Windows goldens). "Done" here = compiles cross-platform
      + best-effort smoke; live Windows-host verification is a recorded known gap.
- [x] Live-VM verification gate for the RFB client + input layer (node
      `050-live-vm-gate`: `tests/live-vm-gate.rs` — input landing, show-menu,
      ZRLE/Tight/Raw capture, live Vision OCR; macOS golden, env+`#[ignore]`d).

**Distribution / finish**:
- [ ] Golden-image creation as a `vm create-golden --platform <p>` subcommand,
      retiring the external `vm-create-golden-*.sh` scripts. **Decision (070):
      FULL RUST PORT** (not a façade over the scripts — user override; scripts
      deleted). Builds on existing `testanyware-vm` (`tart.rs`, `qemu.rs`,
      `lifecycle.rs`); net-new = ssh provisioning + multi-boot/recovery + (macOS)
      SIP cycle & TCC grants. macOS first (`110`, verifiable); linux/win Tier 2.
      The `vm-create-golden` schema is **not yet** in `surface.rs` — add it.
- [ ] Distribute Rust `testanyware` via Homebrew (macOS + Linux) and Windows zip.
      Releases run locally from `scripts/` on an arm64 Mac — no CI. **Decision
      (070): CROSS-COMPILE via `zig cc`**, proven by an early **feasibility spike**
      (`080`, runs on current HEAD: wgpu+ring → linux+windows). Fallback if the
      spike fails: **build-on-target via VMs**. macOS Homebrew arm64 is native &
      ships in Tier 1 (`120`); linux/win distribution is Tier 2.
- [ ] Final parity verification → **delete `cli/`** and de-transition `CONTEXT.md`.
      **Decision (070): after macOS PARITY (mid-grove, `130`)** — `cli/` is
      macOS-only, so Tier-1 completion is the parity bar; Tier-2 additive work
      then proceeds on the clean tree.

## Notes

The retired `LLM_STATE/core/backlog.yaml` numbers tasks (e.g. "backlog task 8" =
viewer, "12" = tart, "14" = Windows host); those numbers still appear in code
comments. They are descriptions only — current status lives in the code, per
grove constraint 1 (artifacts, not state).
