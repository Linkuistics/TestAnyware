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
