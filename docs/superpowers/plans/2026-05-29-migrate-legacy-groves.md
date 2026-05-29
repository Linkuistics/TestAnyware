# Migrate Legacy Workstreams Into Grove — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `groves/rust-cli-port` and the three `LLM_STATE/` workstreams into the live grove system as three groves (`rust-cli-port`, `ocr-accuracy`, `vision-pipeline`), folding `core` into `rust-cli-port`, then remove the legacy dirs from `main`.

**Architecture:** Each grove becomes its own worktree+branch off `main` carrying only its `.grove/` tree. Seed-only: each new grove gets a BRIEF (with the distilled backlog as roadmap) + a single `010` triage/decompose planning leaf that grows the real tree when first driven — mirroring how `rust-cli-port` already works (BRIEF + `010-audit`). Durable learnings distil into grove-local `CONTEXT.md`/ADRs; full old YAML stays recoverable in git history and is cited, not transcribed.

**Tech Stack:** `grove` / `grove-llm` CLIs, git worktrees, markdown.

**Reference spec:** `docs/superpowers/specs/2026-05-29-migrate-legacy-groves-design.md`

**Key constants:**
- Citation SHA (LLM_STATE still present): **`a062072`**
- Main worktree: `/Users/antony/Development/TestAnyware`
- ADR numbering on grove branches starts at **`0002`** (`0001` is reserved by the `fix-agent-upload` grove's streaming ADR to avoid collision on eventual merge to `main`).

---

## Task 0: Pre-flight verification

**Files:** none (read-only checks)

- [ ] **Step 1: Confirm main worktree is clean and at the citation SHA**

Run:
```bash
M=/Users/antony/Development/TestAnyware
git -C "$M" rev-parse --abbrev-ref HEAD          # expect: main
git -C "$M" rev-parse --short HEAD               # expect: a062072
git -C "$M" status --short                       # expect: only ?? .grove-meta/ and ?? .grove-worktrees/
```
Expected: on `main`, HEAD `a062072`, no tracked modifications.

- [ ] **Step 2: Confirm the legacy dirs are present on main and citable**

Run:
```bash
git -C "$M" ls-files groves LLM_STATE | wc -l     # expect: 29
git -C "$M" show a062072:LLM_STATE/core/memory.yaml | head -1   # expect: "entries:"
```
Expected: 29 tracked legacy files; `git show` resolves the old content.

---

## Task 1: `rust-cli-port` grove (relocate + fold `core`)

**Files:**
- Create (worktree): `.grove-worktrees/rust-cli-port/` via `grove start`
- Move: `groves/rust-cli-port/BRIEF.md` → `.grove/BRIEF.md`; `groves/rust-cli-port/010-audit-swift-surface.md` → `.grove/010-audit-swift-surface.md`
- Modify: `.grove/BRIEF.md` (add core roadmap + QEMU-done note), `.grove/010-audit-swift-surface.md` (cite core backlog as audit input), `CONTEXT.md` (process-control terms)
- Create: `docs/adr/0002-swift-cli-process-control-lore.md`
- Remove: `groves/`, `LLM_STATE/` (within this branch)

- [ ] **Step 1: Create the worktree without launching a session**

Run:
```bash
cd /Users/antony/Development/TestAnyware
grove start rust-cli-port --no-launch --start-point main
W=/Users/antony/Development/TestAnyware/.grove-worktrees/rust-cli-port
git -C "$W" rev-parse --abbrev-ref HEAD          # expect: rust-cli-port
```
Expected: new worktree at `$W` on branch `rust-cli-port`, grove materialised (`.claude/skills/grove/` present).

- [ ] **Step 2: Relocate the existing grove tree (history-preserving)**

Run:
```bash
cd "$W"
mkdir -p .grove
git mv groves/rust-cli-port/BRIEF.md .grove/BRIEF.md
git mv groves/rust-cli-port/010-audit-swift-surface.md .grove/010-audit-swift-surface.md
ls .grove                                         # expect: 010-audit-swift-surface.md  BRIEF.md
```
Expected: `groves/` now empty (git stops tracking it); `.grove/` holds both files.

- [ ] **Step 3: Edit `.grove/BRIEF.md` — fold core's roadmap**

In `.grove/BRIEF.md`, append a new section before `## Pointers` capturing core's live port-task inventory as the known roadmap the `010` audit consumes (distilled from `a062072:LLM_STATE/core/backlog.yaml`):

```markdown
## Roadmap absorbed from LLM_STATE/core (2026-05-29 migration)

`LLM_STATE/core` was this workstream's predecessor backlog. Its live
port-tasks are the known decomposition the `010` audit refines (full
detail: `git show a062072:LLM_STATE/core/backlog.yaml`):

- Port `testanyware doctor` with Linux-host preflight checks.
- Port `testanyware record` to embedded libav (`ffmpeg-next`), not subprocess.
- Build cross-platform VNC viewer (egui) replacing the AppleScript launcher;
  needs an RFB client crate (handshake, Raw, CopyRect; then ZRLE/Tight) and a
  live-VM verification gate for the RFB + input layers.
- Port tart runner for the macOS-host→macOS-guest path (`cfg(target_os=macos)`).
- Distribute Rust `testanyware` via homebrew (macOS+Linux) and Windows zip.
- Cross-platform pass for Windows-host support.
- Build golden-image creation into the CLI as a subcommand (see memory
  `project_golden_creation_in_cli.md`).
- Retire Swift `cli/` once the Rust port is at parity (this grove's root goal).

**Already done (do not re-leaf):** the QEMU runner + VM lifecycle port —
merged 2026-05-22 (`0634fa6`); core listed it `not_started` but it shipped.
```

- [ ] **Step 4: Edit `.grove/010-audit-swift-surface.md` — cite core as audit input**

In the `## Context` section of `.grove/010-audit-swift-surface.md`, add one bullet:

```markdown
- Roadmap input: the port-task inventory absorbed from `LLM_STATE/core`,
  recorded in this grove's BRIEF (full detail
  `git show a062072:LLM_STATE/core/backlog.yaml`). Reconcile the audit against
  it rather than enumerating Swift commands blind.
```

- [ ] **Step 5: Write the process-control lore ADR**

Create `docs/adr/0002-swift-cli-process-control-lore.md` distilling `a062072:LLM_STATE/core/memory.yaml`:

```markdown
# Process-control lore carried from the Swift CLI

/ Status: accepted /

## Context

The Swift CLI's VM/exec layer accumulated hard-won process-control knowledge
(`LLM_STATE/core/memory.yaml`, retired 2026-05-29; full text
`git show a062072:LLM_STATE/core/memory.yaml`). These facts shaped exec and
VM-stop and must not be silently lost in the Rust port.

## Decision

Record the load-bearing facts and how they translate to Rust:

- **Darwin pipe EOF needs all write-FD holders to exit.** Foundation
  `readDataToEndOfFile()` blocks until every process holding the write end
  exits, not just the direct child; long-lived bash descendants stall EOF.
- **Foundation `Process` does not `setsid`/`setpgrp`.** Children stay in the
  parent's process group; when bash exits, descendants reparent to launchd and
  the tree becomes unrecoverable.
- **Snapshot `pgrep -P <pid>` before killing bash.** Descendants reparented
  after bash exits cannot be found by PID-tree traversal afterward.
- **Temp-file capture beats pipes** for output from commands that spawn
  subprocesses — it removes the all-holders-must-exit invariant.
- **Process-tree kill after exec is best-effort** under Foundation — children
  spawned between snapshot and signal can leak.

**Translation to Rust (do not port verbatim):** use `tokio::process`; place
children in their own group via `nix` `setsid` under `#[cfg(unix)]` and
`CREATE_NEW_PROCESS_GROUP` on Windows. With explicit process groups, most of
the Foundation workarounds above dissolve — re-validate the kill sequence in
Rust rather than transcribing the temp-file/pgrep dance.

## Consequences

The Rust exec/VM-stop layer is designed around explicit process groups, so the
Darwin-specific workarounds become history, not requirements. This ADR is the
record of why they existed.
```

- [ ] **Step 6: Promote process-control glossary terms**

Append to `CONTEXT.md` (root, host-CLI bounded context) under `## Language`:

```markdown
**Process-group exec discipline**:
Running guest/host child processes in their own process group so the whole
tree can be signalled and reaped. The Rust CLI uses `nix` `setsid` (Unix) /
`CREATE_NEW_PROCESS_GROUP` (Windows); the Swift CLI could not (Foundation
`Process` limitation), forcing `pgrep`-snapshot + best-effort kills. See
ADR-0002.
_Avoid_: "kill the process" — say whether the whole group is signalled.
```

- [ ] **Step 7: Remove the legacy dirs from this branch**

Run:
```bash
cd "$W"
git rm -r --quiet LLM_STATE
git status --short groves                          # expect: empty (groves already untracked after mv)
```
Expected: `LLM_STATE/` removed; `groves/` already gone via the Step-2 moves.

- [ ] **Step 8: Commit the seeding**

Run:
```bash
cd "$W"
git add -A
git commit -q -m "grove(rust-cli-port): seed from groves/ + fold LLM_STATE/core

Relocate the rust-cli-port grove into its own worktree (.grove/), absorb
core's port-task roadmap into the BRIEF, promote core's process-control
lore to ADR-0002 + CONTEXT.md, and drop the legacy LLM_STATE dir from
this branch. Full old YAML: git show a062072:LLM_STATE/core/.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 9: Verify the grove picks a live leaf**

Run:
```bash
cd "$W" && grove-llm pick
```
Expected: prints `.grove/010-audit-swift-surface.md`.

---

## Task 2: `ocr-accuracy` grove (translate)

**Files:**
- Create (worktree): `.grove-worktrees/ocr-accuracy/` via `grove start`
- Create: `.grove/BRIEF.md`, `.grove/010-triage-and-decompose.md`
- Create: `docs/adr/0003-easyocr-default-engine.md`, `docs/adr/0004-center-distance-matching.md`
- Create: `CONTEXT-MAP.md`, `CONTEXT-ocr.md` (OCR bounded-context glossary)
- Remove: `groves/`, `LLM_STATE/` (within this branch)

- [ ] **Step 1: Create the worktree**

Run:
```bash
cd /Users/antony/Development/TestAnyware
grove start ocr-accuracy --no-launch --start-point main
W=/Users/antony/Development/TestAnyware/.grove-worktrees/ocr-accuracy
git -C "$W" rev-parse --abbrev-ref HEAD            # expect: ocr-accuracy
```

- [ ] **Step 2: Write `.grove/BRIEF.md`**

Create `$W/.grove/BRIEF.md`. Goal: OCR-accuracy research/evaluation. Current state: EasyOCR adopted as default engine (Session 41, 2026-04-13) — wins aggregate text-F1 +9–20pp over Apple Vision on every platform; dense-monospace gap is Apple-Vision-specific. Roadmap (distil from `a062072:LLM_STATE/ocr-accuracy/backlog.yaml`, cite for full detail) grouped as: **analyzer/engine** (in-progress: long-lived OCR analyzer daemon for interactive `find-text`; EasyOCR macOS-Retina runtime opt; per-region engine router; verify per-engine confidence cutoffs; follow-on engine survey PaddleOCR/TrOCR/Kraken/Calamari), **evaluation re-measures** (matcher-layer S28–29, GT-additions S24–26, S33 button/textfield filter, S30 AT-SPI fix, macOS S23→now F1 regression, baseline refresh, pre-command terminal drag), **ground-truth** (drop GTK4 spatial GT, constructed GT for TextEdit), **agent/generator bugs** (Windows UIA oversized-bbox, windowsterminal focus bug, GTK4 per-element position recovery, generator bbox-bounds validation), **infrastructure** (canonical baseline snapshots, batch analyzer rerun, Linux agent test infra, connect.json bootstrap helper). End with a Pointers section citing ADR-0003/0004 and `CONTEXT-ocr.md`, and a note: full backlog/session history at `git show a062072:LLM_STATE/ocr-accuracy/`.

- [ ] **Step 3: Write `.grove/010-triage-and-decompose.md` (planning leaf)**

Create `$W/.grove/010-triage-and-decompose.md`, `**Kind:** planning`. Goal: re-validate which BRIEF roadmap items are still live post-migration (some may have shipped since the YAML froze), then decompose the live set into the category node tree (analyzer-engine / evaluation / ground-truth / agent-generator-bugs / infrastructure) per grove conventions. Done-when: the live items are confirmed against the current code (`agents/`, `cli-rs/`, OCR pipeline), the tree has grown into the category nodes with ordered leaves, and any newly-surfaced OCR term is added to `CONTEXT-ocr.md`. Notes: the in-progress daemon task is the highest-priority analyzer leaf; cite source backlog ids when writing each leaf.

- [ ] **Step 4: Write ADR-0003 (EasyOCR default)**

Create `docs/adr/0003-easyocr-default-engine.md` distilling `a062072:LLM_STATE/ocr-accuracy/memory.yaml`:

```markdown
# EasyOCR as the default OCR engine

/ Status: accepted /

## Context

A multi-engine survey (Session 40) compared Apple Vision, Tesseract, and
EasyOCR across platforms and content classes. Full data:
`git show a062072:LLM_STATE/ocr-accuracy/memory.yaml`.

## Decision

EasyOCR is the default offline OCR engine. It wins aggregate text-F1 by
+9–20pp over Apple Vision on every platform; on dense-monospace terminal
buckets it is ~3–4× Apple Vision. `OCRConfig` dispatches among
`apple_vision` / `tesseract` / `easyocr` (schema 0.2.0). Canonical
`data/ocr-vm-{platform}/predictions/` hold EasyOCR output; prior Apple Vision
predictions are preserved under `predictions-apple-vision/` for per-engine A/B.

## Considered alternatives

- **Apple Vision (prior default)** — loses aggregate F1 everywhere; the
  dense-monospace gap is Apple-Vision-specific, not OCR-task-bound.
- **Tesseract** — second on dense terminals (~3× Linux, 1.3× macOS) but below
  EasyOCR aggregate.

## Consequences

- The interactive `find-text` path needs a long-lived analyzer daemon: EasyOCR
  cold-start is 4.8–5.5s/call (non-viable interactively); warm inference is
  0.79–3.92s/sample. (Tracked as the in-progress analyzer-daemon leaf.)
- `find-text` interactive default remained Apple Vision pending the daemon;
  offline/batch default is EasyOCR.
```

- [ ] **Step 5: Write ADR-0004 (center-distance matching)**

Create `docs/adr/0004-center-distance-matching.md`:

```markdown
# Center-distance matching replaces IoU for spatial OCR evaluation

/ Status: accepted /

## Context

AX element bounding boxes include padding/hit areas (e.g. menu-bar items: GT
30px tall vs OCR 12px), making them 1.5–2.5× larger than OCR text bounds.
Median IoU for text-matched pairs is 0.305 — no IoU threshold fixes this.
Source: `git show a062072:LLM_STATE/ocr-accuracy/memory.yaml`.

## Decision

Spatial evaluation uses `center_distance_match()`
(`match_detections(spatial_mode="center")`): a prediction matches when its
center is inside the GT box (10px margin) or the GT fully contains the
prediction; greedy matching ranked by center proximity. Text-content matching
uses two-phase single-then-multi-word adjacent fuzzy matching; single-char
non-alphanumeric tokens are dropped before matching (they block otherwise-valid
labels).

## Consequences

IoU is retained only where it is meaningful; center-distance is the spatial
metric of record. GTK4 elements with `(0,0)` per-element positions still
distort the IoU denominator until the "drop GTK4 spatial GT" leaf ships.
```

- [ ] **Step 6: Create the OCR bounded-context glossary**

Create `$W/CONTEXT-MAP.md` naming two bounded contexts: the **host-CLI** context (`CONTEXT.md`) and the **OCR/vision** context (`CONTEXT-ocr.md`). Create `$W/CONTEXT-ocr.md` as a terse glossary (per `CONTEXT-FORMAT.md`) defining at least: `by_app bucket`, `center-distance matching`, `A/B snapshot`, `EasyOCR` / `Apple Vision` / `Tesseract`, `GT (ground truth)`, `text-content matching`. Definitions distil from `a062072:LLM_STATE/ocr-accuracy/memory.yaml`; keep each to a sentence or two, no implementation detail.

- [ ] **Step 7: Remove legacy dirs and commit**

Run:
```bash
cd "$W"
git rm -r --quiet LLM_STATE groves
git add -A
git commit -q -m "grove(ocr-accuracy): seed from LLM_STATE/ocr-accuracy

Translate the OCR-accuracy workstream into a live grove: BRIEF with the
distilled backlog roadmap, a 010 triage/decompose planning leaf, ADR-0003
(EasyOCR default) + ADR-0004 (center-distance matching), and an OCR
bounded-context glossary (CONTEXT-MAP.md + CONTEXT-ocr.md). Drop the
legacy dirs. Full history: git show a062072:LLM_STATE/ocr-accuracy/.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 8: Verify**

Run:
```bash
cd "$W" && grove-llm pick                          # expect: .grove/010-triage-and-decompose.md
```

---

## Task 3: `vision-pipeline` grove (translate)

**Files:**
- Create (worktree): `.grove-worktrees/vision-pipeline/` via `grove start`
- Create: `.grove/BRIEF.md`, `.grove/010-triage-and-decompose.md`
- Remove: `groves/`, `LLM_STATE/` (within this branch)

- [ ] **Step 1: Create the worktree**

Run:
```bash
cd /Users/antony/Development/TestAnyware
grove start vision-pipeline --no-launch --start-point main
W=/Users/antony/Development/TestAnyware/.grove-worktrees/vision-pipeline
git -C "$W" rev-parse --abbrev-ref HEAD            # expect: vision-pipeline
```

- [ ] **Step 2: Write `.grove/BRIEF.md`**

Create `$W/.grove/BRIEF.md`. Goal: build the VM-based vision pipeline (region/widget detection, visual properties, layout) feeding the accessibility-testing surface. Roadmap (distil from `a062072:LLM_STATE/vision-pipeline/backlog.yaml`, cite for detail) as a feature sequence, each feature paired with its code-review leaf: VM-based region generator + YOLO semantic classifier → VM-based widget generator (remaining widget types) + YOLO classifier → visual properties (port from Redraw) + font detection → icon classification → layout analysis → WebView connector → deterministic VM evaluation scenarios → pipeline orchestrator. Record the **cross-grove dependency**: this grove consumes `ocr-accuracy`'s OCR outputs (EasyOCR predictions, evaluation snapshots) — coordinate via the inbox, not a hard link. Pointers: shares the OCR/vision bounded context (see `ocr-accuracy`'s `CONTEXT-ocr.md` once merged). Note: full backlog/session history at `git show a062072:LLM_STATE/vision-pipeline/`.

