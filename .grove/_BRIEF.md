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

### Working increments and dependency order

The first increment delivers the shared controller and verifies diagnostic
routes. The causal increments then deliver independently runnable diagnostics
and evidence without changing the production lifecycle. Their order gives
related inquiries a reusable starting point, not permission to merge their
conclusions. Each fits a leaf; decompose one if its bounded question proves
larger than a session.

1. `diagnostic-foundation-k11`: self-check the reusable bounded controller and
   resource ledger; establish each public isolated-build route or state its
   concrete unavailable prerequisite before causal measurement.
2. `macos-completion-k3`: establish the ordinary-command wait boundary and
   build identity; safely scope the existing process-test cleanup first.
3. `macos-timeout-inspection-k4`: distinguish timeout cleanup, descendant
   inspection and the later command's wait from the ordinary-completion case.
4. `macos-exec-outcomes-k12`: measure numeric exit, signal, spawn failure,
   deadline and exact/partial capture through process/HTTP/Host CLI seams.
5. `macos-disconnect-availability-k13`: measure known-child survival and
   independent request responsiveness after macOS CLI interruption.
6. `linux-timeout-lifecycle-k5`: separate deadline classification, partial
   capture and known-child survival through agent and Host CLI boundaries.
7. `linux-disconnect-availability-k6`: measure client interruption and serial
   request availability independently of timeout classification and cleanup.
8. `download-consistency-k7`: locate the historical macOS successful truncated
   download across file identity, framing, transport and host finalization;
   deliver reusable file/HTTP diagnostics.
9. `linux-download-consistency-k14`: reuse those diagnostics for the separate
   Linux comparison; a Linux result is not a historical macOS diagnosis.
10. `lifecycle-evidence-k8`: consume all eight causal reports and k11's route
    evidence and specify the smallest coherent supported lifecycle and evidence
    contract. An unresolved cause is retained as uncertainty or gets a precise
    new investigation before design relies on it; it is never filled by a
    source-based guess.

The causal leaves hand off green ordinary checks plus explicitly named
diagnostic commands that can report the known defect. Do not turn the normal
suite red for queued repair work, or assert the broken behavior as the desired
contract. Keep a controlled old-path reproducer outside the passing regression
gate until its correction lands, with exact expected failure and invocation.
Any narrow test seam is exercised through the existing process/HTTP/transport
boundary and separately identified from an uninstrumented production build.

### Common investigation contract

The controller API, verified failure controls and current public route
prerequisites are documented in
[`exec-reliability-diagnostic-foundation.md`](../docs/experiments/exec-reliability-diagnostic-foundation.md).
Its live-route unavailable classifications are preparation limits, not causes
or repair results. Recheck answering identities for each new diagnostic run.

`diagnostic-foundation-k11` owns the reusable controller, ledger, forced-failure
self-checks and the per-OS build/routing prerequisite table. Causal leaves reuse
that handoff and add only their question's concrete fixtures and diagnostics.
They verify the answering app/agent identities for their run rather than assume
the earlier preflight is still current. Preparation proves neither a cause nor
a production correction. A route still unavailable stays visible in each
affected report; k8 must insert further work before relying on a missing fact.

Each causal leaf reads issue #14 and the retained report, then freezes a finite
case list, tools, checker, budgets and inputs before its diagnostic run. It
records hypotheses, contrary evidence, per-case observations and unanswered
questions in its own durable `docs/experiments/` report. Raw evidence and runnable
fixtures belong under a project-owned `experiments/exec-reliability/` subject
directory, with hashes and stable references from the report; preserve earlier
artifacts in place and do not commit private host configuration. Report a
missing original as unavailable rather than reconstructing it. Old failures,
instrumented controls and corrected acceptance have distinct identities.

For live work, use at most one unchanged clone and one isolated instrumented
clone per affected OS in each causal leaf. Declare every workload and its count
before running; a failed clone is retained as a failed sample, never replaced.
Further diagnostic conditions require a stated new condition and, if larger
than this bounded inquiry, a new leaf. Lower-level deterministic fixtures use
unique temporary directories and positively identified processes; bound their
whole invocation independently, including failed wait/inspection cleanup.

Record the source commit and dirty diff, lockfiles and dependencies, executable
hashes and versions, app bundle/build/signing identity, agent binary or source
hashes, guest OS and baseline image identity. A matching version string is not
source provenance. The old macOS binary's source remains unproved unless the
investigation establishes it. Hash every read subject before and after the run,
use unique run IDs and one writer, retain argv/raw streams/status, HTTP or
transport evidence where available, and monotonic phase timings. No edits
under a measurement, retries, padding sleeps or raised comparison ceilings.

