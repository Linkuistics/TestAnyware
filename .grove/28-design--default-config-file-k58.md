# default-config-file-k58

## Goal

Decide whether TestAnyware installs a minimal configuration file when it finds none, and if so which program writes it, where, and with what content.

## Context

- The owner raised this on 2026-10-22, during `linux-golden-disk-size-k57`, in these words: "We should probably install a minimal configuration file if none is found."
- The words do not name the file. The likely file is `~/.config/testanyware/config.rhai`. It does not exist on this Mac today.
- ADR-0021 makes that file optional. A file that fails makes the app run on its defaults, and makes the Host CLI refuse to run. A written default file changes that story, so the ADR may need a rework.
- The brief says one optional Rhai file configures the app and the Host CLI. The brief's done-when list says "optional".
- Questions the design must answer:
  - Which program writes the file: the Host CLI, the app at its start, the brew formula, or `doctor`?
  - What does the file hold? ADR-0021 allows only tool locations, `state_dir` and `data_dir`. A file of commented examples changes no setting.
  - What happens when the file exists already? The design must never overwrite it.
  - Does a written file break a test that names no file or `TESTANYWARE_CONFIG=none`?
  - Is the benefit worth the cost? A written file shows a user what the file can hold, and `doctor` could show that as well.

## Done when

- A design records the answers to the questions above, the rejected options and the reasons.
- The ADR and the spec change if the decision changes a behaviour that they state.
- The design keeps every command, flag, environment variable, exit code and `--json` field.

## Notes

- This leaf is outside the issue #8 delivery and outside `clone-disk-size`. The owner asked for focused deliveries, so it is a separate one.
- `requirements` may be the better first kind if the owner's words do not settle which file is meant. A `design` session can hand the question back to the owner.
