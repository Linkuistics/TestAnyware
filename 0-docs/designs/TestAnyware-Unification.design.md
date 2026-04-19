# TestAnyware Unification — Design

**Date:** 2026-04-19
**Status:** Awaiting user review
**Companion plan:** `0-docs/plans/TestAnyware-Unification.plan.md` (to be written)
**Companion prompt:** `0-docs/prompts/TestAnyware-Unification.prompt.md`

## 1. Context

Five projects under `/Users/antony/Development/` cover overlapping
territory in a single product arc — AI-driven GUI testing of virtual
machines. Their relationships:

- **GUIVisionVMDriver** — Swift CLI `guivision` + driver library, three
  cross-platform guest agents (Swift/macOS, Python/Linux, C#/Windows),
  tart + QEMU VM lifecycle, XDG-compliant paths. **Currently in active
  development.** Has a sparse embryonic Python `pipeline/`.
- **GUIVisionPipeline** — Mature Python uv-workspace vision pipeline.
  Stage-based: window → chrome → element → menu → OCR → state. Includes
  CoreML inference wrappers (Swift) and programmatic-ground-truth
  training infrastructure.
- **Redraw** — Swift + Python research on drawing-primitive extraction
  (colors via k-means, borders, shadows, font matching via SSIM).
  Designed as an embeddable module.
- **TestAnyware** (v1) — macOS-only Swift predecessor. Own CLI
  `testanyware`, port-9200 agent, own RoyalVNCKit fork. Superseded by
  GUIVisionVMDriver but contains unique icon-classification work.
- **TestAnywareRedux** (v2) — cross-platform rewrite attempt.
  Superseded in practice by GUIVisionVMDriver.

GUIVisionVMDriver has absorbed the production features of the two
predecessors. GUIVisionPipeline and Redraw are orthogonal research
branches designed to feed into the production system.

**The unification target:** one monorepo at `/Users/antony/Development/
TestAnyware/`, using `testanyware` as the sole CLI name. Downstream
LLM-driven projects across `~/Development/` depend on the current
surface and must be migrated in the same plan.

## 2. Goals

1. Collapse the five projects into `/Users/antony/Development/TestAnyware/`.
2. Rename `guivision` → `testanyware` as a single clean break. No
   transitional aliases, no backward-compat shims.
3. Preserve the live surface: two-channel VNC + HTTP/8648 architecture,
   three cross-platform guest agents, tart + QEMU VM lifecycle,
   XDG-compliant paths, the full CLI subcommand set, the connection-spec
   JSON schema.
4. Absorb research work: GUIVisionPipeline's stage model as `vision/`,
   Redraw as a stage inside it, TestAnyware v1's icon classifier as a
   new stage.
5. Two documentation entry points at the root: `README.md` for humans,
   `LLM_INSTRUCTIONS.md` for LLMs.
6. Scan `~/Development/**` for every reference to the old names and
   migrate downstream consumers. Raveloop templates included.
7. Push new repo to `github.com/linkuistics/TestAnyware`; delete the
   five old repos from the Linkuistics org.

## 3. Non-Goals

- Git history migration. The user does not need history in the merged
  repo.
- Backward-compat aliases. The old CLI, env vars, paths, and image
  names are removed entirely.
- New features. Pure reorganize + harvest + rename. Feature work
  resumes after unification lands.
- New platform support. macOS host only; guest platforms stay
  macOS/Linux/Windows. A Linux host driver is anticipated soon; the
  structure (`cli/linux/` as a sibling of `cli/macos/`) accommodates
  it but it is not implemented here.
- Parallels VM backend from TestAnywareRedux — dropped entirely, not
  even backlogged.

## 4. Directory Structure

Flat by component, each directory a self-contained buildable unit in
its native tooling:

```
TestAnyware/
├── README.md                     # human entry point
├── LLM_INSTRUCTIONS.md           # LLM entry point (canonical CLI/API reference)
├── LICENSE                       # Apache-2.0
├── .gitignore
│
├── 0-docs/                       # meta: design history, plans, prompts
│   ├── designs/
│   ├── plans/
│   └── prompts/
│
├── LLM_STATE/                    # Raveloop plan state (migrated from GUIVisionVMDriver)
│   ├── core/                     # general backlog
│   ├── vision-pipeline/
│   └── ocr-accuracy/
│
├── cli/
│   ├── macos/                    # Swift Package — host CLI + driver library
│   │   ├── Package.swift
│   │   ├── Sources/
│   │   │   ├── testanyware/                  # CLI executable
│   │   │   ├── TestAnywareDriver/            # VNC capture, agent client, session mgmt
│   │   │   └── TestAnywareAgentProtocol/     # shared JSON-RPC 2.0 types
│   │   ├── Tests/TestAnywareDriverTests/
│   │   └── README.md
│   └── linux/                    # placeholder — README only; planned cross-platform host
│       └── README.md
│
├── agents/
│   ├── macos/                    # Swift Package, depends on ../../cli/macos for protocol
│   │   ├── Package.swift
│   │   ├── Sources/{testanyware-agent, TestAnywareAgent}/
│   │   └── README.md
│   ├── linux/                    # Python 3.12 (http.server + AT-SPI2)
│   │   ├── testanyware_agent/
│   │   ├── pyproject.toml
│   │   └── README.md
│   └── windows/                  # C# / .NET 9 (ASP.NET + UIA via FlaUI)
│       ├── TestAnywareAgent/
│       ├── TestAnywareAgent.csproj
│       └── README.md
│
├── vision/                       # Python uv workspace (from GUIVisionPipeline + Redraw)
│   ├── pyproject.toml            # workspace root
│   ├── uv.lock
│   ├── common/                   # shared types, NMS, image I/O
│   ├── stages/
│   │   ├── window-detection/
│   │   ├── chrome-detection/
│   │   ├── element-detection/
│   │   ├── icon-classification/  # new — from TestAnyware v1
│   │   ├── drawing-primitives/   # new — absorbed Redraw
│   │   ├── menu-detection/
│   │   ├── ocr/
│   │   └── state-detection/
│   ├── pipeline/                 # orchestrator chaining stages
│   ├── swift/                    # CoreML inference wrappers
│   ├── data/                     # golden datasets
│   └── README.md
│
├── provisioner/                  # VM lifecycle: golden-image builders + vm-*.sh
│   ├── scripts/
│   │   ├── vm-start.sh           # thin wrapper around `testanyware vm start`
│   │   ├── vm-stop.sh
│   │   ├── vm-list.sh
│   │   ├── vm-delete.sh
│   │   ├── vm-create-golden-macos.sh
│   │   ├── vm-create-golden-linux.sh
│   │   └── vm-create-golden-windows.sh
│   ├── autounattend/             # Windows unattend XML, ISO staging
│   └── README.md
│
├── vendored/                     # language-neutral vendored deps
│   └── royalvnc/                 # vendored RoyalVNCKit fork (one canonical copy)
│
└── docs/
    ├── user/                     # narrative deep-dives
    │   ├── quick-start.md
    │   ├── golden-images.md
    │   ├── multi-vm-networking.md
    │   ├── video-recording.md
    │   └── troubleshooting.md    # Gotchas (Tahoe drop-shadow, NSTextField, Electron --disable-gpu, etc.)
    ├── architecture/
    │   ├── overview.md           # two-channel design
    │   ├── agent-protocol.md     # JSON-RPC 2.0 schema
    │   ├── vision-pipeline.md    # stage model, I/O contracts
    │   └── vm-lifecycle.md       # tart vs QEMU, spec file, XDG paths
    ├── reference/                # exhaustive tables (embedded by LLM_INSTRUCTIONS.md)
    │   ├── cli-commands.md
    │   ├── env-vars.md
    │   ├── connection-spec.md
    │   ├── key-names.md
    │   └── error-codes.md
    └── components/               # maintainer-facing per-component docs
        ├── cli.md
        ├── agents-macos.md
        ├── agents-linux.md
        ├── agents-windows.md
        ├── vision.md
        └── provisioner.md
```

## 5. Component Boundaries

- **`cli/macos/`** — single Swift package, three targets. The `testanyware`
  executable, the `TestAnywareDriver` library (VNC/agent clients), and
  the `TestAnywareAgentProtocol` types library. Protocol is the contract
  between host and agents.
- **`cli/linux/`** — placeholder. A README documents the planned scope.
  No code yet. Signals that the structure supports a Linux host.
- **`agents/macos/`** — separate Swift package. Path-depends on
  `../../cli/macos` to reuse `TestAnywareAgentProtocol`. Decoupled so
  the agent can ship as its own binary without dragging in the driver.
- **`agents/linux/`, `agents/windows/`** — same JSON-RPC 2.0 protocol in
  Python and C# respectively. Contract documented in
  `docs/architecture/agent-protocol.md` and
  `docs/reference/connection-spec.md` — not enforced by a shared
  compile-time type.
- **`vision/`** — Python uv workspace. Orthogonal to Swift. The CLI can
  invoke it via `uv run` subprocess or HTTP (implementation detail
  settled in the plan).
- **`provisioner/`** — bash scripts + platform-specific builder assets.
  Scripts are thin wrappers around `testanyware vm {start,stop,list,
  delete}` — the CLI is the source of truth.
- **`vendored/royalvnc/`** — one canonical copy. Consumed by both
  `cli/macos/` and `agents/macos/` via
  `.package(path: "../../vendored/royalvnc")`. Eliminates the three
  duplicate forks across the source repos.

## 6. Harvest Map

| Source | Harvested | Target | Dropped |
|---|---|---|---|
| GUIVisionVMDriver | Swift CLI + driver + protocol + all three guest agents + `scripts/macos/` + autounattend assets + `LLM_STATE/` + architecture docs + Gotchas | `cli/macos/`, `agents/*`, `provisioner/scripts/`, `provisioner/autounattend/`, `LLM_STATE/`, content into `docs/` | Sparse `pipeline/` (replaced by vision/) |
| GUIVisionPipeline | All Python stages, `common/`, `pipeline/` orchestrator, `swift/` CoreML wrappers, `data/`, training scripts, uv workspace | `vision/*` (as-is with renames) | Duplicate `agents/` |
| Redraw | Python tier-2/tier-3 code: color extraction, border/shadow detection, font matcher, state extraction | `vision/stages/drawing-primitives/` (rewritten to fit stage contract) | Swift API, FastAPI server, separate ModelManager |
| TestAnyware v1 | Icon classification + training data collection | `vision/stages/icon-classification/` (new stage, runs after `element-detection/`) | Everything else (port-9200 agent, own RoyalVNC fork, old CLI — all superseded) |
| TestAnywareRedux | Nothing | — | Dropped entirely. F# Windows agent, duplicate core, duplicate vision, Parallels backend — all either superseded or explicitly out of scope |

## 7. Rename Table

Every symbol that renames. Anything not listed stays as-is.

| Kind | Old | New |
|---|---|---|
| CLI executable | `guivision` | `testanyware` |
| macOS agent exe | `guivision-agent` | `testanyware-agent` |
| Linux agent exe | `guivision-agent` | `testanyware-agent` |
| Windows agent exe | `GUIVisionAgent.exe` | `TestAnywareAgent.exe` |
| Swift lib (driver) | `GUIVisionVMDriver` | `TestAnywareDriver` |
| Swift lib (protocol) | `GUIVisionAgentProtocol` | `TestAnywareAgentProtocol` |
| Swift lib (macOS agent) | `GUIVisionAgentLib` | `TestAnywareAgent` |
| Env var — VM id | `GUIVISION_VM_ID` | `TESTANYWARE_VM_ID` |
| Env var — VNC endpoint | `GUIVISION_VNC` | `TESTANYWARE_VNC` |
| Env var — VNC password | `GUIVISION_VNC_PASSWORD` | `TESTANYWARE_VNC_PASSWORD` |
| Env var — agent endpoint | `GUIVISION_AGENT` | `TESTANYWARE_AGENT` |
| Env var — platform | `GUIVISION_PLATFORM` | `TESTANYWARE_PLATFORM` |
| XDG state dir | `$XDG_STATE_HOME/guivision/vms/` | `$XDG_STATE_HOME/testanyware/vms/` |
| XDG data dir (goldens) | `$XDG_DATA_HOME/guivision/golden/` | `$XDG_DATA_HOME/testanyware/golden/` |
| XDG data dir (clones) | `$XDG_DATA_HOME/guivision/clones/` | `$XDG_DATA_HOME/testanyware/clones/` |
| XDG data dir (cache) | `$XDG_DATA_HOME/guivision/cache/` | `$XDG_DATA_HOME/testanyware/cache/` |
| Golden image — macOS | `guivision-golden-macos-tahoe` | `testanyware-golden-macos-tahoe` |
| Golden image — Linux | `guivision-golden-linux-24.04` | `testanyware-golden-linux-24.04` |
| Golden image — Windows | `guivision-golden-windows-11` | `testanyware-golden-windows-11` |
| Clone id prefix | `guivision-<hex8>` / `guivision-test-<hex8>` | `testanyware-<hex8>` / `testanyware-test-<hex8>` |
| macOS LaunchAgent label | `com.linkuistics.guivision-agent` | `com.linkuistics.testanyware-agent` |
| Linux systemd unit | `guivision-agent.service` | `testanyware-agent.service` |
| Windows Scheduled Task | `GUIVisionAgent` | `TestAnywareAgent` |
| Agent install path | `/usr/local/bin/guivision-agent` | `/usr/local/bin/testanyware-agent` |

**Unchanged:** port 8648, JSON-RPC 2.0 wire protocol, connection-spec
JSON schema, script filenames (`vm-start.sh` et al.), connect-spec
filename format (`<id>.json`, `<id>.meta.json`).

## 8. LLM_STATE Migration

`GUIVisionVMDriver/LLM_STATE/` contains three Raveloop-managed plan
dirs:

- `core/` — general backlog (currently 12+ open items)
- `vision-pipeline/`
- `ocr-accuracy/`

Each holds `backlog.md`, `memory.md`, `phase.md`, `session-log.md`, and
latest-session / baseline sidecars. These are **living documents** — the
next Raveloop session reads them and plans against them.

Migration is not a `cp -r`. Every `.md` file inside must be rewritten:

- Path literals (`~/.local/state/guivision/`, `~/.local/share/guivision/`)
- Env var references (`GUIVISION_VM_ID`, etc.)
- CLI invocations (every `guivision <subcommand>` → `testanyware <subcommand>`)
- Image names in task descriptions
- Swift symbol references (`GUIVisionVMDriver`, `GUIVisionAgentProtocol`)
- File-path references to the old directory layout

Date stamps, task-status metadata, and historical session-log entries
must be preserved verbatim — the plan's job is to update references to
things that still exist, not to rewrite history.

**Verification**: after migration, `grep -rc 'guivision\|GUIVision\|
GUIVISION_' LLM_STATE/` returns `0`.

## 9. `~/Development` Reference Scan

First-class plan phase. Four buckets:

**Code** (Swift, Python, C#, Rust, JS/TS, F#, Go, shell) — patterns:
`guivision\b`, `GUIVision\w*`, `GUIVISION_\w+`, import statements
naming `GUIVisionVMDriver` or `GUIVisionAgentProtocol`, shell
invocations of `` `guivision ` ``.

**Config/data** (YAML, JSON, TOML, plist, dotfiles) — patterns:
`guivision` substrings, path fragments (`.local/state/guivision/`,
`.local/share/guivision/`), LaunchAgent labels, systemd unit names.

**Documentation** (Markdown, rst, adoc, HTML, plain text) — patterns:
any mention of `guivision`, `GUIVisionVMDriver`, `TestAnyware` in the
v1 sense, `TestAnywareRedux`, `Redraw`, `GUIVisionPipeline`.

**Raveloop plan state across projects** (`~/Development/*/LLM_STATE/
**/*.md`) — backlogs and memory files referencing the old names.

**Output:** a single inventory file at
`0-docs/designs/TestAnyware-Unification.rename-inventory.md` listing
every matching file grouped by project, with the matched lines and a
recommended action per file (rewrite / leave as historical / review).

**Migration sequence:** code + config first (runtime correctness), docs
second, Raveloop plan state third (most sensitive — read-then-rewrite,
not blind `sed`).

**Special case — Raveloop itself:** scan `Raveloop/defaults/`,
`Raveloop/src/`, `Raveloop/docs/` for default templates or examples
mentioning `guivision`. These shape every project Raveloop onboards.

## 10. Documentation Structure

Two entry points at the root:

- **`README.md`** — human-facing: overview, quick start, architecture
  summary, link to `LLM_INSTRUCTIONS.md`, link to `docs/`.
- **`LLM_INSTRUCTIONS.md`** — LLM-facing: comprehensive CLI/API
  reference, exact env-var names, connection-spec JSON schema,
  workflow patterns. Direct replacement for the current
  `instructions-for-llms-using-this-as-a-tool.md` with all renames and
  the new vision surface applied. Must be complete enough for a cold
  LLM to drive the CLI without reading the README.

Under `docs/`:

- `docs/user/` — narrative deep-dives (quick-start, golden-images,
  multi-vm-networking, video-recording, troubleshooting — the
  Gotchas section migrates here).
- `docs/architecture/` — design rationale, protocol specs, lifecycle
  model.
- `docs/reference/` — exhaustive tables (CLI commands, env vars,
  connection-spec schema, key names, error codes). These are the
  source-of-truth tables that `LLM_INSTRUCTIONS.md` embeds or links.
- `docs/components/` — maintainer-facing per-component details.

Each top-level component directory (`cli/macos/`, `agents/macos/`,
`agents/linux/`, `agents/windows/`, `vision/`, `provisioner/`) gets
its own short `README.md` for contributors working in that dir.

No repo-wide `CONTRIBUTING.md` for now — the repo has one primary user.

## 11. Execution Sequence

Nine milestones with verification gates. Destructive actions
(GitHub deletion, local archive removal) require explicit user
confirmation at execution time.

**Milestone 1 — Prepare.** `git status` in each source repo + Raveloop;
refuse to proceed if GUIVisionVMDriver is dirty. Create
`/Users/antony/Development/_archive/`. Move the five source repos
in. Create the `TestAnyware/` skeleton with all dirs empty.
*Gate 1:* archive contains five repos; skeleton is in place;
unrelated projects untouched.

**Milestone 2 — Harvest & Rename.** Substeps 2a–2h cover Swift core,
agents, RoyalVNCKit, provisioner, vision base (GUIVisionPipeline),
Redraw as drawing-primitives stage, icon-classification stage, and
LLM_STATE rewrite. Every rename in §7 applied.
*Gate 2:* `swift build --package-path cli/macos` passes;
`swift test --package-path cli/macos` (unit only) passes;
`uv sync && uv run pytest -m unit` in `vision/` passes;
`grep -rc 'guivision\|GUIVision\|GUIVISION_' LLM_STATE/` returns 0.

**Milestone 3 — Docs.** README, LLM_INSTRUCTIONS, `docs/reference/`,
`docs/architecture/`, `docs/user/` (Gotchas migrated), component
READMEs. Commit this design doc to `0-docs/designs/`.
*Gate 3:* LLM_INSTRUCTIONS.md comprehensive enough for cold-LLM
CLI use. Manual review.

**Milestone 4 — Build & smoke test.** Release build; install at
`/usr/local/bin/testanyware`; smoke offline subcommands.
*Gate 4:* `strings /usr/local/bin/testanyware | grep -ci guivision`
returns 0.

**Milestone 5 — Golden images.** Rebuild all three with new names.
*Gate 5:* `tart list` + `testanyware vm list` show the three new
goldens.

**Milestone 6 — Integration tests.** End-to-end smoke per platform
(`vm start`, screenshot, agent health, agent snapshot, input, exec,
`vm stop`). Full integration suite. Vision integration tests.
*Gate 6:* all green across macOS/Linux/Windows guests. **Safe-point:
after this, old stuff can start coming down.**

**Milestone 7 — `~/Development` scan + migration.** Run the four-bucket
scan; review inventory with user; apply rewrites per-project with a
commit per project. Update Raveloop templates. Spot-check one or two
downstream projects.
*Gate 7:* zero residual `guivision`, `GUIVisionVMDriver`,
`GUIVISION_*` references outside `_archive/` and historical session
logs.

**Milestone 8 — GitHub operations.** Ordered carefully because the
new repo name collides with the old v1 repo name:
1. `git init && git add . && git commit -m "..."` locally.
2. **[User confirmation required]** `gh repo delete linkuistics/
   TestAnyware --confirm` to free the v1 name.
3. `gh repo create linkuistics/TestAnyware --public --source=. --push`
   with Apache-2.0 license per Linkuistics policy.
4. Verify: `git remote -v`, `gh repo view linkuistics/TestAnyware`.
5. **[User confirmation required]** `gh repo delete` for the four
   remaining old repos: `GUIVisionVMDriver`, `GUIVisionPipeline`,
   `Redraw`, `TestAnywareRedux`.
*Gate 8:* `gh repo list linkuistics | grep -E 'GUIVision|Redraw|
TestAnywareRedux'` is empty; `gh repo view linkuistics/TestAnyware`
shows the new repo.

**Milestone 9 — Final local cleanup.** **[User confirmation required]**
`rm -rf /Users/antony/Development/_archive/`. Final sanity check.
*Gate 9:* `~/Development/` clean.

## 12. Risk & Rollback

- **Between gates 2 and 6:** old code still in `_archive/`. Any
  harvest mistake can be compared against originals.
- **Between gates 6 and 8:** old GH repos still online. A total local
  failure is recoverable by re-cloning from GitHub.
- **After gate 8:** old GH repos gone. The safe rollback window closes
  here.
- **After gate 9:** local `_archive/` gone. True point-of-no-return on
  disk.
- **Golden-image rebuilds** are reversible in principle (rerun the
  builder) but expensive in time. Do not trigger them until
  Milestone 2 is fully green.

## 13. Open Items for the Plan

These are not design decisions — they are implementation details the
plan will decide:

- Whether Swift CLI calls vision via `uv run` subprocess or over HTTP
  to a local FastAPI server. Both are viable; GUIVisionPipeline's
  current shape is `uv run` subprocess-oriented; Redraw's was
  HTTP-server-oriented.
- Exact Python workspace member reorganization (whether
  `drawing-primitives` and `icon-classification` are workspace
  members peer to the existing stages or nested differently).
- Whether Windows agent keeps C# / .NET 9 unchanged or rewrites to
  match another convention — default assumption: unchanged (it works).
- Exact set of `docs/reference/` tables — the list in §4 is a start;
  the plan may add or merge.
