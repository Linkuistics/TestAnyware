> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# First-leader-reap review integration (k93)

Review: `first-leader-reap-k92` (`7f9975ef4d7a35e098a5d52c24986a3c76375d75`),
reviewing producer `6b678ed`. All five findings are real issues. The original
first-attempt loop remains intact: authority closes before an exact-child WNOHANG
attempt, even at or after K; success at K is inclusive; unsuccessful late
attempts neither poll nor sleep. This integration adds receipt evidence and
repairs the evidence tools rather than changing that policy.

## Findings and repairs

| Finding | Classification and result |
| --- | --- |
| F1 | Real receipt issue. Receipts now record `reap_attempts` before waitpid and `reap_last_result` as `reaped`, `not_ready`, or `error`. ECHILD keeps its original errno. Failure says the leader remains unreaped at the fixed deadline. |
| F2 | Real inventory and identity issue. `prior-tracked.json` lists exactly 92,812 evidence entries tracked at the review revision, with relative names, file/link kind and SHA-256. This includes 18 symlinks hashed as link-target bytes rather than followed directories. Equality is checked against `jj file list` at that revision. Historical checkout paths relocate by their original prefix; external host dependencies retain their identity. |
| F3 | Real auditor issue. Every historical or archived subject has an explicit sample/path binding. An unrelated archive cannot satisfy another subject. Controls cover inventory omission/addition/absolute paths, wait attempts, errno, deadline, receipt attempt/result, duplicate case identity, relocation, and both corruptions through the final `main` entrypoint. |
| F4 | Real coverage issue. Nine direct boundary cases and six caller cases cover ready/withheld/ECHILD at exact K and late boundaries. Caller cases reject later PID/group signals, including signal-zero probes. Changing `<= K` to `< K` fails exactly the direct and caller success-at-K cases. |
| F5 | Real discoverability issue. Check, tools, audit-controls and audit tasks now sit together in `Taskfile.yml`, using the k93 tools. The old recipe bytes remain in the historical evidence. |

The old manifest had 103,182 entries rather than its own baseline's 91,193
tracked entries: 12,009 ignored/local entries were included and 20 tracked ones
were absent. The new count uses the later review revision, which also includes
the k88 evidence; these are different baselines.

## Certified samples

The focused order is the same 25 cases in all three final conditions. Inputs
are frozen in `final-red-inputs`, `final-exclusive-inputs`, and `final-inputs`.
The checker ceiling stays 20 seconds for focused/controls work, 120 seconds for
the full self-check, and 30 seconds for lint/format/types. Checker escalation
stays TERM + 2 seconds, then KILL/reap + 2 seconds. No ceiling was increased.

| Sample | Result |
| --- | --- |
| `red-3` | 15 expected receipt failures, 10 passes, before the receipt repair. |
| `exclusive-2` | Two expected failures, 23 passes: direct ready/exact-success and caller exact-ready reject the exclusive-boundary mutant. |
| `green-2` | All 25 focused cases pass. |
| `full-1` | Actual `diagnostic:self-check`: all 469 tests pass; lint, formatting and strict types pass. Pytest took 50.82 seconds within 120 seconds. |
| `audit-controls-4` | All 13 corruption/relocation controls pass in 2.08 pytest seconds (2.31 checker seconds), within the original 20-second ceiling. Subjects are unchanged and the checker is reaped. |
| `audit-final-4.json` | Final certification of historical/current samples, exact tracked inventory, artifact and subject identities, and unchanged before/after hashes. |

The repaired tools also pass Ruff, formatting and strict Pyright. Auxiliary
control/harness files pass Ruff and formatting. Audit controls are separate from
the 469-case diagnostic suite.

Successful full audits repeat identity validation after the complete inventory
before-hashes and compare all hashes again afterward. A fast identity precheck
now rejects corrupt inputs before that expensive scan, so a deliberately wrong
subject does not consume the control's entire budget.

## Retained failures and limits

No earlier evidence was edited. The final audit certifies the k88 red-1,
green-1, full-1 and green-2 samples using exact historical identities.

- `red-1/launch-error.txt` records initial inventory/link and input setup failures.
- Interim `red-2`, `green-1`, and `exclusive-1` keep their raw observations, but
  their runner/Taskfile bytes changed without an archive. They are explicitly
  **not subject-identity certified**; final samples replace their evidential role.
- `audit-red-1` records the original auditor accepting both decoded corruptions:
  an omitted inventory entry and an unrelated subject/archive hash. This initial
  observation lacked the later outer subject-hash receipt.
- `audit-red-2` failed because the legacy harness ran from the diagnostic directory
  rather than the repository root. `audit-red-3` timed out at 20 seconds and was
  TERM-reaped; it supplies no fresh claim that both corruptions were accepted.
- `audit-final-1` failed on the new auditor's incorrect historical checker-order
  expectation. `audit-final-2` and `audit-final-3` passed with earlier helper bytes.
- `audit-controls-1` passed 12/13; its relocation assertion incorrectly treated
  the checkout-local venv executable as external. The corrected test checks both
  the relocated venv path and the resolved external interpreter path.
- `audit-controls-2` and `audit-controls-3` retained 20-second timeouts while
  reaching the final unrelated-archive rejection. The latter was isolated.
  Reordering early rejection fixed this; `audit-controls-4` keeps the same 13
  identities and ceiling. The prior timeouts remain failures.

`tools-1`, `tools-2`, and `tools-3` retain helper revisions before repairs.
Final red/mutant/green subjects bind the original audit, legacy harness and
control-test identities specifically to `tools-1`; no any-archive hash exception
exists. Earlier audit/control receipts can be compared with their corresponding
helper revisions without pretending they ran the final bytes.

Withheld WNOHANG and ECHILD are simulations around real owned children. Direct
cases independently observe a real zombie and, after cleanup, absence. The
caller keeps real exit observation and final process-table inspection before
freezing reap time. No actual kernel waitpid stall is induced. The relocation
control uses a different physical root with selected historical files; it is
not a whole-checkout execution on another host. Host tools/interpreter bytes
remain required dependencies.

Aggregate diagnostic reliability is still **UNMET**. Persistent-worker/shared
scheduler, stale-callback and aggregate-control work belongs to
`complete-worker-scheduler-k91`; its charter is unchanged. The k28/k17 nodes,
ten-clone 0/10 result, retained-file k21 and storage k18 concerns remain open.
No live guest, app, privileged or unsafe process-suite work was performed.

## Reproduction

Use fresh output paths:

```sh
mise exec -- task diagnostic:leader-reap-tools
mise exec -- task diagnostic:leader-reap-check INPUTS=experiments/exec-reliability MODE=controls OUT=<fresh-focused-directory>
mise exec -- task diagnostic:leader-reap-audit-controls OUT=<fresh-controls-directory>
mise exec -- task diagnostic:leader-reap-audit OUT=<fresh-audit-json>
mise exec -- task diagnostic:self-check OUT=<fresh-full-directory>
```

Run the whole-inventory audit separately from time-budgeted corruption controls
to avoid concurrent filesystem work. Historical provenance uses the stored jj
revision; the new inventory is deliberately larger than jj's default new-file
limit and must be retained in the integration commit.
