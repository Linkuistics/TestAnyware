# TestAnyware Rename Inventory — 2026-04-20

Inventory of every `guivision` / `GUIVision` / `GUIVISION_` reference (and
project-level `Redraw` / `TestAnywareRedux` mentions) that lives outside the
new `~/Development/TestAnyware/` monorepo and outside `~/Development/_archive/`.
Generated for Task 7.1 of the TestAnyware Unification plan; consumed by
Task 7.2.

## Source scans

- **Bucket 1 (code).** Token: `guivision|GUIVision|GUIVISION_`.
  Globs: `*.{swift,py,cs,fs,rs,ts,tsx,js,go,rb}`.
  Plan-listed bash recipe:

  ```bash
  grep -rln --include='*.swift' --include='*.py' --include='*.cs' \
    --include='*.fs' --include='*.rs' --include='*.ts' --include='*.tsx' \
    --include='*.js' --include='*.go' --include='*.rb' \
    'guivision\|GUIVision\|GUIVISION_' \
    --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git .
  ```

  Run via the Grep tool with the same glob/token, then post-filtered to
  exclude `/_archive/...` and `/TestAnyware/...`.

  **Important addendum:** the plan's bucket-1 glob omits `*.rkt`. Modaliser-
  Racket's whole `guivision-sdk/` is Racket source. I added a parallel scan
  of `Modaliser-Racket/` (no glob filter) to catch them; they appear in the
  per-project detail under "Code (extension not in plan glob)".

- **Bucket 2 (config/data).** Token: `guivision|GUIVISION_`.
  Globs: `*.{yml,yaml,json,toml,plist,sh,zsh,bash}` plus `.env*`.
  Plan-listed bash recipe:

  ```bash
  grep -rln --include='*.yml' --include='*.yaml' --include='*.json' \
    --include='*.toml' --include='*.plist' --include='*.sh' --include='*.zsh' \
    --include='*.bash' --include='.env*' \
    'guivision\|GUIVISION_' \
    --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git .
  ```

  Result, post-filter: zero live (non-archive, non-TestAnyware) hits.

- **Bucket 3 (docs).** Tokens:
  `guivision|GUIVision|GUIVisionVMDriver|GUIVisionPipeline|Redraw|TestAnywareRedux`.
  Globs: `*.{md,rst,adoc,html,txt}`.
  Plan-listed bash recipe:

  ```bash
  grep -rln --include='*.md' --include='*.rst' --include='*.adoc' \
    --include='*.html' --include='*.txt' \
    'guivision\|GUIVision\|GUIVisionVMDriver\|GUIVisionPipeline\|Redraw\|TestAnywareRedux' \
    --exclude-dir=_archive --exclude-dir=TestAnyware --exclude-dir=.git .
  ```

  `Redraw` is filtered manually (capital-R is a common verb) — see "Notes &
  special cases" below.

- **Bucket 4 (LLM_STATE plan state).** Token: `guivision|GUIVision|GUIVISION_`.
  Plan-listed bash recipe:

  ```bash
  find . -path './_archive' -prune -o -path './TestAnyware' -prune -o \
    -path '*/LLM_STATE/*' -name '*.md' -print | \
    xargs grep -l 'guivision\|GUIVision\|GUIVISION_' 2>/dev/null
  ```

  In practice every live LLM_STATE hit was already surfaced by the bucket-3
  scan (LLM_STATE files are `.md`); this bucket reclassifies them under the
  LLM_STATE column rather than the Docs column.

## Excluded directories

- `_archive/` — read-only history of the five harvested old repos
  (`GUIVisionVMDriver`, `GUIVisionPipeline`, `Redraw`, `TestAnyware`,
  `TestAnywareRedux`). Hundreds of matches, all historical.
- `TestAnyware/` — the new monorepo. Already renamed and verified end-to-end
  in Milestones 1–6.
