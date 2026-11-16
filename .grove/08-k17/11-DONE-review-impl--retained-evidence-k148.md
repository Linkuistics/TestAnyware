# retained-evidence-k148

**Reviews:** retained-evidence-k21
**Creator:** declared claude

## Goal

Review the retained-evidence implementation before k17 consumes its stream pins, result identity and verifier.

## Context

Review the commit that retires retained-evidence-k21.
Read that leaf's done-when list and its running decision log in full.
Read `experiments/exec-reliability/retention.py` and the `_call` and `execute` changes in `experiments/exec-reliability/controller.py`.
Read `experiments/exec-reliability/test_retention.py` and the late-writer modes in `experiments/exec-reliability/peer.py`.
Read the k21 section and the F5 and F6 rows of `docs/experiments/exec-reliability-diagnostic-foundation.md`.
Use the retained evidence in `experiments/exec-reliability/evidence/k21-red-02`, `k21-green-01` and `k21-audit-02`.

## Done when

- Each claim in the k21 section has a check against the code or the retained evidence.
- The review states whether each stream pin and the exact stdout oracle use the same bytes.
- The review states whether the identity and the verifier detect changed, missing and added files.
- The review states whether the late-writer control keeps both the termination failure and the later rejection.
- The review states whether the documented limits are complete and true.
- Each finding has a severity, a file and line, and a failure scenario.
- When findings need action, an `integrate-review-impl` leaf exists for them.
- No live guest, app or acceptance claim follows from this review.

## Notes

k17 stays open after this review. Unavailable setuid `ps` and UID-denial controls stay unavailable.

## Decisions (running log)

- Review scope is producer commit `14f1e91d8bcbd8506f5205a966672002a011d438`.
  This session inspects source and committed verification records only; it runs
  no tests, builds, linters, formatters or diagnostic workloads.
- The graph generation is `2026-11-15T02:19:52Z`. Coverage reports the new
  retention module and controls as untracked and existing changed files as
  metadata changed. Direct source reads and the producer diff supply the
  implementation evidence; stale graph call edges do not establish behavior.
- The raw stream pin and exact stdout oracle share the bytes returned by
  `observe_file`. Identity publication follows `result.json`, excludes itself,
  and later verification independently compares file and stream pins. Retained
  green controls support these guarantees within the documented snapshot limit.
- The retained late-writer audit preserves the original termination failure,
  detects the subsequent append and positively observes the declared child
  absent after release. No later digest replaces a failed process verdict.
- The handoff's audit categories contain one minor factual discrepancy: run
  `2b4b4846280f4b21a24f44f86640769f` has a successful `result.json` and its
  identity was deliberately removed by the missing-identity control. Thus six
  missing-identity directories are incomplete/non-run fixtures, and one is a
  deliberate corruption control; fourteen deliberate corruption cases exist
  overall. All seven still reject. Record this as an optional documentation
  correction because it changes neither acceptance nor a safety verdict; it
  does not earn an integration task under the material-risk threshold.
- Retirement's whole-contract check leaves k17 open. Its remaining concrete
  failures and real-control prerequisites are owned by flat sibling
  diagnostic-failure-controls-k149, which must attempt acceptance and repair
  instead of handing off another unchanged aggregate charter. No node closes,
  no ADR decision changes and no producer-review Creator line is rewritten by
  this review's retirement.

## Inspection findings

No actionable defect was established in producer commit
`14f1e91d8bcbd8506f5205a966672002a011d438`. No integration leaf is required.
The following checks use committed source and retained records; no verification
suite or diagnostic workload was rerun.

