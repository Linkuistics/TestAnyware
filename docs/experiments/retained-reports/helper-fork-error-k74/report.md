> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Helper fork-error review integration — k74

The three source defects from helper-fork-error-k72 are repaired. A valid fork-error message confirms no child only after both channels end without partial or conflicting evidence. Failure to publish that message preserves the original fork errno. Every distinct valid helper identity remains an independent obligation. The query still fails when its worker exits uncleanly or has not been reaped.

This integrates the review committed at `b8d52826287cb6e1352f4872453a8cead22820a1`, which reviewed k69 at `adf436d473df`. Findings were checked against that commit and the current sources before repair. k71 retains the complete original scheduler charter.

## Finding dispositions

| Finding | Classification and retained reproduction | Repair |
| --- | --- | --- |
| Partial evidence preserves no-child | Real issue. All five RED channel controls failed: trailing control bytes, short header, truncated frame, oversized frame and open pipe tail. | Track both channel EOFs and revoke confirmation before framing guards when bytes remain. Preserve the existing empty-terminal-loss case when the explicit fork failure and EOFs are valid. |
| Callback overwrites fork errno | Real issue. RED callback timeout lost EAGAIN; broken pipe substituted EPIPE. | Catch the publication OSError inside the fork handler, retain its detail alongside the original error, and rethrow the original fork exception. The clean-path error message is unchanged. |
| Second identity disappears | Real issue. RED retained only one of two valid distinct synthetic PIDs. | Keep the first token key and add token:PID keys for additional identities; itemize every distinct identity in worker evidence. Publication grants no signal authority. |
| Late/worker-outcome controls missing | Real control gap. The four added controls already passed before production edits. | Rename the old mode to `worker-publication-expired`; add two actual late-parent-receipt controls plus unclean-exit and withheld-reap controls. |

## Conditions and bounds

Commands used the existing locked Taskfile workflows: `mise exec -- task diagnostic:format`, `diagnostic:self-check OUT=<condition>` and `diagnostic:resource-audit AUDITOR=<frozen script>`. Every self-check freezes declared subjects and hashes them before and after; it retains raw checker streams, JUnit cases, runtime receipts and artifact hashes. Checker bounds remain 30 seconds for lint, format and types, and 60 seconds for pytest, with the existing bounded termination/reap policy. New per-query controls use 0.5 or 0.6 second outer bounds. Neither source condition changed during measurement.

| Condition | Lint / format / types / tests | Cases |
| --- | --- | --- |
| [k74-red-01](../k74-red-01/verdict.json) | 0 / 0 / 0 / 1 | 315 pass, eight intended failures |
| [k74-green-01](../k74-green-01/verdict.json) | 0 / 0 / 0 / 0 | All 323 pass; pytest checker 33.52 seconds |

[Per-case comparison](../k74-audit-03/case-comparison.json) confirms exactly those eight failures became passes; every other named case was unchanged. Every checker was reaped without a timeout. `k74-inputs-red-01` and `k74-inputs-green-01` retain the corresponding Python sources, Taskfile, manifest and lockfile. `k74-inputs-01/prior-evidence.json` declares the 69,221 earlier evidence files protected before edits.

All new worker controls fork actual direct-child workers. EAGAIN is injected exactly at the helper fork boundary rather than induced by resource exhaustion. Framing and callback errors are adapters at the write seam; the open-tail control leaves an actual pipe open. The distinct-identity control publishes only synthetic PIDs 987654321 and 987654322 and never signals either. Parent-delay controls retain proof that the failure was published within its original query deadline before the parent read or select adapter delays receipt. The unclean worker exits 23. The unreaped adapter withholds the normal parent WNOHANG observation; exact-child cleanup is independently performed afterward, without rewriting the original receipt. Six copied publication/reap witnesses and their original-path SHA-256 manifest remain in `k74-audit-01`.

## Independent audit and limitations

[k74-audit-03/result.json](../k74-audit-03/result.json) verifies both complete conditions, their artifacts, final live/frozen sources, 14 earlier helper-identity controls, all 23 earlier fork-failure controls, all 12 repair controls, and all 69,221 earlier evidence files unchanged. Its 2,393 declared subjects have equal before/after hashes. A live/missing/mixed/invalid selector oracle preceded the OS observation: none of the 1,242 retained real positive exact PID selectors was present. Synthetic selectors were excluded. It checked 592 scratch directories and removed two exactly plan-matched `owned` mock markers only after the clean process observation. The audit signaled no process.

Rejected auditor attempts are preserved. Audit01 first failed lint on a long line (the original is `audit-lint-rejected.py`), then its measured version incorrectly applied legacy assertions to new controls. Audit02 excluded those controls but still counted the two new parent-late receipts in the legacy total. Audit03 corrects that selection. Its initial preparation assertion and premature missing-script Taskfile refusal are documented in `preparation-error.txt`; no measurement occurred during those two preparation failures. Frozen failed audit scripts and their diagnostic text remain intact.

Later PID absence does not acknowledge helper reap, prove historical identity, or repair unknown runtime receipts. Unpublished helpers remain unknown. The two synthetic identities are evidence-only obligations. There was no guest, installed-application or privileged fixture change. Ten-clone acceptance remains 0/10 unavailable; k18 still owns storage stalls.

## Handoff

This bounded review integration completes only k74. Persistent worker bootstrap before resources, full scheduler identities and epochs, the shared immutable scheduler, outstanding parent controls and final aggregate review remain with complete-worker-scheduler-k71. Every aggregate ancestor stays open. The changes repair the reviewed slice and require no new architectural decision or separate redesign chain.
