# diagnostic-routes-k182

**Reviews:** diagnostic-routes-k19

<!-- tc: off because the creating session wrote this charter and a review does not rewrite it -->

## Goal
Adversarially review the public route probe, the inventory checker and the
golden-name guard before the first causal leaf (k3) relies on them.

## Context
- Read the producer commit for `diagnostic-routes-k19`, this grove's root
  brief, and the section "Baseline public routes (k19)" of
  `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- The checker is `scripts/vm_inventory.py`. Its controls are
  `experiments/exec-reliability/test_vm_inventory.py`. The probe is
  `experiments/exec-reliability/routes_k19.py` with
  `experiments/exec-reliability/routes_k19_run.sh`.
- The guard is the Taskfile task `vm:golden-name-free` and its call in both
  `vm:create-golden-*` recipes.
- Raw evidence is under `experiments/exec-reliability/evidence/k19-*`.

## Done when
- Check whether the checker can report absence while either inventory lists
  the name, or while an inventory read failed. Check whether disposal can
  accept any result other than a proved stop or a proved `VM_NOT_FOUND`.
- Check whether the fault controls (`k19-red-01`, `k19-red-02`) would reject
  a broken checker or guard that the 52 controls do not name.
- Check the frozen probe plans against the raw evidence: plan order, bounds,
  one owned clone per OS, retained failed samples, and the one revision
  between the Linux and macOS runs. Decide whether that revision is
  admissible under the root brief.
- Check each identity claim (app signature, guest agent hashes, guest OS)
  against the raw streams, and check the golden preservation claim against
  the before and after listings.
- Check that the guard runs before any create in both recipes, and that its
  executable defaults and variable precedence cannot bypass it.
- Report each finding with its evidence. Do not edit the producer's files.

## Notes
- Do not run `agent:macos:test`, start a clone, or create a golden. The
  review needs no live VM.

## Decisions (running log)

Reviewed producer `diagnostic-routes-k19`, commit
`e41868954fb0ebc22b5997e1175322af58a856e2`, against the current unchanged
producer files. This was inspection only: no tests, builds, formatters, live
VM commands, golden creation or production/test edits. The two findings below
meet the material-risk bar and need integration before k3 consumes this handoff.

### F1 — preserve the checked name through the create command (high)

`Taskfile.yml:1151` shell-quotes the name passed to the checker, but
`Taskfile.yml:1166` and `Taskfile.yml:1181` interpolate it unquoted into the
create command. The checker and creator can therefore act on different names.
For example, if the Task variable contains the literal value
`"testanyware-golden-linux-24.04"` (including the two quote characters), the
guard checks that quoted string, which is absent even when the ordinary
retained golden is listed. The create shell removes those quotes and passes
`testanyware-golden-linux-24.04` as the name. The double-quoted nonempty
preconditions at `Taskfile.yml:1148`, `Taskfile.yml:1161` and
`Taskfile.yml:1176` also accept this value; they do not enforce literal identity.
The same mechanism applies to the macOS recipe. No command was executed to
exercise it; this follows directly from the generated shell text.

This defeats the new promise that the recipes cannot replace a retained
golden, despite the guard running first. Keep the exact name as one literal
argument through validation and creation, or reject unsupported names before
any shell interpretation. The existing unquoted create lines predate k19,
but the added guard's preservation guarantee depends on repairing this seam.
`experiments/exec-reliability/test_vm_inventory.py:372` only inspects a dry
render with `NAME=k19-order`; the direct guard controls never run the composed
create path. Neither k19-red-02 mutation challenges this name mismatch. The
integration should verify this case safely with fake peers, for both recipes,
without creating a golden or invoking their real build dependencies.

### F2 — require an affirmative stop result for the requested identity (medium)

`scripts/vm_inventory.py:148` classifies every exit-0 schema-1.0 envelope with
`ok: true` as `stopped`. It does not check `stopped`, `dry_run`, or `id`, and
`scripts/vm_inventory.py:172` supplies no requested identity to the parser.
Thus a reply containing only `{"schema_version":"1.0","ok":true}`, or one
with `stopped: false`, `dry_run: true`, or another VM's id, followed by two
successful absent inventories, is reported as `disposed ID stopped` with exit
0. This is a source-derived counterexample, not an executed test.

The public CLI distinguishes these outcomes: real success emits the requested
`id` and `stopped: true` at
`cli-rs/crates/testanyware-cli/src/commands/vm.rs:335`; its dry-run success
emits `dry_run: true`, that `id`, and `stopped: false` at
`cli-rs/crates/testanyware-cli/src/commands/vm.rs:315`. Inventory absence still
protects against declaring a listed resource gone, but it cannot turn an
explicitly non-stop or unrelated reply into the required proved stop. A broken
stop route can consequently receive a successful disposal verdict whenever
the requested name is independently absent. Require the affirmative operation
and matching identity in the success branch while retaining independent
absence and the distinct `VM_NOT_FOUND` branch.

The fixture called `STOPPED` at
`experiments/exec-reliability/test_vm_inventory.py:242` itself omits `stopped`.
The refused-result table at
`experiments/exec-reliability/test_vm_inventory.py:256` checks exit/schema/error
failures, but none of the contradictory or unrelated success replies above.
The five k19-red-01 mutations therefore demonstrate their named defects, not
this success-branch proof obligation. Integration should add meaningful
controls for these replies and preserve coverage of real success and proved
`VM_NOT_FOUND` with independent absence.

### Other review conclusions

The absence checker reads both inventories before deciding. It checks public
schema, completion/count fields and entry identity, rejects malformed tart
entries, and treats failed or timed-out reads as uncertainty. No false absence
was identified in the inspected branches. The recorded 52 passing controls
and all seven rejected checker/guard mutations support those specific cases;
they do not establish the uncovered cases in F1/F2.

Both recipes sequence their guard before create and pass their task-local
checkout CLI to it. No executable-default or variable-precedence defect was
identified. The self-referencing defaults follow Task's documented template
default pattern; task-local values take precedence over call variables
([Task variable resolution](https://taskfile.dev/docs/guide/variables)). The
controls supply both fake executables explicitly, so they do not independently
exercise the default paths. F1 is about preservation of the argument value.

Inspected each OS's `inventory.json`, per-plan `plan.json` and `result.json`,
and raw call streams under
`experiments/exec-reliability/evidence/k19-route-linux-01` and
`experiments/exec-reliability/evidence/k19-route-macos-01`. Both inventories
freeze host, never-created, failed-acquisition, route, after-route in that
order. Bounds are boot 420 s, health 30 s, identity 22 s, listing 5 s, stop
30 s, disposal 41 s plus two 2 s termination phases, inspection 55 s and
doctor 60 s. Each OS acquired one owned route clone; failed missing-base
acquisitions were retained with exit 1 rather than the expected 3, never
replaced, and disposal/independent inspection succeeded. All ten results have
equal before/after subject hashes and empty residual sets.

Moving doctor last between Linux and macOS is admissible: these are separately
declared OS conditions with separately frozen inventories and different probe
hashes, not a retry of the failed Linux host plan. No edits occurred within
either recorded run, and budgets/oracles stayed fixed. Linux's unreached
signature/missing-name calls remain visible; the report does not claim they ran.

The macOS raw signature identifies `dev.antony.TestAnyware`, Developer ID
Application: Antony Blakey (TA43A4RUP3), and CDHash
`1997a6207bacd6e11b4971654a4138bcd3295b96`. Both OS runs hash the same app
binary (`2be50799f51fd7b0c4732f81478ea6a3da286e925b797766d280e611cff0e91c`).
Linux's nine guest source hashes match the installed bundle and its stream
reports Ubuntu 24.04/aarch64. The macOS stream reports 27.0.1/arm64 and agent
hash `1282f4cf53fe2ede6c7e94a2687888b5fa4ada3e2a920d227db2715679b28a84`,
distinct from installed bundle hash
`22dd56cec883a9c471420172d0261617ed02a4fea218f949ee598a5311bab9f5`.
Those claims and the unresolved old-agent source identity are represented
accurately. Both outer VM listings are byte-identical before/after; all seven
tart entries retain Name/Size/Disk/State, with access timestamps changing.
This supports inventory preservation, not a disk-content integrity proof.

The host/failure-plan verdicts remain failures in the summary; neither the
probe's process exit 0 nor its successful route result is end-to-end
acceptance. The report keeps 0/10 unavailable. One minor report limitation is
that `inventory.json` contains subject paths rather than hashes: the actual
before/after hashes are in the per-run results. This does not erase the retained
hash evidence and earns no additional repair obligation here.

Graph evidence was Tier 2 with generation `2026-11-15T02:19:52Z`. Exact scoped
symbol searches returned no usable route/checker/stop symbols. Coverage marked
the new scripts untracked and Taskfile/controller/report metadata changed;
direct source reads supplied the evidence (full checker/probe/shell/controls,
relevant Taskfile/controller/report and CLI stop ranges). The CLI VM source
had matching metadata and no recorded coverage issue. These are bounded
source and raw-evidence conclusions, not a graph completeness claim.
