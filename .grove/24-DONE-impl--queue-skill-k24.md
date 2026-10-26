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

## Decisions (running log)

**The checkout is `/Users/antony/Development/grove`, and the edit is in a separate jj workspace.**
`Linkuistics/grove` is on this Mac. Its `main` is at `63073e62` (v22.7.0). The edit is in a jj
workspace in the scratchpad, `grove-skill-ws`, so the checkout of the human, which
`plugins/install.sh` symlinks into the skill directories, does not change before the human agrees.
The workspace has one change, described "testanyware / using-testanyware: pass --wait to a macOS vm
start". It edits `plugins/testanyware/skills/using-testanyware/SKILL.md` and adds one entry under
`## Unreleased` in `CHANGELOG.md`, as that file requires for a skills change.

**The skill passes `--wait` to a macOS start in step 3, and a new section holds the rest.** Step 3
shows `vm start --platform macos --wait` and says that `--wait` is for a macOS start only. The
section "Waiting for a macOS slot" says to pass `--wait` to every macOS start, to run a long wait in
the background (a bare `--wait` gives up after 600 s, and a shell tool call ends sooner), to read
`vm slots --json` while a start waits, and to run a macOS `vm create-golden` in the background. It
repeats the sentences of `LLM_INSTRUCTIONS.md` under "Several guests on one Mac", so the two texts
agree. The workflow of reuse is not in it (issue #9).

**The skill works on the installed 2.5.0, which has `--wait` and no `vm slots`.** The skill says
that an older release has no `vm slots` and that `start.err` names the holders there. So the skill
does not tell an agent to run a command that fails today. The skill claims no order of service,
because 2.5.0 does not serve waiters in order and the queue arrives with the release of this grove.

**Checks.** `bash plugins/install.test.sh` in the workspace: 28 passed, 0 failed. The other
`check:skills` steps cover the `grove` plugin and the Rust build, not this file, and I did not run them.

**The human agreed to the push, and the commit is `43290da0d3e3bb15d2742b55978ac1ff388d70b8`.**
The question named the repository (`Linkuistics/grove`), the branch (`main`, at `63073e62`), the two
files and the diff. The answer was "Push to main". I moved `main` to the change with jj and pushed
it (`main` moved forward from `63073e62b294` to `43290da0d3e3`). `git ls-remote` on
`https://github.com/Linkuistics/grove.git` shows `refs/heads/main` at that commit. The marketplace
now carries the skill change. I forgot the scratchpad workspace, and the checkout of the human was
not touched. No review leaf follows, and the in-session allowance stays unspent: the change is
text, and the claim that a skill can carry (that `vm slots` is absent on 2.5.0) I checked by
running `testanyware vm slots --help` on the installed 2.5.0.
