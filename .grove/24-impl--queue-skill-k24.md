# queue-skill-k24

## Goal

The `using-testanyware` skill tells an agent to pass `--wait` for a macOS start
and to run a long wait in the background. Agents then use the queue that this
grove built.

## Context

- The decision log of `plan-k1` records the choice of the human. The canonical
  skill is `plugins/testanyware/skills/using-testanyware/SKILL.md` in the Grove
  repository, `Linkuistics/grove`. The `linkuistics` marketplace updates itself
  from the `main` branch of that repository.
- `--wait` exists since 2026-10-19, and the installed 2.5.0 has it. The change
  need not wait for the release. The queue only makes the wait fair.
- The text of `llm-instructions` from `queue-vm-slots-k20` says the same to an
  agent. Keep the two texts consistent.
- The workflow of reuse stays in issue #9. Do not add it here.

## Done when

- The workflow of the skill passes `--wait` for a macOS `vm start`. It runs a
  long wait in the background and reads `vm slots` when a start waits.
- Before the push to the Grove repository, the session asks the human. The
  question names the repository, the branch, the file and the diff. The session
  pushes nothing until the human agrees.
- After the push, the decision log records the commit.

## Notes

- This leaf stops at the question, so it sits late in the order. A pick of this
  leaf may need a live human. Move it ahead of the AFK leaves only if the human
  wants the skill sooner.
- Find the checkout of the Grove repository first. If it is not on this Mac, ask
  the human where to work, and do not clone it without asking.
