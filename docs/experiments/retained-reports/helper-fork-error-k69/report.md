> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Explicit helper fork failure — k69

The interim per-query worker now publishes a failed helper `os.fork()` separately from its terminal result. A timely, validated failure preserves the true errno and confirms that this attempt created no helper even if terminal delivery is lost. The query still fails. Setup errors, silence after intent, invalid/late publications and contradictory messages do not establish no-child evidence.

`inspection._spawn_helper` invokes the new optional callback only in the OSError handler immediately surrounding `os.fork`; pipe preparation, admission, gate release and exec cannot produce this message. The control message carries exactly token, positive integer errno, failed_monotonic and the original query deadline. The controller validates types, request/deadline and dispatch <= failure <= receipt < deadline. A complete terminal must additionally match errno, no PID, admitted state and start <= failure <= finish. Duplicate/conflicting control or invalid terminal evidence revokes the no-child classification. Any valid known helper identity remains independently retained until matching reap acknowledgement.

## Retained conditions

All commands used the locked diagnostic environment. Each self-check hashes its complete declared subjects before and after execution, records bounded checker completion, raw stdout/stderr, JUnit cases, runtime receipts and artifact hashes. Subjects were unchanged in all three samples. Earlier samples remain untouched.

| Condition | Lint / format / types / tests | Cases | Meaning |
| --- | --- | --- | --- |
| k69-red-01 | 1 / 0 / 0 / 1 | 288 pass, 4 fail | New boundary/lost-result controls expose missing metadata and unknown-creation conflation; one new import ordering lint defect also retained. |
| k69-green-01 | 0 / 0 / 1 / 1 | 309 pass, 2 fail | One optional-subscript type defect plus existing remedy-preservation and duplicate-error-text regressions; all repaired. |
| k69-green-02 | 0 / 0 / 0 / 0 | 311 pass | Final complete run; pytest checker 34.40 seconds, no timeout and all checkers reaped. |

`k69-inputs-01` retains RED source copies and a SHA-256 manifest of all 65,742 pre-existing evidence files. `k69-inputs-02` retains final sources, manifest, lockfile and Taskfile matching the final declared hashes. Intermediate green-01 has declared hashes, but no historical source-copy claim.

The four boundary controls run actual direct-child workers: injected EAGAIN at the helper fork call, the same failure with lost terminal, injected EMFILE during pre-fork pipe preparation with lost terminal, and disappearance at the helper fork boundary after intent. These are explicitly injected errors, not induced kernel resource exhaustion. Nineteen further transport controls cover errno bool/zero/negative/string/missing, extra fields, wrong request/deadline, pre-dispatch/future/NaN time, duplicate key/message, both failure/created orders, terminal errno/admission/time mismatch and late publication. The two contradictory-created controls publish synthetic PID 987654321; neither creates nor signals that identity. Existing healthy helper, gate, reap, admission and timeout controls also pass.

## Independent audit

`k69-audit-01/audit.py` passed the diagnostic resource-audit lint, format and execution recipe. It verified every retained condition's verdict/artifact hashes, final source copies, all 65,742 earlier evidence files unchanged, 14 existing identity controls and all 23 new fork-failure controls. A live/missing/mixed/invalid selector oracle preceded the `/bin/ps` observation: all 1,808 retained positive exact PID selectors were absent, with synthetic selectors excluded. It checked 833 scratch directories and removed three exactly plan-matched `owned` mock marker files only after the process observation. No process was signaled by the audit.

This later absence observation neither acknowledges helper reap nor repairs failed/unknown runtime receipts. It does not prove historical PID identity beyond the retained records. Tests retain their exact worker/helper evidence; unknown interrupted creation remains unknown. No guest, installed-app or privileged fixture changes occurred. Ten-clone acceptance remains 0/10 unavailable, and storage stalls belong to k18.

## Handoff

Only the explicit fork-error slice is complete. Persistent pre-resource worker bootstrap, full run/worker/request/workload/epoch identities, shared immutable scheduler, all outstanding parent controls and final aggregate review remain in complete-worker-scheduler-k71 with the original charter. All aggregate ancestors remain open. A separate review-impl leaf must assess whether message/terminal conflicts can incorrectly remove helper obligations or classify an uncertain creation as no child; no empty integration is pre-created.
