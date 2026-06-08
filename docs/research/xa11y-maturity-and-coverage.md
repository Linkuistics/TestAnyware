# xa11y maturity & coverage — research note

**Grove:** `investigate-xa11y` · leaf `020-research-xa11y-maturity-and-coverage`
**Date:** 2026-06-08 · **xa11y version assessed:** 0.8.2 (latest at time of writing)
**Question:** per platform, can xa11y **replace / augment / reject** the *Agent
a11y surface* (the a11y-tree subset of the in-VM agents)?

> **Method (from the leaf brief):** apply four maturity/licensing gates *first*
> (any hard failure short-circuits), then score the three-tier fidelity rubric
> per platform. `snapshot` fidelity is scored at the **attribute** level, not
> as "has a snapshot call."

## TL;DR

- **No platform can score "replace" today** — global Gate 3 (API stability)
  fails the replace bar: xa11y is pre-1.0 with **six breaking 0.x minor
  releases in ~10 weeks**. Replace stays off the table until ≥1.0 + stable API.
- **Licensing (Gate 2) is clean and machine-enforced**; **cadence (Gate 1)
  passes** but carries an **acute bus-factor caveat** (one human maintainer, a
  <3-month-old repo).
- **API coverage is strong**: all five **Tier-1** endpoints are covered at the
  API level, with the *one material gap* being the **role taxonomy** — xa11y
  normalizes to **~42 roles** vs. the agents' **~115** — recoverable via xa11y's
  raw-platform-data escape hatch + the agents' existing `RoleMapper`. **Tier-2**
  `show-menu` is covered; **window-{resize,move,close,minimize} is entirely
  absent** (source-confirmed) → a thin native shim is unavoidable.
- **Per-platform verdicts:** **macOS → augment** (prototype-worthy; the one
  platform with primary real-world evidence); **Windows → augment, unproven**
  (identical API, good data model, *no* demo or dependents); **Linux → reject**
  (maintainer documents AT-SPI2 as "far too slow for a single step").
