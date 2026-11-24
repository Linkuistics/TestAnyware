> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# k155 review integration of the k152 soak

Review k153 recorded five findings against the k152 soak evidence. Each finding
is real, and each changes a claim that the k152 evidence makes. So this leaf
repairs all five. No finding needs a new soak. The frozen soak, inputs, plans,
control, failures and k152 checker stay unchanged.

## Dispositions

| Finding | Disposition | Repair |
|---|---|---|
| R1: the prebuffer stamp name claims a completed write | Real issue, repaired | The live fixture field is now `identity_write_started_monotonic`. The recheck renames the retained field. |
| R2: the checker enforces child spawn before child readiness | Real issue, repaired | The live peer comment states the order. The recheck drops that pair and enforces handler before readiness. |
| R3: the control never deletes a KILL | Real issue, repaired | The recheck requires KILL where the controller must send it. It deletes each KILL and expects detection. |
| R4: the helper predicate ignores fixture `/bin/ps` timeouts | Real issue, repaired | The recheck inspects each retained JUnit failure. The k152 claim now names the group-helper subset. |
| R5: the report states the host stall as proven | Real issue, repaired | The k152 report, verdicts and foundation section now label the shared pause as an inference. |

R1 to R4 each weaken a k152 evidence claim. R5 overstates one. Each finding
therefore meets the material-risk threshold of the integration.

## Repaired rules

The R1 stamp precedes witness I/O and pipe delivery. The shutdown branch holds
delivery until TERM. So only `helper_end_bound`, the bound after reap, follows
delivery.

The R2 parent stamps `child_spawned` after Popen, a declaration write and a
rename. The child runs independently, so its readiness can come first. The
parent stamps `handler_installed` before Popen, and both stamps use one host
clock. So handler before readiness is a real order.

For R3, the rule comes from `controller.py`. A group TERM always escalates to
KILL at the TERM deadline. A PID TERM whose leader exit precedes the deadline
needs no KILL. A return code of -9 also requires KILL. For any other PID TERM,
the evidence cannot decide, and the recheck reports "kill undetermined". No
retained record reaches that branch.

For R4, a failure or error that names both `/bin/ps` and `TimeoutExpired` is a
fixture helper timeout.

## Verification

`task diagnostic:soak-recheck` reads only retained files and runs no workload.
It wrote [recheck.json](recheck.json).

- It checked 282 records in 47 runs, 1 control run and 46 soak runs.
- No record has a problem under the repaired rules.
- It detected all 4277 damaged copies. These include each KILL deletion and a
  -9 PID record with no KILL.
- It accepted all 188 permitted sequences with readiness before the spawn stamp.
- KILL rules: 94 group deadline records require and hold KILL. 47 graceful
  group exits and 47 PID TERMs before the deadline permit its absence.
- Two JUnit failures exist, both in run 42. Neither is a fixture `/bin/ps`
  timeout. The predicate detects a synthetic fixture timeout.

The recheck also applies two weakened rule sets. Each one restores one frozen
k152 limit, and the recheck passes only when each weakened set fails. Without
the KILL rule, 188 damaged copies go undetected. With the R2 pair restored, the
weakened set flags all 188 permitted sequences.

The live peer and fixture keep their behaviour. The 27 prebuffer tests,
selected by `-k late_created_identity`, pass.

## Limits

- The recheck uses only the retained records. It does not extend the soak.
- The renamed stamp affects only future runs. Retained witnesses keep the
  k152 name, and the recheck maps it.
- Only damaged copies cover the "kill undetermined" branch. No retained
  record exercises it.
- The run 42 fixture leak stays with `worker-deadline-cleanup-k154`.
- The cause of the run 42 observation gaps stays unknown.

## Command

- `task diagnostic:soak-recheck` writes `recheck.json` in this directory.
