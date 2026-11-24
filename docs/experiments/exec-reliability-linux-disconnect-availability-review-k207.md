# Linux disconnect availability implementation review

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

Review handle: `linux-disconnect-availability-k207`.
Producer: `linux-disconnect-availability-k6`, commit
`a4db78569a248510c4b5c5bec0964f873290c418`.

The retained container controls support serial request handling as the blocking
boundary. Four material gaps remain before the evidence consumer can accept all
of the report's timing, ownership and provenance claims.

## Findings

### P2 — A queued inspection does not establish when the live handler ended

Anchor: `experiments/exec-reliability/linux_k6_check.py:459–464`.

When `linux_bracket` has no sighting of the payload shell alive, `live_handler`
passes its upper bound alone to `handler_class` and labels the result
`queue-release`. An answer near 20 seconds becomes `deadline` even though the
shell could have ended earlier. The answer proves that the shell was gone by
that time. It supplies no lower bound on when it disappeared, or on when the
original handler released the server. Delay elsewhere in the request path can
produce the same upper bound.

In `live-02`, the term inspection starts at 2.009867792 seconds and ends at
20.076721042 seconds. Its shell row describes adopter `systemd --user`, PID
1130, with child 2194. The original CLI exited at 2.009563667 seconds after
SIGTERM. There is no payload-shell sighting alive and no original deadline
reply in this mode. Nevertheless, the retained checker reports `deadline`.
`live-01` has the same evidential limit: the term inspection answers at
20.069263750 seconds and records adopter PID 1128.

The k13 rule at `experiments/exec-reliability/disconnect_k13_check.py:511`
required a late lower sighting for non-control modes. Filtering out the Linux
adopter is justified; replacing that missing evidence with the upper bound
alone is not. Container observations support a queue-release hypothesis but
do not measure a bound on app/transport delay in the live term case.
`QUEUE_RELEASE` is also not enforced by the live classifier.

Keep the live term end as an upper bound or an explicitly conditional inference
unless independent evidence supplies the missing bound. Reconcile the definite
deadline claims in
`docs/experiments/exec-reliability-linux-disconnect-availability.md:32,59–64,128–129`.
The 18.066853250-second inspection latency independently supports `blocked`
against the control baseline; that finding need not be discarded. The expected
Linux timeout replies in control and concurrent provide evidence unavailable
after the term CLI has exited.

### P2 — Live cleanup can signal a process that the run did not create

Anchor: `experiments/exec-reliability/linux_k204.py:170–185`.

The live plan calls this imported cleanup at
`experiments/exec-reliability/linux_k6.py:170–171`. It reads a numeric PID from
`child.pid`, checks once that `ps` reports `sleep 120`, then signals that PID
with TERM. After polling, it can signal the same numeric PID with KILL without
rechecking its command or birth identity.

If the child exits and its PID is reused by another `sleep 120` before the
initial check, the replacement passes. If it exits after that check, any
replacement can receive TERM or the later KILL. A PID file and a generic command
string do not bind either signal to the created child. The cleanup therefore
does not satisfy the explicit requirement that it cannot kill a process outside
the run, and the report's claim of cleanup "by its identity" at line 34 is
stronger than the implementation.

The retained cleanup records show successful TERM cleanup, not a wrong-process
kill. Preserve that distinction. Capture the child's stable identity and
validate it through a stable signalling handle before acting, or otherwise
establish ownership without the numeric-PID reuse window. The container helper
`experiments/exec-reliability/linux_k5_seam.py:280–314` already demonstrates
pidfd-backed signalling with a start-tick check; a second `ps` followed by a
numeric `kill` would leave the race.

### P2 — Live provenance omits the app identity and installed-agent after-hashes

Anchor: `experiments/exec-reliability/linux_k6.py:172–180`.

The live subject set hashes the CLI, Tart, host Python, checkout inputs and the
installed agent's `server.py`. It does not identify the installed app executable
or retain its bundle/build/signing identity. The outer wrapper hashes all
installed Linux agent modules only before measurement
(`experiments/exec-reliability/linux_k6_run.sh:38`). Its after-hashes at line 60
cover the checkout/tooling list, not those installed modules. Thus the installed
bundle's `system_endpoints.py` has no paired after-hash. These host-bundle hashes
also must not be confused with a measurement of the clone's runtime sources.

