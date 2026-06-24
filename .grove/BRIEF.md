# add-testanyware-skill-to-peer-skills-project — brief

## Goal

Add a **`using-testanyware` skill** to the peer skills project so that using
TestAnyware for GUI testing becomes **standard practice** — auto-triggered by a
sharp skill `description`, instead of the user telling every session to use it.

## Done when

- A new **`testanyware` plugin** exists in `~/Development/skills`
  (`github.com/Linkuistics/skills`), registered in
  `.claude-plugin/marketplace.json`, with its own
  `plugins/testanyware/.claude-plugin/plugin.json`.
- It contains one skill, `plugins/testanyware/skills/using-testanyware/SKILL.md`,
  whose `description` fires on "run / test / screenshot / record a GUI app",
  especially when the app would **interfere with the host machine** or **needs a
  Windows/Linux environment**.
- The skill body is a thin trigger + delegate shim (see Decisions): a when-to-use
  decision guide, a minimal workflow spine, an explicit hand-off to `testanyware
  llm-instructions` / `--help` / `capabilities` / `schema` for command detail,
  and host-side gotchas only — **no duplicated command reference**.
- Changes committed in `~/Development/skills`; the `linkuistics` plugin is
  untouched.

## Decisions (settled in plan-k1; full rationale in its running log)

1. **Purpose** — tool-usage capability skill (not a contributor guide). Reader =
   a Claude in any project that needs to run a GUI app. Core value =
   discoverability / auto-trigger.
2. **Placement** — a *new* `testanyware` plugin in the existing
   `Linkuistics/skills` marketplace (keeps `linkuistics` coherent as
   coding-standards; installs independently).
3. **Granularity** — one skill, `using-testanyware`; bundle a `references/` file
   only if the cheat-sheet earns it.
4. **Content** — thin trigger + delegate. TestAnyware already self-documents for
   LLMs (`LLM_INSTRUCTIONS.md`, `llm-instructions`, `capabilities`, `schema`,
   `doctor`, `--help`), so duplicating its command reference would rot. Delegate
   detail; the skill owns only the trigger, the when-to-use decision, the
   workflow spine, and host-side gotchas.

## Pointers

- Peer repo: `~/Development/skills` — marketplace at `.claude-plugin/`,
  existing plugin at `plugins/linkuistics/` (8 coding-standards skills; the
  shape to mirror for `plugins/testanyware/`).
- TestAnyware self-docs (authoritative, do not copy): `LLM_INSTRUCTIONS.md`;
  command surface `cli-rs/crates/testanyware-cli/src/surface.rs`; contract
  `docs/architecture/cli-design-contract.md`; glossary `CONTEXT.md`.
- Workflow spine source: `LLM_INSTRUCTIONS.md` "Mental model" + "Quick start"
  (connection resolution → `vm start` → drive `agent`/`input`/`screen` →
  `vm stop`).
- Skill-authoring conventions: the `superpowers:writing-skills` skill and the
  existing `plugins/linkuistics/skills/*/SKILL.md` frontmatter shape.
- Host-side gotchas to fold in (from user memory, not in the CLI's own docs):
  `testanyware doctor` first; golden images must already exist; `tart list`
  state column not `tart ip`; minimal-image philosophy.

## Cross-repo note

The grove's process state (`.grove/`) lives in this TestAnyware worktree, but
the **deliverable lands in `~/Development/skills`**. The work session makes two
commits: the plugin + skill in `~/Development/skills`, and the grove bookkeeping
(leaf retire) on this grove branch.

## Notes
