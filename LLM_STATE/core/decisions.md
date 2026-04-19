# Core Architectural Decisions

Running log of decisions made during monorepo unification that diverge
from the original plan/design or are not otherwise derivable from code.

---

## 2026-04-19 — No shared Swift protocol between CLI and agent

**Decision:** `agents/macos/` is a fully self-contained Swift package with
its own `Sources/TestAnywareAgentProtocol/` tree. It does NOT path-depend
on `cli/` for protocol types. `cli/`'s `TestAnywareAgentProtocol` target
and `agents/macos/Sources/TestAnywareAgentProtocol/` are two independent
copies of the same types that happen to share a wire format.

**Why this diverges from design §5:** The design said "`agents/macos/` ...
path-depends on `../../cli/macos` to reuse `TestAnywareAgentProtocol`."
That was predicated on Swift-on-both-sides being a durable architectural
invariant. It isn't — the host CLI will migrate to Rust for cross-platform
support. The true invariant is the wire protocol (JSON-RPC 2.0 over HTTP,
port 8648), not Swift types.

**Also:** Swift Package Manager derives path-dep identity from the last
path component. With `cli/macos/` and `agents/macos/` both ending in
`macos`, a path-dep between them produces an identity collision that SPM
cannot disambiguate. Consolidating would have required either renaming a
directory (contradicting design §4) or extracting the protocol into a
third package (extra directory not in design §4). With the Rust migration
planned, neither was worth doing.

**How to apply:** When editing protocol types, update both copies until
the Rust migration lands. The divergence risk is low (protocol is small;
wire format is the contract) and will be eliminated structurally when the
CLI changes language.

**Small current divergence:** `agents/macos` has `AXWebArea → .webArea`
in `RoleMapper.swift`; `cli/` has the same entry (merged during this
session). Both copies currently agree. If they drift, the agent's copy
is authoritative (it runs accessibility queries inside the guest VM;
the host CLI only consumes role strings via JSON).

---

## 2026-04-19 — No per-platform subdirectories under `cli/`

**Decision:** The Swift host CLI sits at `cli/` directly (Package.swift,
Sources/, Tests/), not at `cli/macos/`. `cli/linux/` placeholder is not
created.

**Why this diverges from design §4:** The design anticipated `cli/macos/`
+ `cli/linux/` siblings under `cli/` to signal multi-platform scope. That
framing assumed per-platform host implementations. The actual direction
is a single cross-platform Rust CLI replacing the Swift one — no
per-platform split needed. The Swift CLI sits at `cli/` transitionally;
the Rust CLI will replace it in place.

**How to apply:** Refer to the host CLI at `cli/` (no platform suffix).
Guest agents keep per-platform subdirectories (`agents/macos/`,
`agents/linux/`, `agents/windows/`) because those legitimately are
different implementations for different guest OSes. Update design §4
and §5 in Milestone 3 docs pass.

**Follow-up:** When writing docs in Milestone 3, rewrite `cli/macos/` →
`cli/` throughout. The flattening affects `swift build --package-path`
invocations, CI, install scripts, and LLM_INSTRUCTIONS.md examples.

---

## 2026-04-19 — Rust migration planned for CLI

**Fact:** The `testanyware` host CLI will be rewritten in Rust to support
Linux hosts (currently macOS-only). Timing is not committed; this note
exists so future sessions don't re-evaluate Swift-specific design
choices without knowing the language change is coming.

**How to apply:** When choosing abstractions or dependencies in the
current Swift CLI, prefer approaches that translate cleanly to Rust
(e.g., argument parsing, subprocess management, HTTP clients are all
direct ports). Avoid Swift-only runtime features (e.g., heavy
dependence on ObjC bridging, KVO, or macro-heavy DSLs) in new code.
The agent stays Swift (macOS guest, needs AppKit / Accessibility APIs).

---

## 2026-04-19 — Vision pytest requires `--import-mode=importlib`

**Fact:** `cd vision && uv run pytest` fails collection under default
import mode because two stage test files share the module path
`tests.test_stage` (one in `stages/drawing-primitives/tests/`, one in
`stages/icon-classification/tests/`). Both stages follow the repo's
convention of naming the stage entry test `test_stage.py`, and pytest's
default `prepend`/`append` import mode uses legacy namespace rules that
collide on duplicate short-paths.

**Workaround:** Run pytest with `--import-mode=importlib`. Expected to
be added as a default via `[tool.pytest.ini_options] addopts =
"--import-mode=importlib ..."` in `vision/pyproject.toml` during
Milestone 3 (docs + config polish).

**How to apply:** When running the vision test suite from any script,
CI, or command-line invocation, pass `--import-mode=importlib` (or rely
on the addopts once set).

---

## 2026-04-19 — Historical `guivision` mentions preserved in plan docs

**Fact:** `vision/docs/superpowers/plans/2026-04-03-*-window-detection.md`
contain historical `GUIVision*` and `guivision_*` references. These are
dated plan documents (2026-04-03) describing past implementation work
when the project was still named GUIVisionPipeline. Left as-is per the
"don't rewrite history" rule.

**How to apply:** Grep sweeps for stale references should exclude
these files (they're historical by design). Same applies to dated
session logs elsewhere in LLM_STATE.

---

## 2026-04-19 — `provisioner/helpers/` (not `autounattend/`)

**Decision:** The provisioner auxiliary directory is `provisioner/helpers/`
(matching the source archive's `scripts/helpers/`). NOT
`provisioner/autounattend/` as the plan Task 2d.1 named it.

**Why:** `autounattend/` implies Windows-only install-time assets, but
the directory actually contains a mix:
- Windows install assets: `autounattend.xml`, `SetupComplete.cmd`,
  `desktop-setup.ps1`, `set-wallpaper.ps1`
- macOS runtime assets: `com.linkuistics.testanyware.agent.plist`
  (LaunchAgent), `set-wallpaper.swift`

The scripts (both macOS and Windows builders) reference `helpers/`, not
`autounattend/`. Changing the scripts would be mislabeling the wrong way.
Keeping the original name matches what the scripts expect and matches
the archive.

**How to apply:** Refer to `provisioner/helpers/` wherever plan/design
said `provisioner/autounattend/`. Update design §4 accordingly during
docs refresh.


