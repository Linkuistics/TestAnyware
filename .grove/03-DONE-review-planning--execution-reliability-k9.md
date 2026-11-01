# execution-reliability-k9

**Reviews:** execution-reliability-k2


## Goal

Challenge the causal-investigation decomposition before it governs live work,
and identify material gaps in scope, evidence, dependencies or continuation.



## Context

Read the producer's commit by `execution-reliability-k2`, issue #14, `plan-k1`,
the root brief and the six leaves it created. The commissioned requirements and
owner's test seams are settled; inspect the actual source/fixtures where needed
to judge whether the leaves can produce the promised evidence.

## Done when

- Test whether the five inquiries are independently useful and each fits one
  focused session, especially macOS wait/inspection instrumentation and the
  two-OS transfer investigation. Identify concrete narrower cuts where needed.
- Challenge attribution across actual agent builds, dependencies, Host CLI
  transport and the app relay. Check that the plan can isolate a cause without
  bypassing ADR-0004, pretending source provenance or adopting a presumed fix.
- Check old-fail/controlled-reproducer requirements, failure retention,
  deterministic controls, exact bytes, child identity and independently bounded
  observation/cleanup/disposal. A clean probe after non-reproduction is no repair.
- Verify design follows its causal inputs, and the lazy design review,
  reconciliation and later planning obligations cannot silently omit material
  implementation review or the full frozen ten-clone trial and durable handoff.
  Compare against every acceptance case and unchanged bound in issue #14/root.
- Commit actionable findings with evidence or a clear clean verdict. If there
  are actionable findings, cut `integrate-review-planning` with this review's
  handle and place it before the first later sibling holding live work, so it
  runs before `macos-completion-k3`. Do not transcribe findings as obligations
  into the integration body; that session must triage them from this commit.

## Notes

This is an inspection-only review of the produced plan, not another requirements
interview or implementation session. No empty reconciliation is required for a
clean verdict. Preserve the remaining causal/design leaves and root continuation.

## Findings

Reviewed artifact: commit `eaabbb6` by `execution-reliability-k2` (root brief
and leaves k3–k8), against issue #14, `plan-k1` and the current source. This
review ran no tests, builds or guest workloads.

### F1 — A clean design review leaves the tree with no live work

