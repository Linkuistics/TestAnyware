# plan-k1

**Kind:** planning

## Goal

Plan the addition of a **TestAnyware skill** to the peer skills project
(`~/Development/skills` = `github.com/Linkuistics/skills`, a Claude Code plugin
marketplace). Decide what the skill is for, who reads it, what it contains, and
where it lives in the marketplace — then grow the tree into the work tasks that
write it.

## Context

- **Peer skills project** = `~/Development/skills`, the `Linkuistics/skills`
  GitHub repo. A Claude Code **plugin marketplace**:
  - `.claude-plugin/marketplace.json` declares one plugin, `linkuistics`.
  - `plugins/linkuistics/` holds 8 skills: `coding-style` (+ 6 per-language
    variants) and `cli-tool-design`. All are **coding-standards** skills.
  - `plugin.json` self-describes as "Linkuistics coding-standards skills."
  - History: grove content was recently *extracted* to a separate
    `Linkuistics/grove` repo — i.e. the maintainer already splits unrelated
    concerns into their own repos/plugins.
- **TestAnyware** (this repo) = host-side **Rust CLI** (`cli-rs/`) +
  per-platform **in-VM agents** for accessibility-API-driven UI testing of apps
  in isolated guest VMs. The CLI is the stable scriptable surface; see
  `CONTEXT.md` for the ubiquitous language (Command surface, Agent a11y
  surface, Golden image, etc.) and `docs/architecture/cli-design-contract.md`
  for the command contract.

## Open questions (grilling)

1. Purpose & audience of the skill — tool-usage vs contributor vs both.
2. Placement — inside the `linkuistics` plugin, a new plugin in the same
   marketplace, or a new marketplace.
3. Single skill vs a small set.
4. Content scope & sourcing — what goes in, and how we keep it from rotting
   against the real CLI surface.

## Decisions (running log)

**Q1 — Purpose & audience → tool-usage, trigger-first.** The skill is a
*tool-usage capability* skill (not a contributor guide). Its reader is a Claude
in any project that needs to run a GUI app. The stated use cases:
1. Test an app whose UI would otherwise **interfere with the user's machine**
   (isolation).
2. Test an app that **requires a Windows/Linux environment** (user is on macOS).
3. **Experiment** with UI.
4. Take **screenshots / movies** for documentation.

The central value is **discoverability**: today the user must tell every
session to use TestAnyware; the skill must make that **standard practice**.
Consequence: the skill's `description` (its auto-trigger) is the load-bearing
field — it must fire on "run / test / screenshot / record a GUI app," with the
interfere-with-host and needs-Windows/Linux conditions called out. Body is
secondary to a crisp trigger.

**Q2 — Placement → new `testanyware` plugin in the same marketplace.** Add
`plugins/testanyware/` (its own `.claude-plugin/plugin.json`) and register it as
a second plugin in `.claude-plugin/marketplace.json`. The `linkuistics` plugin
stays coherent as coding-standards; TestAnyware support installs independently;
matches the maintainer's split-concerns pattern (grove → own repo). Skill path:
`plugins/testanyware/skills/using-testanyware/SKILL.md` (name TBD in Q3).

**Q3 — Granularity → one skill, `using-testanyware`.** A single triggerable
`SKILL.md`; if the command cheat-sheet grows, it goes in a bundled
`references/` file in the same skill dir (on-demand load), not a second skill.
The four use cases share one workflow, so one skill with a sharp trigger covers
them. Split later only if a genuinely distinct trigger emerges.

**Q4 — Content strategy → thin trigger + delegate (anti-rot).** Decisive
finding: TestAnyware is *already* exhaustively self-documenting for LLMs — it
ships `LLM_INSTRUCTIONS.md` (225 lines) and a `testanyware llm-instructions`
command that emits it, plus `capabilities --json` (self-describing surface),
`schema <id>` (per-command JSON schemas), `doctor` (env check), and `--help` at
every level; the binary is already on PATH. So the skill must **not** duplicate
the command reference (it would become a second source of truth that rots
against `cli-rs/.../surface.rs` + `LLM_INSTRUCTIONS.md`). The skill is a thin
shim: sharp `description` trigger + when-to-reach-for-it decision guide + a
minimal workflow spine + "for command detail, run `testanyware
llm-instructions` / `--help` / `capabilities --json` / `schema <id>`" + a few
host-side gotchas the CLI can't self-report.

**No ADR / no PRD.** Decisions are low-cost to reverse (a skill is easy to
rewrite) and the running log + root brief capture the rationale; ADR/PRD would
be ceremony (grove constraints 4, "offer ADRs sparingly").

**Cross-repo note (load-bearing for the work leaf).** The deliverable lands in
a *different* repo — `~/Development/skills` (`Linkuistics/skills`) — not in this
TestAnyware grove worktree. The work session commits the plugin + skill **in
`~/Development/skills`**, and separately commits the grove bookkeeping (leaf
retire) on this grove branch. Two repos, two commits.

## Tree grown

Added one work leaf, `author-skill-k2` (`grove-llm leaf-add .`), to author the
`testanyware` plugin + `using-testanyware` skill in the peer repo. Design is
settled; remaining work is authoring + scaffolding, a single focused session.
Root `BRIEF.md` populated with the settled design so the work session bootstraps
from it.

## Notes
