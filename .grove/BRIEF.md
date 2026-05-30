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
- [ ] `agent show-menu` — blocked on the VNC-input layer (opens menu items via
      VNC click); pairs with the VNC work, not with 010.
- [ ] `screen record` — needs embedded libav (`ffmpeg-next`), not a subprocess.
- [ ] `server` — the hidden `_server` internal command (OCR bridge; the
      `OcrChildBridge` daemon pattern is scaffold for the vision pipeline, not
      residue — keep it).

**Platform / facilities** (not started; materialize as leaves later):
- [ ] Native macOS **Apple Vision** OCR engine at the `OcrEngine` seam → **040**
      (ADR-0002; FFI strategy — objc2 vs Swift shim — decided in that leaf).
- [ ] VNC viewer with `egui` to replace the AppleScript launcher (`--viewer`).
- [ ] ZRLE + Tight encodings for the RFB client crate.
- [ ] tart runner for the macOS-host-macOS-guest path (`#[cfg(target_os=macos)]`).
- [ ] Windows-host support (cross-platform pass).
- [ ] Live-VM verification gate for the RFB client + input layer.

**Distribution / finish**:
- [ ] Golden-image creation as a `vm create-golden --platform <p>` subcommand,
      retiring the external `vm-create-golden-*.sh` scripts.
- [ ] Distribute Rust `testanyware` via Homebrew (macOS + Linux) and Windows zip.
      Releases run locally from `scripts/` on an arm64 Mac — no CI.
- [ ] Final parity verification → **delete `cli/`** and de-transition `CONTEXT.md`.

## Notes

The retired `LLM_STATE/core/backlog.yaml` numbers tasks (e.g. "backlog task 8" =
viewer, "12" = tart, "14" = Windows host); those numbers still appear in code
comments. They are descriptions only — current status lives in the code, per
grove constraint 1 (artifacts, not state).
