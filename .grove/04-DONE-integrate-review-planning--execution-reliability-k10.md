# execution-reliability-k10

**Integrates:** execution-reliability-k9

## Goal

Triage the findings of the `execution-reliability-k9` review of the
`execution-reliability-k2` plan. Apply the real ones to the tree and the root
brief before the first causal leaf runs.

## Context

Read the findings in the commit of `execution-reliability-k9`. That review read
the plan at commit `eaabbb6`.

## Done when

- Each finding of `execution-reliability-k9` has a recorded verdict with its
  reason.
- The tree and the briefs carry each accepted change before
  `macos-completion-k3` runs.

## Notes

The review inserted this leaf, and the later leaves moved up one position.
Name a leaf by its handle, not by its position.

## Decisions (running log)

2026-11-01 — Read F1–F6 from review commit `8ba5e2dc`, against the current
root and k3–k8 bodies, the Grove review/continuation rules, issue #14 and the
retained downstream report. This session changes the plan, not the production
lifecycle or the commissioned acceptance bar.

2026-11-01 — F1 is a contract stated unclearly. Clean reviews cut no successor;
the root-close check can detect missing work, but the plan must not delegate
its normal continuation to a review or rely on that rescue. The design
producer will cut a concrete planning successor first and insert its review
ahead of it as its final producer act. That planner owns concrete preparation,
acceptance and handoff leaves after implementation; implementation producers
insert reviews ahead of the next live work. Findings insert reconciliation
ahead of that successor. Substantive redesign gets a producer/review chain
before dependent work rather than being absorbed by integration.

2026-11-01 — F2 is a real issue. Issue #14 records surviving macOS work after
CLI interruption and the later macOS transfer failure; k4 covers only the guest
deadline and k6 only Linux disconnect. Add a bounded macOS disconnect inquiry,
keeping child survival, channel responsiveness and file consistency separate.

2026-11-01 — F3 is a real issue. Exact `ProcessRunner.runShell` source trims
capture, ignores terminationReason, uses synthetic -1 for timeout and waits
3 seconds after TERM. These are source leads, not new measured failures.
Add a macOS outcome/capture inquiry through process, HTTP and Host CLI seams;
do not overload the ordinary-completion or post-timeout inquiries.

2026-11-01 — F4 is a real ownership/scope issue. Add preparation ahead of k3
to deliver and self-check the reusable bounded controller/ledger and establish
or explicitly mark unavailable each isolated-build route. ADR-0004 and the
Taskfile confirm Local Network and app-socket constraints. The review's claim
that golden creation is the only agent-install route is not established by
the cited override code; preparation must verify supported public alternatives
without bypassing routing, changing installed tools or touching retained
goldens. Human-dependent steps retain the exact prerequisite and remedy.

2026-11-01 — F5 is a real scope issue. Keep k7 as the historical macOS
transfer inquiry and add a Linux comparison that reuses its diagnostic
fixtures. Each yields its own usable report. Investigations may measure a
small immutable-publication control as an option, but design selects no
alternative without evidence and later implementation/preparation verifies
the actual selected contract.

2026-11-01 — F6 is a real dependency issue. Remove lifecycle-record corruption
checks from the transfer inquiry. Keep byte/framing/damaged-transfer controls
there; preparation after reviewed implementation owns all four acceptance
corruption controls against the selected terminal-record format, including
missing outcomes and contradictory termination records.

2026-11-01 — Cut `diagnostic-foundation-k11` ahead of k3,
`macos-exec-outcomes-k12` and `macos-disconnect-availability-k13` ahead of the
Linux inquiries, and `linux-download-consistency-k14` between the narrowed
macOS k7 inquiry and k8. The root and k8 now name every causal input. Kept k7
as a flat macOS producer rather than a node: the two OS reports are separately
useful artifacts, with reusable fixtures as an explicit dependency. No new
production API, requirement or ADR decision is introduced. The initial review
pointer now names k9/k10 rather than sounding like another review to commission.

2026-11-01 — Tier 2 graph evidence was checked against exact runShell/handleExec
snippets, the cited golden-override source and the Taskfile/ADR-0004 route
contract. Coverage generation `2026-11-01T03:47:00Z` recorded metadata matches
and no issues on the evidence and current tree paths; that is best-effort,
not proof of completeness. Discarded heuristic Swift-to-unrelated-Rust call
edges. The downstream Markdown report and issue #14 were read as documentary
evidence; no new guest measurements or production tests were run.

2026-11-01 — Verified every task header, position, required body section and
handle reference in the bounded `.grove/` tree. Corrected the ad hoc reference
check to join a soft-wrapped historical handle; it rejects a deliberately
missing handle and duplicate position and accepts the wrapped valid handle.
Confirmed the exact live dependency order and byte-for-byte preservation of
the root's commissioned requirements, inventory, bounds and agreed seams
before `## Decomposition`. Every task passes `grove-llm brief-chain` and
`mise exec -- task --list` reads the existing workflow. Reviewed the changed
plan against F1–F6. No implementation, live trial or repair result is claimed;
the root still has live investigations/design, so retirement closes no node.
