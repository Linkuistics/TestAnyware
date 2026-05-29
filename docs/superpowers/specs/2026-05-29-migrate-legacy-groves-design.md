# Migrate legacy workstreams into the grove system

/ Status: approved — seed-only execution /
/ Date: 2026-05-29 /

## Problem

Two pre-grove (or partially-grove) bodies of work still live in the repo and are
invisible to the grove tooling:

1. `groves/rust-cli-port/` — already in the *new* grove format (`BRIEF.md` +
   `010-audit-swift-surface.md` planning leaf) but committed on `main` under
   `groves/` instead of living in its own worktree at
   `.grove-worktrees/rust-cli-port/` on a `rust-cli-port` branch.
2. `LLM_STATE/` — an older YAML-based "session-state" system with three
   workstreams (`core`, `ocr-accuracy`, `vision-pipeline`), each carrying
   `backlog.yaml` (tasks), `memory.yaml` (learnings), `decisions.md`,
   `session-log.yaml`, `phase.md`, and prompt/baseline files.

Both formats predate the live grove convention (one grove = one worktree+branch,
task tree under `.grove/`, glossary in `CONTEXT.md`, decisions in `docs/adr/`).

## Disposition (decided 2026-05-29)

| Source | Disposition |
|---|---|
| `groves/rust-cli-port` | **Live grove** — relocate (history-preserving). |
| `LLM_STATE/core` | **Fold into `rust-cli-port`** + archive the rest. |
| `LLM_STATE/ocr-accuracy` | **Live grove** — translate. |
| `LLM_STATE/vision-pipeline` | **Live grove** — translate. |

`core` is the old-system backlog of `rust-cli-port`: its tasks are "port QEMU/VM
lifecycle" (already done & merged 2026-05-22), "port doctor", "port record",
"build VNC viewer + RFB encodings", "port tart runner", "distribute via
homebrew", "Windows-host pass", "build golden-creation into CLI", and "retire
Swift cli once at parity". That is exactly the scope the `rust-cli-port`
`010-audit-swift-surface` leaf is designed to enumerate.

### Calibration (decided 2026-05-29)

- **Fidelity: distill + cite git history.** BRIEF + live-task leaves + only the
  load-bearing learnings into `CONTEXT.md`/ADRs. The full research log
  (session-logs, every backlog line) stays recoverable in git history; distilled
  artifacts cite `git show <commit>:LLM_STATE/...`.
- **Scope: seed only.** Create the three worktrees+branches, build their
  `.grove/` trees, promote learnings, clean `main`. Each grove's actual work is
  driven later in its own session. `fix-agent-upload` stays paused.
- **Learnings: grove-local.** Each grove's learnings live in its branch
  (`CONTEXT.md` + ADRs), promoted to `main` when that grove finishes. One or two
  genuinely cross-grove facts also go to the operator's `~/.claude` auto-memory.

## End state

- Three live groves, each its own worktree + branch off `main`, each carrying
  only its `.grove/` tree (plus the repo code): `rust-cli-port`, `ocr-accuracy`,
  `vision-pipeline`.
- `core` dissolved into `rust-cli-port`.
- `main` cleaned: `groves/` and `LLM_STATE/` removed.
- `fix-agent-upload` grove untouched.

## Per-grove tree shape

### `rust-cli-port`

- `git mv groves/rust-cli-port/* .grove/` — preserves the existing `BRIEF.md`
  and `010-audit-swift-surface.md` with history.
