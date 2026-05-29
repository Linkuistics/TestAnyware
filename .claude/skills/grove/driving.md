# Driving a grove session well

The grove `SKILL.md` and `grilling.md` files state *what* the loop is. This
file is about *how* to drive it well — the moves a human collaborator makes
that turn the loop into productive design work. It is a field guide, not a
specification; treat it as a starting set of habits, not a checklist.

The dogfood reference throughout this doc is the
`capture-issues-for-later-groves` workstream in the grove project's own
repo, which is the worked example for every pattern below. Paths point at
artifacts in that workstream so a reader can trace a real chain
end-to-end.

## When to commission prior-art research

A planning leaf is the right unit for a grilling session when the design
tree fits in one session. When the leaf's design depends on lessons that
prior tools have learned the hard way — and those lessons are not
obvious from the current codebase — *insert a research leaf ahead of
the planning leaf*. The research leaf's job is to surface the failure
modes the planning leaf will otherwise have to learn from scratch.

**Signs you want a research leaf:**

- The planning leaf sits in an architectural neighbourhood with well-known
  prior art (issue trackers, sync protocols, schema registries, etc.).
- Earlier leaves in the subtree have already touched architectural
  decisions the new leaf depends on — i.e. several downstream planning
  leaves share a common evidence base.
- The planning conversation surfaces a question like "has anyone tried
  this before, and what happened to them?" — that question itself is the
  signal.

The grove workstream's leaf
`050-research-in-repo-issue-trackers.md` (retired) is the worked
example. It was inserted *after* the design's first ADRs had been
written but *before* four downstream planning leaves (060, 070, 080,
090). Its findings de-risked the sync-semantics grilling specifically
(the post-mortem survey's "Synthesis for 060" section pre-judged most
of the design tree the 060 grilling had to walk).

## How to write a research leaf brief

The single most leveraged move is to **name the downstream questions the
research must answer**, leaf by leaf. The researcher doesn't have to
guess what's load-bearing.

The 050 leaf's brief (now in `done/`) put it this way:

> The audience is four open planning leaves: 060, 070, 080, 090. Each
> system section ends with a *takeaway for grove* pointing at the
> leaf(es) its lesson informs. The final **Synthesis** section answers
> the leaf-specific questions in one place.

Followed by a list of concrete questions per leaf. The output
(`docs/research/in-repo-issue-tracker-postmortems.md`) is structured
around those questions.

**Bias the search.** "What's already been tried" produces broad,
shallow surveys. "What went wrong after years of real multi-user,
multi-machine use" produces post-mortems. The 050 brief explicitly
demanded post-mortem framing per system, and explicitly biased toward
*non-obvious paradigms* (the previous prior-art survey had already
established that Linear/GitHub-Issues integration was the obvious
fallback to beat, not the starting candidate). Both moves cut the
researcher's degrees of freedom in productive directions.

**Demand a walk-away check per system.** For each prior tool, the
researcher must answer: with the tool uninstalled, what is still
legible? This is the cheapest invariant to require and the most
revealing — it separates the architectures that can be borrowed from
the ones that cannot.

**Demand a citation per failure-mode claim.** "git-bug had this
problem" without an issue link, blog post, or thread quote is mood,
not evidence. The 050 brief required primary sources; the resulting
postmortem doc cites primary issues by URL and quotes from them
directly. When you later sit down to write an ADR, those citations
*are* the ADR's rationale section.

**Acknowledge missing sources.** When the researcher searches and
finds silence, that's a finding too. The 050 doc flags multiple "no
primary source found" notes (e.g. Google's internal use or non-use of
git-appraise, CRDT-merge surprises in Radicle COBs) — the absence is
itself a confidence signal, and recording it stops future readers from
re-doing the same fruitless search.

## When to invoke a design discussion (grilling)

The trigger is: a planning leaf's brief lists three or more questions
whose answers interdepend. Grilling is the procedure that walks the
dependency tree without the LLM making decisions on the human's
behalf.

The grilling skill (`grilling.md`) says it briefly: interview one
question at a time, propose a recommended answer for each, walk down
the design tree until shared understanding is reached. The moves
below make that interview productive rather than ceremonial.

### Ask the LLM "WDYT" before committing

When a question feels close to settled, the easy default is to nod
and move on. **Don't.** Ask the LLM what it thinks — explicitly,
including when you already have a strong view yourself.

Two things happen when you ask:

1. The LLM produces a recommendation it would otherwise suppress out
   of deference. The recommendation is informed by the bootstrap
   context (glossary, briefs, ADRs, research) which is exactly the
   evidence base you want surfaced before you commit.