- `.git/` directories.
- The Grep tool defaults strip `node_modules/`, `.venv/`, `.build/`, `target/`
  via gitignore-aware traversal; no extra exclusions were needed for this
  scan because none of the live hits were inside such directories.

## Summary by project

(Match counts are total occurrences, not file counts. Each project's full
file list lives in the per-project section below.)

| Project | Code | Config | Docs | LLM_STATE | Total files | Total matches | Action |
|---|---:|---:|---:|---:|---:|---:|---|
| Modaliser-Racket | 11 | 1 | 5 | 3 | 20 | 203 | rewrite-heavy |
| Ravel | 0 | 0 | 4 | 9 | 13 | 64 | mixed (live work refs + dated session logs) |
| APIAnyware-MacOS | 0 | 0 | 4 | 5 | 9 | 49 | mixed |
| 0-docs | 0 | 0 | 4 | 0 | 4 | 226 | rewrite-heavy (this is the unification doc set) |
| www.linkuistics.com | 0 | 0 | 6 | 0 | 6 | 73 | review (brand/marketing site) |
| (top-level) /Users/antony/Development | 0 | 0 | 1 | 0 | 1 | 1 | rewrite (project listing) |
| **Totals** | **11** | **1** | **24** | **17** | **53** | **616** | |

Action column legend:
- `rewrite-heavy` — almost every match needs to use the new name going forward.
- `mixed` — file-by-file judgment: live task descriptions / live knowledge
  files get rewritten; dated session-log entries are left alone
  ("don't rewrite history").
- `review-only` — uniformly low confidence; open and decide per file.
- `historical-only` — almost certainly all "leave"; flagged for human eyes.

The 53-file total breaks down to 10 unique action recommendations: **rewrite**
applies to most live code/docs; **leave** applies to dated session-log
entries inside Ravel and APIAnyware-MacOS LLM_STATE; **review** applies to
the public website and a couple of design specs whose status is mixed.

## Per-project detail

### Modaliser-Racket

The heaviest live consumer of GUIVisionVMDriver. The whole `spec/`
subdirectory is a Racket DSL that wraps the `guivision` CLI. It includes a
literal `guivision-sdk/` directory whose name will need to change too (or be
left as a historical name and its `provide`d identifiers renamed — Task 7.2
should decide).

#### Code (extension not in plan glob — `*.rkt`)
- `spec/guivision-sdk/exec.rkt` (12 matches): **rewrite** — defines
  `current-guivision-runner` parameter, `default-guivision-runner`,
  `find-executable-path "guivision"`, `getenv "GUIVISION_VM_ID"`. Every
  identifier and string is a live binding to the renamed CLI.
- `spec/guivision-sdk/agent.rkt` (3 matches): **rewrite** — calls
  `current-guivision-runner`.
- `spec/guivision-sdk/screenshot.rkt` (2 matches): **rewrite** — calls
  `current-guivision-runner`.
- `spec/guivision-sdk/input.rkt` (2 matches): **rewrite** — header comment
  + runner call.
- `spec/guivision-sdk/macos-helpers.rkt` (1 match): **rewrite** — header
  comment.
- `spec/runner/main.rkt` (8 matches): **rewrite** — `require` paths into
  `../guivision-sdk/*`, env-var reads (`GUIVISION_VM_ID`).
- `spec/runner/lifecycle.rkt` (6 matches): **rewrite** — `require` paths,
  CLI invocation strings.
- `spec/runner/driver.rkt` (1 match): **rewrite** — comment refers to
  `guivision-sdk` calls.
- `spec/tests/test-guivision-agent.rkt` (10 matches): **rewrite** — file
  name itself (`test-guivision-agent.rkt`), `require` paths, parameter use.
- `spec/tests/test-guivision-exec.rkt` (10 matches): **rewrite** — file
  name itself, `require`s, env-var reads, status string.