- [ ] **Step 3: Write `.grove/010-triage-and-decompose.md` (planning leaf)**

Create `$W/.grove/010-triage-and-decompose.md`, `**Kind:** planning`. Goal: re-validate the feature roadmap against current code (`agents/`, vision pipeline sources) and decompose into the feature node tree (region-decomposition / widget-detection / visual-properties / icon-classification / layout-analysis / integration), each feature node carrying its build leaf + a code-review leaf. Done-when: live features confirmed, tree grown, cross-grove dependency on `ocr-accuracy` captured as an inbox note if any concrete handoff is identified. Notes: cite source backlog ids per leaf; `vm-based-region-generator` is the first build leaf.

- [ ] **Step 4: Capture the cross-grove dependency note**

Run (records the dependency for the `ocr-accuracy` grove's next bootstrap; harmless if it just informs):
```bash
cd "$W"
grove-llm inbox-add --to=ocr-accuracy --body="vision-pipeline (migrated 2026-05-29) consumes ocr-accuracy OCR outputs (EasyOCR predictions, eval snapshots). Heads-up for coordination; no action required unless OCR output formats change."
```
Expected: prints the created inbox file path on the `grove-meta` branch.

- [ ] **Step 5: Remove legacy dirs and commit**

Run:
```bash
cd "$W"
git rm -r --quiet LLM_STATE groves
git add -A
git commit -q -m "grove(vision-pipeline): seed from LLM_STATE/vision-pipeline

Translate the vision-pipeline workstream into a live grove: BRIEF with the
distilled feature roadmap (region/widget/visual-props/layout → orchestrator,
each with a code-review leaf) and cross-grove dependency on ocr-accuracy, a
010 triage/decompose planning leaf, and drop the legacy dirs. Full history:
git show a062072:LLM_STATE/vision-pipeline/.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 6: Verify**

Run:
```bash
cd "$W" && grove-llm pick                          # expect: .grove/010-triage-and-decompose.md
```

---

## Task 4: Clean `main`

**Files:**
- Remove (main worktree): `groves/`, `LLM_STATE/`

- [ ] **Step 1: Remove the legacy dirs from main**

Run:
```bash
M=/Users/antony/Development/TestAnyware
cd "$M"
git rm -r --quiet groves LLM_STATE
git status --short                                 # expect: staged deletions of all 29 files
```

- [ ] **Step 2: Commit the cleanup**

Run:
```bash
cd "$M"
git commit -q -m "chore: retire legacy groves/ and LLM_STATE/ from main

Content migrated into the rust-cli-port, ocr-accuracy, and vision-pipeline
groves (worktrees + branches). Full historical YAML remains recoverable at
git show a062072:{groves,LLM_STATE}/...; durable learnings live in each
grove's CONTEXT/ADRs per docs/superpowers/specs/2026-05-29-migrate-legacy-groves-design.md.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 3: Verify main is clean of legacy dirs**

Run:
```bash
cd "$M"
git ls-files groves LLM_STATE | wc -l              # expect: 0
git show a062072:LLM_STATE/core/memory.yaml | head -1   # expect: "entries:" (history intact)
```

---

## Task 5: Auto-memory + final verification

**Files:**
- Create: `/Users/antony/.claude/projects/-Users-antony-Development-TestAnyware/memory/project_llm_state_retired.md`
- Modify: `/Users/antony/.claude/projects/-Users-antony-Development-TestAnyware/memory/MEMORY.md`

- [ ] **Step 1: Write the cross-grove memory pointer**

Create `project_llm_state_retired.md` with frontmatter (`type: project`): the LLM_STATE YAML state-system was retired on 2026-05-29 and migrated into three groves (`rust-cli-port` ← also absorbed `core`, `ocr-accuracy`, `vision-pipeline`); full historical research log recoverable at `git show a062072:LLM_STATE/...` and `:groves/...`; new work uses the grove system, not `LLM_STATE`. Link `[[project_golden_creation_in_cli]]` and `[[project_rust_port_conditional_facilities]]`.

- [ ] **Step 2: Add the MEMORY.md index line**

Append to `MEMORY.md`:
```markdown
- [LLM_STATE retired into groves](project_llm_state_retired.md) — 2026-05-29 migration; old YAML at git show a062072:LLM_STATE/
```

- [ ] **Step 3: Final verification of the whole migration**

Run:
```bash
M=/Users/antony/Development/TestAnyware
grove list                                          # expect includes: rust-cli-port, ocr-accuracy, vision-pipeline (+ fix-agent-upload-is-capped-at-8MB)
for g in rust-cli-port ocr-accuracy vision-pipeline; do
  echo "== $g =="; (cd "$M/.grove-worktrees/$g" && grove-llm pick)
done
git -C "$M" ls-files groves LLM_STATE | wc -l       # expect: 0
```
Expected: three new groves listed, each picks its live `010` leaf, main carries no legacy files.

---

## Self-review notes

- **Spec coverage:** Disposition table → Tasks 1–4; distill+cite fidelity → BRIEF roadmaps + ADRs cite `a062072`; seed-only → planning `010` leaves, no work driven; grove-local learnings → ADR-0002/0003/0004 + CONTEXT files on grove branches; cross-grove dep → Task 3 Step 4; main cleanup → Task 4; auto-memory → Task 5.
- **ADR numbering:** 0002 (rust-cli-port), 0003/0004 (ocr-accuracy) — start at 0002 to avoid colliding with the `fix-agent-upload` grove's 0001 on eventual merge to `main`. vision-pipeline raises no ADR at seed time (no hard-to-reverse decision yet).
- **Branch convergence:** every grove branch ends with `groves/`+`LLM_STATE/` removed, matching the main cleanup, so future merges stay conflict-free.
