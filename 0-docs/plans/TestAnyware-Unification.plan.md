# TestAnyware Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify five projects (GUIVisionVMDriver, GUIVisionPipeline, Redraw, TestAnyware, TestAnywareRedux) into a single monorepo at `~/Development/TestAnyware/`, rename `guivision` → `testanyware` across every surface, migrate downstream consumers under `~/Development/`, and replace the five old GitHub repos with one.

**Architecture:** Flat-by-component monorepo. Swift host CLI (`cli/macos/`) + three guest agents (Swift/Python/C#) + Python vision pipeline (uv workspace, absorbs GUIVisionPipeline + Redraw + icon classifier) + VM provisioner + vendored RoyalVNCKit. Two docs entry points at root: `README.md` + `LLM_INSTRUCTIONS.md`. Raveloop `LLM_STATE/` migrates with the repo, references rewritten inline.

**Tech Stack:** Swift 6 (macOS 14+ host, swift-argument-parser, Hummingbird, AVAssetWriter), Python 3.12 (uv, pytest, Ultralytics YOLO, OpenCV, Pillow, FastAPI), C# / .NET 9 (ASP.NET Core, FlaUI UIA), Bash, `tart` (macOS/Linux VMs), QEMU + swtpm (Windows VMs), `gh` CLI (GitHub ops), Apache-2.0 license.

**Design reference:** `0-docs/designs/TestAnyware-Unification.design.md`
**Execution prompt:** `0-docs/prompts/TestAnyware-Unification.prompt.md`

---

## Execution Log

Running log of progress. Use this to resume across sessions.

- **2026-04-19 session 1:** Prerequisites verified. Milestones 1 + 2 mostly
  complete. Completed substeps: 1.1-1.5 (Gate 1 PASS), 2a.1-2a.6 (Swift CLI
  harvested + renamed + built + 251 unit tests pass), 2c.1 (vendored
  royalvnc), 2b.1 (macOS agent — self-contained, see divergence below),
  2b.2 (Linux agent), 2b.3 (Windows agent — builds with `-r win-arm64
  --no-self-contained`), 2d.1 (provisioner scripts + autounattend),
  2e.1 (vision pipeline harvested + 70 tests pass), 2f.1 (drawing-primitives
  stage scaffolded from Redraw tier3). **Milestones 1 + 2 complete, Gate 2
  PASS.** **Next up:** Milestone 3 (docs). Added divergence note: vision
  pytest requires `--import-mode=importlib` (decisions.md); historical plan
  docs at vision/docs/superpowers/plans/ retain old names intentionally.

### Divergences from plan/design recorded in TestAnyware/LLM_STATE/core/decisions.md

1. **No per-platform subdirs under `cli/`** (design §4 said `cli/macos/` +
   `cli/linux/`). The Swift CLI sits at `cli/` directly; the Rust migration
   will replace it in place. `cli/linux/` placeholder not created.
   → **Every plan reference below to `cli/macos/` should be read as `cli/`.**
2. **`agents/macos/` is self-contained** (design §5 said path-dep on
   `../../cli/macos` to reuse TestAnywareAgentProtocol). Two reasons:
   (a) SPM path-dep identity is last path component, causing `cli/macos`
   ↔ `agents/macos` collision; (b) host CLI is migrating to Rust, so a
   shared Swift type layer is a short-lived invariant. The wire protocol
   (JSON-RPC 2.0, port 8648) is the real contract.
3. The agent protocol source is duplicated between
   `cli/Sources/TestAnywareAgentProtocol/` and
   `agents/macos/Sources/TestAnywareAgentProtocol/`. Slight drift is
   acceptable until the Rust migration naturally eliminates the cli copy.
   The `AXWebArea → .webArea` role mapping lives in both.

---

## Prerequisites

Verify before starting:

- [x] `swift --version` reports Swift 6.0+
- [x] `uv --version` reports uv installed
- [x] `dotnet --version` reports .NET 9+
- [x] `tart --version` reports tart installed
- [x] `qemu-system-aarch64 --version` reports QEMU installed
- [x] `swtpm --version` reports swtpm installed
- [x] `gh auth status` shows authenticated to GitHub as `linkuistics` org member with admin rights
- [x] `/Users/antony/Development/GUIVisionVMDriver/` exists and builds (`swift build --package-path cli/macos`)
- [x] All five source repos are at their current paths under `/Users/antony/Development/`

---

## Global Execution Rules

1. **Stop at every gate.** When a milestone ends with a verification gate, run the gate's commands, confirm pass, then proceed. On failure, stop and surface the exact error — do not paper over.
2. **Commit per substep.** Each Task ends with a `git add` + `git commit`. The repo is initialized at Milestone 1; commits start from Milestone 2 onward.
3. **No blind `sed`.** When rewriting references inside LLM_STATE or downstream projects, read the file first, consider each match, then rewrite.
4. **Destructive confirmations.** Milestones 8 and 9 contain steps that delete GitHub repos or local dirs. Each such step starts with `[USER CONFIRM]` — pause and ask the user explicitly before executing.
5. **Old code stays in `_archive/` throughout.** Milestones 1–8 treat `_archive/` as read-only reference material. Milestone 9 deletes it.

---

## Milestone 1 — Prepare

Goal: move the five source repos to `_archive/`, create the empty `TestAnyware/` skeleton, initialize git.

### Task 1.1: Pre-flight git status check

**Files:** none (read-only checks).

- [x] **Step 1:** For each of the five source repos, verify clean or recorded dirty state.

```bash
for d in GUIVisionVMDriver GUIVisionPipeline Redraw TestAnyware TestAnywareRedux; do
  echo "=== $d ==="
  git -C "/Users/antony/Development/$d" status --short
  git -C "/Users/antony/Development/$d" stash list
done
```

- [x] **Step 2:** If GUIVisionVMDriver is dirty, STOP. Report files to the user and ask them to commit or stash before continuing. For the other four, dirty state is acceptable (they're being archived).

- [x] **Step 3:** Also check Raveloop:

```bash
git -C /Users/antony/Development/Raveloop status --short
```

Report findings. Do not modify Raveloop state without user direction.

### Task 1.2: Create the archive directory

**Files:**
- Create: `/Users/antony/Development/_archive/`

- [x] **Step 1:** Create the archive directory.

```bash
mkdir /Users/antony/Development/_archive
```

Expected: directory exists, empty.

- [x] **Step 2:** Verify other non-source projects under `~/Development/` are untouched by listing them.

```bash
ls /Users/antony/Development/
```

Expected: includes `_archive/` (new, empty), all five source repos, plus Raveloop and any other sibling projects.

### Task 1.3: Move the five source repos to `_archive/`

**Files:**
- Move: `/Users/antony/Development/{GUIVisionVMDriver,GUIVisionPipeline,Redraw,TestAnyware,TestAnywareRedux}/` → `/Users/antony/Development/_archive/`

- [x] **Step 1:** Move the five source repos.

```bash
cd /Users/antony/Development
mv GUIVisionVMDriver GUIVisionPipeline Redraw TestAnyware TestAnywareRedux _archive/
```

- [x] **Step 2:** Verify.

```bash
ls _archive/
ls | grep -E 'GUIVisionVMDriver|GUIVisionPipeline|Redraw|TestAnyware|TestAnywareRedux'
```

Expected: `_archive/` contains the five directories. Second command: no matches at top level.

### Task 1.4: Create the `TestAnyware/` skeleton

**Files:**
- Create: `/Users/antony/Development/TestAnyware/` with full directory tree from design §4.

- [x] **Step 1:** Create the skeleton.

```bash
cd /Users/antony/Development
mkdir -p TestAnyware/{0-docs/{designs,plans,prompts},LLM_STATE,cli/{macos,linux},agents/{macos,linux,windows},vision,provisioner/{scripts,autounattend},vendored,docs/{user,architecture,reference,components}}
```

- [x] **Step 2:** Verify the shape.

```bash
find /Users/antony/Development/TestAnyware -type d | sort
```

Expected: every directory from design §4 is present.

### Task 1.5: Initialize git and write placeholder root files

**Files:**
- Create: `TestAnyware/.gitignore`
- Create: `TestAnyware/LICENSE` (Apache-2.0)
- Create: `TestAnyware/README.md` (placeholder — fleshed out in Milestone 3)
- Create: `TestAnyware/LLM_INSTRUCTIONS.md` (placeholder — fleshed out in Milestone 3)

- [x] **Step 1:** Write a minimal `.gitignore`.

```bash
cat > /Users/antony/Development/TestAnyware/.gitignore <<'EOF'
# Swift
.build/
.swiftpm/
*.xcodeproj/
Package.resolved

# Python
.venv/
__pycache__/
*.pyc
.pytest_cache/
.ruff_cache/
*.egg-info/

# .NET
bin/
obj/

# Node (for any tooling)
node_modules/

# OS
.DS_Store

# Editors
.vscode/
.idea/

# Local
.env
.env.local
*.log
EOF
```

- [x] **Step 2:** Write the Apache-2.0 license.

```bash
cp /Users/antony/Development/_archive/GUIVisionVMDriver/LICENSE /Users/antony/Development/TestAnyware/LICENSE
```

Verify it's Apache-2.0:

```bash
head -5 /Users/antony/Development/TestAnyware/LICENSE
```

Expected: "Apache License" and "Version 2.0" in first 5 lines.

- [x] **Step 3:** Write placeholder `README.md`.

```bash
cat > /Users/antony/Development/TestAnyware/README.md <<'EOF'
# TestAnyware

AI-driven GUI testing across virtual machines. Cross-platform guest
support (macOS, Linux, Windows) via VNC + in-VM HTTP agents. Python
vision pipeline for structured screen understanding.

**Under construction — unified monorepo for the project formerly split across
GUIVisionVMDriver, GUIVisionPipeline, Redraw, TestAnyware, TestAnywareRedux.**

Full README to be written in Milestone 3.

- For LLM consumers: see [LLM_INSTRUCTIONS.md](LLM_INSTRUCTIONS.md).
- For contributors: see `docs/`.
EOF
```

- [x] **Step 4:** Write placeholder `LLM_INSTRUCTIONS.md`.

```bash
cat > /Users/antony/Development/TestAnyware/LLM_INSTRUCTIONS.md <<'EOF'
# TestAnyware — LLM Instructions

**Under construction — full CLI/API reference to be written in Milestone 3.**

Canonical CLI: `testanyware`. Two-channel design (VNC + HTTP agent on
port 8648). See `docs/reference/` for machine-readable tables.
EOF
```

- [x] **Step 5:** Initialize git.

```bash
cd /Users/antony/Development/TestAnyware
git init -b main
git add .gitignore LICENSE README.md LLM_INSTRUCTIONS.md
git commit -m "chore: initialize TestAnyware monorepo skeleton"
```

Expected: one commit on `main`, working tree clean.

### Gate 1

- [x] Archive contains the five source repos:
  `ls /Users/antony/Development/_archive/` → five entries.
- [x] Skeleton exists with all design §4 directories:
  `find /Users/antony/Development/TestAnyware -type d | wc -l` ≥ 22.
- [x] Raveloop and other unrelated projects untouched:
  `ls /Users/antony/Development/Raveloop/` matches prior state.
- [x] Git initialized with one commit:
  `git -C /Users/antony/Development/TestAnyware log --oneline` → 1 line.

---

## Milestone 2 — Harvest & Rename

Goal: every piece of code/content that needs to migrate is in place under `TestAnyware/`, fully renamed, building, and tests passing.

For every Task in this milestone, the commit convention is:
`feat(<area>): <what>` where `<area>` ∈ `{cli, agents, vision, provisioner, docs, llm-state}`.

### Task 2a.1: Copy Swift core skeleton from archive

**Files:**
- Copy: `_archive/GUIVisionVMDriver/cli/macos/Package.swift` → `TestAnyware/cli/macos/Package.swift`
- Copy: `_archive/GUIVisionVMDriver/cli/macos/Sources/` → `TestAnyware/cli/macos/Sources/`
- Copy: `_archive/GUIVisionVMDriver/cli/macos/Tests/` → `TestAnyware/cli/macos/Tests/`

- [x] **Step 1:** Copy the whole `cli/macos/` tree (except `LocalPackages/`, which moves separately in Task 2c).

```bash
cd /Users/antony/Development/_archive/GUIVisionVMDriver/cli/macos
rsync -a --exclude='LocalPackages/' --exclude='.build/' --exclude='.swiftpm/' \
  ./ /Users/antony/Development/TestAnyware/cli/macos/
```

- [x] **Step 2:** Verify copy.

```bash
ls /Users/antony/Development/TestAnyware/cli/macos/
```

Expected: `Package.swift`, `Sources/`, `Tests/` (and any other top-level items from the source).

### Task 2a.2: Rename Swift targets in `Package.swift`

**Files:**
- Modify: `TestAnyware/cli/macos/Package.swift`

- [x] **Step 1:** Read the current `Package.swift`.

```bash
cat /Users/antony/Development/TestAnyware/cli/macos/Package.swift
```

- [x] **Step 2:** Apply these renames in `Package.swift`:
  - Package `name:` — whatever the old name was → `"TestAnywareCLI"` (or existing if already renamed — inspect first).
  - Product/target name `"guivision"` → `"testanyware"`.
  - Product/target name `"GUIVisionVMDriver"` → `"TestAnywareDriver"`.
  - Product/target name `"GUIVisionAgentProtocol"` → `"TestAnywareAgentProtocol"`.
  - Any test target `"GUIVisionVMDriverTests"` → `"TestAnywareDriverTests"`.
  - Path strings like `"Sources/guivision"` → `"Sources/testanyware"`.
  - Path strings like `"Sources/GUIVisionVMDriver"` → `"Sources/TestAnywareDriver"`.
  - Path strings like `"Sources/GUIVisionAgentProtocol"` → `"Sources/TestAnywareAgentProtocol"`.
  - `LocalPackages/royalvnc` path references → `"../../vendored/royalvnc"` (relative to `cli/macos/`).
  - Any other `guivision` / `GUIVision` literal → new equivalent per design §7.

Use `Read` then `Edit` per file — do not bulk-`sed`.

- [x] **Step 3:** Rename source directories to match the new target names.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos/Sources
# Rename only if old name exists and new doesn't.
[ -d guivision ] && mv guivision testanyware
[ -d GUIVisionVMDriver ] && mv GUIVisionVMDriver TestAnywareDriver
[ -d GUIVisionAgentProtocol ] && mv GUIVisionAgentProtocol TestAnywareAgentProtocol
```

- [x] **Step 4:** Rename test directory.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos/Tests
[ -d GUIVisionVMDriverTests ] && mv GUIVisionVMDriverTests TestAnywareDriverTests
```

### Task 2a.3: Rename Swift source content (imports, types, string literals)

**Files:**
- Modify: all `*.swift` under `TestAnyware/cli/macos/Sources/` and `TestAnyware/cli/macos/Tests/`

- [x] **Step 1:** Enumerate files needing review.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
grep -rl 'GUIVisionVMDriver\|GUIVisionAgentProtocol\|GUIVisionAgentLib\|guivision\|GUIVISION_' Sources/ Tests/
```

Expect: a list of ~20-80 files (every Swift file with any reference).

- [x] **Step 2:** For each file, apply these replacements. Read → Edit per file; confirm context before changing.

Replacements (exact strings):
  - `import GUIVisionVMDriver` → `import TestAnywareDriver`
  - `import GUIVisionAgentProtocol` → `import TestAnywareAgentProtocol`
  - `GUIVisionVMDriver` → `TestAnywareDriver`
  - `GUIVisionAgentProtocol` → `TestAnywareAgentProtocol`
  - `GUIVisionAgentLib` → `TestAnywareAgent`
  - `GUIVISION_VM_ID` → `TESTANYWARE_VM_ID`
  - `GUIVISION_VNC_PASSWORD` → `TESTANYWARE_VNC_PASSWORD` (do this before the shorter `GUIVISION_VNC` to avoid partial replacement)
  - `GUIVISION_VNC` → `TESTANYWARE_VNC`
  - `GUIVISION_AGENT` → `TESTANYWARE_AGENT`
  - `GUIVISION_PLATFORM` → `TESTANYWARE_PLATFORM`
  - Path literals containing `guivision/vms/` → `testanyware/vms/`
  - Path literals containing `guivision/golden/` → `testanyware/golden/`
  - Path literals containing `guivision/clones/` → `testanyware/clones/`
  - Path literals containing `guivision/cache/` → `testanyware/cache/`
  - Golden-image name constants `guivision-golden-macos-tahoe` → `testanyware-golden-macos-tahoe`
  - `guivision-golden-linux-24.04` → `testanyware-golden-linux-24.04`
  - `guivision-golden-windows-11` → `testanyware-golden-windows-11`
  - Clone-id prefix constants `guivision-` (when used as prefix for instance ids) → `testanyware-`
  - Executable name `"guivision"` string literals (CLI argument name) → `"testanyware"`
  - `/usr/local/bin/guivision` → `/usr/local/bin/testanyware`
  - `/usr/local/bin/guivision-agent` → `/usr/local/bin/testanyware-agent`
  - `com.linkuistics.guivision-agent` → `com.linkuistics.testanyware-agent`

- [x] **Step 3:** Verify no old references remain in Swift sources.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
grep -rc 'GUIVisionVMDriver\|GUIVisionAgentProtocol\|GUIVisionAgentLib\|GUIVISION_' Sources/ Tests/ | grep -v ':0$' || echo "clean"
```

Expected: `clean`.

- [x] **Step 4:** Verify no stray lowercase `guivision` in source code (outside strings intentionally referring to old data/paths).

```bash
grep -rn 'guivision' Sources/ Tests/
```

Expected: no matches. If matches appear, inspect each — could be an intentional historical reference (unlikely in source code) or a missed rename.

### Task 2a.4: Verify Swift build (without RoyalVNC — will fail, that's expected)

**Files:** none modified.

- [x] **Step 1:** Try to resolve + build.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
swift package resolve 2>&1 | head -20
```

Expected: dependency on `../../vendored/royalvnc` cannot resolve (we haven't placed it yet). That's fine — proceed to Task 2c, then come back.

### Task 2c.1: Vendor RoyalVNCKit

**Files:**
- Copy: `_archive/GUIVisionVMDriver/cli/macos/LocalPackages/royalvnc/` → `TestAnyware/vendored/royalvnc/`

- [x] **Step 1:** Copy the vendored fork.

```bash
rsync -a --exclude='.build/' --exclude='.swiftpm/' \
  /Users/antony/Development/_archive/GUIVisionVMDriver/cli/macos/LocalPackages/royalvnc/ \
  /Users/antony/Development/TestAnyware/vendored/royalvnc/
```

- [x] **Step 2:** Verify.

```bash
ls /Users/antony/Development/TestAnyware/vendored/royalvnc/
cat /Users/antony/Development/TestAnyware/vendored/royalvnc/Package.swift | head -10
```

Expected: `Package.swift` + RoyalVNC source tree present.

### Task 2a.5: Build Swift CLI and driver library

**Files:** none modified.

- [x] **Step 1:** Resolve dependencies.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
swift package resolve
```

Expected: no errors; `Package.resolved` appears.

- [x] **Step 2:** Build debug.

```bash
swift build
```

Expected: build succeeds. All warnings reviewed but not fatal.

- [x] **Step 3:** If build fails, fix the error in the relevant source file. Common failures:
  - Missing `import` — a file references `TestAnywareDriver` but still has `import GUIVisionVMDriver`. Fix the import line.
  - Missing type — a rename was applied to the definition but not to every use. Grep and fix.
  - Path-dep misconfigured — re-verify the `../../vendored/royalvnc` relative path resolves from `cli/macos/`.

Re-run `swift build` until green.

### Task 2a.6: Run Swift unit tests

**Files:** none modified.

- [x] **Step 1:** Run tests (unit only — no VM needed).

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
swift test --filter '!IntegrationTests'
```

Expected: all unit tests pass. Integration tests explicitly excluded (they need a VM; we run those at Milestone 6).

- [x] **Step 2:** If tests reference old symbols (e.g., assertions against `"guivision"` CLI output), update them.

- [x] **Step 3:** Commit 2a.

```bash
cd /Users/antony/Development/TestAnyware
git add cli/macos/ vendored/royalvnc/
git commit -m "feat(cli): harvest Swift CLI + driver + protocol from GUIVisionVMDriver, renamed to TestAnyware"
```

### Task 2b.1: Harvest macOS agent (Swift)

**Files:**
- Copy: `_archive/GUIVisionVMDriver/agents/macos/` → `TestAnyware/agents/macos/`

- [x] **Step 1:** Copy.

```bash
rsync -a --exclude='.build/' --exclude='.swiftpm/' --exclude='LocalPackages/' \
  /Users/antony/Development/_archive/GUIVisionVMDriver/agents/macos/ \
  /Users/antony/Development/TestAnyware/agents/macos/
```

- [x] **Step 2:** Update `agents/macos/Package.swift`:
  - Package/product names as listed in Task 2a.2.
  - `agents/macos/` has its own path-dependency on the protocol — it referenced the old repo's `cli/macos`. Update to `.package(path: "../../cli/macos")`.
  - Update its `LocalPackages/royalvnc` (if present) to `.package(path: "../../vendored/royalvnc")`.
  - Product target `GUIVisionAgentLib` → `TestAnywareAgent`.
  - Executable target `guivision-agent` → `testanyware-agent`.

- [x] **Step 3:** Rename Sources directories.

```bash
cd /Users/antony/Development/TestAnyware/agents/macos/Sources
[ -d GUIVisionAgent ] && mv GUIVisionAgent testanyware-agent
[ -d GUIVisionAgentLib ] && mv GUIVisionAgentLib TestAnywareAgent
```

(Source dir names reflect target names; adjust exact source for the source layout used by GUIVisionVMDriver.)

- [x] **Step 4:** Rename Swift source content — same replacements as Task 2a.3 (focus on `GUIVisionAgentLib`, `GUIVisionAgentProtocol`, env vars, path literals, service labels).

- [x] **Step 5:** Build.

```bash
cd /Users/antony/Development/TestAnyware/agents/macos
swift build
```

Expected: green.

- [x] **Step 6:** Run agent unit tests (if any).

```bash
swift test 2>&1 | tail -20
```

Expected: green or "no tests" — both fine.

- [x] **Step 7:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add agents/macos/
git commit -m "feat(agents): harvest macOS agent (Swift)"
```

### Task 2b.2: Harvest Linux agent (Python)

**Files:**
- Copy: `_archive/GUIVisionVMDriver/agents/linux/` → `TestAnyware/agents/linux/`

- [x] **Step 1:** Copy.

```bash
rsync -a --exclude='__pycache__/' --exclude='.venv/' \
  /Users/antony/Development/_archive/GUIVisionVMDriver/agents/linux/ \
  /Users/antony/Development/TestAnyware/agents/linux/
```

- [x] **Step 2:** Rename Python package dir if named after old project.

```bash
cd /Users/antony/Development/TestAnyware/agents/linux
[ -d guivision_agent ] && mv guivision_agent testanyware_agent
```

- [x] **Step 3:** Apply rename rules to Python files: `guivision` → `testanyware`, `GUIVISION_*` → `TESTANYWARE_*`, service name `guivision-agent.service` → `testanyware-agent.service`, path literals.

```bash
cd /Users/antony/Development/TestAnyware/agents/linux
grep -rl 'guivision\|GUIVISION_\|GUIVision' .
```

Apply edits per file.

- [x] **Step 4:** Rename systemd unit file if present.

```bash
find . -name 'guivision-agent.service' -exec git mv {} "$(dirname {})/testanyware-agent.service" \;
```

- [x] **Step 5:** Syntax-check Python.

```bash
python3 -m compileall testanyware_agent/ 2>&1 | tail -10
```

Expected: no syntax errors.

- [x] **Step 6:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add agents/linux/
git commit -m "feat(agents): harvest Linux agent (Python)"
```

### Task 2b.3: Harvest Windows agent (C# / .NET)

**Files:**
- Copy: `_archive/GUIVisionVMDriver/agents/windows/` → `TestAnyware/agents/windows/`

- [x] **Step 1:** Copy.

```bash
rsync -a --exclude='bin/' --exclude='obj/' \
  /Users/antony/Development/_archive/GUIVisionVMDriver/agents/windows/ \
  /Users/antony/Development/TestAnyware/agents/windows/
```

- [x] **Step 2:** Rename project directory and csproj.

```bash
cd /Users/antony/Development/TestAnyware/agents/windows
[ -d GUIVisionAgent ] && mv GUIVisionAgent TestAnywareAgent
find . -name 'GUIVisionAgent.csproj' -exec git mv {} "$(dirname {})/TestAnywareAgent.csproj" \;
find . -name 'GUIVisionAgent.sln' -exec git mv {} "$(dirname {})/TestAnywareAgent.sln" \;
```

- [x] **Step 3:** Apply rename rules in `.cs`, `.csproj`, `.sln` files:
  - C# namespaces `GUIVisionAgent` → `TestAnywareAgent`
  - AssemblyName / RootNamespace in csproj
  - Scheduled Task name `GUIVisionAgent` → `TestAnywareAgent`
  - Env var references, path literals as in prior tasks

- [x] **Step 4:** Build.

```bash
cd /Users/antony/Development/TestAnyware/agents/windows/TestAnywareAgent
dotnet build
```

Note: dotnet build on macOS host targets the right framework; ensure .NET 9 SDK. Expected: green or explicit issue flagged.

- [x] **Step 5:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add agents/windows/
git commit -m "feat(agents): harvest Windows agent (C#)"
```

### Task 2d.1: Harvest provisioner scripts

**Files:**
- Copy: `_archive/GUIVisionVMDriver/scripts/macos/` → `TestAnyware/provisioner/scripts/`
- Copy: any autounattend / ISO staging assets → `TestAnyware/provisioner/autounattend/`

- [x] **Step 1:** Copy scripts.

```bash
rsync -a /Users/antony/Development/_archive/GUIVisionVMDriver/scripts/macos/ \
  /Users/antony/Development/TestAnyware/provisioner/scripts/
```

- [x] **Step 2:** Locate autounattend / Windows staging assets (GUIVisionVMDriver keeps these alongside Windows golden builder or in an assets dir — inspect).

```bash
find /Users/antony/Development/_archive/GUIVisionVMDriver -name 'autounattend.xml' -o -name '*.iso.xml' 2>/dev/null
```

Copy each into `TestAnyware/provisioner/autounattend/` preserving relative layout if necessary.

- [x] **Step 3:** Apply rename rules across every file in `provisioner/`:
  - Path literals referencing `$XDG_STATE_HOME/guivision/`, `$XDG_DATA_HOME/guivision/`
  - Golden image names (`guivision-golden-*` → `testanyware-golden-*`)
  - Clone id prefix `guivision-` → `testanyware-`
  - `guivision-test-` → `testanyware-test-`
  - Agent service labels / unit names / Task Scheduler names
  - Agent binary install paths (`/usr/local/bin/guivision-agent` → `/usr/local/bin/testanyware-agent`)
  - `guivision vm start/stop/list/delete` invocations → `testanyware vm start/stop/list/delete`
  - macOS LaunchAgent plist labels `com.linkuistics.guivision-agent` → `com.linkuistics.testanyware-agent`
  - Environment variable reads (`$GUIVISION_VM_ID` → `$TESTANYWARE_VM_ID`, etc.)
  - `.guivision-vmid` sentinel filenames (if any) → `.testanyware-vmid`

Read-then-Edit each script. Scripts to look at in depth:
`vm-start.sh`, `vm-stop.sh`, `vm-list.sh`, `vm-delete.sh`,
`vm-create-golden-macos.sh`, `vm-create-golden-linux.sh`,
`vm-create-golden-windows.sh`, plus any helper shell functions.

- [x] **Step 4:** Verify no stale references.

```bash
cd /Users/antony/Development/TestAnyware/provisioner
grep -rn 'guivision\|GUIVision\|GUIVISION_' .
```

Expected: no matches (or only intentional ones, reviewed).

- [x] **Step 5:** Shell-syntax-check each script.

```bash
for f in /Users/antony/Development/TestAnyware/provisioner/scripts/*.sh; do
  bash -n "$f" && echo "OK $f" || echo "FAIL $f"
done
```

Expected: every line `OK`.

- [x] **Step 6:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add provisioner/
git commit -m "feat(provisioner): harvest VM lifecycle scripts and golden builders"
```

### Task 2e.1: Harvest Python vision pipeline base (GUIVisionPipeline)

**Files:**
- Copy: `_archive/GUIVisionPipeline/{common,stages,pipeline,swift,data}/` → `TestAnyware/vision/{common,stages,pipeline,swift,data}/`
- Copy: `_archive/GUIVisionPipeline/{pyproject.toml,uv.lock}` → `TestAnyware/vision/`
- Copy: `_archive/GUIVisionPipeline/docs/` → merge content into `TestAnyware/docs/architecture/` (Milestone 3)

- [x] **Step 1:** Copy everything except the duplicate `agents/` dir.

```bash
rsync -a --exclude='.venv/' --exclude='__pycache__/' --exclude='.pytest_cache/' \
  --exclude='agents/' \
  /Users/antony/Development/_archive/GUIVisionPipeline/ \
  /Users/antony/Development/TestAnyware/vision/
```

- [x] **Step 2:** Remove anything that shouldn't be at `vision/` top level (e.g., old README — will be rewritten in Milestone 3, CLAUDE.md — drop, .git if copied — drop).

```bash
cd /Users/antony/Development/TestAnyware/vision
rm -f README.md CLAUDE.md LICENSE .gitignore .python-version
rm -rf .git
```

- [x] **Step 3:** Inspect `pyproject.toml` workspace members. The workspace root should reference each stage as a member.

```bash
cat /Users/antony/Development/TestAnyware/vision/pyproject.toml
```

- [x] **Step 4:** If the common lib is named `guivision_common`, rename:

```bash
cd /Users/antony/Development/TestAnyware/vision
[ -d common/guivision_common ] && mv common/guivision_common common/testanyware_common
# Also update the common/pyproject.toml package name.
```

- [x] **Step 5:** Apply rename rules to every Python file in `vision/`:
  - `import guivision_common` → `import testanyware_common`
  - `from guivision_common` → `from testanyware_common`
  - Package name references in `pyproject.toml` files (every workspace member)
  - Any `guivision` / `GUIVision` / `GUIVISION_` literal
  - CLI invocations of `guivision` in docs/scripts within `vision/`

```bash
cd /Users/antony/Development/TestAnyware/vision
grep -rln 'guivision_common\|guivision\|GUIVISION_\|GUIVision' --include='*.py' --include='*.toml' --include='*.md'
```

Apply edits.

- [x] **Step 6:** Sync uv workspace.

```bash
cd /Users/antony/Development/TestAnyware/vision
uv sync
```

Expected: lockfile resolves; virtualenv created.

- [x] **Step 7:** Run unit tests.

```bash
uv run pytest -m unit 2>&1 | tail -30
```

Expected: green. If tests reference `guivision_common`, they've been renamed by Step 5 — re-run.

- [x] **Step 8:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add vision/
git commit -m "feat(vision): harvest GUIVisionPipeline stage-based vision pipeline"
```

### Task 2f.1: Create drawing-primitives stage (absorb Redraw)

**Files:**
- Create: `TestAnyware/vision/stages/drawing-primitives/pyproject.toml`
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/__init__.py`
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/color.py` (from Redraw's `python/tier3/color.py`)
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/border.py` (from Redraw's `python/tier3/border.py`)
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/shadow.py` (from Redraw's `python/tier3/shadow.py`)
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/font.py` (from Redraw's `python/tier3/font_matcher.py`)
- Create: `TestAnyware/vision/stages/drawing-primitives/drawing_primitives/stage.py` (new — the stage entry point conforming to the GUIVisionPipeline stage contract)
- Create: `TestAnyware/vision/stages/drawing-primitives/tests/test_stage.py`
- Modify: `TestAnyware/vision/pyproject.toml` to add the new workspace member.

- [x] **Step 1:** Inspect the GUIVisionPipeline stage contract. Read a representative existing stage to understand the I/O shape.

```bash
cat /Users/antony/Development/TestAnyware/vision/stages/element-detection/pyproject.toml
ls /Users/antony/Development/TestAnyware/vision/stages/element-detection/
```

Note: stage contract = inputs (image + previous-stage outputs), outputs (structured data, typed via `testanyware_common`). Document the exact contract — it shapes the stage you're about to write.

- [x] **Step 2:** Scaffold the new stage.

```bash
mkdir -p /Users/antony/Development/TestAnyware/vision/stages/drawing-primitives/{drawing_primitives,tests}
```

- [x] **Step 3:** Write `pyproject.toml` mirroring other stages.

```toml
# TestAnyware/vision/stages/drawing-primitives/pyproject.toml
[project]
name = "testanyware-drawing-primitives"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
    "testanyware-common",
    "pillow",
    "opencv-python",
    "numpy",
    "scikit-learn",           # for MiniBatchKMeans
    "scikit-image",           # for SSIM
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["drawing_primitives"]

[tool.uv.sources]
testanyware-common = { workspace = true }
```

- [x] **Step 4:** Write a failing test — the stage receives an image and element boxes, returns per-element color/border/shadow/font data.

```python
# TestAnyware/vision/stages/drawing-primitives/tests/test_stage.py
from pathlib import Path
import pytest
from PIL import Image
from testanyware_common import BoundingBox, Detection
from drawing_primitives.stage import DrawingPrimitivesStage


def test_extract_primitives_from_single_element():
    img = Image.new("RGB", (200, 100), (240, 240, 240))
    stage = DrawingPrimitivesStage()
    detections = [
        Detection(
            bbox=BoundingBox(x=10, y=10, width=100, height=50),
            label="button",
            confidence=0.9,
        )
    ]
    result = stage.run(image=img, detections=detections)
    assert len(result) == 1
    primitives = result[0]
    assert primitives.element_id == 0
    assert primitives.dominant_color is not None
    assert primitives.border is not None  # may be "none" if no edge detected
    assert primitives.shadow is not None  # may be "none" if no shadow
```

- [x] **Step 5:** Add workspace member to `vision/pyproject.toml`.

```bash
# Edit the [tool.uv.workspace] members list to add:
#   "stages/drawing-primitives"
```

- [x] **Step 6:** Run the test — expect FAIL with import error.

```bash
cd /Users/antony/Development/TestAnyware/vision
uv sync
uv run pytest stages/drawing-primitives/tests/test_stage.py -v
```

Expected: `ModuleNotFoundError: No module named 'drawing_primitives.stage'`.

- [x] **Step 7:** Write the minimal `stage.py` to make the test pass, using Redraw's tier3 code.

```python
# TestAnyware/vision/stages/drawing-primitives/drawing_primitives/stage.py
from dataclasses import dataclass
from typing import List
from PIL import Image
from testanyware_common import Detection
from .color import extract_dominant_color
from .border import detect_border
from .shadow import detect_shadow


@dataclass
class ElementPrimitives:
    element_id: int
    dominant_color: tuple  # (r, g, b)
    border: dict            # {"width": int, "color": (r,g,b)} or {}
    shadow: dict            # {"offset": (x,y), "blur": int, "color": (r,g,b)} or {}


class DrawingPrimitivesStage:
    def run(self, image: Image.Image, detections: List[Detection]) -> List[ElementPrimitives]:
        results = []
        for idx, det in enumerate(detections):
            bb = det.bbox
            crop = image.crop((bb.x, bb.y, bb.x + bb.width, bb.y + bb.height))
            results.append(
                ElementPrimitives(
                    element_id=idx,
                    dominant_color=extract_dominant_color(crop),
                    border=detect_border(crop),
                    shadow=detect_shadow(image, bb),
                )
            )
        return results
```

- [x] **Step 8:** Port the three helper functions from Redraw. For each:
  (a) read the source file at `_archive/Redraw/python/tier3/<name>.py`
  (b) extract the function's core logic
  (c) write a simplified version that takes the narrower inputs above and returns a plain dict
  (d) keep the algorithm (k-means for color, Canny-based for border, gradient-diff for shadow) — only the wrapping shape changes

Example `color.py`:

```python
# TestAnyware/vision/stages/drawing-primitives/drawing_primitives/color.py
import numpy as np
from PIL import Image
from sklearn.cluster import MiniBatchKMeans


def extract_dominant_color(img: Image.Image, n_clusters: int = 4) -> tuple:
    arr = np.array(img.convert("RGB")).reshape(-1, 3)
    if len(arr) == 0:
        return (0, 0, 0)
    n = min(n_clusters, len(arr))
    km = MiniBatchKMeans(n_clusters=n, n_init=3, random_state=0).fit(arr)
    # Dominant = largest cluster.
    counts = np.bincount(km.labels_)
    center = km.cluster_centers_[counts.argmax()]
    return tuple(int(c) for c in center)
```

Port `border.py` and `shadow.py` similarly; adapt Redraw's existing code to the simpler signature. Consult `_archive/Redraw/python/tier3/` for each.

- [x] **Step 9:** Run the test — expect PASS.

```bash
uv run pytest stages/drawing-primitives/tests/test_stage.py -v
```

Expected: PASS.

- [x] **Step 10:** Write a second test — font matching.

```python
def test_font_matching_returns_best_candidate():
    # Skipped unless font db available. Keep minimal; full test in Milestone 6.
    pass
```

(A full font-matching test requires the font reference database from Redraw's `training/fonts/` — that's integration-level work; leave a stub for Milestone 6.)

- [x] **Step 11:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add vision/stages/drawing-primitives/ vision/pyproject.toml
git commit -m "feat(vision): add drawing-primitives stage (absorbed from Redraw)"
```

### Task 2g.1: Create icon-classification stage (from TestAnyware v1)

**Files:**
- Create: `TestAnyware/vision/stages/icon-classification/pyproject.toml`
- Create: `TestAnyware/vision/stages/icon-classification/icon_classification/__init__.py`
- Create: `TestAnyware/vision/stages/icon-classification/icon_classification/classifier.py`
- Create: `TestAnyware/vision/stages/icon-classification/icon_classification/stage.py`
- Create: `TestAnyware/vision/stages/icon-classification/tests/test_stage.py`
- Copy: training data + model artifacts from `_archive/TestAnyware/` → `TestAnyware/vision/stages/icon-classification/data/`
- Modify: `TestAnyware/vision/pyproject.toml` to add workspace member.

- [x] **Step 1:** Locate icon classification work in the TestAnyware v1 archive.

```bash
cd /Users/antony/Development/_archive/TestAnyware
grep -rln 'icon.*classif\|classif.*icon' Sources/ Tests/ docs/ 2>/dev/null
find . -name '*icon*' -type f 2>/dev/null | head -20
```

Expected: a handful of files — training data, a model file (e.g., `.coreml` or `.onnx`), and the Swift code that uses it. Document findings.

- [x] **Step 2:** The classifier was originally Swift-side (CoreML inference). Decide port strategy:
  - If model is a CoreML file: call it via Python CoreML inference (macOS-only) OR re-export to ONNX for cross-platform.
  - If model is ONNX or PyTorch: use directly.
  - If only training data exists (no model): treat this as a future training task; port the training pipeline skeleton and note "model to be trained" in the stage README.

Document the decision inline; proceed with whichever matches what the archive actually holds.

- [x] **Step 3:** Scaffold the stage dir.

```bash
mkdir -p /Users/antony/Development/TestAnyware/vision/stages/icon-classification/{icon_classification,tests,data}
```

- [x] **Step 4:** Write `pyproject.toml`.

```toml
[project]
name = "testanyware-icon-classification"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
    "testanyware-common",
    "pillow",
    "numpy",
    # add coremltools or onnxruntime depending on model format
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["icon_classification"]

[tool.uv.sources]
testanyware-common = { workspace = true }
```

- [x] **Step 5:** Copy model + training data artifacts into `data/`.

```bash
# Adapt to actual filenames.
cp /Users/antony/Development/_archive/TestAnyware/<icon-model-file> \
   /Users/antony/Development/TestAnyware/vision/stages/icon-classification/data/
```

- [x] **Step 6:** Write failing test.

```python
# TestAnyware/vision/stages/icon-classification/tests/test_stage.py
from pathlib import Path
from PIL import Image
from testanyware_common import BoundingBox, Detection
from icon_classification.stage import IconClassificationStage


def test_classifies_back_button():
    fixture = Path(__file__).parent / "fixtures/back_button.png"
    if not fixture.exists():
        # Generate a synthetic fixture if real one not yet ported.
        fixture.parent.mkdir(exist_ok=True)
        Image.new("RGB", (24, 24), (100, 100, 100)).save(fixture)
    img = Image.open(fixture)
    stage = IconClassificationStage()
    detections = [
        Detection(
            bbox=BoundingBox(x=0, y=0, width=24, height=24),
            label="button",
            confidence=0.9,
        )
    ]
    result = stage.run(image=img, detections=detections)
    assert len(result) == 1
    assert result[0].icon_label is not None  # either a class label or "unknown"
```

- [x] **Step 7:** Add workspace member to `vision/pyproject.toml`.

- [x] **Step 8:** Run test — expect FAIL.

```bash
cd /Users/antony/Development/TestAnyware/vision
uv sync
uv run pytest stages/icon-classification/tests/test_stage.py -v
```

- [x] **Step 9:** Write minimal `stage.py` + `classifier.py`.

```python
# icon_classification/stage.py
from dataclasses import dataclass
from typing import List, Optional
from PIL import Image
from testanyware_common import Detection
from .classifier import IconClassifier


@dataclass
class IconClassification:
    element_id: int
    icon_label: Optional[str]
    confidence: float


class IconClassificationStage:
    def __init__(self, model_path: Optional[str] = None):
        self.classifier = IconClassifier(model_path)

    def run(self, image: Image.Image, detections: List[Detection]) -> List[IconClassification]:
        results = []
        for idx, det in enumerate(detections):
            if det.label not in ("button", "toolbar-button", "toggle-button"):
                results.append(IconClassification(element_id=idx, icon_label=None, confidence=0.0))
                continue
            bb = det.bbox
            crop = image.crop((bb.x, bb.y, bb.x + bb.width, bb.y + bb.height))
            label, conf = self.classifier.classify(crop)
            results.append(IconClassification(element_id=idx, icon_label=label, confidence=conf))
        return results
```

```python
# icon_classification/classifier.py
from pathlib import Path
from typing import Optional, Tuple
from PIL import Image


class IconClassifier:
    def __init__(self, model_path: Optional[str] = None):
        default = Path(__file__).parent.parent / "data" / "icon_model.onnx"
        self.model_path = Path(model_path) if model_path else default
        self._session = None
        if self.model_path.exists():
            try:
                import onnxruntime as ort
                self._session = ort.InferenceSession(str(self.model_path))
            except ImportError:
                pass

    def classify(self, img: Image.Image) -> Tuple[str, float]:
        if self._session is None:
            return ("unknown", 0.0)
        # Resize to model input size (adjust to actual model).
        img = img.convert("RGB").resize((24, 24))
        # Run inference — placeholder; implement per real model's IO spec.
        return ("unknown", 0.0)
```

(The skeleton is intentional — it honors the stage contract even if the model isn't ready. The real inference code is plugged in when the model format is confirmed in Step 2.)

- [x] **Step 10:** Run test — expect PASS.

```bash
uv run pytest stages/icon-classification/tests/test_stage.py -v
```

- [x] **Step 11:** Wire into the pipeline orchestrator (Task 2e already harvested `vision/pipeline/`). Add icon-classification to the orchestrator's stage list, after `element-detection`.

Read `vision/pipeline/` orchestrator; add the new stage in the correct position.

- [x] **Step 12:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add vision/stages/icon-classification/ vision/pyproject.toml vision/pipeline/
git commit -m "feat(vision): add icon-classification stage (harvested from TestAnyware v1)"
```

### Task 2h.1: Migrate LLM_STATE with reference rewrites

**Files:**
- Copy: `_archive/GUIVisionVMDriver/LLM_STATE/{core,vision-pipeline,ocr-accuracy}/` → `TestAnyware/LLM_STATE/`
- Modify: every `.md` under the copied tree to replace old references.

- [x] **Step 1:** Copy the tree.

```bash
rsync -a /Users/antony/Development/_archive/GUIVisionVMDriver/LLM_STATE/ \
  /Users/antony/Development/TestAnyware/LLM_STATE/
```

- [x] **Step 2:** Enumerate files to review.

```bash
cd /Users/antony/Development/TestAnyware/LLM_STATE
find . -name '*.md' -print
```

Expect ~15-25 markdown files.

- [x] **Step 3:** For each file, open with `Read`, identify every `guivision`/`GUIVision`/`GUIVISION_` / old-path reference, classify each:
  - **Live reference** (refers to something that still exists, just renamed): rewrite.
  - **Historical reference** (session-log entry describing what happened on date X with the old name): leave as-is — it's history.
  - **Task description referencing future work**: rewrite to the new name since the work will use the new name.

Default rule: **rewrite unless the line is inside a dated session-log entry or explicitly marked as historical.**

Pay specific attention to:
- Path references (every XDG path, every agent-binary path)
- Env var mentions in task descriptions
- Code snippets in backlog items (shell commands that will be run)
- Swift symbol references in "files to modify" lists
- Golden image names
- CLI invocations

- [x] **Step 4:** Apply edits. For any file with only routine CLI invocation / env-var references (no historical context), one pass with `Edit` per pattern is fine. For backlog items describing specific code locations, verify the new path is correct.

- [x] **Step 5:** Verify no live-reference residue.

```bash
cd /Users/antony/Development/TestAnyware/LLM_STATE
grep -rn 'guivision\|GUIVision\|GUIVISION_' --include='*.md' .
```

Expected output: only matches inside clearly-historical sections (session logs, dated entries). If any match looks like a current/future task, rewrite it.

- [x] **Step 6:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add LLM_STATE/
git commit -m "feat(llm-state): migrate Raveloop plan state, rewrite references to new names"
```

### Gate 2

- [x] Swift CLI builds:
  `swift build --package-path cli/macos` → green.
- [x] Swift macOS agent builds:
  `cd agents/macos && swift build` → green.
- [x] Swift unit tests pass:
  `swift test --package-path cli/macos --filter '!IntegrationTests'` → green.
- [x] Linux agent Python compiles:
  `python3 -m compileall agents/linux/testanyware_agent/` → clean.
- [x] Windows agent builds (on the macOS host if SDK present):
  `cd agents/windows/TestAnywareAgent && dotnet build` → green.
- [x] Vision workspace syncs & unit tests pass:
  `cd vision && uv sync && uv run pytest -m unit` → green.
- [x] No stale Swift/Python/config references:
  `grep -rc 'GUIVisionVMDriver\|GUIVisionAgentProtocol\|GUIVisionAgentLib\|GUIVISION_' cli/ agents/ vision/ provisioner/` → 0.
- [x] Provisioner scripts pass syntax check:
  `for f in provisioner/scripts/*.sh; do bash -n "$f"; done` → no errors.

---

## Milestone 3 — Docs

Goal: `README.md` and `LLM_INSTRUCTIONS.md` complete and canonical; `docs/` tree populated; component READMEs present; this design doc committed to `0-docs/designs/`.

### Task 3.1: Port and update the main README

**Files:**
- Modify: `TestAnyware/README.md` (replace placeholder with full content, ported from `_archive/GUIVisionVMDriver/README.md`).

- [ ] **Step 1:** Read the source README.

```bash
cat /Users/antony/Development/_archive/GUIVisionVMDriver/README.md | wc -l
```

- [ ] **Step 2:** Port content, applying all renames per design §7. Rewrite section structure:

1. **What It Does** — keep as-is, renamed.
2. **CLI** — keep command examples, every invocation renamed `guivision` → `testanyware`.
3. **Library** — Swift usage with `TestAnywareDriver` / `TestAnywareAgentProtocol`.
4. **Architecture** — update to reflect the monorepo layout (design §4).
5. **Tech Stack** — expanded to include vision pipeline (Python) and guest-agent languages.
6. **Building from Source** — updated paths (`cli/macos/`, `agents/*`, `vision/`).
7. **Integration Testing** — updated with new image names.
8. **Scripts + Environment variables + Connection resolution** — all in §3 of README is accurate with renames.
9. **Golden Image Contents** — renamed image names.
10. **Requirements** — updated install commands.
11. **Development Conventions** — ported.
12. **Gotchas** — moved to `docs/user/troubleshooting.md` (Task 3.5); README links there.
13. **Key Directories** — rewritten to reflect new layout.
14. **LLM Integration** — link to `LLM_INSTRUCTIONS.md`.

Add a top section:

```markdown
# TestAnyware

> AI-driven GUI testing across virtual machines. Cross-platform guest
> support (macOS, Linux, Windows) via VNC + in-VM HTTP agents. Python
> vision pipeline for structured screen understanding.

**For LLM consumers:** see [LLM_INSTRUCTIONS.md](LLM_INSTRUCTIONS.md).
**For contributors:** see [`docs/`](docs/).
**Design history:** see [`0-docs/designs/`](0-docs/designs/).
```

Write the full file.

- [ ] **Step 3:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add README.md
git commit -m "docs: write canonical README"
```

### Task 3.2: Port and extend `LLM_INSTRUCTIONS.md`

**Files:**
- Modify: `TestAnyware/LLM_INSTRUCTIONS.md`

- [ ] **Step 1:** Read the source.

```bash
cat /Users/antony/Development/_archive/GUIVisionVMDriver/instructions-for-llms-using-this-as-a-tool.md | wc -l
```

- [ ] **Step 2:** Port content, renaming every symbol per design §7. Keep the dense-reference style (tables, copy-paste snippets, exact JSON shapes).

- [ ] **Step 3:** Add a new section for the vision pipeline. Example outline:

```markdown
## Vision Pipeline

A Python `uv` workspace at `vision/` decomposes screenshots into
structured UI data through sequential stages:

| Stage | Input | Output |
|---|---|---|
| window-detection | Screenshot | Per-window bounding boxes |
| chrome-detection | Screenshot + windows | OS chrome regions |
| element-detection | Window crop | Per-element detections |
| icon-classification | Element crops (buttons) | Semantic icon labels |
| drawing-primitives | Element crops | Color, border, shadow, font |
| menu-detection | Screenshot | Menu bar + contextual menus |
| ocr | Screenshot | Text regions with Vision framework |
| state-detection | Elements + OCR | Enabled/disabled/checked inference |

### Running the pipeline

```bash
cd vision
uv run python -m pipeline.orchestrator --image path/to/screen.png
```

### Calling from the CLI

The `testanyware` CLI does not yet shell out to the vision pipeline
directly. Until then, callers invoke the pipeline as a Python
subprocess or via its FastAPI server (see `docs/architecture/vision-pipeline.md`).
```

- [ ] **Step 4:** Commit.

```bash
git add LLM_INSTRUCTIONS.md
git commit -m "docs: write canonical LLM_INSTRUCTIONS.md"
```

### Task 3.3: Write `docs/reference/` tables

**Files:**
- Create: `docs/reference/cli-commands.md`
- Create: `docs/reference/env-vars.md`
- Create: `docs/reference/connection-spec.md`
- Create: `docs/reference/key-names.md`
- Create: `docs/reference/error-codes.md`

- [ ] **Step 1:** Write `cli-commands.md` — exhaustive table of every subcommand, flag, and example. Extract from the CLI's `--help` output.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
swift run testanyware --help > /tmp/testanyware-help.txt
# Enumerate subcommands:
for sub in screenshot input agent vm exec upload download find-text record screen-size; do
  swift run testanyware "$sub" --help >> /tmp/testanyware-help.txt
done
```

Format into a Markdown table: command, synopsis, flags, example.

- [ ] **Step 2:** Write `env-vars.md` — one row per env var (design §7 rename table's env-var rows).

- [ ] **Step 3:** Write `connection-spec.md` — the JSON schema for the spec file (extract from `README.md` section "Per-VM spec file format" or source code).

- [ ] **Step 4:** Write `key-names.md` — letters, digits, specials, arrows, navigation, function keys; modifiers. Source: `README.md` `Key names` section.

- [ ] **Step 5:** Write `error-codes.md` — scan CLI source for error-type enums and agent HTTP error shapes; tabulate.

```bash
grep -rn 'enum.*Error\|throw.*Error\|CliError' cli/macos/Sources/
```

- [ ] **Step 6:** Commit.

```bash
git add docs/reference/
git commit -m "docs(reference): write exhaustive CLI/env/spec/keys/errors tables"
```

### Task 3.4: Write `docs/architecture/`

**Files:**
- Create: `docs/architecture/overview.md`
- Create: `docs/architecture/agent-protocol.md`
- Create: `docs/architecture/vision-pipeline.md`
- Create: `docs/architecture/vm-lifecycle.md`

- [ ] **Step 1:** `overview.md` — two-channel design, component map (1 page + diagram description).

- [ ] **Step 2:** `agent-protocol.md` — JSON-RPC 2.0 schema, endpoint list, request/response shapes, examples. Source: port from `_archive/GUIVisionVMDriver/docs/` if present, otherwise extract from Swift code in `cli/macos/Sources/TestAnywareAgentProtocol/`.

- [ ] **Step 3:** `vision-pipeline.md` — stage contract, I/O types, pipeline orchestrator flow. Source: port from `_archive/GUIVisionPipeline/docs/`.

- [ ] **Step 4:** `vm-lifecycle.md` — tart vs QEMU, spec-file schema, XDG paths, clone/cache/golden layout.

- [ ] **Step 5:** Commit.

```bash
git add docs/architecture/
git commit -m "docs(architecture): overview, agent protocol, vision pipeline, vm lifecycle"
```

### Task 3.5: Write `docs/user/`

**Files:**
- Create: `docs/user/quick-start.md`
- Create: `docs/user/golden-images.md`
- Create: `docs/user/multi-vm-networking.md`
- Create: `docs/user/video-recording.md`
- Create: `docs/user/troubleshooting.md`

- [ ] **Step 1:** `quick-start.md` — install, build, create golden, start VM, take screenshot, stop. ~1 page.

- [ ] **Step 2:** `golden-images.md` — full content of "Golden Image Contents" section from source README.

- [ ] **Step 3:** `multi-vm-networking.md` — multi-VM setup from source README.

- [ ] **Step 4:** `video-recording.md` — `testanyware record` usage, codec details.

- [ ] **Step 5:** `troubleshooting.md` — migrate the entire `Gotchas` section from source README. Every bullet intact, all names renamed.

- [ ] **Step 6:** Commit.

```bash
git add docs/user/
git commit -m "docs(user): quick-start, goldens, networking, recording, troubleshooting"
```

### Task 3.6: Write `docs/components/` and per-component READMEs

**Files:**
- Create: `docs/components/{cli,agents-macos,agents-linux,agents-windows,vision,provisioner}.md`
- Create: `cli/macos/README.md`, `cli/linux/README.md`
- Create: `agents/macos/README.md`, `agents/linux/README.md`, `agents/windows/README.md`
- Create: `vision/README.md`
- Create: `provisioner/README.md`

- [ ] **Step 1:** For each `docs/components/<name>.md`: maintainer-facing details — module layout, key files, build commands, test commands, common pitfalls.

- [ ] **Step 2:** For each component `README.md`: contributor-facing quick reference — "to work on this component, run X; tests live at Y; see `docs/components/<name>.md` for depth".

- [ ] **Step 3:** `cli/linux/README.md` — placeholder: "Planned — cross-platform Linux host driver. See `0-docs/designs/` for scope signals. Not yet implemented."

- [ ] **Step 4:** Commit.

```bash
git add docs/components/ cli/*/README.md agents/*/README.md vision/README.md provisioner/README.md
git commit -m "docs(components): maintainer docs + per-component READMEs"
```

### Task 3.7: Move design + prompt into the repo

**Files:**
- Copy: `/Users/antony/Development/0-docs/designs/TestAnyware-Unification.design.md` → `TestAnyware/0-docs/designs/`
- Copy: `/Users/antony/Development/0-docs/prompts/TestAnyware-Unification.prompt.md` → `TestAnyware/0-docs/prompts/`
- Copy: `/Users/antony/Development/0-docs/plans/TestAnyware-Unification.plan.md` → `TestAnyware/0-docs/plans/`

- [ ] **Step 1:** Copy.

```bash
cp /Users/antony/Development/0-docs/designs/TestAnyware-Unification.design.md \
   /Users/antony/Development/TestAnyware/0-docs/designs/
cp /Users/antony/Development/0-docs/prompts/TestAnyware-Unification.prompt.md \
   /Users/antony/Development/TestAnyware/0-docs/prompts/
cp /Users/antony/Development/0-docs/plans/TestAnyware-Unification.plan.md \
   /Users/antony/Development/TestAnyware/0-docs/plans/
```

- [ ] **Step 2:** Commit.

```bash
cd /Users/antony/Development/TestAnyware
git add 0-docs/
git commit -m "docs(0-docs): commit unification design, plan, and prompt"
```

### Gate 3

- [ ] `README.md` ≥ 150 lines, references only new names.
- [ ] `LLM_INSTRUCTIONS.md` is comprehensive enough for a cold LLM to drive the CLI (manual human review).
- [ ] `grep -rn 'guivision\|GUIVision\|GUIVISION_' docs/ README.md LLM_INSTRUCTIONS.md` → 0 (outside inside `docs/user/troubleshooting.md` where quoting error messages verbatim may preserve historical names in *quoted text only*).
- [ ] `0-docs/designs/TestAnyware-Unification.design.md` in place.
- [ ] Every component directory has a `README.md`.

---

## Milestone 4 — Build & Smoke Test

Goal: a working `testanyware` binary at `/usr/local/bin/testanyware` that runs offline subcommands cleanly and contains no residual `guivision` symbols.

### Task 4.1: Release build

**Files:** none modified.

- [ ] **Step 1:** Build release.

```bash
cd /Users/antony/Development/TestAnyware/cli/macos
swift build -c release
```

Expected: green. Output binary at `cli/macos/.build/release/testanyware`.

### Task 4.2: Install to `/usr/local/bin/testanyware`

**Files:**
- Create: `/usr/local/bin/testanyware` (symlink).

- [ ] **Step 1:** Remove any existing `/usr/local/bin/guivision` (stale from previous installs).

```bash
[ -e /usr/local/bin/guivision ] && sudo rm /usr/local/bin/guivision
```

- [ ] **Step 2:** Install the new binary as a symlink (matches current install convention).

```bash
sudo ln -sf /Users/antony/Development/TestAnyware/cli/macos/.build/release/testanyware \
  /usr/local/bin/testanyware
```

- [ ] **Step 3:** Verify.

```bash
which testanyware
ls -l /usr/local/bin/testanyware
```

Expected: path points at the build output.

### Task 4.3: Smoke test offline subcommands

**Files:** none modified.

- [ ] **Step 1:** `--help` works.

```bash
testanyware --help
```

Expected: CLI help printed; no errors.

- [ ] **Step 2:** `--version` (if implemented).

```bash
testanyware --version 2>&1 || true
```

Accept either version output or a clean "unknown flag" — just no crash.

- [ ] **Step 3:** `vm list` runs cleanly without a VM.

```bash
testanyware vm list
```

Expected: returns "no VMs" (or equivalent) cleanly.

- [ ] **Step 4:** Subcommand help for each area.

```bash
testanyware screenshot --help
testanyware input --help
testanyware agent --help
testanyware vm --help
testanyware exec --help
testanyware find-text --help
testanyware record --help
```

Expected: help text for each; every example uses `testanyware`, no stray `guivision`.

### Task 4.4: Verify binary is free of old symbols

**Files:** none modified.

- [ ] **Step 1:** Check strings.

```bash
strings /Users/antony/Development/TestAnyware/cli/macos/.build/release/testanyware | grep -ci 'guivision\|GUIVision' || echo "clean"
```

Expected: `clean`.

- [ ] **Step 2:** If matches, review each and fix the source. Rebuild and re-verify.

### Gate 4

- [ ] `swift build -c release` passes.
- [ ] `/usr/local/bin/testanyware` exists and runs.
- [ ] All subcommands respond to `--help`.
- [ ] No `guivision` strings in the release binary.

---

## Milestone 5 — Golden Images (rebuild)

Goal: three fresh golden images with the new `testanyware-golden-*` names, the renamed agent binary installed, and the new XDG paths.

### Task 5.1: Rebuild macOS golden

**Files:** golden image `testanyware-golden-macos-tahoe` on local `tart`.

- [ ] **Step 1:** Run the builder.

```bash
cd /Users/antony/Development/TestAnyware/provisioner/scripts
./vm-create-golden-macos.sh
```

Expected: script completes. Budget ~10 minutes. If it errors, fix the script (likely a leftover `guivision` reference caught by the rename pass but mis-scoped).

- [ ] **Step 2:** Verify.

```bash
tart list | grep testanyware-golden-macos-tahoe
```

Expected: one entry.

### Task 5.2: Rebuild Linux golden

- [ ] **Step 1:** Run.

```bash
cd /Users/antony/Development/TestAnyware/provisioner/scripts
./vm-create-golden-linux.sh
```

Budget ~10 minutes.

- [ ] **Step 2:** Verify.

```bash
tart list | grep testanyware-golden-linux-24.04
```

### Task 5.3: Rebuild Windows golden

- [ ] **Step 1:** Locate the Windows ISO cache.

```bash
ls "${XDG_DATA_HOME:-$HOME/.local/share}/testanyware/cache/" 2>/dev/null
```

If the ISO was previously cached under `~/.local/share/guivision/cache/`, move it.

```bash
if [ -d "$HOME/.local/share/guivision/cache" ] && [ ! -d "$HOME/.local/share/testanyware/cache" ]; then
  mkdir -p "$HOME/.local/share/testanyware"
  mv "$HOME/.local/share/guivision/cache" "$HOME/.local/share/testanyware/cache"
fi
```

- [ ] **Step 2:** Run.

```bash
cd /Users/antony/Development/TestAnyware/provisioner/scripts
./vm-create-golden-windows.sh
```

Budget ~20-40 minutes (ISO cached; no re-download).

- [ ] **Step 3:** Verify.

```bash
ls "${XDG_DATA_HOME:-$HOME/.local/share}/testanyware/golden/" | grep testanyware-golden-windows-11
```

### Gate 5

- [ ] `tart list` shows both `testanyware-golden-macos-tahoe` and `testanyware-golden-linux-24.04`.
- [ ] `testanyware-golden-windows-11` present under `$XDG_DATA_HOME/testanyware/golden/`.
- [ ] No VM with the old `guivision-golden-*` name exists locally (if it does, delete with `tart delete <name>` or per-QEMU cleanup — user-confirm destructive).

---

## Milestone 6 — Integration Tests

Goal: end-to-end functional verification across all three guest platforms.

### Task 6.1: macOS integration smoke

- [ ] **Step 1:** Start VM.

```bash
vmid=$(/Users/antony/Development/TestAnyware/provisioner/scripts/vm-start.sh)
export TESTANYWARE_VM_ID="$vmid"
echo "Started $vmid"
```

- [ ] **Step 2:** Basic operations.

```bash
testanyware screenshot -o /tmp/mac-smoke.png && echo "screenshot OK"
testanyware agent health && echo "health OK"
testanyware agent snapshot --mode interact | head -5 && echo "snapshot OK"
testanyware input type "hello" && echo "type OK"
testanyware exec "uname -a" && echo "exec OK"
```

- [ ] **Step 3:** Stop.

```bash
/Users/antony/Development/TestAnyware/provisioner/scripts/vm-stop.sh "$vmid"
```

### Task 6.2: Linux integration smoke

- [ ] **Step 1:** Repeat Task 6.1 flow with `--platform linux` on `vm-start.sh`.

```bash
vmid=$(/Users/antony/Development/TestAnyware/provisioner/scripts/vm-start.sh --platform linux)
export TESTANYWARE_VM_ID="$vmid"
testanyware screenshot -o /tmp/lin-smoke.png
testanyware agent health
testanyware exec "uname -a"
/Users/antony/Development/TestAnyware/provisioner/scripts/vm-stop.sh "$vmid"
```

### Task 6.3: Windows integration smoke

- [ ] **Step 1:** Repeat with `--platform windows`.

```bash
vmid=$(/Users/antony/Development/TestAnyware/provisioner/scripts/vm-start.sh --platform windows)
export TESTANYWARE_VM_ID="$vmid"
testanyware screenshot -o /tmp/win-smoke.png
testanyware agent health
testanyware exec 'systeminfo | findstr /B /C:"OS Name"'
/Users/antony/Development/TestAnyware/provisioner/scripts/vm-stop.sh "$vmid"
```

### Task 6.4: Full Swift integration test suite

- [ ] **Step 1:** Start macOS VM, run tests, stop.

```bash
vmid=$(/Users/antony/Development/TestAnyware/provisioner/scripts/vm-start.sh)
export TESTANYWARE_VM_ID="$vmid"
cd /Users/antony/Development/TestAnyware/cli/macos
swift test --filter IntegrationTests 2>&1 | tail -30
/Users/antony/Development/TestAnyware/provisioner/scripts/vm-stop.sh "$vmid"
```

Expected: all tests pass (or a small known-failing set consistent with current LLM_STATE/core/backlog.md items 12 and 13).

### Task 6.5: Vision integration tests

- [ ] **Step 1:** Start a VM; run vision integration tests that need live screenshots.

```bash
vmid=$(/Users/antony/Development/TestAnyware/provisioner/scripts/vm-start.sh)
export TESTANYWARE_VM_ID="$vmid"
cd /Users/antony/Development/TestAnyware/vision
uv run pytest -m integration 2>&1 | tail -30
/Users/antony/Development/TestAnyware/provisioner/scripts/vm-stop.sh "$vmid"
```

Expected: green.

### Gate 6 (the safe-point milestone)

- [ ] macOS/Linux/Windows smokes all pass.
- [ ] Swift `IntegrationTests` pass (modulo documented backlog).
- [ ] Vision `integration`-marked tests pass.

**After Gate 6 passes, old material can start coming down.**

---

## Milestone 7 — `~/Development` Scan and Downstream Migration

Goal: every active project under `~/Development/` (outside `_archive/`) is updated to use the new names. Nothing references `guivision`/`GUIVisionVMDriver`/`GUIVISION_*` except historical session logs and archived material.

### Task 7.1: Produce the reference inventory

**Files:**
- Create: `/Users/antony/Development/0-docs/designs/TestAnyware-Unification.rename-inventory.md`

- [ ] **Step 1:** Scan bucket 1 — code files.

```bash
cd /Users/antony/Development
grep -rln --include='*.swift' --include='*.py' --include='*.cs' --include='*.fs' \
  --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.js' \
  --include='*.go' --include='*.rb' \
  'guivision\|GUIVision\|GUIVISION_' \
  --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git \
  . | sort
```

- [ ] **Step 2:** Scan bucket 2 — config/data.

```bash
grep -rln --include='*.yml' --include='*.yaml' --include='*.json' --include='*.toml' \
  --include='*.plist' --include='*.sh' --include='*.zsh' --include='*.bash' \
  --include='.env*' \
  'guivision\|GUIVISION_' \
  --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git \
  . | sort
```

- [ ] **Step 3:** Scan bucket 3 — documentation.

```bash
grep -rln --include='*.md' --include='*.rst' --include='*.adoc' --include='*.html' --include='*.txt' \
  'guivision\|GUIVision\|GUIVisionVMDriver\|GUIVisionPipeline\|Redraw\|TestAnywareRedux' \
  --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git \
  . | sort
```

- [ ] **Step 4:** Scan bucket 4 — Raveloop plan state across projects.

```bash
find . -path './_archive' -prune -o -path './TestAnyware' -prune -o \
  -path '*/LLM_STATE/*' -name '*.md' -print | xargs grep -l 'guivision\|GUIVision\|GUIVISION_' 2>/dev/null | sort
```

- [ ] **Step 5:** Write inventory file grouping by project + bucket, with classification column:

```markdown
# Rename Inventory — 2026-04-19

## <ProjectName>
### Code
- `path/to/file.swift` (3 matches): **rewrite** — import + symbol references
- `path/to/other.py` (1 match): **review** — may be a quoted historical name

### Config
...

### Docs
- `README.md` (12 matches): **rewrite** — CLI examples need new name
- `CHANGELOG.md` (4 matches): **leave** — historical changelog entries

### LLM_STATE
- `LLM_STATE/core/backlog.md` (5 matches): **review** — task descriptions may describe current work (rewrite) or historical entries (leave)
```

- [ ] **Step 6:** Commit the inventory to `0-docs/`.

```bash
cd /Users/antony/Development/0-docs
# no git here unless 0-docs is a repo
# Also commit into TestAnyware/0-docs/designs/ if desired.
```

### Task 7.2: Apply rewrites per project

For each project in the inventory, work through its **rewrite** entries. Commit per project.

- [ ] **Step 1:** For each project with rewrite entries:

  1. Read each listed file.
  2. Apply replacements per design §7.
  3. Run the project's own tests / build if quick (not if it requires infrastructure we don't have).
  4. Commit inside that project's repo:
     `git commit -m "chore: rename guivision references to testanyware"`.

- [ ] **Step 2:** For each **review** entry, decide per-line whether to rewrite or leave. Document decisions.

- [ ] **Step 3:** Re-scan after all rewrites.

```bash
cd /Users/antony/Development
grep -rln 'guivision\|GUIVision\|GUIVISION_' \
  --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git \
  . | sort
```

Remaining matches should be exclusively in **leave** (historical) entries. Cross-check against the inventory.

### Task 7.3: Update Raveloop defaults and templates

**Files:**
- Modify: `/Users/antony/Development/Raveloop/defaults/*` (only files that reference `guivision` as an example or template)

- [ ] **Step 1:** Scan Raveloop specifically.

```bash
grep -rln 'guivision\|GUIVision\|GUIVISION_' /Users/antony/Development/Raveloop/ 2>/dev/null
```

- [ ] **Step 2:** For each hit, read and decide:
  - `defaults/*` examples that reference `guivision` as a canonical example → rewrite to `testanyware`.
  - `src/*` Rust code (if any references) → rewrite.
  - `docs/*` → rewrite unless historical.
  - `LLM_STATE/*` inside Raveloop (Raveloop's own plan state) → only rewrite live-reference lines.

- [ ] **Step 3:** Build and test Raveloop.

```bash
cd /Users/antony/Development/Raveloop
cargo build
cargo test
```

Expected: green.

- [ ] **Step 4:** Commit.

```bash
git commit -am "chore: update guivision references to testanyware in defaults and docs"
```

### Task 7.4: Spot-check downstream LLM-driven projects

- [ ] **Step 1:** Identify 1-2 projects that actively use the CLI from LLM-driven workflows. (The user knows which these are — ask if unclear.)

- [ ] **Step 2:** Run a small task through each — one that exercises the renamed CLI end-to-end. Confirm no regressions.

### Gate 7

- [ ] Final scan returns only **leave** (historical) entries:
  `grep -rln 'guivision\|GUIVisionVMDriver\|GUIVISION_' ~/Development/ --exclude-dir=_archive --exclude-dir=.git` should be empty or match the inventory's **leave** list exactly.
- [ ] At least one downstream project has been tested end-to-end with the new CLI and reported green.

---

## Milestone 8 — GitHub Operations

Goal: new repo published to `github.com/linkuistics/TestAnyware`; five old repos deleted from the org.

**All steps in this milestone that contain `[USER CONFIRM]` require an explicit user "yes" before executing.**

### Task 8.1: Local commit hygiene

- [ ] **Step 1:** Ensure working tree is clean in the new repo.

```bash
cd /Users/antony/Development/TestAnyware
git status
```

Expected: `working tree clean`. If not, commit pending work.

- [ ] **Step 2:** View recent commit history.

```bash
git log --oneline -30
```

Sanity-check the list matches the milestones executed.

### Task 8.2: Free the `linkuistics/TestAnyware` name (delete v1 on GH)

**[USER CONFIRM]** This deletes `github.com/linkuistics/TestAnyware` (the v1 repo).

- [ ] **Step 1:** Verify the v1 repo exists and is the one to delete.

```bash
gh repo view linkuistics/TestAnyware
```

Inspect: confirm this is v1 (stars count ~0, last-commit date old, description matches v1).

- [ ] **Step 2:** **[USER CONFIRM]** Ask user explicitly: "About to delete github.com/linkuistics/TestAnyware (the v1 repo). Confirm?" Wait for explicit "yes".

- [ ] **Step 3:** Delete.

```bash
gh repo delete linkuistics/TestAnyware --yes
```

- [ ] **Step 4:** Verify.

```bash
gh repo view linkuistics/TestAnyware 2>&1 | grep -i 'could not\|not found' || echo "unexpected — still exists"
```

Expected: "Could not resolve" or "not found".

### Task 8.3: Create new repo and push

- [ ] **Step 1:** Create + push.

```bash
cd /Users/antony/Development/TestAnyware
gh repo create linkuistics/TestAnyware \
  --public \
  --description "AI-driven GUI testing across virtual machines" \
  --source=. \
  --push
```

- [ ] **Step 2:** Verify.

```bash
gh repo view linkuistics/TestAnyware
git remote -v
git branch -vv
```

Expected: origin set to the new GitHub URL; `main` tracking `origin/main`.

### Task 8.4: Delete the other four old repos

**[USER CONFIRM]** This deletes four repos.

- [ ] **Step 1:** Verify each exists.

```bash
for r in GUIVisionVMDriver GUIVisionPipeline Redraw TestAnywareRedux; do
  echo "=== $r ==="
  gh repo view "linkuistics/$r" 2>&1 | head -3
done
```

- [ ] **Step 2:** **[USER CONFIRM]** Ask user: "About to delete github.com/linkuistics/{GUIVisionVMDriver,GUIVisionPipeline,Redraw,TestAnywareRedux}. Confirm?" Wait for explicit "yes".

- [ ] **Step 3:** Delete.

```bash
for r in GUIVisionVMDriver GUIVisionPipeline Redraw TestAnywareRedux; do
  gh repo delete "linkuistics/$r" --yes
done
```

- [ ] **Step 4:** Verify.

```bash
gh repo list linkuistics --limit 100 | grep -E 'GUIVision|Redraw|TestAnywareRedux' || echo "clean"
```

Expected: `clean`.

### Gate 8

- [ ] `gh repo view linkuistics/TestAnyware` shows the new repo.
- [ ] `gh repo list linkuistics | grep -E 'GUIVision|Redraw|TestAnywareRedux'` is empty.
- [ ] Local `git remote -v` shows `origin` at the new GitHub URL.

---

## Milestone 9 — Final Local Cleanup

Goal: `~/Development/` free of stale material.

### Task 9.1: Sanity check before deletion

- [ ] **Step 1:** Confirm the new repo is fully functional.

```bash
testanyware --help > /dev/null && echo "CLI OK"
cd /Users/antony/Development/TestAnyware
swift build --package-path cli/macos && echo "Build OK"
git status
```

- [ ] **Step 2:** Confirm no active project (outside `_archive/` and `TestAnyware/`) still points at `_archive/` paths.

```bash
cd /Users/antony/Development
grep -rln '_archive/' --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git . | head
```

Expected: empty (or only in historical docs).

### Task 9.2: Delete `_archive/`

**[USER CONFIRM]** This is a destructive local deletion.

- [ ] **Step 1:** Review contents one last time.

```bash
du -sh /Users/antony/Development/_archive/*
```

- [ ] **Step 2:** **[USER CONFIRM]** Ask: "About to `rm -rf /Users/antony/Development/_archive/`. This deletes the five harvested repos locally and is not recoverable from this machine (GitHub copies are already gone). Confirm?"

- [ ] **Step 3:** Delete.

```bash
rm -rf /Users/antony/Development/_archive
```

- [ ] **Step 4:** Verify.

```bash
ls /Users/antony/Development/ | grep -E '_archive|GUIVision|Redraw|TestAnywareRedux' || echo "clean"
```

Expected: `clean`.

### Task 9.3: Close out the plan

- [ ] **Step 1:** Mark plan completion in LLM_STATE.

```bash
cd /Users/antony/Development/TestAnyware/LLM_STATE/core/
# Append to session-log.md or add a completion note.
```

- [ ] **Step 2:** Final commit and push.

```bash
cd /Users/antony/Development/TestAnyware
git add LLM_STATE/
git commit -m "chore: mark TestAnyware unification complete" --allow-empty
git push
```

### Gate 9

- [ ] `ls /Users/antony/Development/` shows only active projects; no `_archive/`.
- [ ] `gh repo list linkuistics` shows `TestAnyware` and no orphans from the merge.
- [ ] Local `TestAnyware` builds, smokes, and has pushed changes to `origin/main`.

---

## Self-Review Notes

Coverage check against the design:

- Design §4 (directory structure) — covered by Tasks 1.4, 3.1-3.7 + per-component harvests.
- Design §5 (component boundaries) — enforced by Package.swift rewrites in 2a.2, 2b.1, and the `vendored/royalvnc` path-dependency setup in 2c.1.
- Design §6 (harvest map) — covered line-by-line: 2a-2d (GUIVisionVMDriver), 2e (GUIVisionPipeline), 2f (Redraw), 2g (TestAnyware v1 icon classifier), 2h (LLM_STATE). TestAnywareRedux intentionally not represented (skip per user decision).
- Design §7 (rename table) — embodied as explicit replacement lists in 2a.3, 2b.2, 2b.3, 2d.1, 2e.5. Final verification in Gate 2.
- Design §8 (LLM_STATE migration) — Task 2h.1 with read-then-rewrite discipline.
- Design §9 (`~/Development` scan) — Milestone 7 with four-bucket scan, inventory file, per-project rewrites, Raveloop-specific task.
- Design §10 (docs) — Milestone 3 covers both entry points + `docs/` tree + per-component READMEs.
- Design §11 (execution sequence) — Milestones map 1:1 to design §11's milestones.
- Design §12 (risk & rollback) — Global Execution Rules + explicit `[USER CONFIRM]` markers on destructive steps.
- Design §13 (open items) — Task 3.2 addresses the Swift/Python handoff ambiguity at doc level; resolution of implementation detail deferred appropriately.
