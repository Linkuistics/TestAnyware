# Bounded startup turns — k134

Worker startup readiness now advances through the existing common turn driver. Each callback performs at most one nonblocking read, bounded by the remaining 4097-byte startup frame allowance. Partial readiness yields to other turns; success, failure and closed-owner paths drop their readiness interests. The original readiness cutoff remains frozen, including rejection at equality. Expiry records its observation inside the callback before unwinding.

This delivers the startup slice of complete-worker-scheduler-k134. The entire original aggregate charter and historical handoffs were copied intact to flat sibling complete-worker-scheduler-k137 before narrowing k134. Worker.start remains synchronous. The production controller, query, workload, termination and retirement owners still need the shared scheduler and teardown overlap required by k28. This corpus does not establish aggregate acceptance. k28/k17 remain open; k18 retains storage work; guest acceptance remains 0/10 unavailable.

## Contract and sources

The runtime change is confined to ../../worker_capture.py, replacing the private select/readiness loop in Worker.start with advance_start/startup_readiness closures passed to run_turns. Admission, child setup, exact session/PID/PGID/SID checks, overflow/EOF/tail validation and sticky failure retirement retain their existing owners. No public asynchronous startup interface is added.

The contract is the narrowed k134 task and docs/specs/diagnostic-process-inspection.md. Relevant prior evidence is bootstrap-bounds-k101, bootstrap-bounds-k105, bootstrap-bounds-k106 and readiness-review-k136. Existing historical failures in those corpora remain evidence, including earlier process-table, deferred-reap, postcutoff, missing-result and expired-request failures. observed-deviations.json records the handoff to k137. No privileged fixture, guest operation, installed application change or unrelated-process operation was performed.

Graph coverage was checked for worker_capture.py, test_bootstrap.py and turns.py at generation 2026-11-15T02:19:52Z. worker_capture.py was metadata_changed, test_bootstrap.py metadata_match and turns.py not_tracked; exact source was read as fallback. No exhaustive graph claim is made.

## Controls and observations

Four new controls in ../../test_bootstrap.py use a real persistent worker. Three fragment its actual readiness reads into 1, 7 and 4097-byte allowances, count reads per callback, run a competing turn, and check terminal callback/readiness behavior before and after close. The fourth delays the first callback to the frozen cutoff with an injected clock, forbids readiness reads there, checks the retained expiry observation and sticky repeated start, and checks terminal callbacks. The three existing valid-frame deadline controls cover just-before, equality and after-cutoff receipt. Injected time is a boundary control, not evidence of a real storage stall.

| Frozen condition | Passed / cases | Observation |
| --- | ---: | --- |
| red-controls | 3 / 7 | Parent rejected by all four new controls |
| candidate-controls | 6 / 7 | Initial expiry-observation failure retained |
| candidate-controls-2 | 7 / 7 | Corrected final candidate |
| parent-full-2 | 903 / 907 | Exactly the four new startup controls fail |
| candidate-full-1 | 907 / 907 | All cases pass |
| parent-full-3 | 903 / 907 | Same four failures, same per-item outcomes |
| candidate-full-2 | 907 / 907 | Same per-item outcomes as candidate-full-1 |

The four full samples were run in the table's parent/candidate/parent/candidate order. Pytest elapsed times from their bounded checker receipts were 80.58, 83.06, 78.37 and 86.85 seconds. These timings describe samples, not a performance improvement. Ruff, formatting and strict Pyright passed in all four clean full samples. The same final evidence instruments were used throughout these samples.

Four frozen single-site mutants were rejected by the seven focused controls:

| Mutation | Failing cases | Sensitivity |
| --- | ---: | --- |
| no-expiry | 1 | Reading after the original cutoff |
| no-terminal | 4 | Terminal callbacks continue advancing |
| frame-limit | 3 | Enlarged read beyond the startup allowance |
| late-equality | 1 | Readiness at equality accepted |

All mutant failures are ordinary assertion failures; none timed out. Exact one-site source changes are recorded in comparison-plan.json and checked against the final frozen candidate by audit.py.

## Retained failures and exclusions

The first candidate failed test_startup_turns_expired_callback_never_reads because the outer handler sampled readiness_finished after the injected driver clock had been restored: 353624.035595625 was compared with the original 353625.030582791 cutoff. The fix records readiness_finished at the callback's expiry decision. inputs-final and candidate-controls retain the failing source and result; inputs-final-2 is the corrected candidate.

parent-full-1 is excluded from acceptance because the evidence instruments changed while the checker was running. Its before/after subject maps and both exact instrument versions remain in instruments-initial and instruments-excluded-parent. That run passed 902/907 cases, failed the four new controls plus test_persistent_worker_expired_request_is_sticky (residual_remedy was None), and Ruff caught an intermediate overlong mutation string. Formatting and Pyright passed. The recurring expired-request failure remains owned by k137; later clean samples are not a repair claim. This exclusion is a measurement error, not an expanded allowance for runtime failures. No failure artifact was overwritten or removed.

## Provenance, ownership and audit

prepare.py freezes top-level runtime/test Python files, pyproject.toml, uv.lock and the root Taskfile.yml into exclusive condition inputs. The parent worker is retained in parent-worker.py.txt. The red and final inputs differ only in worker_capture.py. Runtime/config/Taskfile bytes match inputs-final-2.

run_condition.py hashes all read sources, manifest, lock, frozen/current Taskfile, evidence instruments, comparison plan, deviations, spec, selected Python and fixture binaries before and after each measurement. Each condition has its own writer/output and retains JUnit per-item outcomes, stdout/stderr, commands, bounded checker receipts and artifact hashes. Full suites are bounded at 120 seconds, focused controls at 60, statics at 30; checker timeout fallback is TERM for two seconds followed by KILL/reap for two seconds. No measurement timed out. Every checker child was reaped.

The independent parent checker audits journaled fixture identities using PID/PGID/state/start-time/command outside the tested worker. Any residual fixture is revalidated before exact-identity cleanup and absence is recorded. There are 93 recorded fixture absence audits, including 13 from excluded parent-full-1; the other conditions contribute 80. These audits cover the journaled fixtures, not an assertion that every process was inventoried.

preserve.py records file digests and link text without following directory symlinks. Its before/after inventories cover 254,292 prior evidence paths, excluding this new corpus and interpreter/tool cache directories. No prior path changed, disappeared or was added within that scope.

condition-audit.json verifies subject/artifact digests (including archived instrument versions for the excluded sample), exact failures, all four mutants, unchanged live/frozen sources, prior preservation, checker reap/fixture receipts and the 907 per-item outcomes across both interleaved pairs. It reports integrity_verified=true and aggregate_accepted=false. This is an executable consistency audit; independent adversarial review remains a separate review-impl leaf before k137.

## Reproduction

Use the repository-root diagnostic:startup-turns-{prepare,check,preserve,tools,audit} tasks through mise exec -- task. Each check requires a fresh OUT directory. comparison-plan.json names every retained condition and mutation; existing result directories and exclusive inventory/audit files must not be overwritten. The full and focused checks above exercised these Taskfile entry points. Historical evidence stays immutable; fresh reproductions need a separate corpus or newly named outputs and preservation/audit receipts.