| Handoff claim | Inspection and disposition |
| --- | --- |
| Stream length, SHA-256 and monotonic interval; exact stdout uses the same bytes | `experiments/exec-reliability/retention.py:19` reads once and returns both bytes and pins. `experiments/exec-reliability/controller.py:828` uses that observation for the oracle after termination/reap handling; read failure fails the call. The retained second-read mutation control rejects the broken behavior. Supported. |
| Result identity follows the result without a cycle | `experiments/exec-reliability/controller.py:979` publishes identity after `result.json`; `experiments/exec-reliability/retention.py:38` excludes identity itself from the listing. All other listed run files, including the complete process result, receive pins. Supported. |
| Later verification rejects changed, missing and added files, including streams and result | `experiments/exec-reliability/retention.py:93` compares the file inventory and content pins, then separately compares receipt stream pins. Retained changed/missing stream and result cases, added-file cases, identity corruption cases and the omitted-stream-comparison mutation support the rejection paths. `Taskfile.yml` exposes the verifier as `diagnostic:verify`. Supported within the declared identity trust limit. |
| Green checks pass with frozen subjects and 18 new controls | `experiments/exec-reliability/evidence/k21-green-01/verdict.json` records lint, format, strict types and pytest exiting 0, reaped without timeout or residual remedy. Its `pytest.xml` records 968 tests, zero failures/errors/skips, including all 18 retention cases. All 24 before/after subject hashes agree and match the retained current subjects. All 2,361 recorded artifact digests match retained bytes. Supported as recorded evidence. |
| Old-controller comparison rejects the controls | `experiments/exec-reliability/evidence/k21-red-02/verdict.json` and `pytest.xml` record 19 failures, 238 passes and 711 deselections: all 18 retention cases plus reserved-name validation fail. All 23 before/after subjects agree; the old controller and comparison test inputs intentionally differ from current inputs. All 1,051 recorded artifact digests match retained bytes. Supported. |
| Late writer retains termination failure and adds later rejection | In green run `f583390a0bbe4167a8c0170a53343964`, the call exits 0 and its exact stdout matches, but `termination_ok` and the run verdict are false; `later` remains unreached. The receipt retains leader PID 43240, live child PID 43241 and the revalidation/cleanup remedy. The sibling `-late-writer-audit.json` records verification passing before release and failing afterwards solely for changed `late-writer.stdout`; the original result and call receipt remain unchanged. It positively records the child live before release and absent afterwards with empty `ps` output and exit 1. Supported. |
| Audit classifies every retained run directory | `experiments/exec-reliability/evidence/k21-audit-02/report.json` names exactly the 145 retained green run directories: 62 pass, 63 reject only added audit files, seven lack identity, two have invalid identity and eleven have changed/missing files. The rejection totals are supported; the prose's reason for one missing identity is inaccurate as described below. |
| No fixture survives; filter can detect a live fixture | The second audit's retained process query exits 0 with no matching peer. Its `filter-control.txt` records a positive match for PID 49312 running the current `peer.py hang`. Inspection of `audit.py` confirms the current-peer argv filter avoids the first audit's self-match. This is the recorded observation, not a claim of present process absence. Supported. |
| Earlier evidence is preserved | Read-only comparison of the producer against its parent enumerated all 295,217 prior evidence paths; none was changed or removed. The producer adds evidence while modifying seven existing non-evidence files and retiring its task. Supported. |
| Snapshot, trust and process/storage limits stay explicit | The handoff states that reverted intervening changes are invisible and a writer replacing identity requires an external anchor. Raw observation intervals and subsequent identity reads do not claim an atomic or immutable snapshot. The full process result retains worker/helper/workload distinctions and failed or uncertain inspection outcomes; hashing does not change them. Historical failures, actual setuid-`ps` KILL and genuine UID denial remain unresolved/unavailable, and blocked storage remains k18. These limits are truthful; no broader guarantee follows. |

### Optional documentation correction

Severity: low, nonblocking. Location:
`docs/experiments/exec-reliability-diagnostic-foundation.md:2073`.

Scenario: a reader uses the audit paragraph to distinguish incomplete runs from
deliberate corruption. Green run `2b4b4846280f4b21a24f44f86640769f` has a
successful `result.json`; its identity was deliberately deleted by the
missing-identity control. The paragraph incorrectly attributes all seven missing
identities to incomplete/non-run directories. The correct explanation is six
incomplete/non-run directories and one deliberate deletion, making fourteen
deliberate corruption cases overall. Every missing identity rejects, so the
error affects the narrative classification rather than the verifier or any
process-safety verdict. It does not block agreed acceptance or identify a
material risk and therefore creates no integration obligation.

### Parent assessment

This completes k21's inspection prerequisite only. k17's whole contract does
not close: the retained historical timing failures and k145 interrupted
live-group failure need causal resolution, and the two real privileged controls
remain unavailable. A concrete producer continuation must attempt those missing
conditions, repair demonstrated defects and seek required external prerequisites
without declaring substitutes sufficient. Passing later samples cannot erase
the original failures. The parent retains storage's separate k18 owner and
guest acceptance at **0/10, unavailable**. No guest, app or acceptance claim is
made by this review.
