# execution-reliability-k2

## Goal

Cut the smallest independently useful increments that establish the causes of
issue #14's failures, then enable a coherent command-lifecycle and
evidence-transfer design. Preserve the owner's agreed test boundaries and
acceptance bar while leaving later implementation detail to the findings.

## Context

- `plan-k1` records the commissioning decisions and the owner's explicit test
  seam agreement. Read its commit together with the root brief.
- The complete commissioning input is
  https://github.com/Linkuistics/TestAnyware/issues/14. Its retained evidence
  pointers and unchanged minimal payloads are inputs, not disposable examples.
- Scope boundary: https://github.com/Linkuistics/TestAnyware/issues/10 owns
  durable jobs/leases. Command cancellation here must not require that system.
- Starting source leads: the macOS `ProcessRunner` and `handleDownload`, Linux
  `handle_exec` and `run_server`, Rust `ExecResult` and `AgentClient` execution
  and download paths. Use the knowledge graph and exact source to verify them.
- Existing regression boundaries include macOS `ProcessRunnerTests`, Rust
  client `endpoint_integration` and `transport_seam`, and the public CLI/app
  routing tests. The root brief records the source limits and broad cleanup
  hazard in the existing macOS tests.

The bootstrap read installed 2.7.0 public help/schema and current source only.
It ran no guest workload, reproduced no failure, and selected no cancellation
API or mutable-file guarantee. Installed macOS agent source identity remains
unproved, as the issue states.

## Questions the decomposition must cover

Keep macOS ordinary completion and the earlier post-timeout inspection distinct
until evidence connects them. A causal increment must differentiate command
exit/wait registration, concurrent requests, bounded descendant inspection,
host transport and relay behavior against identified builds. File-backed
capture is already present; pipe EOF is not an established diagnosis.

Separate Linux timeout classification, owned-group lifecycle and serial request
availability after disconnect. A field added to the response cannot establish
cleanup, and a concurrent HTTP server cannot establish cancellation. Include
normal exit 23, signals, spawn errors and partial exact output in the contract
tests, not only deadline expiry.

Investigate the truncated successful download across handler, file-open and
metadata identity, HTTP body and host finalization. Compare different-length
atomic replacements, in-place changes, immutable binary reads and interrupted
transfers. Both a corrected mutable-read contract and a tested stable
publication alternative are permitted by the commission; neither is proved.

Use those findings to choose the smallest coherent lifecycle interface: typed
terminal outcomes, declared ownership, explicit cancellation, a separate
disconnect policy, bounded cleanup and collectable evidence. Reconcile affected
help/schema/doc discrepancies and preserve or deliberately version supported
agent compatibility. Keep the app routing contract intact.

## Done when

- The first causal-investigation leaves are concrete, bounded and independently
  verifiable, each with inputs, expected evidence, resource ownership and a
  failure/reproduction criterion. They preserve original failures and require
  an old-path failure or deterministic controlled reproducer for any repair
  claim. Unavailable old builds or non-reproduced symptoms stay explicit.
- The ordering makes investigation precede the contract choices it informs.
  Scope the design handoff around what the evidence must settle; do not choose
  an unverified implementation merely to fill a complete tree.
- The tree/briefs retain the necessary subsequent design review, material
  implementation review and reconciliation, reusable Taskfile checks, frozen
  ten-clone acceptance and durable handoff/resource audit. Grow each precise
  next leaf when its producer can state it; do not create empty review or
  integration sessions in advance.
- The full fixed trial remains five fresh clones per OS with every required
  case, unchanged comparison ceilings, exact byte/corruption controls and no
  replacement of failed samples. Baseline trials, deterministic regressions
  and final acceptance have distinct claims and identities.
- The plan specifies how isolated test builds and disposable baselines will be
  identified, reached through supported public interfaces and disposed without
  modifying installed tools, retained goldens or unrelated resources.

## Notes

The requirements are already settled; do not repeat the interview. Ask the
owner only when evidence exposes a new material requirement trade-off. The
public Host CLI is the acceptance boundary, with lower process/file/transport
regressions proving the responsible boundary; those regressions never replace
the complete live trial.

Prefer reuse of existing test boundaries and add the narrowest necessary
control at the highest existing interface when a race needs deterministic
forcing. Do not preserve old whitespace-trimming assertions where they
contradict the exact evidence requirement. Cleanup must identify the exact
owned process/group; broad name-based killing is unsuitable.
