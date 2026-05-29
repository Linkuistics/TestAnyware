<!-- grove reference file — the BRIEF.md shape -->

# BRIEF-FORMAT — the node briefing

Every node directory in a grove carries a `BRIEF.md`. It is **process
scaffolding** — neither the glossary (`CONTEXT.md`) nor a decision log
(`docs/adr/`). It exists so that a session executing a leaf can read *three*
ADRs, not fifty: the brief chain, root→leaf, is the curated path into the
project's documented decisions.

A `BRIEF.md` is written by the planning task that creates its node, and is
retired (moved into `done/`) together with its subtree.

## Suggested shape

A guide, not a schema (constraint 3). Nothing validates a brief; nothing breaks
if a section is missing, reordered, or renamed. Include a section only when it
earns its place (constraint 4).

```markdown
# <node name> — brief

## Goal
One or two sentences: what this subtree delivers, and why.

## Done when
The done-criteria rollup for the subtree — the conditions under which every
child is complete and the node retires.

## Decomposition
Why this node is split the way it is, and what the numeric child ordering
encodes (dependencies, natural sequence). One line per child is enough.

## Pointers
- ADRs a session here must read: docs/adr/NNNN-*.md, …
- Glossary terms in play: <term>, <term> (see CONTEXT.md)
- Design specs: docs/specs/*-design.md

## Notes
Anything a session needs that is not yet an ADR or a glossary entry. On
retirement, anything still live here is promoted upward (see SKILL.md, "Retire").
```

## Briefs inherit

A session reads the **whole brief chain**, root→leaf. A child brief states only
what is *new* at its level — it does not repeat the parent. Pointers accumulate
down the chain.