The `lifecycle-evidence-k8` task file (lines 55–60 at `eaabbb6`) and the root brief "Required
continuation" make the review-design leaf cut the next planning leaf when it is
clean. A `review-*` session produces findings only (grove `references/review.md`).
A review that finds nothing creates nothing and retires (grove
`references/decompose.md`, "The chain and the pair are built in opposite
ways").

On the clean branch, nobody cuts the planning leaf. The tree then has no live
leaf, and the driver starts a `finish` session while the root's acceptance
conditions are unmet. The same gap exists later. The root says "After reviewed
fixes, grow concrete preparation, frozen full acceptance and durable handoff".
It names no producer that owns those cuts. A clean `review-impl` cuts nothing.

Consequence: the ten-clone trial and the handoff can be lost on the success
path, which is the path most likely to occur.

Suggested direction: give each continuation cut to a producer. For example,
k8 cuts its `review-design` leaf and then a `planning` leaf after it. The
integration placement rule then inserts any reconciliation before that
planning leaf. The later planner cuts the preparation, acceptance and handoff
leaves after the last implementation increment.

### F2 — macOS abrupt CLI interruption has no inquiry

Issue #14 failure C states "Both OSes left a known sleep child alive" after the
SIGTERM to the CLI. Failure D occurred during the third macOS observation after
an abrupt interruption (`after-abrupt-03`). The acceptance inventory requires
the abrupt-interruption case in each macOS clone. `linux-disconnect-
availability-k6` is Linux only. `macos-timeout-inspection-k4` covers the guest
deadline, not client loss. No leaf measures whether the macOS agent continues
the exec after the client disconnects. No leaf measures whether the known child
survives or whether later macOS requests stay responsive.

Consequence: k8 must choose one disconnect policy for both agents with evidence
from Linux only. The k8 leaf forbids filling such a gap with a source-based
guess, so it would have to stop and insert this inquiry late.

### F3 — macOS outcome classification and exact capture have no inquiry

`linux-timeout-lifecycle-k5` exercises exit 0/23, signal death, spawn error,
partial output with a trailing LF and the root byte oracle, but only for Linux.
No macOS leaf has these cases. The current macOS source has defects of the same
kind:

- `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:83-84` removes
  trailing whitespace from stdout and stderr. The stderr oracle ends in LF.
- `ProcessRunner.swift:86` uses `process.terminationStatus` without
  `terminationReason`. A death by signal reads as a numeric exit.
- `ProcessRunner.swift:69` allows 3 s after SIGTERM. The root bound is at most
  2 s.
- `ProcessRunner.swift:86` gives a synthetic `-1` on timeout.

Consequence: k8 must specify typed outcomes and exact evidence for both agents,
but it receives measured evidence for Linux only. These cases are
deterministic and need no live guest, so a small addition can cover them.

Suggested direction: add the k5 contract cases to a macOS leaf through the
`ProcessRunner`, agent HTTP and Host CLI seams. Alternatively, cut one small
macOS leaf for them.

### F4 — The shared harness and the instrumented-build route have no owner

Every causal leaf relies on the root's outer controller, resource ledger,
bounded calls, receipt hashing and `finally` disposal. Every leaf also permits
"one isolated instrumented clone". No leaf builds the controller or establishes
the instrumented route. Thus k3 must build all of it before it can measure.
The source shows that the route is not simple:

- A checkout-built agent enters a guest only by golden creation with
  `TESTANYWARE_AGENT_BIN_OVERRIDE`
  (`cli-rs/crates/testanyware-vm/src/golden.rs:201-226`,
  `golden_linux.rs:249-268`). Golden creation is the direct path, and ADR-0004
  line 124 says it can need Terminal or another context with Local Network
  access. This session type may not have that access.
- A development app needs the single app socket. `app:bundle-live` says "quit
  a running app first", and `release:upgrade-check` needs "no other app on the
  socket". The root lets a run stop only a test app that it started. If the
  installed app runs, every condition that needs a checkout-built app or an
  app-relay probe is unavailable by construction.

Consequence: the attribution across agent, transport and app relay that k3
and k4 promise can become unavailable in each leaf. Each leaf would rediscover
this. k3 is also too large for one session with this work added.

Suggested direction: insert one preparatory leaf before k3. It builds and
self-checks the shared controller and ledger. It establishes, or declares
unavailable, the instrumented-agent route for each OS and the development-app
condition. It asks the owner where a human step is necessary, for example
golden creation from Terminal or permission to quit the installed app.

### F5 — download-consistency-k7 is too large for one session

k7 has four live clones, one unchanged and one instrumented for each OS. It
also has five file scenarios and an inspection of dependency source. It forces
interleavings at two agents, tests a new publication protocol and checks the
corruption controls. Each
of the macOS reproduction and the Linux comparison is an inquiry that is
useful alone. The leaf's own note allows decomposition, but the plan states
two separable questions as one leaf.

Suggested direction: split k7 into a macOS leaf for the observed failure and a
Linux comparison leaf. Put the deterministic file and HTTP fixtures in the
first leaf so the second can reuse them. Consider whether the trial of the
immutable-publication alternative belongs after k8 chooses it. The root
permits a "stable publication/read alternative" that is independently tested,
and that test can follow the design.

### F6 — k7 owns acceptance-checker controls whose subject does not exist yet

The `download-consistency-k7` task file makes k7 check "appended stdout LF,
flipped binary byte, missing terminal outcome and contradictory termination
records". The root assigns these controls to preparation ("Preparation
validates the independent controller and all corruption controls"). The
"terminal outcome" and "termination records" are parts of the lifecycle record
that k8 has yet to define. k7 must either test a format that does not exist or
invent one before the design.

Suggested direction: remove those controls from k7 and keep them in
preparation. k7 can keep transfer-level byte controls, such as a flipped binary
byte against the `00ff80410a` oracle.

### Checked and found sound

- Causal separation: the macOS ordinary and post-timeout symptoms, and the
  Linux classification and availability questions, are separate leaves. Each
  leaf forbids absorbing a sibling.
- Old failures: each leaf keeps the original receipts, failed samples and an
  old-path reproducer separate. Each leaf records a non-reproduction as such
  and does not count a clean run as a repair.
- Bounds and oracles: the leaves and the root carry each issue #14 ceiling, the
  exact byte oracle and the five-clones-per-OS trial without change.
- Order: k8 follows all five reports and can insert an investigation for a
  missing fact that is load-bearing.
- Evidence: the retained Ravel-Strategies evidence is at the path that the
  root names. The Linux source leads (`HTTPServer`, the `TimeoutExpired`
  handler, `rstrip`) and the macOS leads agree with the current source.

## Decisions (running log)

2026-11-01 — Findings F1–F6 are actionable. F1–F3 are gaps that could make the
plan miss a root acceptance obligation or force a late insertion. F4–F6 are
scope and placement defects. This review cuts one
`integrate-review-planning` leaf before `macos-completion-k3`, and the
integration reads the findings from this commit.