- `spec/tests/test-lifecycle.rkt` (4 matches): **rewrite** — `require`
  paths, parameter use.
- `spec/tests/test-macos-helpers.rkt` (4 matches): **rewrite** — `require`
  paths, parameter use.

Note: the directory `spec/guivision-sdk/` and the test files
`test-guivision-{agent,exec}.rkt` are themselves named for the old binary.
Whether the directory and filenames also rename is a Task 7.2 decision (the
binary name itself is the most load-bearing thing — once that's chosen, the
rename of the wrapper directory follows naturally).

#### Config
- `.gitignore` (2 matches): **rewrite** — line 16 ignores `.guivision-vmid`
  (the per-test-run VM-id handle written by the SDK). The artifact name
  changes when the CLI binary renames.

#### Docs
- `README.md` (9 matches): **rewrite** — top-level README documents the
  `guivision` CLI workflow and links into `../GUIVisionVMDriver/`. Every
  example needs the new name + path.
- `spec/README.md` (3 matches): **rewrite** — same pattern at the spec
  package level.
- `spec/docs/observable-state.md` (1 match): **rewrite** — design doc for
  the observable-state model that references the CLI.
- `docs/superpowers/specs/2026-04-18-modaliser-spec-design.md` (11 matches):
  **review** — dated design doc; check whether it's a frozen historical
  record or a living design spec. If frozen, leave; if living, rewrite.
- `docs/superpowers/plans/2026-04-18-modaliser-spec-v1.md` (102 matches):
  **review (lean rewrite)** — dated plan but heavy ongoing reference; the
  102 matches are mostly within the plan body that the orchestrator still
  reads. Recommend rewrite of the *body* references with a one-line note at
  the top noting the rename was applied post-hoc.

#### LLM_STATE
- `LLM_STATE/modaliser/memory.md` (5 matches): **rewrite** — consolidated
  knowledge file; live by definition.
- `LLM_STATE/modaliser/backlog.md` (7 matches): **rewrite** — task
  descriptions reference the CLI for verification work that will continue
  to use the renamed binary.
- `LLM_STATE/modaliser/latest-session.md` (matches via session-log): **leave**
  — dated session record. (This file appeared in the bucket-3 raw scan but
  inspection shows it is the `latest-session` mirror of `session-log.md`;
  that file is dated history.)

### Ravel

Ravel orchestrates downstream projects (including GUIVisionVMDriver, now
TestAnyware) and references both the `guivision` CLI and the
`GUIVisionVMDriver` project name throughout its sub-project plans. The
distinguishing question: is the file a *plan/memory/work-prompt* (live —
rewrite) or a *session-log* (historical — leave)?

#### Docs
- `docs/superpowers/specs/2026-04-12-sub-B-phase-cycle-design.md`
  (2 matches): **rewrite** — design doc lists `GUIVisionVMDriver` as one of
  the four legacy LLM_CONTEXT projects; needs the new name.
- `docs/superpowers/specs/2026-04-13-sub-A-global-store-design.md`
  (1 match): **rewrite** — example symlink layout includes
  `guivisionvmdriver -> /Users/antony/Development/GUIVisionVMDriver/ravel/`.
  The path/identifier changes.
- `docs/superpowers/specs/2026-04-16-sub-G-migration-design.md` (4 matches):
  **rewrite** — describes the migrate-project workflow on
  GUIVisionVMDriver; will refer to TestAnyware going forward.
- `docs/superpowers/specs/2026-04-16-sub-I-obsidian-coverage-design.md`
  (2 matches): **rewrite** — design doc references the `guivision + OCR`
  evidence pattern as the canonical verification approach.

#### LLM_STATE
- `LLM_STATE/sub-B-phase-cycle/backlog.md` (7 matches): **rewrite** — live
  task descriptions: the symlinks-validation spike will use the renamed
  CLI + golden images. Note `guivision-golden-macos-tahoe` etc are VM
  image names and may also need renaming (golden image names are
  artefacts of the GUIVisionVMDriver scripts — Task 7.2 decision).