The brief explicitly requires app identity and before/after hashes of every
read subject (`.grove/_BRIEF.md:295–305`). A change to an omitted installed
component can escape the current provenance check; equal checkout hashes do
not establish installed-tool continuity. Both live attempts retain the same
gap. Their recorded 39-subject before/after maps do match, as do their outer
tooling hashes, but those comparisons do not cover the missing subjects.

There is no evidence here that the installed components actually changed.
Contemporaneous after-hashes cannot be reconstructed now. Disposition the
retained run's limitation explicitly and complete the frozen provenance if a
new measurement is needed; do not represent today's hashes as the old after-state.

### P2 — Replaying a copied bundle reads the original worktree's raw evidence

Anchor: `experiments/exec-reliability/linux_k6_check.py:399–401,562–564`.

Both entry points read the selected `OUT/summary.json`, then pass its absolute
`run` path directly to the checker. The committed values name the producer's
`/Users/antony/Development/AbstractingComputers/TestAnyware.gh-issue-14/`
worktree.

After copying or checking out the complete bundle elsewhere, the documented
check command still reads raw files from the original worktree if they exist.
If they do not, checking fails despite the selected bundle containing its raw
evidence. Changes to the selected copy can therefore go unexamined while the
checker writes its result into that copy. Resolve the run inside the selected
bundle and keep the original path as provenance metadata. The current
`experiments/exec-reliability/linux_k5_check.py:453–465` already resolves a local
run directory for this reason.

## Evidence and review limits

The retained evidence is under
`experiments/exec-reliability/evidence/linux-disconnect-availability-k6/`:
container run `152e89f879cf43389f6d371c56e6e72a`, live-01 run
`f5158780256d4fad9def8d77a9554797`, and live-02 run
`bafa1b186d534d4cbe0893e48b5168b4`. The live timing and adopter observations
above come from each run's `burst-term.stdout`, not from rerunning the checker.

The Linux reply correction is supported by
`agents/linux/testanyware_agent/system_endpoints.py:33–49`: the timeout response
has exit code -1 and `Process timed out`, with no timeout flag. It is appropriate
to retain the CLI's `timed_out: false` rather than require the macOS shape.
Both failed live-01 acceptance and corrected live-02 evidence remain recorded.

Container health delays are 18.020636 seconds without disconnect,
18.032922 after close, 18.040790 after reset and 18.015609 after half-close;
the threaded control answers in 0.001719 seconds. The abandoned health request
fails at 5.006479 seconds and the next health call waits 13.007587 seconds.
Process and socket observations use the container clock; live request durations
use host timestamps. These comparisons require no subtraction across clocks.
Socket observations support queued requests, but cannot supply the missing
live handler lower bound.

The fixed container setup isolates the sweep in a fresh container. Its narrowed
selector excludes the init's `cases` invocation, and selected PID/start-tick
pairs are signalled through the pidfd helper. Every recorded case cleanup has
an empty `left` list and the final sweep is empty. The final sweep records
pre-kill matches and its checker rejects any such match, so it is not being
mistaken for post-cleanup proof. These observations do not repair the separate
live numeric-PID cleanup finding.

Recorded resource phases stay within their declared bounds, with cleanup and
disposal distinct. Live-02 cleanup takes 0.067776 seconds and disposal 0.710770;
the container records 0.081375 and 0.071504 seconds respectively. Successful
outer inventories support resource disposal independently of process cleanup.

This was an inspection-only review of committed source and retained raw
records. No tests, checker runs, builds, lint, formatting or new measurements
were run. Read-only JSON arithmetic reconciled timings and hashes. Graph
coverage could not be verified: the CBM CLI refused to start because a
pre-coordination or unverified generation was active. Direct source inspection
supplied the review evidence. Integration owns finding disposition and all fixes
and post-fix verification.