Read public help/schema to select supported commands. Reach each live guest by
VM id through the TestAnyware app and Host CLI library. Baseline runs may use
the identified installed tools unchanged. Instrumented runs use explicit paths
to checkout-built Host CLI/app/agents and new, uniquely named disposable test
baselines or owned clones. Clone retained stopped images without booting or
modifying them; install test agents only in owned disposable resources using
supported installation/public transfer interfaces. Do not read private
connection files or route directly to a guest. Golden creation retains only
ADR-0004's existing exception. Verify which app and agent actually answer before
measuring; an unresolved isolated-build/routing prerequisite makes that condition
unavailable, not a reason to overwrite the installed app or retained golden.
The route table names any required owner action, such as a Terminal-only
golden-build recipe or temporary app-socket availability. Do not stop an
installed app without separate explicit authorization. Verify supported public
agent-install alternatives before assuming golden creation is necessary.

The outer controller owns a resource ledger with exact VM/image/process/group,
temporary directory, archive and test-app identities. It remains alive when
the command CLI is interrupted. Its `finally` independently disposes only owned
VMs within 45 seconds, even after observation/transfer/guest cleanup failure.
Record guest cleanup within 30 seconds and disposal separately; inspect public
VM and hypervisor inventories for the recorded identities. Failed inspection
is uncertainty. Stop only a test app this run started. Dispose temporary
baselines/archives after evidence retention; report each residual identity and
concrete remedy. Never stop unrelated processes or use broad name-based kills.
Before invoking the existing macOS process suite, replace its broad `pkill`
cleanup with ownership-scoped fixtures in `macos-completion-k3`.

Extend `Taskfile.yml` only with the concrete recipes each inquiry establishes,
run them through `mise exec -- task`, and verify their failure detection with
controlled damaged inputs or forced failures. Reuse `cli:test`,
`app:routing-test`, `agent:macos:test` and the existing agent/client fixtures as
applicable; do not run the unsafe process suite before its cleanup is scoped.
Distinguish a passing instrument self-check from a defect detected by that
instrument. Preserve raw interface errors even when lower-level evidence works.

### Required continuation after the causal increments

The design producer owns its continuation: once its spec/ADR set exists, it
cuts a concrete `planning` successor for dependency-ordered working product
increments, then inserts its specific `review-design` ahead of that successor
as its last producer act. A clean review creates nothing and retires; the
planning successor is already live. Actionable findings insert reconciliation
before the first later live sibling. Substantial redesign becomes a new
design/review chain ahead of dependent planning, rather than an unreviewed
rewrite inside integration. Do not leave the tree with no live work while the
root's acceptance conditions remain unmet.

The later planner separates independently useful lifecycle, transfer,
compatibility/documentation and validation increments wherever each handoff
works. Keep protocol expansion and the reader that makes it useful together
when neither works alone. That planner also owns concrete preparation, full
acceptance and durable handoff/resource-audit leaves after the implementation
increments, with bodies stating their actual inputs and gates. Each material
implementation producer inserts `review-impl` before its next live consumer
once the change exists; findings insert reconciliation before that consumer.
This also applies to material diagnostic implementation. A clean review cuts
nothing; the next consumer already exists. No empty review or integration
leaves are cut now.

After reviewed fixes, the planned preparation leaf validates the independent
controller and all four corruption controls against the selected lifecycle
record format. Transfer inquiries own byte/framing controls only; they do not
invent terminal outcomes or contradictory termination records before design.
Preparation identifies isolated production test builds and disposable baselines,
and freezes every subject and all new phase bounds. Acceptance then runs the
complete inventory above on five fresh macOS
clones followed by five fresh Linux clones, sequentially, with item-by-item
outcomes. A defect fails the trial; subsequent safety observations preserve the
failure, and unreached cases remain incomplete. Any revised candidate is a new
condition and trial, never a replacement sample. The final handoff states what
Ravel-Strategies must rerun and audits every owned resource. The historical
0/10 unavailable trial is unchanged. This tree's planning is not a repair claim.

The initial planning artifact's review is `execution-reliability-k9`, with
reconciliation in `execution-reliability-k10` before the investigations. These
planning checks do not replace the later design and material implementation
reviews or any acceptance obligation.

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