- **Roll-up: do not replace; do not commit a port.** But **do not
  reject-everywhere either** — no *global* gate hard-fails, so **030
  (macOS snapshot-parity prototype) should proceed** as explicitly
  prototype-acceptable work. See [§Operator note](#operator-note-030-gating).

---

## Sources & evidence base

Primary sources, with how each was obtained (all accessed 2026-06-08):

| # | Source | Obtained via |
|---|---|---|
| S1 | Repo metadata, contributors, release tags | `gh api repos/xa11y/xa11y{,/contributors,/releases,/commits}` |
| S2 | `deny.toml` (license enforcement) | `gh api …/contents/deny.toml` |
| S3 | `README.md` (actions table, selector syntax, version pins) | `gh api …/contents/README.md` |
| S4 | `xa11y-core/src/element.rs` (`ElementData`, `StateSet`, `Rect`, `TreeNode`, actions) | `gh api …/contents/…` |
| S5 | `xa11y-core/src/role.rs` (the `Role` enum) | `gh api …/contents/…` |
| S6 | `xa11y-core/src/app.rs` + grep of `provider.rs`/`locator.rs`/`input.rs` (no window-ops) | `gh api …/contents/…` |
| S7 | crates.io: 0 reverse-deps, 544 downloads | `crates.io/api/v1/crates/xa11y{,/reverse_dependencies}` |
| S8 | Maintainer blog: per-platform perf + macOS Calculator demo | <https://crowecawcaw.github.io/general/2026/05/30/accessibility-for-computer-use.html> |
| S9 | Landing page: version 0.8.2, platforms, MIT | <https://xa11y.dev/> |
| A1 | Agent element model (`role,label,value,description,id,enabled,focused,showing,position*,size*,childCount,actions,platformRole,children`) | `agents/windows/Models/ElementInfo.cs`, `agents/linux/testanyware_agent/models.py` |
| A2 | Agent unified role taxonomy (~115 variants) | `agents/windows/Models/UnifiedRole.cs` |
| A3 | Agent selector model (role + label-substring + id + index, flat) | `agents/linux/testanyware_agent/query_resolver.py` |

Repo: <https://github.com/xa11y/xa11y>.

---

## Step 1 — The four gates

### Gate 1 — Bus-factor / cadence (global) → **PASS (with acute caveat)**

| Signal | Value (S1) |
|---|---|
| Repo created | **2026-03-15** (~12 weeks old) |
| Last push | **2026-06-07** (the day before this note) |
| Human contributors | **`crowecawcaw` (Stephen Crowe): 233** · `morrowgarrett`: 4 · (rest are `dependabot`/`github-actions`/`claude` bots) |
| Releases | **13** tags, `v0.3.0` (2026-03-28) → `v0.8.2` (2026-05-30) |
| Stars / forks / watchers | 28 / 2 / 1 |

The gate rejects only on **single-maintainer *AND* ~6-months stale**. xa11y is
**single-maintainer** (one human author with 233 of ~240 human commits) but the
opposite of stale — pushed yesterday, 13 releases in ~10 weeks. The conjunctive
test is **not** met → **PASS**.

> **Caveat that feeds the verdicts:** the abandonment scenario the gate guards
> against ("we end up owning three native a11y backends in Rust") is *live* —
> one human, 28 stars, a repo younger than this grove's parent workstream.
> Cadence passes the gate but does not retire bus-factor risk; it argues against
> a *committed* replace independent of Gate 3.

### Gate 2 — License purity, transitive (global) → **PASS (strongest gate)**

xa11y is MIT (S9). The transitive tree is **machine-enforced permissive**: the
repo ships a `cargo-deny` config whose license allowlist is permissive-only and
runs in CI (CI badge in S3). From `deny.toml` (S2):

```toml
[licenses]
allow = ["MIT","Apache-2.0","Apache-2.0 WITH LLVM-exception",
         "BSD-2-Clause","BSD-3-Clause","ISC","Unlicense","Zlib","Unicode-3.0"]
```

Copyleft is explicitly excluded by omission ("Reject copyleft licenses — only
allow permissive ones"). The per-platform backends (S9/S3) are themselves
permissive: `windows` crate (MIT/Apache-2.0), `core-foundation` (MIT/Apache-2.0),
`zbus` (MIT). No viral/copyleft backend dependency can enter the tree without
breaking CI.

> *Limitation:* I did not run `cargo tree`/`cargo deny` against a local checkout
> (no xa11y working copy in this grove). The finding rests on the committed,
> CI-gated `deny.toml` + known-permissive backends — strong primary evidence,
> but a hands-on `cargo deny check licenses` in 030 would fully close it.

### Gate 3 — API stability (global) → **FAILS the "replace" bar; prototype-acceptable** ⛔(for replace)

Release history (S1): `v0.3.0, v0.4.0, v0.5.0, v0.5.1, v0.5.2, v0.5.3, v0.6.0,
v0.6.2, v0.7.0, v0.7.1, v0.8.0, v0.8.1, v0.8.2` — **six minor 0.x bumps
(0.3→0.8) in ~10 weeks.** Under SemVer, a `0.x` **minor** bump signals a
breaking change, so this is **~6 breaking releases per quarter**. Corroborating
churn signal: the README's Rust install snippet still pins `xa11y = "0.4"` (S3)
while the current release is `0.8.2` — doc drift consistent with a fast-moving
public API.

Per the leaf's Gate-3 rule (*pre-1.0 + churning public API ⇒ reject for
"replace", acceptable for a throwaway prototype only*): **this is the decisive
global ceiling.** No platform can be scored "replace" today regardless of
fidelity. The best any platform reaches is **augment** (or **prototype**).
This does **not** hard-fail the whole investigation — it is explicitly
*prototype-acceptable*, which is what gates 030.

### Gate 4 — Real-world evidence (per-platform) → **macOS PASS · Windows marginal · Linux NEGATIVE**

The README claims xa11y "is being successfully used by several real world
projects" (S3). Per the brief, README claims don't count — and the primary check
**finds no corroborating source**:

- crates.io: **0 reverse dependencies**, **544 total downloads** (all "recent" —
  the crate is ~12 weeks old) (S7). → **No public dependent-project evidence;
  record "no source found" for the "several real world projects" claim.**

Concrete *primary* evidence reduces to the **maintainer's own** material (S8):

- **macOS — PASS (first-party demo).** The blog shows "macOS Calculator being
  driven by xa11y: 7, +, 3, = → 10" with a screenshot. Maintainer's own, but a
  concrete, reproducible demo on a real app. AXUIElement is described as flexible
  (caveat: "each element attribute needs to be individually read").
- **Windows — marginal.** Praised as "the most structured data model" with
  efficient prefetch (`FindAllBuildCache + CacheRequest`), but **no demo shown,
  no dependents**. Strong *design* story, zero *usage* evidence → defaults toward
  reject per the gate, held at weak-augment only because the API is identical to
  macOS's (shared `xa11y-core`).
- **Linux (AT-SPI2) — NEGATIVE primary signal.** The maintainer documents the
  framework as "the most problematic": *"AT-SPI2 requires individual API calls
  for each property on an element… Reading the whole a11y tree for a real
  application could take multiple seconds — far too slow for a single step."*
  (S8). This is a first-party admission of a performance problem that bites the
  Tier-1 `snapshot` anchor directly.

**Gate roll-up:** G1 pass (caveat) · G2 pass · **G3 fails replace / prototype-ok**
· G4 macOS-pass / Windows-marginal / Linux-negative. Because **G3 caps every
platform at augment-or-below**, the Q2 roll-up rule ("replace only if it lands
on a majority") is **unsatisfiable today** → the investigation rolls up to
**not-replace**. The remaining live question is **augment-vs-reject per
platform**, decided below.

---

## Step 2 — Three-tier fidelity rubric

The three backends share one API (`xa11y-core`), so **API-level coverage is
identical across platforms**; what differs is backend maturity/evidence
(Gate 4) and known performance (Linux). The rubric below scores the shared API;
the per-platform verdicts then apply the Gate-4 differences.

### Endpoint coverage (shared API)

| Tier | Endpoint | xa11y API (S3/S4/S6) | Score |
|---|---|---|---|
| **1** | `snapshot` | `Element::tree(max_depth)` → `TreeNode`; `dump()` | ✅ *with role-taxonomy caveat* (see below) |
| **1** | `inspect` | `App::locator("button[name='OK']")` — CSS selectors: `=`,`^=`,`*=`, `>`, descendant, `:nth(n)` | ✅ **superset** of agents' flat resolver (A3), modulo role vocabulary |
| **1** | `press` | `Element::press()` | ✅ |
| **1** | `focus` | `Element::focus()` / `blur()` | ✅ |
| **1** | `set-value` | `set_value()` + `type_text()` + `set_numeric_value()` + `select_text()` | ✅ **richer** than agents |
| **2** | `show-menu` | `Element::show_menu()` | ✅ |
| **2** | `window-resize` | — | ❌ **absent** |
| **2** | `window-move` | — | ❌ **absent** |
| **2** | `window-close` | — | ❌ **absent** |
| **2** | `window-minimize` | — | ❌ **absent** |

**Tier-1 result:** all five covered → **no immediate-reject trigger**. Beyond the
five, the agents' `windows` enumeration / `window-focus` map cleanly onto
`App::list_with()` + `App::children()` (windows-as-Elements) + `Element::focus()`
(S6).

**Tier-2 result:** `show-menu` covered; **all four window-geometry verbs absent.**
Source-confirmed: `app.rs` exposes only `children()`/`locator()`/`tree()` — no
`resize`/`move`/`close`/`minimize`/`maximize`/`set_bounds`; and grep of
`provider.rs`, `locator.rs`, `input.rs` returns **no** window-management methods
(S6). Windows are *readable* (an `Element` with `role: window` and `bounds`) but
not *mutable* through the a11y layer. `input.rs` can synthesize raw mouse/keyboard
events, but driving a title-bar drag/resize that way is pixel-fragile, not an
a11y-tree action. → **A thin native shim for window-{resize,move,close,minimize}
is unavoidable on every platform** (this is the Tier-2 "gap ⇒ augment" case from
the brief, and it is platform-independent).

### `snapshot` attribute coverage (the load-bearing finding)

The host CLI's snapshot JSON (`ElementInfo`, A1) vs. xa11y's `ElementData` (S4):

| Agent attribute (A1) | xa11y `ElementData` field (S4) | Parity |
|---|---|---|
| `role` (unified, **~115** values, A2) | `role: Role` (**~42** values, S5) | ⚠️ **PARTIAL — the one material gap** (see below) |
| `label` | `name: Option<String>` (bidi-stripped; raw kept) | ✅ |
| `value` | `value: Option<String>` | ✅ |
| `description` | `description: Option<String>` | ✅ |
| `id` | `stable_id` = AXIdentifier / AutomationId / D-Bus object_path | ✅ **exact semantic match** |
| `enabled` | `states.enabled` | ✅ |
| `focused` | `states.focused` | ✅ |
| `showing` | `states.visible` | ✅ (≈; "showing" ↔ visible) |
| `positionX/Y` | `bounds.x/y` (`Rect`, i32) | ✅ |
| `sizeWidth/Height` | `bounds.width/height` (u32) | ✅ |
| `childCount` | *(no field)* — derive from `children().len()` | ⚠️ derivable, **extra lazy call** (perf-relevant on Linux) |
| `children` | `children()` — lazy, **not cached**, re-queries provider | ✅ (lazy; cost note) |
| `actions[]` | `actions: Vec<String>` (snake_case) | ✅ (minor name-mapping vs agents') |
| `platformRole` | `raw["AXRole"/uia_*/atspi_*]` (raw escape hatch) | ✅ via `RawPlatformData` |
| — | `states.{checked,selected,expanded,editable,focusable,modal,required,busy}`, `numeric_value/min/max`, `pid` | ➕ xa11y **superset** here |

**The role-taxonomy gap.** xa11y's `Role` (S5) is deliberately small — *"scoped
to roles commonly surfaced by real desktop applications"* (~42 variants). The
agents' `UnifiedRole` (A2) is an ARIA/Chromium-flavored ~115-variant set. xa11y's
taxonomy is a **lossy reduction** of the agents': roles the agents distinguish —
`color-well`, `date-picker`, `disclosure-triangle`, `grid`/`grid-cell`,
`list-box`/`list-box-option`, `toggle-button`, `split-button`, `search`,
`menu-item-checkbox`/`-radio`, `tab-list`/`tab-panel`, and the entire
web/document family (`banner`, `figure`, `region`, `row-group`, `ruby*`, `pdf-*`,
`iframe*`, `paragraph`, …) — collapse to xa11y's `group`/`unknown`/coarser roles.

Because the agents' selector filters **by role first** (A3), any host-CLI query
or snapshot assertion keyed on a role outside xa11y's 42 would silently break.
This is precisely the *"snapshot fidelity is attribute-level, not binary"*
caveat, localized to the **role** attribute — the single most load-bearing one.

**But it's recoverable, not fatal.** xa11y preserves the **native** role string
in `raw` (`AXRole`/`uia_*`/`atspi_*`, S4) and exposes `actions[]` and
`stable_id`. The agents already own a per-platform `RoleMapper` that turns native
roles into the ~115-value `UnifiedRole`. So an augment integration would feed the
agents' existing `RoleMapper` from xa11y's `raw` platform role rather than
reimplementing role normalization — keeping the richer taxonomy while using xa11y
for tree-walking, geometry, states, and actions. **That is the concrete shape of
"replace → augment + translation layer."**

---

## Per-platform verdicts

| Platform | Verdict | Why |
|---|---|---|
| **macOS** | **AUGMENT** (prototype-worthy) | Full Tier-1 API coverage; the **only** platform with primary real-world evidence (Calculator demo, S8); AXUIElement is where snapshot parity is most likely. Known costs: role-translation layer + window shim. **This is the platform 030 should probe.** |
| **Windows** | **AUGMENT — unproven** | Identical shared API; backend praised as the best-structured data model (S8); but **no demo, 0 dependents** (S7) ⇒ Gate-4 "absence defaults toward reject." Held at weak-augment by the shared API. Do **not** commit without its own probe. |
| **Linux** | **REJECT** | Gate-4 **negative** primary signal: maintainer documents AT-SPI2 tree reads as "multiple seconds… far too slow for a single step" (S8). The Tier-1 `snapshot` anchor — and the lazy, uncached `children()` model + derived `childCount` — would be too slow as a drop-in for the Python agent's `/snapshot`. Adds a dependency without removing the perf problem. |

This is exactly the per-platform asymmetry the leaf brief predicted ("proven on
macOS/Windows but aspirational on AT-SPI2/Linux") — sharpened by the source: the
asymmetry is **macOS-proven, Windows-plausible-but-unshown, Linux-known-slow.**

## Roll-up recommendation

**Do not replace; do not commit a port now.**

- Per Q2's roll-up rule, "replace" needs a *majority* of platforms. Today
  **replace is globally blocked by Gate 3** (pre-1.0, churning) and would clear
  on **0/3**; the best achievable is augment on macOS (proven) + Windows
  (unproven), reject on Linux. A majority-replace does not exist.
- A single- or even dual-platform augment that keeps three agent languages **and**
  adds a fast-moving pre-1.0 dependency is the "worst of both worlds" the
  roll-up rule is designed to catch. So the standing recommendation is
  **not-replace**, revisit only **after xa11y ≥ 1.0 with a stabilized API** and
  third-party usage evidence emerges.

<a id="operator-note-030-gating"></a>
## Operator note — 030 gating

The leaf's Notes ask whether a *global* gate hard-fails (in which case skip 030
and finish as reject). **It does not.** Gate 3 fails the *replace* bar but is, by
its own wording, **prototype-acceptable**; gates 1 and 2 pass; Gate 4 **passes
for macOS** (the 030 target). Therefore:

- **Proceed with 030** (macOS snapshot-parity prototype). It is the
  highest-value next step: empirically measure the **role-taxonomy / attribute
  parity** this note flags as the single material gap, on a real app, against the
  live Swift `/snapshot`. While there, run `cargo deny check licenses` on a real
  checkout to fully close the Gate-2 limitation.
- **Do not** short-circuit straight to 040-reject — that would only be warranted
  if a *global* gate had hard-failed, which none did.
- 040 then decides **augment-vs-defer for macOS** (and whether a Windows probe is
  worth commissioning), folding in 030's measured parity. The likely landing
  zone, absent a surprise, is **"augment-capable on macOS but defer the commit
  until xa11y ≥ 1.0."**
