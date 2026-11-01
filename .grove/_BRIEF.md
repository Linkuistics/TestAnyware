# TestAnyware.gh-issue-14 — brief

## Goal

Make TestAnyware command execution, interruption, cleanup and evidence transfer
reliable enough for Ravel-Strategies to use it for executable tutorial
verification. The owner's commissioning input is
[TestAnyware issue #14](https://github.com/Linkuistics/TestAnyware/issues/14).
An occasional unexpected timeout is a hard failure, including an inspection,
cancellation, health or transfer operation. Preserve every failed sample.

## Done when

- The old failures or deterministic controlled reproducers are retained and
  the corrected controls demonstrate repair at the responsible boundary.
  Account separately for macOS ordinary completion and the original
  post-timeout inspection unless evidence establishes one cause. A clean run
  after non-reproduction does not establish a repair.
- Supported structured interfaces distinguish numeric guest exit (including
  23), timeout, cancellation, signal death, spawn error and transport failure.
  A synthetic -1 or stderr text is not an expected numeric guest failure.
- Timeout and explicit cancellation stop new work and terminate/reap the
  declared owned process group within stated bounds. Preserve partial evidence,
  verify known child identities, and state escaped-descendant limits. Failed
  inspection never proves absence.
- Explicit guest cancellation and abrupt CLI interruption have separate tested
  contracts. The outer controller stays alive. Observation, health and
  transfer remain responsive; VM disposal does not excuse a blocked channel.
- Exact stdout, stderr, status and binary evidence survive supported transfer.
  Missing, truncated or contradictory evidence never counts as success.
- Mutable-file reads have an explicit demonstrated consistency contract, or a
  clear exclusion plus an independently tested stable publication/read
  alternative. Atomic rename alone is not proof of a coherent remote read.
- Material implementation is reviewed and findings are reconciled. Shared
  protocol changes preserve compatibility or deliberately version it, update
  the agents, host types, help, schemas and documents together, and test the
  affected supported-agent wire shapes. No measured Windows reliability
  result is implied by the macOS/Linux experiment.
- The frozen full acceptance trial passes on five fresh macOS clones and five
  fresh Linux clones, sequentially, with the inventory and bounds below in
  every clone. Every unexpected timeout, wrong/missing outcome, surviving
  declared child group, byte mismatch, failed transfer or uncertain disposal
  fails the attempt and trial. Unreached cases are incomplete or unavailable.
- The durable public handoff gives causal findings, exact source/build/release
  identities, support limits, regression and integration results, resource
  audit, and exactly what the downstream tutorial trial must rerun. All owned
  resources are disposed or have concrete residual identities and remedies.

The previous tutorial acceptance trial is **0/10, unavailable**. Neither its
successful prefix nor the disposal of its four diagnostic VMs is acceptance
evidence for the correction.

## Constraints

Retain TestAnyware integration and fix the upstream boundaries. Durable jobs,
leases, pooling, placement, Grove orchestration and replacement backends are
outside this workstream. Issue #10 owns durable jobs and leases; issue #8 and
ADR-0004 own the existing app routing. A command cancellation interface may be
needed, but its exact API and disconnect policy belong to evidence-led design.

Use this isolated jj workspace and the repository's mise/Taskfile workflow.
Never patch the globally installed CLI/app or replace an existing golden to
obtain a pass. Integration uses explicitly identified isolated builds/agents
and disposable test baselines. Record any later owner-authorized installation
or baseline promotion as a separate operation. Never boot or modify a retained
golden, touch unrelated guests, or read private connection files to bypass the
public interface. On a macOS host, tests reach guests by VM id through the
TestAnyware app and Host CLI library (ADR-0004).

Freeze every input, checker, tool/image identity, case order and budget before
measurement. Use unique run IDs, one writer, before/after hashes of every read
subject, raw receipts and monotonic timings, and item-by-item results. Finish
edits before a run; do not edit beneath it or replace a failed sample. Changed
tools/images are new conditions, not reinterpretations of the old failures.
Keep raw interface failures visible even if another path yields better evidence.

The bounded result promises neither zero future failures nor hostile-code
containment, arbitrary descendant containment, controller-crash recovery or
tutorial export parity. A disposable test controller is not the production fix.

## Acceptance inventory and bounds

Every fresh clone checks absent installation/marker, performs the offline
installation, and verifies the intact byte result. It then runs ten ordinary
process-observation/health operations, expected numeric exit 23, the 2-second
child timeout, ten after-timeout operations and child checks, explicit
cancellation triggered after two seconds, ten after-cancel operations and
child checks, a separate abrupt CLI-interruption/fallback case, transfer and
corruption controls, guest cleanup and independent VM disposal. The next
fresh clone demonstrates reset. Use the issue's unchanged minimal payloads
for baseline comparisons; retain the complete fixtures where required.

The exact byte oracle is stdout hex
`4578706c61696e2074686973206368616e67652e` (20 bytes, no LF), stderr hex
`7365706172617465207374646572720a` (16 bytes), binary hex `00ff80410a`
(5 bytes), and numeric status 0. Independent damaged copies must reject an
appended stdout LF, a flipped binary byte, a missing terminal outcome and
contradictory termination records; verify the originals still match afterward.

| Operation | Comparison ceiling |
| --- | --- |
| Ordinary observation | 30 s |
| Inventory | 60 s |
| Intentional guest timeout | 2 s |
| Raw host exec | Guest budget +20 s |
| Transfer | 150 s |
| Independent disposal | 45 s |
| Guest cleanup | 30 s |
| Termination grace | At most 2 s |
| Following kill/drain/reap | At most 2 s |

Declare boot, slot, export and any new protocol-phase bounds before freezing
the run. No added sleeps, retries or raised ceilings may hide a stall. For the
abrupt-interruption comparison, the controller sends SIGTERM to that CLI after
two seconds, allows at most two seconds, then SIGKILL with another two-second
bound if needed. The outer controller survives and disposes only its VM in
`finally`, even when observation, transfer or guest cleanup fails.

## Test seams — agreed with the owner on 2026-11-01

Use the public Host CLI as the acceptance boundary. Reuse the existing
agent process/HTTP and Rust client transport boundaries to make failures
deterministic and prove causes without creating a parallel execution backend.

1. **Agent execution and file handlers.** Real owned subprocess fixtures prove
   completion, typed outcomes, cancellation, deadline races, bounded cleanup
   and exact captures. HTTP fixtures prove health/observation/transfer
   responsiveness during execution. File fixtures exercise different-length
   atomic replacement, in-place truncation/growth, immutable binary data and
   interrupted reads. Add a narrow injectable seam only where necessary to
   force a demonstrated race; propose it at the highest existing boundary.
2. **Agent protocol through the real Host CLI.** Extend existing HTTP/transport
   fixtures and CLI/app routing tests to check receipt fields, text/JSON
   semantics, timeouts, disconnects, transfer completion and version skew.
   Keep actual guest exit separate from host transport success. Include the
   affected supported agents' compatibility cases.
3. **The complete public path on disposable VMs.** The frozen ten-clone trial
   drives identified production test builds through the app on a macOS host.
   An independently bounded outer controller checks exact bytes and known
   process identities and records cleanup, transfer and disposal separately.
   Controlled lower-level passes do not replace this acceptance trial.

The owner agreed these testing boundaries. Exact API names, scheduling
mechanics, process ownership implementation and mutable-file semantics remain
design work after causal investigation.

## Decomposition

This exceeds one sitting: the macOS wait and transfer causes are unproven,
and their findings can change the lifecycle design. The bootstrap requirements
leaf preserves the commissioned contract and agrees the seams. A fresh
planning session will cut the smallest useful causal-investigation increments
before design, then grow implementation, review and frozen acceptance work as
the evidence makes their scope concrete. Do not prewrite a complete fix tree
from the source leads alone.

- `plan-k1`: establish the commissioned requirements and agree test seams.
- `execution-reliability-k2`: plan the first causal investigations and the
  evidence-dependent design handoff.

## Pointers

- Required ADRs: `docs/adr/0001-streaming-file-transfer.md` and
  `docs/adr/0004-each-command-opens-its-own-guest-connections.md`.
- Existing interfaces: `docs/architecture/agent-protocol.md`,
  `docs/architecture/cli-design-contract.md` and the public command
  `--help` / `schema` output. Reconcile discrepancies in this work's affected
  surface; do not assume that either prose or old schemas already defines the
  desired lifecycle.
- Glossary: Host CLI, In-VM agent, Healthy agent, TestAnyware app, Guest stream,
  Direct path and Golden image in `CONTEXT.md`.
- Reusable task entry point: `Taskfile.yml`; use `mise exec -- task <task>` and
  extend tasks as concrete test recipes emerge. Existing relevant entries
  include `cli:test`, `app:routing-test` and `agent:macos:test`.
- Retained evidence is identified in issue #14. Its full report was at
  `/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1/docs/experiments/tutorial-vm-reliability.md`;
  locate it in that project's history if the workspace has moved. Preserve
  originals and never manufacture a missing receipt.

## Notes

Bootstrap source inspection is **not a reproduction**. In this checkout,
Linux `handle_exec` still omits a timeout field on `TimeoutExpired`, uses
`subprocess.run` without a declared owned-group contract, and trims output.
Its server still constructs `HTTPServer`. macOS `ProcessRunner.runShell`
captures to files, trims trailing whitespace and waits through
`waitForExit`; `handleDownload` calls `FileIO.loadFile`. These are leads, not
proof of the old installed macOS binary's failure mechanism.

Existing `ProcessRunnerTests` exercise capture and timeout, and currently
expect trimmed output. Some cleanup uses broad `pkill -f 'sleep 30'`; do not
copy that cleanup or run it against unrelated host processes. Scope new
fixtures and cleanup to positively identified owned processes.

Existing Rust client `endpoint_integration` and `transport_seam` tests provide
HTTP and replaceable-stream boundaries. The current `ExecResult` has optional
`timedOut`; the installed 2.7.0 help/schema describes JSON transport success
separately from guest status, while the CLI design document's subprocess-exit
paragraph differs. Design must settle and test the affected contract together.

The Tier 2 graph inspection used project
`Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`,
generation `2026-11-01T03:26:22Z`. Targeted source and test paths had matching
coverage metadata and no recorded gap; documentation is excluded and was read
directly. No exhaustive code or call-graph claim is made. The graph returned
unrelated Rust targets for Swift system calls; only source-confirmed links
support this brief.
