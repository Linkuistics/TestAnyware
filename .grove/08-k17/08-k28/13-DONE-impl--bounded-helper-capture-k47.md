# bounded-helper-capture-k47


## Goal
Bound capture and reap after a synchronous inspection helper has been created.



## Context
Read the parent contract and existing inspection and workload ownership paths.
The private worker/shared scheduler remain k48, including unbounded creation.

## Done when
- Replace subprocess.run in the actual group query with nonblocking bounded
  stdout/stderr reads, a 256 KiB combined cap, exact-PID WNOHANG reap and no
  Popen destructor polling. Keep the caller's immutable inspection cutoff.
- Reserve a fixed portion of the existing query allowance for helper KILL/reap;
  record actual timestamps, errno, PID and residual owner separately from the
  workload. Do not signal a released helper identity. Admission failure is a
  sticky inspection failure; no later safety query retries it.
- Exercise real timeout/KILL, stdout/stderr overflow, partial output/EOF,
  successful capture and execute safety continuation. Simulate unreapability
  separately and retain strong ownership until independently reconciled.
- Retain frozen test/control receipts and an independent exact-resource audit;
  update the foundation report with this partial guarantee and its limitations.
- Insert a concrete review-impl bounded-helper-capture before k48. The final
  bounded-inspection-k30 whole-runtime review before k21 stays k48's obligation.

## Notes
No claim about creation latency, independent teardown, privileged denial,
setuid-ps KILL, live guests, storage isolation or immutable evidence publication.

## Decisions (running log)

The helper keeps controller-local ownership until k48 moves it into the private
worker. Use nonblocking pipes and waitpid(pid, WNOHANG), never communicate/wait
or Popen context-manager cleanup. Reserve min(50 ms, remaining allowance / 4)
before the existing cutoff for KILL/reap; no late turn extends either deadline.
Executable behavior controls and the scheduled fresh-context review contest this
interim ownership boundary; no extra in-session reviewer duplicates that read.

## Verification

The actual group query now uses the bounded capture path. Frozen condition
`experiments/exec-reliability/evidence/k47-check-01` retained 130 passing tests
and the initial lint failure; `experiments/exec-reliability/evidence/k47-final-01`
passed lint, formatting, strict types and all 130 tests after the lint fixes and
helper argv receipt field landed. No acceptance ceiling changed. Real timeout,
both stream overflows, healthy and partial capture, admission rejection and
execute safety continuation have retained receipts; simulated non-observation
has separate independent fixture reconciliation.

The independent `experiments/exec-reliability/evidence/k47-audit-01` audit passed
all input/artifact hashes, exact absence of 616 recorded real PIDs, 84 resource
markers and cleanup of 106 scratch directories. The foundation report records
the measured behavior and remaining synchronous creation/controller-local waits.
All private worker, publication, reset, protocol, scheduler, concurrent teardown
and final whole-runtime review obligations remain live in worker-scheduler-k48.
