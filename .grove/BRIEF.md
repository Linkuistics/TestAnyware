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
- **Tier 2 — Linux-host + Windows-host additive (beyond-parity, but
  self-verifiable):** **decomposed by `140-tier2-plan` (2026-06-04, ADR-0009).**
  **TestAnyware tests its own host CLI by running it inside native-arch
  (aarch64) host-VMs** that drive a real tart macOS golden's agent/RFB endpoint
  through a **host port-forward** (guest→host-gateway is the reliable NAT edge).
  Cross-compiled locally via **`cargo-zigbuild`** (supersedes the hand-rolled
  `zcc`; `080` proved it). **Matrix = four triples** — `x86_64`/`aarch64` ×
  `linux-gnu`/`windows-{gnu,gnullvm}`; **aarch64 is first-class** (the only arch
  the harness natively verifies on this Mac), x86_64 is build/link-verified with
  the runtime gap logged. Sequencing: **Linux leads** (self-contained: stock
  Ubuntu ARM64 + ssh), **Windows trails** (depends on the Windows golden + a
  working Windows in-VM agent, since Windows ships no SSH). Distribution per OS
  **trails that OS's host-pass + harness** — never ship a binary the harness has
  not run green.
  - **First wave (DONE, all retired):** `160-crossbuild-matrix-spike` (fail-fast
    all 4 triples + `ffmpeg-next` link risk), `170-ffmpeg-video-encoder` (the
    non-macOS `VideoEncoder` arm, ADR-0006), `180-linux-host-pass` (cfg/paths/
    facility wiring), `190-linux-verification-harness` (ADR-0009, Linux-first) —
    **Linux aarch64 host CLI is now runtime-GREEN** (all three bands, incl.
    EasyOCR `screen find-text`; see the Tier-2 checklist line below).
  - **Deferred (materialize when their turn comes):** Windows-host pass
    (`monitor.rs` AF_UNIX→named-pipe/TCP + the already-`#[cfg]`-paired
    `process/spec/detached/doctor`), Windows verification harness — **reuses
    `190`'s machinery verbatim**: the in-process host→golden TCP forward,
    host-gateway discovery, the band-agnostic `run_band` driver, the
    `--agent`/`--vnc` endpoint targeting, *and* the `ocr_analyzer` EasyOCR daemon
    + venv recipe (`vision/stages/text-ocr`); **only the `ProvisionChannel` trait
    gets a 2nd impl** (ssh → in-VM agent `file upload`/`exec`, since Windows
    ships no sshd) plus a Windows HUT image. **linux/win distribution**
    (`cargo-zigbuild` per triple, Homebrew Linux + Windows zip — `080` sketched
    `scripts/`; **Linux distribution DONE (`210`, 2026-06-04)** — `cargo-zigbuild`
    both triples + a Homebrew formula bundling the ffmpeg-8 `.so`s (RUNPATH
    `$ORIGIN/../lib`) and building the `ocr_analyzer` EasyOCR venv at
    `<prefix>/libexec/venv`; aarch64 runtime-verified, x86_64 build-only. The
    **Windows distribution DONE (`220/050`, 2026-06-08)** reused this machinery —
    a `.zip` per Windows triple, ffmpeg DLLs co-located beside the `.exe`, no OCR
    venv; aarch64 zip runtime-smoked in-guest, x86_64 build-only), **linux/win
    `vm create-golden`** (full Rust port reusing `110`'s russh layer; macOS-host
    work, no cross binary needed). **Planning leaf `200` decomposes this wave.**
- **Cross-cutting:** `080-crosscompile-spike` (front-loaded fail-fast for the
  zig-cc cross-build path) and `090-viewer-live-verify` (carried-over follow-up).

## Pointers

- ADRs a session here must read: `docs/adr/0001-streaming-file-transfer.md`;
  for Tier-2, `docs/adr/0009-self-hosted-host-cli-verification-harness.md`.
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
- [ ] **Linux-host support** (cross-platform pass). **Decision (070+): IN SCOPE,
      Tier 2.** Net-new beyond parity (Swift was macOS-only), but **lighter than
      Windows-host** — `process.rs`/`qemu_profile.rs` already carry the *Unix*
      path; Linux mainly needs paths + `#[cfg]` facility wiring + the EasyOCR /
      ffmpeg-next / wgpu-on-Vulkan facilities (already anticipated by
      ADR-0002/0005/0006). **Verified** by running up a Linux host-VM with
      TestAnyware and smoke-testing the cross-compiled binary (see below).