- Fold `core`:
  - Distil core's **live port tasks** into the BRIEF's decomposition roadmap so
    the `010` audit consumes a known roadmap rather than rediscovering blind:
    doctor, record (embedded libav), VNC viewer (egui) + RFB client crate (+
    ZRLE/Tight encodings + live-VM verification gate), tart runner
    (`cfg(target_os macos)`), homebrew/zip distribution, Windows-host support
    pass, golden-image creation as a CLI subcommand, Swift `cli/` retirement.
  - Promote core's **process-control lore** from `memory.yaml` into the grove's
    `CONTEXT.md` + an ADR: Darwin pipe-EOF requires all write-FD holders to
    exit; Foundation `Process` does not `setsid`/`setpgrp`; take `pgrep -P`
    snapshot before killing bash; temp-file capture beats pipes;
    process-tree-kill after exec is best-effort. Flag that most of these are
    Foundation-specific and should be *re-validated*, not ported verbatim, in
    Rust (`tokio::process` + `nix` setsid on Unix / `CREATE_NEW_PROCESS_GROUP`
    on Windows).
  - Convert core's `decisions.md` into ADR(s) where a decision is still
    load-bearing.
  - core's "port-qemu" task is already done — recorded as done in the BRIEF, no
    leaf.

### `ocr-accuracy`

- New grove. BRIEF: goal = OCR-accuracy research/eval; current state = EasyOCR
  adopted as the default OCR engine (wins aggregate text-F1 +9–20pp over Apple
  Vision on every platform).
- Decompose the 24 live tasks into category nodes: evaluation re-measures;
  analyzer/engine (including the one **in-progress** task — long-lived OCR
  analyzer daemon for interactive `find-text`); ground-truth; generator/agent
  bug-fixes (Windows UIA oversized-bbox, windowsterminal focus bug, GTK4
  per-element position); infrastructure (canonical snapshots, batch rerun,
  connect.json helper).
- ADRs for the load-bearing verdicts: *EasyOCR as default OCR engine*;
  *center-distance matching replaces IoU for spatial evaluation*.
- OCR-specific glossary as a bounded context: introduce `CONTEXT-MAP.md` + an
  OCR glossary, since OCR terms (`by_app` bucket, center-distance matching,
  A/B snapshot, EasyOCR/Apple Vision/Tesseract, GT) differ from the host-CLI
  bounded context in the root `CONTEXT.md`.

### `vision-pipeline`

- New grove. BRIEF + a feature-roadmap tree from its 13 tasks: region generator
  + YOLO semantic classifier → widget detection → visual properties + font
  detection → icon classification → layout analysis → webview connector →
  pipeline orchestrator, each feature paired with its code-review leaf.
- Record its **cross-grove dependency on `ocr-accuracy`** via an inbox capture
  (`grove-llm inbox-add --to=vision-pipeline ...` / note in the BRIEF), not a
  hard link.

## Learnings promotion

Grove-local: each grove's `CONTEXT.md`/ADRs, promoted to `main` when that grove
finishes. Cross-grove facts to `~/.claude` auto-memory (≤2): e.g. "LLM_STATE
retired into per-grove worktrees on 2026-05-29; full historical research log at
`<commit>:LLM_STATE/`."

## Git sequencing

1. `grove start <name> --no-launch` ×3 — branches off current `main` (which
   still holds the legacy dirs, so each branch inherits them).
2. In each branch, one **seeding commit**: build `.grove/`, promote learnings to
   `CONTEXT.md`/ADRs, `git rm -r groves LLM_STATE` (for `rust-cli-port`, `git
   mv` its own dir out first). Every branch converges to "no legacy dirs," so
   later merges to `main` stay clean.
3. One **cleanup commit** on `main` (run from the main worktree
   `/Users/antony/Development/TestAnyware`) removing `groves/` and `LLM_STATE/`.

## Dropped (old-system tooling, not migrated)

`session-log.yaml`, `phase.md`, `*-baseline`, `*-word-count`,
`latest-session.yaml`, `prompt-*.md`, `compact-baseline` — git already provides
the history these append-only state files emulated; only decisions and live work
are lifted.

## Out of scope

- Driving any new grove's actual work tasks.
- The `fix-agent-upload-is-capped-at-8MB` grove.

## Verification

- `grove list` shows `rust-cli-port`, `ocr-accuracy`, `vision-pipeline`.
- `grove-llm pick` in each new worktree returns a live leaf.
- `git -C <main> ls-files groves LLM_STATE` returns empty.
- `git log` on `main` shows old `LLM_STATE`/`groves` content still reachable
  (e.g. `git show <pre-cleanup-commit>:LLM_STATE/ocr-accuracy/memory.yaml`).
