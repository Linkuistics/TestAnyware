# author-skill-k2

**Kind:** work

## Goal

Author the `testanyware` plugin and its single `using-testanyware` skill in the
peer repo `~/Development/skills`, per the design settled in `plan-k1` (see the
root `BRIEF.md`). All edits and commits happen **in `~/Development/skills`**, not
in this grove worktree.

## Context

Read the root `BRIEF.md` first (Decisions + Pointers). Then, to ground the
authoring:

- **Mirror the existing plugin's shape.** `~/Development/skills/plugins/linkuistics/`
  — `.claude-plugin/plugin.json` + `skills/<name>/SKILL.md` with `name` +
  `description` frontmatter. Match that style.
- **Read `superpowers:writing-skills`** for the SKILL.md frontmatter/structure
  conventions and how `description` drives auto-triggering — the trigger is the
  load-bearing field here.
- **Source the workflow spine from `LLM_INSTRUCTIONS.md`** ("Mental model" +
  "Quick start"): connection resolution (`--connect` / `--vm` /
  `TESTANYWARE_VM_ID` env) → `vm start --platform {macos,linux,windows}` →
  drive `agent` / `input` / `screen` → `vm stop`. Do **not** copy the command
  tables; point at `testanyware llm-instructions` instead.

## Steps

1. `plugins/testanyware/.claude-plugin/plugin.json` — name `testanyware`,
   description for the TestAnyware GUI-testing skill, author/repository mirroring
   linkuistics, sensible keywords (gui-testing, accessibility, vm, vnc, ui).
2. Register the plugin as a second entry in
   `~/Development/skills/.claude-plugin/marketplace.json`.
3. `plugins/testanyware/skills/using-testanyware/SKILL.md`:
   - **`description`** (the trigger) — fires on run / test / screenshot / record
     / experiment-with a GUI app, **especially** when the app would interfere
     with the user's machine **or** needs a Windows/Linux environment. This is
     what makes TestAnyware "standard practice" — phrase it so it auto-fires.
   - **When to use / when not** — short decision guide (the four use cases:
     host-isolation, cross-platform, UI experimentation, screenshots/movies for
     docs). When NOT: pure logic/unit tests with no GUI.
   - **Workflow spine** — doctor → vm start → (export `TESTANYWARE_VM_ID`) →
     drive agent/input/screen → observe (snapshot/screen capture/record) →
     vm stop.
   - **Delegate** — explicit hand-off: "for full command detail run
     `testanyware llm-instructions`; discover specifics with `--help`,
     `capabilities --json`, `schema <id>`." No duplicated reference.
   - **Host-side gotchas** (not in the CLI's own docs): run `testanyware doctor`
     first; golden images must already exist (`vm list` / `vm create-golden`);
     `tart list` state column, not `tart ip`; minimal-image philosophy.
4. Optional: a `references/` file only if the body genuinely needs offloading;
   prefer keeping it lean.
5. Optional: update the marketplace `README.md` if it enumerates plugins.

## Done when

- The three artifacts above exist in `~/Development/skills`, the `linkuistics`
  plugin is untouched, and the marketplace manifest validly lists both plugins.
- The SKILL.md `description` clearly auto-triggers on the GUI-testing scenarios.
- Changes are committed **in `~/Development/skills`** (its own repo) with a
  focused message. Then return here to retire this leaf on the grove branch.

## Notes

Doubt pass candidate (driving.md): before finalising, have a fresh-context
reviewer check the `description` trigger actually fires on the intended
scenarios and not on unrelated ones — the trigger is the whole point.