- `LLM_STATE/sub-B-phase-cycle/memory.md` (1 match): **rewrite** —
  consolidated knowledge.
- `LLM_STATE/sub-B-phase-cycle/prompt-work.md` (4 matches): **rewrite** —
  live work-prompt referenced by the orchestrator at run time.
- `LLM_STATE/sub-B-phase-cycle/related-plans.md` (1 match): **rewrite** —
  declares peer-project dependency `{{DEV_ROOT}}/GUIVisionVMDriver`; the
  path itself changes.
- `LLM_STATE/sub-G-migration/backlog.md` (4 matches): **rewrite** — Task 6
  literally is "Run migration on GUIVisionVMDriver"; the task name and
  description target the renamed project.
- `LLM_STATE/sub-I-obsidian-coverage/backlog.md` (2 matches): **rewrite** —
  task descriptions referencing the `guivision + OCR evidence pattern`.
- `LLM_STATE/ravel-orchestrator/memory.md` (6 matches): **rewrite** —
  knowledge entries on non-disruption rules ("Never bulk-stage in
  APIAnyware-MacOS or GUIVisionVMDriver") become "Never bulk-stage in
  APIAnyware-MacOS or TestAnyware". Live operational guidance.
- `LLM_STATE/ravel-orchestrator/prompt-work.md` (1 match): **rewrite** —
  live work prompt naming the four dependent projects.
- `LLM_STATE/ravel-orchestrator/session-log.md` (29 matches): **leave** —
  dated session entries that recount what was done at time T. The
  "don't rewrite history" rule applies. The 29 matches are inside
  Sessions 1–N and accurately describe the project's state at the time of
  writing.

### APIAnyware-MacOS

Documents and tests Racket-OO sample apps via the `guivision` CLI in macOS
VMs. README + developer guide are user-facing and need the rename. Memory
file is live consolidated knowledge. Session log is historical.

#### Docs
- `README.md` (11 matches): **rewrite** — `### GUI Testing with
  GUIVisionVMDriver` heading, `GVD={{DEV_ROOT}}/GUIVisionVMDriver`
  variable, `GV=$GVD/cli/macos/.build/release/guivision`, env-var names
  `$GUIVISION_AGENT`/`$GUIVISION_VNC`, every CLI example. All live.
- `generation/targets/racket-oo/README.md` (1 match): **rewrite** —
  one-line reference to GUIVisionVMDriver evidence.
- `generation/targets/racket-oo/docs/developer-guide.md` (7 matches):
  **rewrite** — full developer guide section on VM testing; mirrors README.
- `docs/specs/2026-04-16-sample-app-portfolio-design.md` (2 matches):
  **rewrite** — design doc rationale references GUIVisionVMDriver as the
  automation tool. Even though dated, the rationale stays valid post-rename.

#### LLM_STATE
- `LLM_STATE/targets/racket-oo/memory.md` (14 matches): **rewrite** —
  consolidated knowledge: agent reliability notes, VM-spec resolution
  order, env-var precedence (`GUIVISION_VM_ID`, `GUIVISION_VNC`,
  `GUIVISION_AGENT`), accessibility quirks, exec-detached pattern. All
  live operational knowledge.
- `LLM_STATE/targets/racket-oo/backlog.md` (2 matches): **rewrite** — task
  acceptance criteria reference `GUIVisionVMDriver` for verification.
- `LLM_STATE/targets/racket-oo/related-plans.md` (1 match): **rewrite** —
  declares `{{DEV_ROOT}}/GUIVisionVMDriver` as the peer dependency.
- `LLM_STATE/targets/racket-oo/latest-session.md` (1 match): **rewrite** —
  this is the orchestrator-consumed "current state" mirror, not a dated
  log; the single match is a current-state summary that needs the new name.
- `LLM_STATE/targets/racket-oo/session-log.md` (10 matches): **leave** —
  dated session entries (Session 1 through current). Each match sits
  inside a date-headed entry that records what happened at that time.

### 0-docs

The unification effort's own design / plan / prompt — these *describe* the
rename and naturally use both the old and new names. They are the artefact
of Task 7 itself.

#### Docs
- `designs/TestAnyware-Unification.design.md` (60 matches): **review** —
  this is the canonical design that establishes the new names. Most matches
  are inside "before" examples, "old name → new name" tables, or the
  rationale section. The file should remain *internally consistent* —
  references that explain "here is what the old name was" stay; references
  that *use* the old name as if it were current become rewrites. Open
  per-section.
- `plans/TestAnyware-Unification.plan.md` (154 matches): **review** —
  same as above; the plan body necessarily mentions both names. Most matches
  are inside the per-task scan/rewrite recipes (which by definition refer
  to the old token). Don't blanket-rewrite — the plan needs the old token
  to remain quotable for future readers.
- `prompts/TestAnyware-Unification.prompt.md` (4 matches): **review** —
  same caveat.
- `prompts/APIAnyware-Reorganise.prompt.md` (8 matches): **rewrite** — this
  is a *different* project's prompt (APIAnyware reorganisation) that
  happens to reference GUIVisionVMDriver as a peer project and as a
  successor-to-TestAnyware. References should use the new name post-rename.

  Note: this file also contains the proposal "AppSpec/" two-repo split
  (lines around 156-161) that mentions a future `guivision-sdk/` folder
  inside AppSpec. Whether the future folder name itself changes is a
  Task 7.2 design decision — flagged.

### www.linkuistics.com

The public Linkuistics marketing site. Five of the six files mention
`Redraw`, `TestAnywareRedux`, `GUIVisionVMDriver`, or both as project
cards. Whether the marketing site rebrands the projects to "TestAnyware"
externally is a brand decision, not a mechanical rename — these are
flagged **review** as a block.

#### Docs
- `index.html` (5 Redraw + 2 guivision = 7 matches): **review** —
  homepage cards listing each project. Likely rewrite, but the scope
  ("does the public brand change too?") needs the founder's call.
- `k9m2x7f4w8.html` (6 Redraw + 4 guivision = 10 matches): **review** —
  appears to be an unlisted variant of the homepage (slug-named).
- `projects/guivisionvmdriver.html` (2 Redraw + 31 guivision = 33 matches):
  **review** — full project page. If the project page is renamed to
  `testanyware.html`, this file gets retired; otherwise the heavy rewrite
  is real.
- `projects/testanyware.html` (1 Redraw match, 0 guivision): **review** —
  this file already exists. Worth checking whether it's the new home or a
  historical project page.
- `projects/testanywareredux.html` (9 Redraw + ? matches): **review** —
  page about the (now archived) TestAnywareRedux project. Probably stays
  as a historical record (TestAnywareRedux genuinely existed and was
  archived); leave as-is.
- `projects/redraw.html` (13 Redraw matches): **review** — page about
  the (now archived) Redraw project. Same reasoning as above.

### /Users/antony/Development (top-level)

#### Docs
- `README.md` (1 match): **rewrite** — line 19 lists `Redraw` in a
  project enumeration. If Redraw is being absorbed into TestAnyware, this
  list entry goes away (or becomes "TestAnyware (includes Redraw)" — a
  Task 7.2 decision).

## Notes & special cases

### `Redraw` filtering applied

The bucket-3 grep for `Redraw` produced the file list above. Each file was
opened to confirm the match was a *project reference*, not the verb.
Concrete filter outcomes:
- All 6 `www.linkuistics.com/` matches are project-name references in
  marketing cards (`<h3>Redraw</h3>` style). Kept.
- `/Users/antony/Development/README.md` line 19 is in a flat list of
  project names (Reagent, Redeveloper, Redraw, Roadmap, …). Kept.
- All 0-docs unification-doc matches are explicit project references
  ("Redraw is one of the five repos being unified"). Kept.
- `_archive/Redraw/...` and other archive paths excluded as a directory.
- Non-project verb matches in unrelated files: none surfaced (the
  glob-restricted grep already kept the surface area small, and every
  surviving file lives in a context where `Redraw` capital-R is the
  project name).

### `_archive/` is enormous but irrelevant

Bucket 1 found 113 files inside `_archive/GUIVisionVMDriver/` and
`_archive/GUIVisionPipeline/`; bucket 2 found 73; bucket 3 found dozens
more. **Zero of these are inventoried** because the rule is to leave the
harvested old repos alone. Total archived match count is in the multiple
hundreds and would dominate the inventory; they are deliberately dropped.

### A scan-tool note

The plan recipe uses `bash grep -rln --exclude-dir=...`. The Grep tool
used here doesn't accept `--exclude-dir` — exclusions were applied by
prefix-matching the returned absolute paths and dropping anything under
`/Users/antony/Development/_archive/` or `/Users/antony/Development/TestAnyware/`.
A few raw results came back as bare relative paths (e.g.
`LLM_STATE/core/backlog.md`, `0-docs/plans/...`, `vision/docs/...`); these
were verified to live inside `/Users/antony/Development/TestAnyware/` and
were therefore excluded.

### `*.rkt` is missing from the plan's bucket-1 glob

Modaliser-Racket's whole CLI-binding layer is Racket source. The plan's
`grep --include='*.swift' --include='*.py' ...` recipe doesn't pick them
up. The Modaliser-Racket section above includes them under "Code (extension
not in plan glob)". **For Task 7.2:** treat `.rkt` as bucket 1 too.