- [ ] Windows-host support (cross-platform pass). **Decision (070): IN SCOPE,
      Tier 2.** The Swift CLI is **macOS-host-only** (`cli/Package.swift`:
      `platforms: [.macOS(.v14)]`), so this is **net-new beyond parity**
      ("backlog task 14" stubs in `qemu_profile.rs`/`process.rs`). **Verified**
      by running up a Windows host-VM with TestAnyware (not "unverifiable" — that
      earlier framing is superseded).
- [x] **Parallels Desktop backend** — **investigated, not adopted** (`150`,
      2026-06-04, ADR-0010). Rejected on the gating question: Parallels Desktop
      for Mac exposes **no host-side framebuffer** (its `--vnc-*` flags are
      Cloud Server / Virtuozzo, a different product), so it cannot serve the
      pre-boot/recovery RFB the stack and golden creation (ADR-0008) depend on.
      Lifecycle/golden mechanics mapped cleanly and Windows-on-ARM is a real
      win, but both are moot without a framebuffer. Durable output is the
      **host-side-framebuffer invariant** (ADR-0010, new [[CONTEXT.md]] term),
      the criterion future candidates (VMware Fusion, UTM) are judged against.
      Findings: `docs/research/parallels-backend-feasibility.md`.
- [x] **Self-hosted host verification harness — Linux aarch64 GREEN** (Tier 2,
      node `190`, ADR-0009). `cli-rs/.../tests/linux-host-harness.rs` clones a
      stock Ubuntu ARM64 HUT, ssh-provisions the cross-compiled binary, forwards
      a real macOS golden's agent+VNC through the host, and runs all three bands
      green: endpoint-free (caps/schema/doctor/…), endpoint-driven (agent HTTP,
      `input *`, `screen capture`/`size`/`record`→mp4 — ffmpeg-8 libx264 runtime-
      proven), and **OCR** (`screen find-text` via the EasyOCR daemon,
      `engine=easyocr_daemon`). **x86_64-linux is BUILD-verified only** (no native
      x86_64 guest on this Mac; gap logged in the harness doc-comment, ADR-0009
      no-silent-caps).
