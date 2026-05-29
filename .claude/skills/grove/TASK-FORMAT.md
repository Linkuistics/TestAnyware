<!-- grove reference file — the task-file shape -->

# TASK-FORMAT — the leaf task file

A **leaf** in a grove is a single `.md` task file, named with a numeric prefix
in tens (`010-`, `020-`, …). One task is one session (constraint: one task per
session). The file is freeform markdown — a guide follows, not a schema.

## The two kinds

Every task file states its **kind**. There are two:

- **work** — produces code, docs, or tests. The deliverable is an artifact.
- **planning** — grills, sharpens the glossary, may raise an ADR or a PRD, and
  **grows the tree**: replaces an oversized leaf with a node directory of child
  briefs and ordered leaves. The deliverable is *more tree*.

A task too big for one focused session *is* a planning task — its job is to
decompose, not to do.

## Suggested shape

```markdown
# <NNN-task-name>

**Kind:** work          (or: planning)

## Goal
What this one session must deliver.

## Context
Pointers *beyond* the brief chain — specific files, prior leaves, ADRs — that
this task in particular needs. The brief chain and glossary are read anyway;
list only the extras.

## Done when
Concrete, checkable completion conditions for this task.

## Notes
Anything else the executing session should know.
```

## Planning tasks — extra guidance

A planning task additionally:

- runs the grilling procedure (`grilling.md`) to interrogate the design;
- updates `CONTEXT.md` **inline** as terms are resolved — never batched;
- raises ADRs **sparingly** — only decisions hard to reverse, surprising, or a
  real trade-off (`ADR-FORMAT.md`);
- MAY write a PRD (`docs/prd/`) when the increment is a genuine agreement point;
- writes the child `BRIEF.md`(s) and ordered leaf files for any node it grows
  (`BRIEF-FORMAT.md`).