### Three categories of golden-image / VM-id artefact names

Inside the inventoried files, several non-`guivision*` strings are derived
from the CLI name and may need their own rename pass:
- `guivision-golden-macos-tahoe` and `guivision-golden-linux-24.04` —
  Tart VM image names baked into Ravel's sub-B backlog.
- `guivision-default` — the conventional VM id used in APIAnyware-MacOS.
- `~/.local/state/guivision/vms/<id>.json` and `$XDG_STATE_HOME/guivision/`
  — the per-VM connection-spec state directory referenced in
  APIAnyware-MacOS memory.md and developer-guide.md.

These rename together with the binary or stay as-is by design — Task 7.2
should decide and apply uniformly.

### Marketing site is a brand decision

The www.linkuistics.com files are flagged `review` as a block, not because
the matches are ambiguous (they are clearly the project name) but because
*whether to rebrand externally* is not a mechanical decision. If the public
brand is "TestAnyware" going forward, the rename ripples through the
homepage, the project page, and possibly the unlisted `k9m2x7f4w8.html`
variant. If the public brand keeps "GUIVisionVMDriver" (with the new
internal monorepo name being a tooling implementation detail), the website
stays as-is.

### Top-3 rewrite-effort predictions for Task 7.2

1. **Modaliser-Racket** — 11 code files + a directory rename + 1 config
   file + 5 docs + 3 LLM_STATE files. Real Racket bindings; the
   `current-guivision-runner` parameter alone touches a dozen call sites.
2. **0-docs** — 226 raw matches across 4 files. The bulk is inside the
   plan/design/prompt themselves and most need to *remain* mentioning the
   old name (it's the historical artefact of the rename), but the
   APIAnyware-Reorganise prompt is straightforward rewrite.
3. **APIAnyware-MacOS** — README + developer guide are user-facing and
   need careful, complete rewrites; LLM_STATE memory has 14 matches of
   live operational knowledge that all rewrite.

(Ravel and the website are large by file count but the per-file changes
are mostly mechanical text replacement; complexity is lower than the top
three.)