- [x] **Self-hosted host verification harness — Windows aarch64 2/3 bands GREEN**
      (Tier 2, node `220/040`, 2026-06-05). `cli-rs/.../tests/windows-host-harness.rs`
      (standalone; **duplicated** 190's machinery, not extracted) boots the Windows
      agent-golden as a QEMU+swtpm HUT, agent-provisions the cross binary + ffmpeg
      DLLs (the `ProvisionChannel` 2nd impl: in-VM agent `/exec`+`/upload`+`/download`,
      no sshd), forwards a macOS golden's agent+VNC via the slirp gateway (10.0.2.2),
      and runs **endpoint-free (6/6) + endpoint-driven (10/10, incl. `screen record`
      → ffmpeg-8 libx264 MP4)** GREEN on aarch64-windows. **x86_64-windows
      build/link-verified only** (gap logged). **OCR band deferred** — EasyOCR is
      uninstallable on win-arm64 (opencv-python-headless has no `win_arm64` wheel),
      the low-regret kill signal that hoisted `215` (docker host unification, was
      `240`). **`215` REPORTED REJECT (2026-06-07,
      `docs/research/240-docker-host-unification.md`):** containerizing the whole
      host binary fails the host-side-framebuffer gate (ADR-0010) on macOS/Windows
      and *adds* native surface on Windows — ship the native cross-compiled Windows
      binary, do not replace it. The spike's narrow payoff: OCR is host-side compute
      *downstream* of framebuffer capture (no hypervisor dep), so only the **OCR
      engine** can be containerized (Linux EasyOCR container, gate-irrelevant).
      **`220/050` is UNGATED** (ships OCR-less 2/3-green surface); the Windows OCR
      band — containerized Linux EasyOCR vs native `Windows.Media.Ocr` vs
      accept-the-gap, at the ADR-0002 seam — is decided in new leaf
      **`220/060-windows-ocr-band`**.
- [x] Live-VM verification gate for the RFB client + input layer (node
      `050-live-vm-gate`: `tests/live-vm-gate.rs` — input landing, show-menu,
      ZRLE/Tight/Raw capture, live Vision OCR; macOS golden, env+`#[ignore]`d).

**Distribution / finish**:
- [x] **macOS** golden-image creation as `vm create-golden --platform macos`
      (node `110`, ADR-0007 ssh-via-russh + ADR-0008 recovery-over-RFB/OCR).
      FULL RUST PORT (user override) — boot-1 SSH provisioning, the SIP/TCC
      recovery cycle, agent-health gate, clean shutdown + `tart clone`. The
      `vm-create-golden` schema is in `surface.rs`; the external
      `vm-create-golden-macos.sh` is **deleted**. **Live-verified 2026-06-03**
      (golden produced first-try; fresh clone reachable + accessibility granted).
      **linux/win golden remains Tier 2** (`140-tier2-plan`), built on this same
      `testanyware-vm` foundation.
- [x] Distribute Rust `testanyware` via Homebrew (macOS + Linux) and Windows zip.
      Releases run locally from `scripts/` on an arm64 Mac — no CI. **Decision
      (070): CROSS-COMPILE via `cargo-zigbuild`** (supersedes hand-rolled `zcc`),
      proven by `080`/`160`/`170`. **macOS** Homebrew arm64 ships in Tier 1 (`120`).
      **Linux DONE (`210`, 2026-06-04):** `cargo-zigbuild` both `*-unknown-linux-gnu`
      triples; the Homebrew formula installs the BtbN ffmpeg-8 `.so` bundle into the
      keg `lib/` (binary linked RUNPATH=`$ORIGIN/../lib` + all five sonames forced
      *direct* NEEDED, so it self-locates with no `LD_LIBRARY_PATH`) and builds the
      `ocr_analyzer` EasyOCR venv (pinned `easyocr` resource) at
      `<prefix>/libexec/venv`. **aarch64-linux runtime-verified** — install-layout
      `screen find-text` green with no env crutches (`linux_dist_install_layout`
      harness test); **x86_64-linux build/link-verified ONLY** (no native guest,
      ADR-0009 no-silent-caps). **Windows DONE (`220/050`, 2026-06-08):**
      `cargo-zigbuild` both Windows triples (`aarch64-pc-windows-gnullvm` first-class,
      `x86_64-pc-windows-gnu` build/link-verified only) → a **`.zip` per triple** (no
      Homebrew on Windows). The zip co-locates the five BtbN ffmpeg-8 DLLs
      *beside* `testanyware.exe` in `bin/` (PE image-directory search, the Windows
      analogue of the Linux RUNPATH trick) and ships **NO OCR** (EasyOCR
      uninstallable on win-arm64 — `screen find-text` an unsupported documented gap
      until `220/060`). `scripts/release-{build,doctor,publish}.sh` extended;
      `release-publish.sh` uploads the zips as GitHub-release assets.
      **aarch64-windows runtime-verified** — the shipped zip unzips into a clean
      in-guest prefix and runs all 6 endpoint-free contract checks green
      (`windows_dist_zip_smoke`, the pre-publish real-artifact gate);
      **x86_64-windows build/link-verified ONLY** (no native x86_64 Windows guest,
      ADR-0009 no-silent-caps).
- [x] Final parity verification → **delete `cli/`** and de-transition `CONTEXT.md`.
      **DONE 2026-06-03 (node `130`).** macOS parity proven — `cli-contract.rs`
      green on the full offline surface (22 passed / 0 failed; the 4 remaining
      ignores are correctly live-VM-gated, not unfinished ports). `cli/` deleted
      (106 files, incl. the Swift `_server` tree — ADR-0004). `CONTEXT.md`
      de-transitioned; Swift-referencing docs (README, docs/, website/,
      provisioner, cli-design-contract §10) rewritten or re-framed as historical.
      Tier-2 additive work (`140`) now proceeds on the clean tree.

## Notes

The retired `LLM_STATE/core/backlog.yaml` numbers tasks (e.g. "backlog task 8" =
viewer, "12" = tart, "14" = Windows host); those numbers still appear in code
comments. They are descriptions only — current status lives in the code, per
grove constraint 1 (artifacts, not state).
