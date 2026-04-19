# TestAnyware Unification — Execution Prompt

Execute the TestAnyware Unification plan.

- **Plan:** `0-docs/plans/TestAnyware-Unification.plan.md`
- **Design (reference):** `0-docs/designs/TestAnyware-Unification.design.md`

## Execution mechanics

Use the `superpowers:subagent-driven-development` skill. Dispatch a
fresh subagent per task (not per milestone), review between tasks,
and use the two-stage review the skill prescribes. Do not execute
tasks inline in the main session.

If a task fails review, fix forward in the next subagent dispatch —
do not retry the same subagent.

## What this does

Unifies five overlapping projects under `~/Development/`
(GUIVisionVMDriver, GUIVisionPipeline, Redraw, TestAnyware,
TestAnywareRedux) into a single monorepo at `~/Development/TestAnyware/`,
renames `guivision` → `testanyware` everywhere (CLI, env vars, XDG
paths, golden images, Swift libraries, service names), migrates every
downstream consumer under `~/Development/`, and replaces the old
GitHub repos on `github.com/linkuistics` with the new unified one.

## Execution rules

1. **Read the plan first**, then work it milestone by milestone. Do
   not skip ahead or reorder.
2. **At each gate**, run the verification commands the plan specifies
   and report pass/fail explicitly before proceeding. On failure,
   stop and report — do not attempt to paper over.
3. **Destructive steps require explicit user confirmation** at
   execution time:
   - Milestone 8: `gh repo delete linkuistics/<old>` for the five old
     repos.
   - Milestone 9: `rm -rf /Users/antony/Development/_archive/`.
   Ask, wait for an answer, act only on explicit "yes".
4. **On harvest ambiguity, ask — do not guess.** When the plan says
   "harvest X from source Y" and the source is ambiguous (multiple
   candidates, unclear which belongs), surface the ambiguity with
   concrete file paths and wait.
5. **LLM_STATE rewrites are read-then-rewrite, never blind `sed`.**
   Each file gets read, every reference decision is considered
   (update / preserve as history), then written.
6. **Commit per logical step.** Each milestone substep is a separate
   commit so progress is inspectable and revertable.
7. **Never use `--no-verify`, `--force`, `-D`, or similar bypass
   flags** unless explicitly instructed for a specific operation.
8. **Report progress at each gate.** Short: which milestone, which
   gate, pass/fail, what's next.

## Starting state assumed

- `~/Development/` contains the five source repos plus Raveloop and
  other unrelated projects.
- `~/Development/TestAnyware/` **does not exist yet** (current v1 dir
  of that name is moved to `_archive/` in Milestone 1).
- `github.com/linkuistics/` currently contains (at least some of) the
  five old repos. Authenticated `gh` CLI available on the host.
- GUIVisionVMDriver may have uncommitted work — the plan checks and
  refuses to proceed if it does.

## Ending state

- `~/Development/TestAnyware/` contains the unified monorepo, builds,
  tests pass, integration tests pass against fresh golden images.
- `~/Development/_archive/` removed.
- `github.com/linkuistics/TestAnyware` exists with the initial commit
  pushed; the five old repos deleted from the org.
- No residual `guivision` / `GUIVisionVMDriver` / `GUIVISION_*`
  references anywhere under `~/Development/` (outside historical
  session logs the plan explicitly preserves).
- Raveloop `LLM_STATE/` under `TestAnyware/` is consistent with the
  new names and resumable for the next session.

## On failure

If any gate fails irrecoverably, stop, report the exact failure, and
wait. Do not roll back unless the user asks — the archive and the old
GitHub repos exist precisely so the user can decide how to recover.
