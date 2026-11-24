> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Raw helper fork and reset — k62

The inspection helper now uses an explicit fork/reset/exec boundary with a plain local owner instead of a Popen object. The parent records the exact child before restoring its signal mask. The child redirects its two output pipes, resets inherited process state, opens stdin on /dev/null, makes it survive exec, and uses os._exit(70) if reset or exec fails. Existing capture limits and exact-child WNOHANG/KILL rules remain in force.

## Retained verification

Run with `mise exec -- task diagnostic:self-check OUT=experiments/exec-reliability/evidence/raw-helper-fork-k62/<run>`; format with `mise exec -- task diagnostic:format`. Every run retains its declaration, before/after subject hashes, checker receipts, stdout/stderr, pytest XML, artifact hashes and actual test receipts. The `sources/` snapshot in each run matches its recorded source hashes. Source subjects were not edited during a run. Checker budgets remain 30 seconds, pytest 60 seconds, then TERM for 2 seconds and KILL/reap for 2 seconds as declared by the existing runner.

| Run | Lint / format / types / pytest exit | Tests | Meaning |
| --- | --- | --- | --- |
| red01 | 0 / 0 / 0 / 1 | 251 passed, 4 failed | Old Popen boundary bypasses the new explicit child reset/exec controls. |
| green01 | 0 / 0 / 1 / 1 | 257 passed, 1 failed | Intermediate raw implementation: redundant stream guard and old ENOENT/no-child expectation need repair. |
| green02 | 0 / 0 / 0 / 0 | 258 passed | Intermediate green before adding the actual stdin inheritance check. |
| stdio-red01 | 0 / 0 / 0 / 1 | 256 passed, 2 failed | Preexec and postexec controls catch stdin closing on exec. |
| green03 | 0 / 0 / 0 / 0 | 258 passed | Final source, including stdin inheritance; pytest reports 30.966 seconds. |

The real child controls record PID/PPID/PGID/SID, default USR1/INT/PIPE handlers, cleared signal mask and wakeup FD, disabled GC, empty local owner registry, closed inherited evidence/wakeup descriptors and inheritable stdio. A pending real SIGUSR1 terminates the helper with the default disposition after reset. Reset-error injection and a real missing executable terminate with status 70 without inherited exit hooks. The closed-stdio fixture closes 0/1/2 before calling capture and verifies actual postexec stdin reads EOF. Existing healthy/partial/hanging/output-overflow cases and the controller/interim-worker suite pass.

Fork EAGAIN and parent mask-restoration EIO are explicitly labeled adapters, not proof of real kernel resource exhaustion. The retained residual-reaping control simulates waitpid non-observation, then independently reconciles the real child. These controls do not establish the aggregate scheduler fault contract.

## Preservation and limits

`prior-evidence-before.json` and `prior-evidence-after.json` cover every pre-existing file on disk beneath the evidence directory, outside this new subtree, including ignored files. SHA256 comparison reports exactly 50,053 files before and after, no additions, removals or changed bytes. This is a disk-preservation claim; a commit alone does not prove preservation of ignored evidence. All five new runs, including failures, are retained. `artifact-inventory.json` hashes this new subtree except itself.

No child-error channel was added: status 70 cannot distinguish reset failure, exec failure, or an executable choosing 70. A parent-side fork refusal still records its errno and no child. Helper creation and reset may still block inside the interim per-query worker; private capture/reap loops remain. This slice supplies no persistent worker, gated publication, controller-owned helper acknowledgement, independent identity authentication or shared immutable scheduler. Worker death or worker-group KILL does not prove helper death. The complete original remaining contract lives in complete-worker-scheduler-k64 and all aggregate ancestors stay open. The independent review is a separate review-impl leaf before k64.

No guests, installed applications, privileges or unrelated processes were changed. Ten-clone acceptance remains 0/10 unavailable; privileged ps-KILL and UID-EPERM controls remain unavailable without their separately authorized prerequisites.