2. If the LLM's recommendation diverges from yours, that divergence
   is the cheapest signal you have that the question still has
   something to teach. Don't dismiss the divergence — interrogate
   it.

In the 060 grilling, the LLM's recommendation for inbox-shape was
"directory of files" (heavily pre-judged by the research). The
recommendation for entry-naming was a UUID short-suffix. The user's
"WDYT, but the slug should be descriptive" steer shifted the
sub-decision toward content-hash suffixes for idempotency — a
materially better outcome than either party's initial proposal. That
particular step exists in the conversation record because the user
asked for the LLM's view before committing.

### Ask for pushback when the LLM agrees too easily

Models default to agreement under social pressure. When the LLM
agrees with your proposal without surfacing trade-offs, push back
yourself: "What would push you toward the other option?" or "What
breaks if we do it this way?" or simply "Pushback please."

The grilling format is built around recommended-answers-with-evidence
specifically to make pushback structural rather than personal. When
the recommendation cites primary evidence, the pushback is a debate
about that evidence, not about whose preference wins.

### Don't merge questions

The grilling skill's "ask the questions one at a time" rule is
load-bearing. Two questions in one prompt — even closely related ones
— produce answers that conflate. Resist the urge to batch.

Where two questions truly interdepend, sequence them: ask the
*foundational* one first, propose the recommended answer, wait, then
ask the *derived* one with the foundational answer already in hand.
The 060 grilling sequenced Q1 (shape) before Q5 (entry naming)
specifically because the entry-naming decision is only meaningful
once shape is settled.

### Record decisions inline, while they're fresh

The running-log pattern: each settled question gets a paragraph
appended to a `## Decisions (running log)` section in the task file
*at the moment it settles*, not at the end of the session. This
serves two purposes:

- Survives interruption. A grilling session can run an hour or more;
  if the conversation drops, the log is the source of truth for what
  has and hasn't been settled.
- Produces audit trail without a separate phase file. The grove
  spine's first constraint (`SKILL.md` constraint 1) forbids phase
  files; the inline log gives the same legibility for free.

The decisions log is *not* the ADR. ADRs come at the end of grilling
(or sometimes during, if a decision is genuinely durable enough to
deserve one). The log is for the conversation; the ADR is for the
durable record.

## When to retire research into ADRs versus leave it in `docs/research/`

Research outlives the grove that commissioned it (constraint 6).
ADRs are the place where research findings become *binding* on
future work.

The rule of thumb: a research finding that *changed a decision* gets
cited in the relevant ADR's rationale section. A research finding
that *confirmed an existing decision* gets a "validated here against
…" note in the relevant ADR, or stays in the research doc with a
forward pointer.

The 060 grilling's "Findings adopted" pattern is the bridge in both
directions: the research doc gets a section pointing forward at the
ADRs its findings landed in, and each ADR has a rationale section
citing the survey by primary source. A future reader of either
artifact can trace the evidence chain without re-doing the research.

## Anti-patterns

- **The wizard.** A capture verb that opens an interactive prompt
  sequence ("title? description? type? component?") breaks
  mid-flow concentration. The prior-art evidence on this is
  emphatic — see ditz's failure mode in the postmortem survey.
  Capture must be one non-interactive gesture (flags or stdin).
- **The decision summary at session end.** Don't reconstruct
  decisions in the commit message or a session-summary file. The
  inline running log and the ADRs together are the durable record;
  anything else duplicates and rots.
- **The "ask if you have questions" non-prompt.** Vague invitations
  produce vague responses. If you want input on a specific
  trade-off, ask about that trade-off specifically. The grilling
  format's recommended-answer-with-evidence structure makes the
  trade-off visible by construction.
- **The pre-baked answer.** If you already know the answer and just
  want it executed, you don't need a grilling session — that's a
  work task. The grilling discipline exists for genuinely open
  decisions; using it for pre-decided ones is theatre.
- **The runaway tree.** Decomposition is meant to be lazy
  (constraint 4). If a planning session grows new child leaves
  faster than you can settle the current question, stop and ask
  whether the parent question was the wrong unit. The 020 subtree
  rolled its renumber four times — that pattern is exactly the
  signal that motivated the `grove leaf insert` CLI verb candidate
  in leaf 080.

## The shortest version

If you remember one paragraph of this doc, remember this:

> Commission research with a brief that names the downstream
> questions; demand primary-source citations and per-system
> walk-away checks. When the research is in, grill one question at
> a time with recommended-answer-and-evidence; ask the LLM "WDYT"
> before you commit and ask for pushback when it agrees too
> easily; record each settled decision inline in the task file as
> it lands. At the end, ADRs cite the research by primary source;
> the research doc gets a "Findings adopted" pointer back. That's
> the loop.
