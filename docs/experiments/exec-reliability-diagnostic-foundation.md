# Execution reliability diagnostic foundation

The local diagnostic controller passes its revised forced-failure self-check on a
macOS host. It now observes graceful leader exit, distinguishes PID and group
interruption with real child controls, and records group inspection before
reaping. Review integration corrects false success with live group members,
fixture cleanup and checker timeout reporting. Initial fork admission now rejects
extra Python or native threads, changed Python or kernel SIGCHLD disposition,
native SA_NOCLDWAIT auto-reaping and unavailable native observations before
acquisition, retaining failed results.
Review integration k32 independently reproduced and repaired the Python signal
cache gap. The leader-ownership condition measured in k52-fixed-02 reaps leaders
with exact-PID nonblocking waits before a fixed deadline; failed
reaps retain an explicit owner and residual without destructor polling, except
ECHILD, which closes wait ownership without establishing an exit status.
The controller now requests at most one final process table, after the final
signal and observed leader exit. Missing exit evidence leaves inspection
unknown without a query; a live or unknown final table cannot trigger a retry.
It preserves k37's strict process-table acceptance, k33's KILL ordering and
k36's PID-only deadline cap. The
complete worker/scheduler boundary remains k30's work, continued by k34's
worker-scheduler-k38 subtree, continued by worker-scheduler-k40 and its
worker-scheduler-k42 subtree, worker-scheduler-k44 subtree and remaining
worker-scheduler-k48 subtree, with its remaining contract in
complete-worker-scheduler-k122.
A failed local inspection now remains failed throughout that execution.
Independent cleanup, disposal and public absence commands still run, but later
local observations cannot restore disposal certainty.
Independently supervised process inspection, retained-file identity,
stalled-storage disposal and live route
preparation still need the work named below before causal use. In particular,
the controller starts one persistent inspection worker before resource
acquisition. Helper creation runs inside that worker, and helper ownership is
published independently of result capture. Raw worker fork remains a synchronous
OS operation. Worker close now offers nonblocking begin/advance operations,
while its compatibility consumer still drives them synchronously. Shared
scheduling, actual shutdown/teardown overlap and the remaining aggregate controls
are unfinished; the bounded k110 result and its evidence limits are recorded
below.
This is preparation for the causal inquiries in
[issue #14](https://github.com/Linkuistics/TestAnyware/issues/14), not a repair
of TestAnyware. No guest was booted, no guest command was run, and no app was
started or stopped. Installed live guest routes are **not yet tested**; the
isolated app routes have the missing prerequisites listed below. The historical
acceptance result remains **0/10, unavailable**.

## Use and boundaries

From the repository root:

```sh
mise exec -- task diagnostic:self-check OUT=experiments/exec-reliability/evidence/self-check-new-condition
mise exec -- task diagnostic:preflight OUT=experiments/exec-reliability/evidence/preflight-new-condition
```

The self-check uses the locked Python project in
[`experiments/exec-reliability`](../../experiments/exec-reliability/).
Its [`controller.py`](../../experiments/exec-reliability/controller.py) exposes
immutable `Call`, `Resource` and `Plan` declarations and `execute(plan, evidence_root)`.
The evidence directory is created exclusively from a fresh `uuid.uuid4().hex`.
An existing directory is rejected. The self-check now requires explicit `OUT`,
creates it exclusively, and retains checker logs, JUnit per-test verdicts and a
separate overall verdict. `RUN` no longer selects a shared evidence directory.
Calls have unique names and explicit absolute
executables. The declaration is written before acquisition or measurement.

For example, a causal fixture can retain one independently bounded call:

```python
import uuid
from pathlib import Path
from controller import Call, Plan, execute

plan = Plan(
    run_id=uuid.uuid4().hex,
    calls=(Call("version", ("/opt/homebrew/bin/testanyware", "--version"), 10),),
    subjects=(Path(__file__).resolve(),),
)
evidence = execute(plan, Path("evidence"))
```

The caller must declare **every fixture/input file it reads**, including
checkers, manifests, exclusions and image/export identities. The controller
automatically hashes itself, its inspection module, the interpreter and explicit
command executables;
it also hashes declared subjects before and after. It does not discover
transitive Python imports, dynamic libraries, subprocess dependencies or guest
images automatically. Record those dependencies and their identities in the
run's declaration. Missing subjects prevent acquisition/cases; drift fails the
result. An equal before/after digest does not detect an intervening change
that was reverted.

Each call retains argv, raw binary stdout/stderr, numeric host return code or
spawn error, PID/group, wall start, monotonic start/elapsed time and signals.
It never interprets a guest terminal-record format or treats Host CLI JSON
transport success as guest success. `expected_exit` is a **host** exit oracle;
causal checkers must separately validate the guest receipt. Raw output remains
available even if JSON is malformed. An optional exact stdout oracle supports
positive absence checks. There is no decoding or newline normalization.

`expected_reason` defaults to `"exit"`. A workload may explicitly expect
`"interrupted"` or `"deadline"`, with the exact `expected_exit` still required.
For example, an interruption of a TERM-ignoring fixture expects reason
`"interrupted"` and host return code `-9`. A match allows later observations in
the same plan and on the same resource. The raw reason and return code remain
unchanged. Unexpected interruptions, observation/spawn errors and mismatched
exit/stdout still stop cases. Resource actions must expect ordinary exit;
expected interruption cannot turn a timed-out disposal into success.

The controller creates a separate process session for each command. A call
deadline signals its recorded local process group with TERM, grants at most
two seconds, then sends KILL and bounds leader reaping by at most two seconds.
`interrupt_after` instead signals **only the command CLI PID**, matching the
issue's abrupt-interruption comparison. The outer controller survives. This
does not send a guest cancellation request or contain detached descendants.
macOS kqueue observes exit before reaping so the leader identity is retained
until signals finish. Observation errors fail the call and still attempt
bounded termination/reaping.

TERM records a monotonic leader-exit observation without running `ps` during
grace. PID-only interruption skips KILL when the leader has exited. Group
deadlines wait until the fixed TERM deadline T and attempt KILL even after
leader exit, before any process-table query. Each group snapshot retains its
monotonic start/end, raw `ps` stdout/stderr (hex in JSON), status, member
PID/PGID/state and signal-0 result. The leader stays unreaped through every
snapshot and signal. Only a clean table containing that leader can classify
the observed group as live or zombie-only. A missing leader, usage error,
timeout, malformed table or other failed inspection stays unknown and fails
the call. Denied group TERM remains `permission_unknown` and fails termination;
a later zombie-only table cannot repair that earlier receipt. For group KILL,
EPERM with observed live members is permission failure, observed zombie-only
EPERM is distinct, and EPERM with failed inspection remains unknown.
These observations do not establish the kernel's historical cause for an
earlier EPERM or containment of arbitrary descendants.

The sole final query must start before K minus min(100 ms, `kill_wait` / 4),
reserving the tail for workload-leader reap. No query occurs without observed
leader exit, and a failed or late observation is never retried. Passing controls
do not establish a full inspection bound. KILL, inspection and
leader reaping share one deadline: T plus `kill_wait` for group termination,
or the earlier of observed PID exit and T, plus `kill_wait`, for PID-only
interruption. Late exit observation does not move the group deadline.
The receipt's `within_process_bounds` must be true for success.
There is no fresh reap allowance after inspection. Snapshots describe only
their recorded intervals, and group probes stop before reaping permits identity
reuse. The known-child fixtures perform separate exact-PID observations and
cooperative cleanup. A live final group now fails termination after normal exit
or PID-only interruption too, retaining observed members, PGID and a remedy.
PID-only interruption still sends no group signal. The caller must independently
revalidate and clean known children; no group signal is safe after leader reap.
Normal exit can still leave a writer: stream/result digests and demonstrated
late-write rejection remain the next increment.

The timing statements above remain conditional on process creation and host
scheduling. The actual query no longer uses `subprocess.run(timeout=...)` and
its post-timeout blocking wait. It captures nonblocking pipes and uses
exact-PID WNOHANG reap with fixed deadlines, as measured in k47 below. The
private worker and shared scheduler must still isolate creation and permit
concurrent teardown before causal use. Blocked evidence storage remains separate.

`budget` is the pre-termination waiting budget. Allow its value plus
`term_grace + kill_wait` within the operation's comparison ceiling: for example,
use at most 41 seconds with default termination bounds for a 45-second disposal.
Validation enforces that allowance for resource cleanup (30 seconds) and
disposal (45 seconds). Boot, slot, export and inspection bounds must be declared
by each causal inquiry. OS scheduling, process creation and filesystem I/O are
not hard real-time operations; a measured ceiling overrun is a failed diagnostic,
never permission to extend the ceiling. The current instrument runs on **macOS
hosts**, including those driving Linux guests; it is not a Linux-host controller.

Resources declare kind, exact run-named identity and separate acquisition,
guest-cleanup, disposal and absence-inspection commands. Every action must
name the exact identity as an argv token. Intent is retained **before** an
acquisition attempt, since a failed start can leave a resource. `finally`
attempts cleanup, disposal and inspection independently, in reverse acquisition
order. Cleanup failure remains a failure even if disposal succeeds. Disposal
is recorded as `disposed` only when both disposal and its explicit absence
oracle succeed; otherwise the ledger retains `uncertain` and the exact remedy
argv. The manifest is trusted code: identity validation is not hostile-command
containment. Choose exact owned commands, never broad process-name kills.

The names `plan`, `result`, `subjects-before` and the `intent-` prefix are
reserved before acquisition. Raised `OSError` from an action's evidence I/O
becomes a failed in-memory receipt; later teardown actions still run. An intent
write error prevents acquisition and still attempts safety work. Failure to
write the final result can still leave no result file. A **stalled** synchronous
open/read/write/hash can still block this controller, including its `finally`:
the 45-second disposal ceiling does not hold under that condition. The separate
`diagnostic-disposal-k18` design and its reviewed implementation must address
this before live measurement relies on independent disposal.

The first unexpected failed case stops the declared workload; later cases and
skipped acquisitions are listed as unreached. Safety cleanup still runs subject
to the I/O limit above. Controller crash, disk failure,
uninterruptible kernel operations, arbitrary descendant containment and guest
cleanup correctness are outside this instrument's demonstrated guarantee.
Normal local command completion alone proves neither absence of local helper
descendants nor guest process cleanup. Later fixtures must positively inspect
their known child identities. Mock disposal is not evidence of live VM disposal.

## Review integration and remaining prerequisites

Review `diagnostic-foundation-k15` is committed at `4bb47c05`, against producer
`dfd48c9e`. Integration `diagnostic-foundation-k16` records these dispositions:

| Finding | Disposition and practical limit |
| --- | --- |
| F1, interruption prevents later observation | Applied: explicit expected reason, exact host exit, reaped leader and stdout oracle govern continuation. Default failures still stop. |
| F2, live routes described as unavailable without a prerequisite | Applied clarification; actual probe, bounds and dual-inventory checker move to `diagnostic-routes-k19` ahead of causal consumers. No installed live route is proved here. |
| F3, raised or stalled controller I/O skips disposal | Applied reserved-name validation and per-action raised-I/O isolation. Blocking-storage architecture moves to `diagnostic-disposal-k18` and its implementation/review continuation. A blocked evidence filesystem still voids the disposal ceiling. |
| F4, unconditional escalation and ambiguous group evidence | k20 is reviewed in k22 and reconciled in k23 below. Live-member false success is corrected. Sampling and helper supervision remain explicit prerequisites; termination bounds are conditional until their reviewed repair. |
| F5, mutable raw captures and no stream/result identity | Real, moved to k17. The self-check wrapper now hashes retained artifacts after the suite, including result files; general `execute` receipts still lack stream lengths/digests and cannot freeze a surviving writer. Neither a snapshot nor normal leader exit proves stable publication. |
| F6, weak controls and missing checker verdict | k16 supplies wrong-stdout rejection and checker verdicts. k20 adds opposed PID/group fixtures, deliberate scope mutations, a separately observed controller survivor and graceful fork reaping. Remaining late-writer controls belong to `retained-evidence-k21`. The old index stays historical. |
| F7, incomplete historical fake-resource audit | Applied by new itemized local audits. Every historical uncertain entry has its exact path checked; original uncertain receipts remain unchanged. See the audit below. |
| F8, already-absent disposal remains uncertain | Retain the conservative result now. k19 must demonstrate a specific VM_NOT_FOUND outcome plus independent absence before accepting it; arbitrary exit 3 is insufficient. Existing remedy argv may fail for an absent resource. |
| F9, small inconsistencies | Applied explicit exclusive OUT, preservation of missing preflight subjects, acquisition accounting and macOS docstring. Existing-golden refusal moves to k19: NAME-nonempty alone is insufficient. Do not use those recipes as a demonstrated safe replacement guard. |

These are three working increments before the causal leaves. k17 is now a node:
k20 delivers process controls, followed by its review and k23 integration.
Bounded-inspection design, review and implementation precede k21, which delivers
retained-file controls and its review. k18 delivers the storage
isolation design and cuts its concrete implementation/planning and review chain;
k19 verifies routes only after those repairs. This integration does not convert
their open properties into an instrument guarantee.

The new evidence is separate from all historical runs:

- [`k16-red-01`](../../experiments/exec-reliability/evidence/k16-red-01/) retains a checker-launch failure: resolving the venv interpreter symlink selected a Python without pytest. It supplies no controller failure evidence.
- [`k16-red-02`](../../experiments/exec-reliability/evidence/k16-red-02/) has eight failing new controls and two passing controls against the unchanged controller. [`k16-red-03`](../../experiments/exec-reliability/evidence/k16-red-03/) adds the failing missing-subject control: nine fail, two pass. The expected-reason controls first fail on the missing declaration field; the collision and receipt-exception controls execute the failing old paths.
- [`k16-green-01`](../../experiments/exec-reliability/evidence/k16-green-01/) passes all 24 tests but fails the runner's strict argv typing. [`k16-green-02`](../../experiments/exec-reliability/evidence/k16-green-02/) passes all four checks after that annotation repair.
- [`k16-final-01`](../../experiments/exec-reliability/evidence/k16-final-01/) passes all four checks with explicit root Python lint subjects, so archived audit scripts do not silently become new checker inputs. The missing-OUT guard initially failed during template expansion; the Taskfile now gives its intended actionable precondition message. [`k16-final-02`](../../experiments/exec-reliability/evidence/k16-final-02/) is the delivered condition after that Taskfile repair: Ruff lint/format, strict Pyright and **24/24 tests** pass. Its [verdict](../../experiments/exec-reliability/evidence/k16-final-02/verdict.json) records every checker exit, before/after subject hashes and each retained artifact digest. Its [JUnit record](../../experiments/exec-reliability/evidence/k16-final-02/pytest.xml) maps each test to its verdict. Forced command failures remain failed raw outcomes in the run receipts.

The new behavioral controls cover same-resource observation after an expected
interruption, stopping on unexpected interruption, rejecting expected resource
deadlines, four reserved-name collisions, continued disposal of two resources
after a receipt exception, missing-subject acquisition accounting, wrong stdout
with exit 0, and preflight subject preservation. Those k16 controls made no claim
of PID-versus-group behavioral separation; k20 supplies that control below.
The self-check runner freezes 30 seconds
each for lint/format/types and 60 seconds for pytest, with at most 2 seconds TERM
and 2 seconds KILL/reap on checker timeout. That wrapper does not contain child
sessions created by the controller; its timeout means uncertain local cleanup.
The retained passing final suite did not exercise the wrapper timeout path.
No guest, boot, slot, export or production comparison ceiling was raised.

The [first local audit](../../experiments/exec-reliability/evidence/k16-audit-01/result.json)
checks historical and interim integration samples. The second audit adds the
first final suite. The
[final local audit](../../experiments/exec-reliability/evidence/k16-audit-03/result.json)
adds the delivered suite: all 54 declared fake-resource entries are checked, with 18
original uncertain ledger entries preserved, including all six from the two
historical conditions. It removes the final owned blocked-disposal marker and
checks every exact marker path absent. The first audit had already removed two
interim markers; the second audit removed its own remaining marker. Earlier pytest paths are absent; their old remedy argv is
obsolete, not evidence that disposal succeeded originally. For 229 recorded
leader/known-child PIDs, the bounded `ps` returns exit 1 with **both** stdout and
stderr empty. No process is signaled. The audit script and every input file are
digested before/after with equality. These checks establish local absence at
audit time, not process-history provenance or live VM disposal. No local fake
marker or recorded process remains observed; the venv and retained evidence
are intentional artifacts.

The delivered controller SHA-256 is
`18158d76db1f4b083da769598592742fe4c0b33183d9c621fe829b90ea8afb8b`;
the final suite verdict SHA-256 is
`335ca0bec8f58c80103708c3a7c45c47d48378238acb7b291dfc125cbc8a768a`;
the final audit result SHA-256 is
`bdb35407a7773408e35c965ea2055d3e6af566b1c9b69354d6e1f5bfbea7f569`.
The source parent for this integration is `4bb47c05`; its own task commit pins
the delivered diff. These identities describe separate measured conditions.

## Forced-failure evidence

### Termination evidence (k20)

Run new conditions through the same `diagnostic:self-check OUT=...` recipe.
This producer starts from commit `16d7e86084b79ce2b5daf1566bbbd790b69647af`.
The delivered commit carries the dirty source changes; each condition pins its
actual Python sources, Taskfile, manifest, lockfile and interpreter. Controller
receipts additionally hash `/bin/ps`; the last condition explicitly declares
the fork fixture's `/bin/sleep` executable. Python is 3.12.15. There are no guest,
agent, image or app identities for these local-only conditions.

| Condition | Retained checker outcome |
| --- | --- |
| [`k20-red-01`](../../experiments/exec-reliability/evidence/k20-red-01/) | Old controller: 2 failures, 16 passes, 13 deselected. Graceful PID exit still receives KILL; the reentrant fork handler exits -9. |
| [`k20-green-01`](../../experiments/exec-reliability/evidence/k20-green-01/) | 31 tests and strict types pass; test-source line length fails lint. |
| [`k20-green-02`](../../experiments/exec-reliability/evidence/k20-green-02/) | 36/37 tests pass. A group poll launched with 0.7 ms left causes a retained inspection timeout. |
| [`k20-green-03`](../../experiments/exec-reliability/evidence/k20-green-03/) | All four checkers and 39 tests pass after reserving inspection time within each phase. |
| [`k20-final-01`](../../experiments/exec-reliability/evidence/k20-final-01/) | All four checkers and 39 tests pass; nested controller source is now retained in peer.py, and checker/runner PIDs are recorded. |
| [`k20-final-02`](../../experiments/exec-reliability/evidence/k20-final-02/) | All four checkers and 39 tests pass with the capped combined TERM/KILL/reap deadline and measured-bound check. |
| [`k20-final-03`](../../experiments/exec-reliability/evidence/k20-final-03/) | All four checkers and 39 tests pass, additionally declaring `/bin/sleep`; its verdict and JUnit record pin exact source hashes and itemized checks. |

The tests compare opposite real outcomes: PID-only interruption leaves the
known child live with this run's unique release-marker argv and recorded PGID;
group interruption removes it. Swapping the actual signal primitives makes
the same scope checker return false. Dropping KILL leaves a positively observed
child after leader exit and makes `termination_ok` false. The k20 scope and
stubborn fixtures release and check their known child independently; its fork
fixture lacked failure-path cleanup, corrected in k23 below. A separate outer controller observes
the nested controller's continuation output, successful exit, inner result and
later call, replacing the old tautological assertion. k20's guard was only
called directly after a successful run; k23 adds actual early-reap mutations.

The permission controls distinguish a forced live TERM denial from a forced
zombie-only return race; the latter sends the real TERM first and waits for
unreaped leader exit. This is an instrumented classification control, not a
claim that a real permission denial was repaired. Usage, timeout and foreign
group tables stay unknown despite a reaped leader and matching numeric exit.
Ordinary process checks are green; these local fixtures establish neither a
production lifecycle repair nor the ten-clone acceptance result.

New itemized local audits preserve all original receipts:
[`k20-audit-01`](../../experiments/exec-reliability/evidence/k20-audit-01/result.json)
checks the first five conditions, and
[`k20-audit-02`](../../experiments/exec-reliability/evidence/k20-audit-02/result.json)
adds final-02. Each audit pins every retained input before/after, checks exact
fake-marker paths, records known child PIDs, removes only known pytest fixture
outputs and checks the resulting scratch-directory absence. The process oracle
requires exit 1 and empty stdout **and** stderr; live-process and usage-error
controls used a different argv shape and did not exercise the shared predicate;
k23 supplies that control. The final
[`k20-audit-03`](../../experiments/exec-reliability/evidence/k20-audit-03/result.json)
includes final-03: 414 recorded workload/checker/member PIDs, 49 known-child
references and 52 fake-resource entries were checked absent at audit completion.
Its scratch array contains 97 real directory paths and 18 joined command lines;
the earlier totals of 115 exact directories are withdrawn. Each real directory
also had its own entry. The earlier audit arrays likewise contain 67 real paths
plus 12 joined commands, and 82 real paths plus 15 joined commands. This is a later absence observation, not a rewrite
of any original uncertain receipt. No process was signaled by these audits.
The inspection commands themselves return through `subprocess.run`, which
waits for their direct child; snapshots do not enumerate arbitrary descendants.
The venv and evidence directories remain intentional workspace artifacts.

### Termination review integration (k23)

Review k22 is commit `0ee42bc4`, against k20 at `6b9766f6`. Integration
starts at that review commit and preserves every earlier condition. All nine
findings have these dispositions:

| Finding | Disposition and evidence |
| --- | --- |
| R1, live group accepted after expected leader exit | Real issue, repaired. Normal exit and PID-only interruption with a live child both fail and stop later cases, despite matching reason and numeric exit. The call retains residual members, PGID and a revalidation/cleanup remedy. Scope remains PID-only. |
| R2, sampling reserve shorter than measured inspection | Real issue, externalized to bounded-inspection design and reviewed implementation ahead of k21. The current conservative unknown/failure behavior stays visible. No fixed reserve or clean rerun establishes an inspection-time guarantee. |
| R3, `ps` timeout can wait without bound | Real architecture issue, externalized with R2. Creation, setuid signal permission and post-KILL wait must be independently supervised. The old immediate-raise timeout stub proves classification only. k18 still owns storage isolation. Neither open property is claimed repaired. |
| R4, joined command lines credited as exact paths | Real audit/prose issue, repaired in new audits. Structured argv supplies paths; joined commands are rejected. The exact absence helper rejects live, mixed and invalid-selector responses, and accepts a freshly reaped helper's missing PID. Historical arrays are classified above without rewriting them. |
| R5, pinning guard never challenged by real early reap | Real control gap, repaired. `poll`, `Popen.wait`, `waitpid` and `wait` each actually reap the owned leader before the guarded snapshot. Every mutated execution is rejected before a group operation reaches the OS; the unchanged path succeeds. This is a test guard, not production protection against arbitrary future reaping. |
| R6, reap bound has no isolating control | Real control gap, repaired. A delayed reap with successful termination and exit 0 independently fails the measured bound. Removing its success gate exposes false success. A separate late observation detects removal of the shared deadline cap. |
| R7, fork child lacks failure-path cleanup | Real fixture issue, repaired. The cooperative finite child has a unique release marker; its PID is published independently before stdout. Both fork tests release in `finally` and positively inspect the known child. A direct fixture control discards stdout and still cleans the published child. |
| R8, controller survival has no failing control | Real control gap, repaired. A deliberate inner-controller self-signal loses the outer controller's observed survival proof. The control first ends its known inner child and checks absence afterward. The unchanged inner controller passes. |
| R9, exhausted checker wait loses verdict | Real issue, repaired. A second KILL/reap timeout records null exit, `reaped: false`, exact checker PID/PGID and a remedy, stops later checkers, and writes a failed verdict. Its deterministic unreapable-wait fixture is simulated, not an actual unkillable process. |

The frozen conditions use the existing Taskfile commands and Python 3.12.15:

| Condition | Outcome |
| --- | --- |
| [k23-red-01](../../experiments/exec-reliability/evidence/k23-red-01/verdict.json) | Both live-group cases falsely succeed; checker timeout raises without verdict. Three failures, 26 passes, 13 deselected. |
| [k23-green-01](../../experiments/exec-reliability/evidence/k23-green-01/verdict.json) | All 42 tests pass; lint/format/type failures are retained. |
| [k23-red-02](../../experiments/exec-reliability/evidence/k23-red-02/verdict.json) | Old fork fixture fails independent child publication; 29 passes, one failure, 13 deselected. |
| [k23-green-02](../../experiments/exec-reliability/evidence/k23-green-02/verdict.json) | All four checkers and 54 tests pass, including early-reap, bound and self-signal mutations. |
| [k23-final-01](../../experiments/exec-reliability/evidence/k23-final-01/verdict.json) | Historical delivered source condition: all four checkers and 54 tests pass; failed checker fixture artifacts are now retained with other run artifacts. |

Each condition pins its actual source, manifest, lockfile, Taskfile, interpreter
and per-artifact digests. The final [JUnit record](../../experiments/exec-reliability/evidence/k23-final-01/pytest.xml)
gives item-by-item verdicts. These are instrument controls; intentionally failed
raw outcomes remain failed. No case is a guest or production repair measurement.

The [first new audit](../../experiments/exec-reliability/evidence/k23-audit-01/failure.json)
failed before retaining its control responses. Its failure summary explicitly
states that limitation. [Audit 02](../../experiments/exec-reliability/evidence/k23-audit-02/controls.json)
retains the actual responses: the assumed missing PID is outside macOS's valid
range. [Audit 03](../../experiments/exec-reliability/evidence/k23-audit-03/result.json)
uses a freshly reaped helper and passes the oracle controls, but conservatively
fails its overall result on a reused historical PID and three preflight scratch
directories. Its raw process table shows an unrelated `eza` process; the original
receipt records successful reap of that old owned leader. No unrelated process
was signalled. The scratch checker artifacts it found are retained by digest
before removal, rather than silently discarded.

The [final audit 04](../../experiments/exec-reliability/evidence/k23-audit-04/result.json)
passes, with matching before/after hashes for all declared inputs, 757 recorded
real PIDs absent at its observation, 97 known-child references, 84 fake-resource
entries and 176 exact scratch paths. It archives the remaining available
preflight scratch outputs before cleanup. Earlier removed paths stay itemized
in audit 03; the later observation does not rewrite that failed audit. The
simulated unreapable checker's impossible PID is explicitly excluded from real
process claims. Audit commands themselves are direct children waited by the
auditor. Evidence directories and the venv remain intentional retained artifacts.

Red R7 deliberately discarded stdout from the old fixture, which had no
independent child publication. Its child PID is unavailable. The retained
parent/group identity has a later empty group observation; that is not a
reconstruction of the missing child receipt or proof about escaped descendants.
These audits remain conditional on `ps` returning. They establish no live VM,
image, archive or app ownership, and the historical 0/10 result is unchanged.

Run a new frozen auditor with:

```sh
mise exec -- task diagnostic:resource-audit AUDITOR=<new-frozen-audit-script>
```

The scripts name their finite inputs and preserve previous audit attempts.
Do not rerun one into its existing directory. k21 must preserve the new
live-group failure/residual contract while adding its surviving-writer and
stream/result-identity controls. Inspection supervision must land and be
reviewed before the remaining k17 contract can close or causal work can use it.

The final [integrity verification](../../experiments/exec-reliability/evidence/k23-verification-01/result.json)
recomputes each retained artifact and audit input, checks the delivered source
against final-01, and passes 15,049 item checks over 3,466 distinct read subjects.
It checks equal before/after hashes; it does not establish immutable publication
or resolve the inspection/storage limitations.

### Bounded inspection design

The [inspection spec](../specs/diagnostic-process-inspection.md) selects one
prestarted worker for helper creation, exec, capture and wait. The outer
controller remains the sole workload signal/reap owner. Worker results cross
bounded nonblocking channels and are accepted only in their original observation
epoch. A missed observation deadline fails the evidence while safety actions continue.
Review k26 is reconciled by integration k27. Planner k25 assigns implementation
to bounded-inspection-k28 before k21. k28 now contains the measured initial
admission slice k29 and the remaining complete worker/scheduler slice k30.
k32-final-01 is the current measured admission condition after k29's specific
review and integration; the inspection limitation measured at k23-final-01
remains unchanged. The historical acceptance result is
still **0/10, unavailable**.

- [Ownership](../specs/diagnostic-process-inspection.md#ownership) — Assigned target signal authority to the controller and helper mechanics to the worker, with separate pinned groups.
- [Deadlines and safety ordering](../specs/diagnostic-process-inspection.md#deadlines-and-safety-ordering) — One scheduler serves all waits and timers. Group grace has no query. KILL at T precedes the sole final query. PID-only KILL remains conditional.
- [Test seams and required controls](../specs/diagnostic-process-inspection.md#test-seams-and-required-controls) — Frozen bootstrap adapters cover inherited state, creation races, real helper KILL/wait and independent cleanup. Setuid ps and genuine EPERM have separate evidence prerequisites.
- [Visual document](../design/diagnostic-process-inspection/README.md) — Editable ownership, deadline and helper-publication views express the reconciled contract.

The source evidence is `_group_snapshot`, `_terminate`, `_call` and `execute`
in [controller.py](../../experiments/exec-reliability/controller.py) at parent
commit `44dc137e`. The synchronous `subprocess.run` is inside the same call
path that must return before `execute` enters teardown. The worker design
removes that inspection dependency; it does not fix workload command spawn or
evidence filesystem stalls. Python's own
[subprocess documentation](https://docs.python.org/3.12/library/subprocess.html#subprocess.run)
confirms that `run` waits for the killed helper and that creation can exceed
the timeout. Apple's
[kill documentation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html)
defines signal 0 as an error-checking probe and documents group EPERM; neither
fact makes EPERM evidence of absence.

Tier 2 graph discovery and coverage used project
`Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`, generation
`2026-11-01T05:03:14Z`. The four diagnostic source paths, Taskfile, glossary,
cited routing/transfer ADRs and this report had matching coverage metadata with
no recorded gap. Targeted search/trace pages were complete; the trace supplied
spurious cross-language callees, so only exact source supports the dependency
claim. No exhaustive graph claim or new runtime measurement is made.

The design requires no new ADR: the local diagnostic ownership choice is
reversible. Its durable spec is the shared contract for retained-file evidence
and the separately owned stalled-storage design. A bounded planning successor
must cut the concrete implementation and its review before retained-evidence-k21;
the reconciled design supplies that planner. k17 cannot close until those
deliverables and k21 account for the whole node contract.

#### Design review integration (k27)

Review commit `12d0bdba` contains seven findings against producer `53589d41`.
The integration accepts each finding, with the distinctions below. It changes
the design and its consumers, not the delivered controller or retained samples.

| Finding | Disposition and repair |
| --- | --- |
| F1, query near grace expiry retires a healthy worker | Real issue. Select the review's first alternative: no pre-escalation query, unconditional group KILL at T, then one final query. Accept full grace and a redundant KILL attempt on zombies. Preserve conditional PID-only KILL. |
| F2, inherited fork state | Unclear contract. Require checked thread/SIGCHLD preconditions, child signal/wakeup/descriptor reset, fresh ownership state and `os._exit`. Add host-state and worker-TERM controls. Unsupported embedding environments fail preparation. |
| F3, per-helper ACK | Real trade-off. Remove the ACK. Publication before gate release and helper expiry checks remain. Controller-crash recovery is excluded, so recording before exec supplies no required guarantee. Bootstrap readiness still has an ACK. |
| F4, controls exceed the stated adapter | Unclear contract. Freeze hold points and helper identity at bootstrap. Separate deterministic helper KILL from the actual setuid-ps path. A privileged owner must supply and clean the genuine UID-based EPERM fixture. Unavailable prerequisites stay itemised. |
| F5, undefined deadline terms and outcomes | Unclear contract. Define K for group, PID-only and normal exit. State zombie-only EPERM's non-failing consequence, final-query limits, fork-failure taxonomy and declared bootstrap/close bounds. |
| F6, concurrent timers without a scheduler | Unclear contract with implementation consequences. One scheduler must serve all wait phases and worker timers. k25 explicitly owns that restructuring and migration of subprocess-run stubs to adapter value cases. |
| F7, status and work orders in the spec | Real artifact issue. Keep delivery status here and migration orders in k25. The spec, README and manifest describe the contract. |

The single final query retains two limits: late observed leader exit can leave
insufficient inspection time, and members still exiting can yield a live table.
Both fail the call, even if the processes later exit. These controls remain
required. No retry, added allowance or later valid observation repairs a failure.

The fork reset follows Python's documented
[signal handling](https://docs.python.org/3.12/library/signal.html) and
[child exit](https://docs.python.org/3.12/library/os.html#os._exit) behaviour.
`os._exit` avoids cleanup handlers and stdio flushing. The worker must also
avoid inherited host callbacks before exit. Apple's
[signal permission rule](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html)
allows a matching real or effective UID. A setuid-root ps retains the caller's
real UID, so setuid status alone does not produce EPERM. Actual setuid-ps KILL
can lose its timing window. That result stays unavailable rather than becoming
proof from the deterministic substitute helper.

Integration source checks used Tier 2 graph generation `2026-11-01T05:35:55Z`
and direct reads of `_wait`, `_terminate`, `_call` and the existing test stubs.
Targeted graph pages were complete. Coverage had no recorded source gap, but
reported Mermaid gaps in ownership lines 1–13 and helper-start line 16.
Direct reads covered those ranges. This establishes the design repair's basis,
not a runtime reliability result. k18 still owns blocked evidence storage.

#### Implementation continuation (k25)

The concrete continuation is **bounded-inspection-k28**, one implementation
increment before retained-evidence-k21. Its observable result must be the real
`execute` path attempting owned-resource safety actions on unchanged deadlines
while inspection stalls, with failed observations and residual identities
retained. Scheduling, worker/helper ownership, bounded messages and epoch
validation belong together: an isolated worker or scheduler does not establish
that result at the caller. The independently useful next increments remain
k21's retained-file integrity and k18's storage isolation.

k28 owns the controller, narrow inspection code, local fixtures/tests, explicit
runner/hash/type-check subjects, concrete Taskfile recipes and this handoff.
Its controls include inherited host state, creation/gate races, actual helper
KILL followed by a wait stall, invalid handoff, signal/reap authority, the sole
final query's timing limits and worker cleanup deadlines during an open
teardown command. Real setuid-ps KILL and UID-based EPERM stay separate from
deterministic helper and classifier controls. Their missing prerequisites
produce itemized unavailable receipts, never a full guarantee claim. No
privilege acquisition is authorised by this continuation.

Once its artifact exists, k28 must insert its specific implementation review
before k21. Review traces actual creation, drains, waits and shutdown hooks,
including safety-owner blocking and released identities. Actionable findings
reconcile before k21; a runtime counterexample requiring redesign earns a
precise design continuation before its consumer. No empty integration is cut.
k17 remains open through reviewed implementation and k21, with every missing
real control visible when its whole contract is checked. This planning change
runs no diagnostic workload and changes no earlier receipt or runtime source.

#### Initial fork admission (k29)

The first independently useful child of k28 rejected an unsafe initial controller
environment through the real `execute` boundary. It records Python and native
thread counts, SIGCHLD disposition, and native-observation error/errno in
`result.json`. Rejection happens before the resource intent ledger: acquisition
and workload stay unreached, with no calls or resource ownership reported.
A default single-threaded controller still acquires, runs, cleans, disposes and
positively checks absence. The controller includes `inspection.py` in its
before/after subject hashes.

Native thread observation uses macOS libproc `PROC_PIDTASKINFO`. The ctypes ABI
uses the [XNU task-info structure and flavor](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h):
six 64-bit fields then twelve 32-bit fields, including `pti_threadnum`.
Unavailable libraries/symbols, a return length other than 96 bytes, and
nonpositive thread counts all fail closed. System dynamic libraries are not
automatically hashed; the retained observations describe this host condition.

The frozen [k29-final-01 verdict](../../experiments/exec-reliability/evidence/k29-final-01/verdict.json)
records zero exits for lint, formatting, strict runtime types and pytest.
All 67 tests passed in 18.81 seconds under Python 3.12.15. The
[JUnit record](../../experiments/exec-reliability/evidence/k29-final-01/pytest.xml)
identifies each case, including these 13 admission controls:

| Control | Observed boundary |
| --- | --- |
| Default single-threaded controller | All five resource/workload calls run; resource disposed. |
| Live Python thread | Failed `python_threads` receipt, Python/native counts 2/2. |
| Real native pthread absent from Python registry | Failed `native_threads` receipt, counts 1/2; fixture joins its pthread. |
| Python-installed SIG_IGN and SIGCHLD handler | Separate failed `sigchld` receipts with no acquisition. |
| Native returns 0, -1, 95 or 97 bytes | Four failed native-observation receipts. |
| Native returns 96 bytes with count 0 or -1 | Two failed native-observation receipts. |
| Native library unavailable or query symbol missing | Separate retained ENOENT and ENOSYS failures. |

All twelve rejection results remain failed, with no calls/resources, acquisition
and workload unreached, and unchanged declared subject hashes. Native
short/error/library cases use explicit adapters; the Python thread, pthread and
signal controls exercise real host state. Development runs exposed the old
boundary and the missing-symbol error before their fixes; those transient red
runs are not a separate frozen countercondition. No fixture is a guest or a
production reliability measurement.

The independent [audit 02 result](../../experiments/exec-reliability/evidence/k29-audit-02/result.json)
retains before/after hashes of the condition, auditor, runtime/test sources,
manifest, lockfile, Taskfile, interpreter, ps and true. All 94 recorded PIDs,
including independently published fork children and checker identities, were
absent in the later observation. Its exact absence predicate rejected live,
mixed and invalid selectors and accepted a freshly reaped helper. It checked
21 marker resource declarations and removed 67 exact fixture directories;
the deliberately blocked-disposal marker was present and was removed after
identity/content validation. Preflight scratch evidence was copied before
removal. The simulated unreapable-checker PID is explicitly excluded. These are
later observations, without an escaped-descendant or inspection-time proof.
[Audit 01](../../experiments/exec-reliability/evidence/k29-audit-01/failure.json)
failed on a readiness marker mistaken for a PID, before any process controls
or cleanup. Its failure and original script remain unchanged.

This admission snapshot does not lock out a new host thread or changed signal
state. No inspection worker, nonblocking channel, helper ownership protocol or
single deadline scheduler is delivered. The old synchronous ps creation and
post-timeout wait can still block resource safety. The unintegrated worker draft
was replaced rather than shipped; jj snapshot `0f8aca80` preserves it for
reference, without any correctness claim. bounded-inspection-k30 retains the
full parent spec/control inventory and must repeat admission before every actual
fork, implement signal masking/reset and worker/helper lifecycle, and demonstrate
resource safety during inspection stalls. Its specific review must still precede
k21. Review k31 and its integration k32 complete the initial admission slice
before k30; k17, k21 and k18 remain open, and historical acceptance remains
**0/10, unavailable**.

#### Native SIGCHLD review integration (k32)

Review k31 identified that Python's `signal.getsignal` reports a cached table:
a native SIGCHLD ignore or handler can leave Python reporting SIG_DFL. The
independent [red-01 run](../../experiments/exec-reliability/evidence/k32-red-01/pytest.xml)
reproduced both admissions before the repair (3 failed, 53 passed). Native
controls use real libc `sigaction` calls and restore the complete previous
action in `finally`. The handler is a real native callback. The
[red-02 run](../../experiments/exec-reliability/evidence/k32-red-02/pytest.xml)
also exposed the absent native-query failure handling (8 failed, 52 passed).
Both runs retain matching before/after hashes and source snapshots in
`k32-red-inputs-01` and `k32-red-inputs-02`.

Admission now queries the kernel through read-only Darwin `sigaction`, using
the [public structure](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/signal.h)
(handler pointer, 32-bit mask, integer flags). The Python-visible check remains;
both dispositions must be SIG_DFL. Receipts distinguish `sigchld` and
`native_sigchld`. An unavailable library or symbol, a failed query, or an
unexpected nonzero return fails closed before acquisition with retained errno.
This query creates no subprocess and changes no signal action.

The frozen [k32-final-01 verdict](../../experiments/exec-reliability/evidence/k32-final-01/verdict.json)
records zero exits for lint, formatting, strict runtime types and all 73 tests
(18.93 seconds). The [per-case record](../../experiments/exec-reliability/evidence/k32-final-01/pytest.xml)
confirms each failing red case now passes. The six additional admission controls
cover native ignore, native handler, missing library, missing symbol, query errno
and unexpected return. Preflight now declares both inspection sources, and
`diagnostic:format` formats only top-level Python files so frozen evidence scripts
and source snapshots stay unchanged. The parent briefs identify this partial
admission slice separately from the remaining worker/scheduler implementation.

The independent [k32-audit-02 result](../../experiments/exec-reliability/evidence/k32-audit-02/result.json)
checks all 1,294 indexed artifacts across both red conditions and the final
condition, their input snapshots, and the final delivered source hashes.
All 1,336 audit inputs have equal before/after hashes. It enumerates every
recorded `pid`, `runner_pid`, `child` and `pgid`, plus numeric stdout and ready
markers; only the declared simulated checker PID is excluded. All 264 distinct
identities were absent in the later observation, with live, mixed, invalid and
freshly reaped helper controls for the absence predicate. It checked 69 resource
declarations, removed 133 exact fixture directories after identity/content
validation, and recorded 27 already absent. Preflight scratch evidence was
copied before removal. [Audit 01](../../experiments/exec-reliability/evidence/k32-audit-01/failure.json)
stopped before process controls or cleanup when pytest had already removed an
older session directory; its script, failure and unchanged input hashes remain.
All earlier evidence, including k29, is unchanged.

These are later identity observations, without proof against escaped descendants
or inspection-time races. The native query is an admission snapshot; k30 still
must check before every fork and implement the complete worker/scheduler boundary.
Synchronous ps and stalled-storage limits remain unchanged. No installed CLI,
app or VM workload was run, and historical acceptance remains **0/10, unavailable**.

### Group KILL ordering slice (k33)

The k30 implementation was decomposed into an independently useful ordering
repair and the remaining worker/scheduler boundary. This slice changes the real
controller's termination path; it does not deliver supervised helper ownership,
bounded protocol handling or shutdown concurrent with teardown. Those obligations
remain on k30 and its next producer, bounded-inspection-k34.

The [red condition](../../experiments/exec-reliability/evidence/k33-red-01/verdict.json)
retains 57 passing and five failing review controls against the original
controller. Four failures demonstrate the behavior being repaired: group KILL
was skipped after graceful exit, a slow real `ps` operation preceded KILL for a
TERM-ignoring process, and two earlier TERM-denial receipts were retrospectively
classified from later tables. The fifth failure is a migrated mutation seam
absent from the original source, so it is not behavioral disproof.
The exact eleven red inputs are retained in
[k33-red-inputs-01](../../experiments/exec-reliability/evidence/k33-red-inputs-01/manifest.json).

The [intermediate full run](../../experiments/exec-reliability/evidence/k33-final-01/verdict.json)
passed 74 tests and failed one platform expectation: macOS denied KILL to the
already zombie-only group. Its
[input snapshot](../../experiments/exec-reliability/evidence/k33-intermediate-inputs-01/manifest.json)
is separate. The corrected expectation accepts either sent KILL or final
`zombie_only_eperm`, while requiring the earlier `permission_unknown` and failed
overall result. The [delivered self-check](../../experiments/exec-reliability/evidence/k33-final-02/verdict.json)
passes Ruff lint/format, strict Pyright and **75/75 tests** in 20.94 seconds,
with all eleven input hashes equal before/after. Its
[JUnit record](../../experiments/exec-reliability/evidence/k33-final-02/pytest.xml)
retains the individual outcomes.

The ordering controls use real processes, signals and process-table output;
their adapter delays only a query incorrectly placed before KILL. Both graceful
and TERM-ignoring cases attempt KILL before their first query, 0.66 ms and
1.84 ms after T in this run. Existing real-child, early-reap, PID-only scope and
deadline-overrun mutation controls also pass. These measured timings do not
establish a hard scheduling bound. Denied TERM controls inject EPERM; they are
not a genuine privileged-owner control.

The [first audit](../../experiments/exec-reliability/evidence/k33-audit-01/failure.json)
stopped before scratch cleanup because recorded PID 23889 was present as a
macOS metadata worker. The original acquire receipt records that diagnostic
leader reaped, but the audit conservatively made no reuse inference and sent
no signal. A separate later observation found that PID absent. The
[second audit](../../experiments/exec-reliability/evidence/k33-audit-02/result.json)
retains the failed audit as an input, checks all 1,345 indexed artifacts from
the three conditions and both input snapshots, and records equal before/after
hashes for all 1,393 audit inputs. All 257 recorded PIDs are absent in that later
observation. Live, mixed, invalid-selector and freshly reaped controls contest
the absence predicate. The audit checks 77 declared fake resources, removes two
remaining owned markers and 212 fixture directories after path/content
validation, and preserves preflight scratch bytes before removal. Of those
directories, 107 came from retained argv paths and 105 from session-wide
`test_*` enumeration in pytest-81, pytest-82 and pytest-83. The latter were
validated before deletion, but the receipts do not establish exact attribution
to the three retained conditions. This clarifies the existing audit's scope;
its code and evidence have not been rerun or changed.

The sticky TERM EPERM is an accepted fail-safe trade-off. If the group has
already become zombie-only when TERM is denied, the call can still fail:
a later zombie-only table cannot explain the earlier permission denial. That
possible false failure remains visible rather than being reclassified as
successful disposal. No earlier inspection is added before KILL; genuine
permission evidence and the complete worker/scheduler contract remain with k34.

These are later PID observations, without escaped-descendant or process-history
proof. Post-KILL `ps` capture and its kill/wait path remain synchronous and can
block disposal; repeated snapshots and blocking leader reap also remain for
k34 to replace. No live guest, app, installed CLI, privilege acquisition or
kernel-uninterruptible workload was used. Acceptance remains **0/10, unavailable**.

The delivered controller SHA-256 is
`1fc6e51b494ca48724fb2ba840f646d627e2e821cf00e4ee0ba81d825599a0e8`;
the final verdict SHA-256 is
`2fa68b7279e359da0424ce40938bae626a07d9b66aaee57f8ff2ff186fe9f1cb`;
the final audit result SHA-256 is
`c9b312c0af9f9b0333c5dda8bbbc05f5a6d5c2abb340cfe550f01a258e36ba1c`.

### k36 review integration: PID-only late observation

The k35 review identified a real coverage gap: the existing deadline mutation
exercised the group branch after k33, leaving the PID-only cap untested. The new
`test_review_pid_late_observation_cannot_extend_reap_deadline` interrupts a real
PID-only `hang` call, observes actual TERM delivery and delays the first later
exit observation by 0.2 seconds. It requires leader reap, PID-only signaling,
an observation after T and a reap deadline no later than T + 0.3 seconds.
Production code is unchanged.

The [frozen-input manifest](../../experiments/exec-reliability/evidence/k36-cap-inputs-01/manifest.json)
retains two repository-shaped input trees, equal except for replacing
`min(time.monotonic(), term_deadline) + plan.kill_wait` with
`time.monotonic() + plan.kill_wait` in the PID-only branch. The trees reuse this
checkout's existing virtual environment via explicit symlinks. Running their
copied Taskfiles' `diagnostic:review-controls` recipes gives **63 passed** in
[normal](../../experiments/exec-reliability/evidence/k36-cap-normal-01/verdict.json)
and **62 passed, 1 failed** in
[removed-cap](../../experiments/exec-reliability/evidence/k36-cap-removed-01/verdict.json).
Only the new PID test changes outcome: the removed cap chooses deadline
256944.37021325, exceeding T + A = 256944.312743041. Raw streams, JUnit results,
controller receipts and before/after input hashes are retained in both runs.

The [first full check](../../experiments/exec-reliability/evidence/k36-final-01/verdict.json)
passes lint, formatting, types and 75 tests; the existing resource-continuation
test fails when 0.2-second cleanup/disposal budgets expire and later `ps`
queries time out. That condition is preserved. An unchanged-input
[second full check](../../experiments/exec-reliability/evidence/k36-final-02/verdict.json),
run with `mise exec -- task diagnostic:self-check
OUT=experiments/exec-reliability/evidence/k36-final-02`, passes lint, formatting,
strict Pyright and **all 76 tests** in 27.17 seconds. The new PID test passes
in both full conditions; this rerun does not remove the conditional scheduling
limits assigned to k34.

The independent later
[k36 audit](../../experiments/exec-reliability/evidence/k36-audit-01/result.json)
verifies the frozen trees and sole mutation, all 1,742 indexed artifacts across
the four conditions, the control's outcome difference, and equal before/after
hashes for 1,782 audit inputs. All 332 recorded PIDs, including known-child
receipts, are absent at that observation; live, mixed, invalid-selector and
freshly reaped controls contest its oracle. It validates 100 resource
declarations, removes three remaining owned markers and 246 fixture directories,
and preserves preflight scratch bytes. The directory scope is explicitly
138 retained-argv paths plus 108 session-enumerated `test_*` paths; the latter
have path/content validation but no exact attribution claim. These are later
PID observations, with the same escaped-descendant and synchronous wait limits
as k33. The earlier k33 audit and receipts remain unchanged. The corrected k33
directory attribution above resolves finding 2 as an unclear scope claim;
the documented sticky TERM EPERM resolves finding 3 as an accepted trade-off.
Acceptance remains **0/10, unavailable**.

The controller SHA-256 remains
`1fc6e51b494ca48724fb2ba840f646d627e2e821cf00e4ee0ba81d825599a0e8`;
the k36 final verdict SHA-256 is
`c14f7fa1525730061e5f1a9884f9ed83b8554c7a9d086a2d17a27b7ad6d321a8`;
the k36 audit result SHA-256 is
`c94a0709024efd0c3b00c7fb7a36cd1ef13d49dd7617353c954cda31d6aa4d50`.

### Process-table acceptance slice (k37)

The actual controller's `_group_snapshot` now requires a clean helper exit,
empty stderr, complete rows, unique positive PIDs, matching PGIDs, supported
macOS process states and the pinned leader. It rejects more than 256 KiB of
combined raw stdout/stderr or 4096 rows, retaining the original bytes without
accepting a truncated prefix. Rejected tables cause no group probe. The
controller checks its deadline before the signal-zero probe and rejects a
result that becomes late during that probe. These are acceptance limits:
`subprocess.run` still captures and waits synchronously, so this change does
not bound helper creation, memory use, capture or disposal time.

The new 26 controls exercise the real observation function with helper output,
probe and clock adapters below that boundary: 19 rejection cases, five valid
cases (including exactly 256 KiB and 4096 rows), and two deadline cases.
The [frozen previous-controller replay](../../experiments/exec-reliability/evidence/k37-previous-01/verdict.json)
passes its lint, formatting and type checks, then fails exactly 16 of the new
controls; its other 86 tests pass. The
[input manifest](../../experiments/exec-reliability/evidence/k37-inputs-01/manifest.json)
pins that controller's jj revision and the copied current tests and supporting
sources. Only the controller differs between conditions.

The [delivered self-check](../../experiments/exec-reliability/evidence/k37-final-01/verdict.json),
run with `mise exec -- task diagnostic:self-check
OUT=experiments/exec-reliability/evidence/k37-final-01`, passes lint, formatting,
strict types and [all 102 tests](../../experiments/exec-reliability/evidence/k37-final-01/pytest.xml).
No checker timed out; all were reaped. The
[per-test comparison](../../experiments/exec-reliability/evidence/k37-audit-01/comparison.json)
checks identical case identities and every outcome across both conditions.
The source and artifact hashes match their frozen declarations.

The [independent later audit](../../experiments/exec-reliability/evidence/k37-audit-01/result.json)
observes all 194 retained real PIDs absent, excludes the explicitly simulated
unreapable PID, and accounts for 54 exact fixture resource markers. Two markers
were still present before audit cleanup; all are absent afterward. It accounts
for 152 scratch directories: 76 from exact retained argv paths and 76 from
enumerating those same pytest sessions. Live, mixed, invalid-selector and
already-reaped controls verify the absence predicate. This is a later
observation of recorded identities, not proof against escaped children or a
bound on the auditor's own helper creation and wait.

The worker, protocol, shared scheduler, shutdown and remaining real-controller
control inventory remain live in worker-scheduler-k38. The final full-subsystem
review remains required after those runtime changes. No guest acceptance or
privileged-process inspection claim follows from this slice.

### Nonblocking workload-leader reap slice (k39)

The actual controller now closes its workload signal/probe authority before
attempting exact-PID `os.waitpid(pid, os.WNOHANG)`. It attempts no reap once
the fixed K deadline has expired and performs no later repair wait. A private
`Popen` subclass suppresses destructor polling and subprocess cleanup
registration; a strong owner remains registered when reaping fails. Receipts
distinguish the residual leader from residual group members, retain wait errors
and errno, and fail the call. A late-returning reap also fails the existing
within-bounds gate.

Inspection reserves R=min(100 ms,A/4) inside the existing allowance, for normal
exit and termination paths alike. This leaves a reap window after an unsuccessful
table or exit observation without extending K. Synchronous inspection can still
overrun that cutoff; the controller then retains the leader rather than reaping
it late. Repeated synchronous queries and blocking helper creation/capture/waits
remain pending work.

The [previous-controller replay](../../experiments/exec-reliability/evidence/k39-previous-01/verdict.json)
fails all three new controls: withheld wait results, simulated ECHILD, and an
inspection return after expiry. Its raw failed outcomes remain unchanged.
The [input manifest](../../experiments/exec-reliability/evidence/k39-inputs-01/manifest.json)
pins the previous jj revision and both source snapshots; only controller.py
differs between them. Each control uses a real completed child and independently
reaps or checks that exact PID afterward. In the delivered condition all three
remain zombies through explicit destructor invocation and another subprocess
creation, then are independently reaped with exit zero and observed absent.
These are injected wait outcomes and timing holds, not genuine OS unreapability
or actual child-ownership loss.

The [delivered self-check](../../experiments/exec-reliability/evidence/k39-final-01/verdict.json)
passes Ruff lint/format, strict Pyright and
[105/105 tests](../../experiments/exec-reliability/evidence/k39-final-01/pytest.xml)
in 22.33 seconds for pytest. The pinning control forbids workload `Popen.wait`
and checks signals/observations finish before authority closure and reap. The
late-result mutation still demonstrates failure when the deadline gate is
present and false success when removed. Existing early-reap, real-child,
signal-scope, table-rejection and deadline-cap controls pass.
Existing checker/input declarations cover the changed top-level sources;
no runtime module was added.

The [independent audit](../../experiments/exec-reliability/evidence/k39-audit-01/result.json)
verifies 543 indexed replay/final artifacts and equal before/after hashes for
581 inputs. All 103 recorded real PIDs are absent in its later observation;
the explicitly simulated checker PID is excluded. It accounts for 27 declared
resource markers and removes one remaining owned marker while cleaning 38
fixture directories derived from exact retained argv paths. All checked paths
are absent afterward. Live, mixed, invalid-selector and freshly reaped controls
contest the absence predicate. This audit makes no session-wide scratch,
escaped-child, helper-supervision or hard scheduling claim.

The complete private worker, protocol, shared scheduler, shutdown/concurrent
teardown and remaining control inventory stay live on worker-scheduler-k40
under k38. The final full-subsystem review remains mandatory. This slice makes
no guest or privileged-process acceptance claim; historical acceptance remains
**0/10, unavailable**.

### Single final process observation slice (k41)

The actual controller now waits for observed leader exit before requesting its
sole final process table. On termination paths, the request also follows the
last signal attempt. Live, malformed or late evidence fails the call without
another query. Missing exit evidence or an exhausted inspection cutoff records
unknown without launching a helper or performing its signal-0 probe. Normal
completion uses the same cutoff gate.

A denied final group KILL uses that sole post-signal table for classification.
A later zombie-only table cannot repair an earlier denied TERM. Group KILL
still occurs at T, and PID-only KILL remains conditional on missing exit
evidence. The fixed reap allowance starts from the recorded exit observation
on normal and early PID-only completion. k39's authority closure, nonblocking
exact-PID reap and strong residual ownership remain in place.

The [previous-controller replay](../../experiments/exec-reliability/evidence/k41-previous-01/verdict.json)
fails all four new control cases, with 66 other controls passing. The
[input manifest](../../experiments/exec-reliability/evidence/k41-inputs-01/manifest.json)
pins the previous jj revision and both source snapshots. Only controller.py
differs, and both conditions use the same new tests. Group and PID-only controls
withhold exit observations while retaining real workload signals and independent
reaping. The live-table control releases its child after the first real table,
exposing false success when the previous controller retries. A separate injected KILL-EPERM control
requires the sole table to follow the completed signal attempt. These injected
observation and signal outcomes do not demonstrate genuine OS permission denial.

The [delivered self-check](../../experiments/exec-reliability/evidence/k41-final-01/verdict.json)
passes Ruff lint/format, strict Pyright and
[109/109 tests](../../experiments/exec-reliability/evidence/k41-final-01/pytest.xml).
Pytest took 23.04 seconds. Existing ordering, deadline-cap, parser, permission,
early-reap and residual-owner controls pass. Existing checker/input declarations
cover the changed sources, and no runtime module was added.

The [independent audit](../../experiments/exec-reliability/evidence/k41-audit-02/result.json)
verifies 967 indexed replay/final artifacts and equal before/after hashes for
1,004 inputs. All 182 recorded real PIDs are absent in its later observation.
It excludes the explicitly simulated checker PID and accounts for 50 declared
resource markers. One marker remained before cleanup. It accounts for 71
fixture directories derived from exact retained argv paths, with every checked
path absent afterward. Live, mixed, invalid-selector
and freshly reaped controls contest the absence predicate. The
[first audit attempt](../../experiments/exec-reliability/evidence/k41-audit-01/failure.txt)
stopped before process observations or cleanup because its source-snapshot
comparison incorrectly included the virtual-environment interpreter. The fresh
audit corrects that comparison and checks the interpreter through its input hashes.

This audit makes no session-wide scratch, escaped-child or helper-supervision
claim. Synchronous helper creation, capture and waits can still overrun the
cutoff. The private worker, protocol, shared scheduler, concurrent teardown,
shutdown and remaining controls stay live on worker-scheduler-k42 under k40.
The final full-subsystem review remains mandatory. No guest or privileged-process
acceptance follows from this slice. Historical acceptance remains
**0/10, unavailable**.

### Sticky inspection failure (k43)

A failed local observation previously allowed teardown to query again and mark
resource disposal certain. The controller now owns one inspection state per
`execute` invocation. Its first unknown observation retains the call name and
original observation in `result.json` as `inspection_failure` (narrowed to an
attempted query by the k46 refinement below). Later commands
receive an unknown observation with no local query or signal-zero probe. The
remaining workload stays unreached, while independent safety commands run in
reverse resource order and retain failed disposal certainty. A valid live table
remains a termination failure without disabling later local inspection. A fresh
execution starts with fresh inspection state.

The [first prior-controller replay](../../experiments/exec-reliability/evidence/k43-previous-01/verdict.json)
retains five failing controls and 70 passing controls. The
[expanded replay](../../experiments/exec-reliability/evidence/k43-previous-02/verdict.json)
retains seven failing controls and 70 passing controls. Five controls inject an
unknown observation after a real process-table query at acquisition, workload,
cleanup, disposal or public inspection. They check that later local queries stop,
actual disposal removes its marker, certainty remains failed, and a fresh run
recovers. The two additional controls check all six reverse safety actions over
two resources, including a real timed-out cleanup with TERM/KILL and bounded
reaping, and the distinction between a valid live classification and inspector
failure. These adapters alter returned classification or raw evidence; they do
not demonstrate an actual malformed system `ps` response. Existing parser and
live-child controls remain separate evidence.

The [frozen input comparison](../../experiments/exec-reliability/evidence/k43-inputs-02/manifest.json)
records the same seven controls against prior and current source; only
`controller.py` differs between those snapshots. The
[delivered self-check](../../experiments/exec-reliability/evidence/k43-final-01/verdict.json)
passes Ruff lint/format, strict Pyright and
[116/116 tests](../../experiments/exec-reliability/evidence/k43-final-01/pytest.xml).
Every checker was reaped, no checker timed out, and read-subject hashes stayed
equal. The [per-test comparison](../../experiments/exec-reliability/evidence/k43-audit-01/comparison.json)
confirms each expected prior failure passes in the delivered condition.

The [independent audit](../../experiments/exec-reliability/evidence/k43-audit-01/result.json)
checks 518, 568 and 728 indexed artifacts for the first replay, expanded replay
and delivered condition, respectively, with equal before/after input hashes.
All 362 recorded real PIDs are absent in its later exact-PID observation; one
explicitly simulated checker PID is excluded. Live, mixed, invalid-selector and
freshly reaped controls contest the absence predicate. It accounts for 94
resource markers, with one present before cleanup, and 122 scratch directories
derived from retained argv paths, with 37 already absent. Every checked marker
and scratch directory is absent afterward; owned leftover files are recorded
before allowlisted cleanup. Prior samples remain unchanged.

These observations do not prove absence of escaped descendants or account for
all session scratch. Helper creation, capture and waits remain synchronous and
can overrun the cutoff. The private worker, protocol, shared scheduler, concurrent
teardown and shutdown, privilege controls and complete ancestor control inventory
remain live in worker-scheduler-k44 under k42. A specific k43 review precedes that
work; the final full-subsystem review remains mandatory. No guest or privileged
acceptance follows from this slice. Historical acceptance remains
**0/10, unavailable**.

### Skipped queries remain distinct from inspection failure (k46)

Review k45 found that k43 also latched the synthetic unknown observations for
an unobserved leader exit or an exhausted final-query cutoff. Neither case
attempts inspection. Both must fail the affected call, while later independent
safety calls retain their opportunity to inspect.

`GroupObservation.query_attempted` is now false until the controller invokes
the process-table helper. The execution-owned latch accepts an unknown result
only when that field is true. Skipped and previously disabled observations keep
it false, retain their distinct error strings, and make no signal-zero probe.
The remaining workload still stops after the unobserved call. Later cleanup,
disposal and public inspection can establish disposal when their own process
observations succeed. A real query failure still retains its first receipt and
disables every later local query and probe within that execution.

The [pre-fix controls](../../experiments/exec-reliability/evidence/k46-red-01/verdict.json)
retain **4 failed, 76 passed**. Both skip-reason controls and both real withheld-exit
controls fail specifically because `inspection_failure` is populated for a
query that never ran. The new real-helper control passes in this condition:
an argv adapter invokes actual `/bin/ps -o` without its required format on the
workload's query. Its nonzero status and usage stderr flow through the actual
parser and latch. The subsequent cleanup, disposal and public inspection run
and reap, but issue no group table or signal-zero probe; resource certainty
stays failed despite removal of the marker. This is a deliberate real usage
failure, not evidence of a spontaneous system-table fault or permission denial.

The [delivered self-check](../../experiments/exec-reliability/evidence/k46-final-01/verdict.json)
passes Ruff lint/format, strict Pyright and
[119/119 tests](../../experiments/exec-reliability/evidence/k46-final-01/pytest.xml).
No checker timed out, every checker was reaped, and source hashes stayed equal
within each measured condition. The
[before](../../experiments/exec-reliability/evidence/k46-inputs-01/before.json) and
[after](../../experiments/exec-reliability/evidence/k46-inputs-01/after.json)
snapshots retain the exact sources; only `controller.py` and `test_controller.py`
differ. The latter difference adds two assertions of the new field to the
already-passing real-helper control. The four failing controls are unchanged.

The advisory k45 finding also corrects the interpretation of the k43 expanded
replay. Its valid-live control failed with `KeyError: 'inspection_failure'`,
not a later-inspection behavior difference. The retained prior audit shows all
four local queries ran and its result records the resource disposed. The
[new comparison](../../experiments/exec-reliability/evidence/k46-audit-01/comparison.json)
checks those original receipts. The control now checks query continuation
before using a schema-neutral lookup for the optional historical field. The
old evidence remains unchanged; its seventh failure is not a seventh behavioral
regression repaired by k43.

The [independent audit](../../experiments/exec-reliability/evidence/k46-audit-01/result.json)
verifies the frozen sources and indexed raw artifacts without changing the
measured conditions. All 300 recorded real PIDs are absent in its later exact-PID
observation; the explicitly simulated checker PID is excluded. Live, mixed,
invalid-selector and freshly reaped controls contest the absence predicate.
All 72 declared resource markers and 92 scratch directories derived from exact
retained argv paths are absent after allowlisted cleanup. The auditor's initial
lint check rejected one long line before any measurement or cleanup; that line
was wrapped before the successful audit.

These controls discharge k45's findings on this slice. Synchronous helper
creation, capture and waits remain conditionally bounded. The complete private
worker, shared scheduler, concurrent teardown, shutdown and fault inventory
remain live on worker-scheduler-k44; its final full-subsystem review is still
required. No guest or privileged-process acceptance follows. Historical
acceptance remains **0/10, unavailable**.

### Bounded post-creation helper capture (k47)

The actual group query now uses `inspection.capture_helper`. Every helper
creation gets a fresh Python/native thread and SIGCHLD admission check. Creation
still uses synchronous Popen in the controller. After creation, stdout and
stderr are nonblocking, with one read per ready stream per turn (at most 32 KiB)
and a 256 KiB combined retained-output cap. One excess detection byte fails the
query without being retained. Partial EOF is captured verbatim; the existing
strict process-table parser remains responsible for rejecting incomplete rows.

For caller cutoff D, capture stops at D minus min(50 ms, remaining allowance / 4).
That fixed reserve covers direct exact-PID KILL and WNOHANG reap attempts; there
is no fresh post-timeout wait allowance. Successful reap or any wait error
closes helper signal authority. An unreaped helper retains its Popen owner in
`_helper_owners`, with its PID, argv, admission, timestamps, errno and external
reconciliation remedy. Its destructor performs no poll or implicit wait. These
helper receipts remain distinct from workload-leader ownership. A failed
capture or admission makes inspection sticky for the current execute call;
cleanup, disposal and public absence commands still run without later local
query retries.

The [final retained self-check](../../experiments/exec-reliability/evidence/k47-final-01/verdict.json)
passes Ruff lint/format, strict Pyright and
[130 tests](../../experiments/exec-reliability/evidence/k47-final-01/pytest.xml)
in 26.41 seconds. The runner includes every top-level Python source, the locked
project configuration, Taskfile, interpreter and `/bin/ps` in before/after
hashes. The earlier [k47-check-01](../../experiments/exec-reliability/evidence/k47-check-01/verdict.json)
also passed all 130 tests and strict types, but failed lint on an overlong remedy
and import ordering. It is retained as that failed condition; it was not replaced.

| Real helper control | Final capture elapsed | Retained bytes | Exit |
| --- | ---: | ---: | ---: |
| Healthy stdout and stderr | 0.015942 s | 11 | 0 |
| Partial output then EOF | 0.014994 s | 7 | 0 |
| Ten-second hang, bounded KILL/reap | 0.456604 s | 0 | -9 |
| Unbounded stdout, bounded KILL/reap | 0.027784 s | 262144 | -9 |
| Unbounded stderr, bounded KILL/reap | 0.020463 s | 262144 | -9 |

All five controls used an unchanged 500 ms cutoff. Additional real execute
controls inject a hang, overflow on each pipe, or a live-thread admission failure
after resource acquisition. Each records exactly two query attempts, preserves
all three safety commands, reaps workload leaders and removes the fake marker;
the resource ledger remains uncertain. A separate adapter makes WNOHANG always
report non-observation for a real child. It proves strong ownership and absence
of implicit wait/destructor polling, then independently reaps the fixture. This
is simulated unreapability, distinct from the real helper KILL controls above.

The [independent audit](../../experiments/exec-reliability/evidence/k47-audit-01/result.json)
checks every retained input/artifact hash, both suites' per-case verdicts, helper
bounds, safety controls, every recorded real helper/workload/checker PID and
exact fixture marker. Live, mixed, invalid-selector and freshly reaped controls
contest the absence predicate. It removes only allowlisted scratch artifacts
after later exact-PID absence observations. Counts and itemized paths are in
the audit receipt; those later observations do not prove process history or
containment of escaped descendants.

This is the first green child of worker-scheduler-k44. The complete private
worker, every-fork reset, publication/protocol, shared scheduler, concurrent
teardown/shutdown and full fault inventory remain live on worker-scheduler-k48,
including the final whole-runtime review before k21. True UID denial and real
setuid-ps KILL remain unavailable and distinct from deterministic controls.
No creation, privilege, storage isolation, immutable-publication or live-guest
guarantee follows. Historical acceptance remains **0/10, unavailable**.

### Earlier foundation controls

This section describes the unchanged original producer conditions. Use the
integration verdict above for the revised controller.

`mise exec -- task diagnostic:self-check RUN=self-check-final` passed **13 tests**
on 1 November 2026, including Ruff lint/format and strict Pyright. The interpreter
was CPython 3.12.15; the lock selects pytest 9.2.0, Ruff 0.17.3 and Pyright
1.1.414. Raw declarations, streams and receipts are retained in
[`self-check-final`](../../experiments/exec-reliability/evidence/self-check-final/).
The [item index](../../experiments/exec-reliability/evidence/self-check-index.json)
also includes the earlier 12-test condition under `self-check/`; those receipts
describe their own earlier source digest and are not relabelled as the final build.

The rows below name the original foundation's final-condition run directories. Their `result.json` may
correctly say `ok: false`: the **test passes because the forced failure was
detected**, not because that command succeeded.

| Control | Run directory under `self-check-final/` | Observation |
| --- | --- | --- |
| Exact binary stdout and separate stderr | `b744181d8c2d42309bc146c6aeb2f7a5` | `00ff80410a` and `separate stderr\n` preserved exactly; subject hashes equal |
| Overlong call | `30fe9008e3954abc97d23ec8cf9e5957` | Deadline, exit -15, partial stdout retained; next case unreached; 0.352 s |
| CLI ignores TERM | `b22264e5d64f49099e3045929ea7db5e` | PID-only TERM then KILL, exit -9, reaped; original survival assertion was tautological; k20 supplies independent observation; 0.460 s |
| Owned local fork | `b7edf56ad7804d9eb9ce968f9f237ee8` | Group deadline; known child absent by independent PID check; leader reaped; 0.458 s |
| Blocked transfer and failed cleanup | `be30cc27bafb4dc280e95e7bd07da2fc` | Transfer deadline and cleanup exit 23 retained; disposal and absence check still succeeded; unrelated marker intact |
| Failed inspection | `242a2ddd302a4c1eb253212db8ab6aea` | Inspection exit 23 leaves disposal uncertain despite successful disposal command |
| Blocked disposal | `444a6cccc6b546599ef421539c5a01a9` | TERM ignored, KILL/reap, residual identity retained; 0.354 s |
| Blocked cleanup and inspection | `349b178700884c9593492f2a62bf0a05` | Both deadlines detected; disposal ran between them; uncertainty retained |
| Transfer failure | `f1aaff0bad01421bb0a4ead4bbfb5fe1` | Exit 23 and raw stderr retained; no success conversion |
| Subject drift | `68b2bc9f7ce14fb88ae1ee3961ffe1bc` | Changed input digest fails the run despite command exit 0 |
| Spawn failure | `b2f0507bb5254ea4ba324f36d913e357` | Non-executable owned file gives `spawn_error`, no numeric exit |
| Exit observer failure | `d77bff4629b74ecab089d5e3e5799c85` | Injected OS-observation error retained, owned process terminated/reaped; 0.154 s |
| Foreign identity and reused output | `a03dc090aa2b4325b86f3eb3a3ba78a9` | Foreign identity rejected before calls; exclusive directory creation rejects reuse |

The short local controls declare 0.2/0.3-second waits, 0.15-second TERM grace
and 0.3-second kill/reap allowance. They are instrument checks, not substitutes
for the issue's unchanged two-second guest controls. There are no live guest
samples or production repair results in this leaf.

During development, the missing controller import established the initial red
check. An incorrectly rooted pytest invocation also collected unrelated vision
tests; the Taskfile now runs from the diagnostic project. macOS lacks Python's
`os.waitid`, and kqueue has no Python context manager. The corrected observer
uses explicit closing and checks event errors. A zero-time reap also proved
insufficient after exit observation, so reaping uses the declared bounded wait.
An injected observer-error check then exposed an exception escaping without a
receipt; the final condition retains that failure and reaps its command.
These development errors are not TestAnyware causal findings.

## Public route preflight

The complete finite read-only command inventory, raw streams, durations and
subject digests are in
[`preflight-20261101-01`](../../experiments/exec-reliability/evidence/preflight-20261101-01/).
Each condition has its own UUID and result. The preflight wrapper finishing
successfully does **not** mean every condition passed: Doctor returned exit 1
and `ok: false`. Its only failing check was Start at Login disabled. Nothing
changed that setting. The preflight used the earlier controller digest recorded
in those receipts; the final self-check separately validates the amended observer.

The installed Host CLI and app both report 2.7.0, protocol range 1–2. Public app
status identifies `/Applications/TestAnyware.app/Contents/MacOS/testanyware-app`;
`lsof` identifies PID 67637 holding the default socket. The app reports an
**earlier** Local Network success (`observed_at: 1793431658`), not a fresh guest
health result. Both public inventories show only stopped baselines and no
running guest. Checkout debug CLI/app, development bundle and release macOS
agent paths were absent. No retained image was exported or rehashed in this
preparatory run; its current archive identity remains unverified.

| Guest OS and route | Established fact | Untested or unavailable condition / next action |
| --- | --- | --- |
| macOS, unchanged installed CLI/app/agent | Installed CLI/app identities and stopped `testanyware-golden-macos-27` selector found | Untested, not a proved missing prerequisite. Installed start help both denies macOS support and gives macOS examples; checkout routing cannot prove this binary. k19 resolves public support and freezes boot/slot/probe/disposal bounds before one owned-clone identity/health probe by VM ID. |
| Linux, unchanged installed CLI/app/agent | Same installed host route; stopped `testanyware-golden-linux-24.04` found | Untested. k19 freezes bounds and a dual-inventory absence checker before one owned-clone identity/health probe. A missing probe is not an unavailable prerequisite; QEMU help's 10–14 minute agent wait must be accounted for explicitly. |
| macOS, checkout CLI through installed app | Source supports the ordinary public app route and explicit executable paths | CLI not built. `mise exec -- task cli:build` supplies an isolated CLI, but it still reaches the installed 2.7.0 app. Record that mixed condition explicitly; do not call it an isolated app run. |
| Linux, checkout CLI through installed app | Same host-side routing; Linux agent is Python source, not a compiled build | Same CLI build prerequisite and mixed-app limit; checkout agent is not installed in any guest. |
| macOS, checkout CLI + checkout app + checkout agent | Build/sign and uniquely named golden recipes exist | All three builds absent; installed app occupies default socket; development bundle has no observed grant. Build with `cli:build`, `app:bundle-dev`, `agent:macos:build`. A person must arrange the app handover and allow the development bundle's Local Network request. Then build a **new named** baseline using `vm:create-golden-macos NAME=<unique>` from Terminal. No live proof yet. |
| Linux, checkout CLI + checkout app + checkout agent | `vm:create-golden-linux` accepts the checkout agent directory override | Same app handover/grant and Host CLI prerequisites. From Terminal, `mise exec -- task vm:create-golden-linux NAME=<unique> VERSION=24.04` prepares a new baseline. Python source needs no build, but guest dependency/runtime and installed source hashes must be verified. No live proof yet. |

The app handover recipe is concrete but **not executed or requested as a gate
for this leaf**: build `app:bundle-dev`, record bundle/executable/signature/UUID
and source diff, ask the owner for an interval to quit the installed app, open
the printed `TestAnyware Dev.app` through LaunchServices, allow Local Network,
then verify public `/v1/status` executable/build before the one owned guest
probe. Restore the installed app after stopping only the test app the run
started. This temporarily interrupts other clients of the installed app; an
occupied socket is not permission to quit it. Baseline causal work can use the
unchanged installed app without that handover. An isolated-app condition stays
unavailable until the owner supplies the interval and grant.

An alternate socket exists only in the app's **headless test mode**, requiring
`--test-mode --socket --state-dir --data-dir`; Host CLI selects it with
`TESTANYWARE_APP_SOCKET`. It is useful for fake peers. It has no LaunchServices
grant proof and is not an established live development-bundle route. Do not
substitute direct guest IPs or private connection files for this missing proof.

Public `agent --help` exposes health and accessibility operations, not an agent
installer. Public `file upload --vm <id> <local> <remote>` and `file exec --vm
<id>` can transfer and run a staged installer in an **owned clone**. Thus golden
creation is not assumed to be the only possible agent-install route. However,
no documented transactional agent-replacement command was found on that public
surface. macOS path/signature changes invalidate Accessibility permission;
Linux requires the system Python/AT-SPI dependencies and a service restart.
Transfer alone proves none of service replacement, fresh process identity or
permission. A staged installer remains an unverified alternative; use the
existing new-name golden tasks for the documented full provisioning recipe.

Those tasks explicitly set `TESTANYWARE_AGENT_BIN_OVERRIDE` to the checkout
macOS executable or Linux source directory. macOS provisioning installs at
`/usr/local/bin/testanyware-agent`; Linux provisions `/opt/testanyware` with a
systemd user service. Golden creation is ADR-0004's direct-network exception
and must run from a Terminal context that may reach guest IPs. New names are
mandatory: the public command warns that rerunning a name replaces its golden.
Record its temporary setup VM, new golden and archives before invoking it.
Dispose owned clones through `vm stop <id> --json` within 45 seconds and verify
the exact identity absent in **both** public VM and hypervisor inventories.
After retaining evidence, delete only the new baseline with `vm delete <name>`;
never pass a retained baseline name. Inspect failed starts separately: a stop
error or failed inventory cannot prove absence.

## Provenance and resource audit

The source parent was `01952865af50ad47be5c10ee369cd31b6e597e7c`;
the preflight's `source-diff` receipt retains its dirty diff. Source changes
after that measurement belong to the final self-check condition. The leaf's
commit identifies the complete delivered artifact. The host is macOS 27.0.1,
build 26A434, arm64. Stable before/after preflight identities include:

| Subject | SHA-256 |
| --- | --- |
| Installed Host CLI | `2298b1bd55af4f208286c57b6ff67b98e79a5ea8641c1c584632895bcc707720` |
| Installed app executable | `b7a51852f24ae4402185cd18f7239ac7f7baf0a02ff4314bd138bcf8ddfc60bb` |
| Tart 2.41.1 | `a5e51a515d1b96c14e369e10352532f9aa5230c2233e8f39b611122bf6852276` |
| Original producer's final diagnostic controller | `907f73196a76588b43825eec365e6232247705794df8f36e8531b65e6f3503b4` |
| Diagnostic uv.lock | `1c3ab314b597770f6568fa159164b8ac72af7d9c3c9fc74ef3d9cf47b73cfd89` |
| cli-rs/Cargo.lock | `96ee8c89fbc69a1e74f97af03df5e85528bae27ddacd1143ab568468607793e2` |

The installed app's UUID is `90A64C4E-D078-31DB-8462-583D5EA4ABE4`,
bundle ID `dev.antony.TestAnyware`, Developer ID team `TA43A4RUP3`.
Matching historical tool hashes establishes continuity of those files only;
it does not prove the installed macOS guest agent's source. No fresh guest
OS/agent hash or baseline archive attestation exists in this run.

The independent
[resource audit](../../experiments/exec-reliability/evidence/resource-audit/6440c5d98be14627a702a4a30a6770d0/result.json)
checked the exact recorded local process PIDs and known fork children: `ps`
returned exit 1 with empty stdout and stderr, so none of those identities was
present at audit time. It removed one fake-VM marker
`diag-444a6cccc6b546599ef421539c5a01a9-vm` and positively checked absence.
Original uncertain-disposal receipts remain unchanged. Earlier pytest-owned
scratch directories may already have been pruned by pytest; this is not live
VM disposal. This original audit did not account for all six historical
uncertain entries; the integration's new itemized audits above do. The diagnostic
venv and retained raw evidence are intentional
workspace artifacts. No VM, image, archive or test app was acquired by this
leaf, so there is no live VM/image/app teardown to report.

The graph supplied Tier 2 discovery for the app, CLI and golden provisioning
boundaries. Coverage checks found no recorded parse gaps on the relied-on
source paths; this is a best-effort signal, not a completeness proof. Changed
Taskfile and untracked lockfile metadata were checked directly. Relevant source
is in `testanyware-cli/src/{main,resolve,commands/vm}.rs`,
`testanyware-app/src/main.rs`, `testanyware-vm/src/golden_linux.rs`,
`scripts/build-app.sh`, and the two `vm:create-golden-*` Taskfile entries.

Before the next causal inquiry, the diagnostic evidence, disposal and route
increments named above must deliver their reviewed handoffs. The later macOS
inquiry owns scoping the unsafe process-test cleanup. Unchanged installed
macOS/Linux live routes have not yet been demonstrated; k19 owns preparation
rather than leaving hidden setup in the first causal sample. Checkout agent/app live
conditions cannot be claimed until the table's build/install/grant facts are
established. If that preparation requires its own session, insert it before
measurement; do not absorb an unbounded setup into a causal sample. The
controller's scheduled implementation review precedes these consumers.


## k50: bounded-helper review integration

The k49 review of producer `2d258277` identified three receipt/cleanup defects.
All three are repaired in this narrow integration; synchronous helper creation
and the shared worker/scheduler remain `worker-scheduler-k48` work.

1. **Close failure — real issue.** Each pipe close now has independent
   `OSError`/`ValueError` handling. `close_errors` retains the diagnostic, the
   other close still runs, reap attempts continue and the finished receipt
   reaches the controller. Separate stdout/stderr wrappers close a real pipe
   then raise EIO; both real children are KILLed and reaped with failed receipts.
2. **Final sleep — real issue.** A final exact-PID WNOHANG observation follows
   the wait loop, even at/after the original cutoff. A deterministic clock/wait
   adapter makes exit become observable during the last sleep and verifies
   reap at the cutoff. It has no real child or signal. No new wait allowance is
   granted. Scheduling can still return late; lateness remains a failed result.
3. **Late Popen — real issue.** A helper whose synchronous creation returns
   after the deadline still receives exact-PID KILL while its unreaped identity
   remains owned, followed by the final nonblocking observation. The fixed
   deadline is unchanged. A real child plus delayed-constructor adapter verifies
   the late signal, empty capture and failure; independent fixture wait reaps
   it if that final observation cannot. This does not bound process creation.
4. **ECHILD — real issue.** ECHILD removes the stale strong owner and sets
   `ownership_lost`, without claiming an observed exit, return code or live
   residual. The error stays failed and signal authority stays closed. The
   competing-reaper control independently waits a real child before injecting
   ECHILD. Other wait errors conservatively retain the owner and close signal
   authority. Neither control establishes genuine privileged denial.
5. **Obsolete TimeoutExpired handler — real issue.** Removed from group
   inspection after the transition away from `subprocess.run`.
6. **Adapter scope — accepted test boundary.** Controller return-value adapters
   establish controller reactions. Real-pipe helper controls establish byte
   caps and KILL/reap behavior; clock/wait, competing-reaper and close-failure
   adapters establish their stated injected conditions only. Simulated
   unreapability still means WNOHANG always returns `(0, 0)`, with a real child,
   real KILL and independent fixture reap. No UID-denial result is claimed.

Retained conditions (all checker processes reaped; input hashes stable):

| Condition | Result |
| --- | --- |
| `k47-check-01` | Historical lint failure retained unchanged. |
| `k47-final-01` | Historical 130-test success retained unchanged. |
| `k50-red-01` | Five new cases reproduce the defects; inherited poll control also fails: 6 failed, 90 passed, 39 deselected. |
| `k50-final-01` | 135 tests pass; strict typing rejects the unparameterized list factory. |
| `k50-final-02` | Ruff lint/format, strict Pyright and all 135 tests pass (pytest checker 26.675 s). |

The inherited early-reap poll control could observe NOTE_EXIT before waitpid
reported a waitable child. Its injected poll now retries nonblocking for at most
one second and requires an actual reap before testing released identity. This
changes the control's precondition, not controller behavior. The list factory
was parameterized after the retained type failure.

`evidence/k50-audit-01` independently checks every retained artifact digest,
per-test result and expected checker code, and compares the final condition's
source hashes with current inputs. Its exact-PID absence predicate is challenged
with live, mixed, invalid-selector and known-reaped controls. Named pytest
scratch paths come from retained argv; additional paths are explicitly labeled
session enumeration. Cleanup occurs only after every recorded real PID is
absent; preflight inventories are copied byte-for-byte before removal. This is
later observation of recorded identities, not proof against unrecorded escaped
children. The audit result is authoritative for completion of that reconciliation.
No live guest, app route, scheduler-isolation or historical acceptance claim
changes: acceptance remains **0/10, unavailable**.

## k51: isolate synchronous helper creation

The live controller now calls `worker_capture.capture_helper`. Each query forks
one private worker; that worker establishes its own session and runs the existing
k47/k50 helper engine. A helper constructor that blocks in the worker no longer
blocks the controller's channel loop. Native/Python single-thread and default
SIGCHLD admission precede the worker fork. Catchable signals are blocked across
fork, the parent restores its mask, and the child resets handlers, mask, wakeup
descriptor and inherited descriptors before doing helper work. Child paths use
`os._exit` so inherited exit hooks do not run.

The parent reads separate nonblocking control/result pipes, at most 64 KiB per
turn. Control bytes are capped at 4096, result frames at 1 MiB, and combined
helper output at 256 KiB. A random query token, exact worker session identity,
frame length, field set, helper argv and child deadline are checked. Up to
`min(50 ms, remaining allowance / 4)` is reserved inside the original caller
deadline for worker shutdown. Parent reads stop at the earlier query deadline;
TERM, KILL and exact-child WNOHANG observations share the original outer deadline.
No group signal is permitted before readiness establishes PID=PGID=SID. A reaped
worker closes signal authority. Channel-close errors are recorded independently
so another close and the final failure receipt still complete.

Real-process controls cover a ten-second delay before helper construction,
a ten-second startup delay before worker readiness, healthy capture, inherited
TERM/INT/PIPE handlers, blocked signals, wakeup/evidence descriptors and exit
hooks. The delayed-construction receipt returns within its original 500 ms
allowance and records a reaped worker plus unknown helper creation. The startup
control uses PID-only shutdown and returns within 400 ms. These are local
creation-boundary controls. The fork-error control injects Darwin EAGAIN without
creating a child; it does not establish real resource exhaustion or privileged
denial. The close-error control closes a real parent pipe and then injects EIO.
Existing direct-engine and controller return-value adapters retain their earlier
scope; they do not establish the new worker protocol's full adversarial contract.

| Retained condition | Result |
| --- | --- |
| `k51-red-01` | Initial two tests fail because the worker module does not exist; JUnit retained without a full runner envelope. |
| `k51-check-01` | 137 tests pass; strict typing rejects five new-code expressions. |
| `k51-check-02` | All four checks pass; 141 tests pass. |
| `k51-close-red-01` | Real-close/injected-EIO control reproduces receipt loss; JUnit retained without a full runner envelope. |
| `k51-final-01` | Ruff lint/format, strict Pyright and all 142 tests pass; before/after subject hashes agree. |

An intermediate Darwin platform error (`os.pipe2` unavailable) led to using
`os.pipe` with explicit nonblocking configuration; that targeted failure has no
separate retained artifact. All retained failed samples remain unchanged. The
final self-check freezes the worker, tests, local helper engine, controller,
manifest, lock, runner, Taskfile, interpreter and process inspector. The independent
`evidence/k51-audit-02` auditor checks these hashes and the retained per-test
results, challenges its absence predicate with live/mixed/invalid/missing PID
controls, and accounts for exact fixture paths before removing them. Its result
records the later process observations and any resource reconciliation. The
close-error RED lost its receipt, so its worker PID is unavailable to that audit.

This slice uses a worker per query. Raw worker fork remains a synchronous OS
operation. Worker reap or group KILL never proves helper absence; an incomplete
result retains unknown creation, and a known helper residual stays in the local
capture receipt. The persistent worker before acquisition, gated helper identity
publication, every-helper child reset, complete run/request/workload/epoch/time
validation, and one scheduler spanning workloads, teardown and worker shutdown
remain in `complete-worker-scheduler-k55`. Its charter retains the full ancestor
specification and controls, including overlap, sticky failure, native admission,
backpressure, malicious frames and independent helper reconciliation. The final
whole-runtime review still belongs before retained evidence k21. No guest, app,
privilege, storage independence or acceptance claim changes: **0/10, unavailable**.

The first auditor, `k51-audit-01`, stopped during schema validation because the
earliest receipts lack the later worker deadline fields. It performed no process
checks or cleanup. Its script, frozen inputs and failure are retained; the
second auditor explicitly validates each historical schema.

### k54: reviewed worker transport repairs

Review k53's closed-stream and decoder findings reproduced against the per-query
worker. Reset now preserves explicitly allowed descriptors even when they occupy
0, 1 or 2. A shared readiness/result JSON loader rejects duplicate keys and turns
decoder recursion exhaustion into a protocol failure. Failed publication retains
unknown helper creation, shutdown evidence and the existing residual remedy.
Inspection docstrings now identify the synchronous engine's live worker caller.
The suggested field-type `TypeError`/`AttributeError` escape was not reproduced:
the existing type guards reject the bad-field-type control before those operations.

Seven real subprocess controls close every nonempty combination of standard
streams before invoking the live worker. A separate real controller group query
observes an owned unreaped sleep session through the live worker, helper and
parser, then kills and reaps the fixture. Twelve protocol adapters replace only
child publication to exercise malformed, nested, duplicate-key, wrong-token,
wrong-timestamp, wrong-type, oversized, partial and early-exit messages. Parent
channel handling, fork/reset, shutdown and reap remain real. Those adapters are
not evidence of a live malicious helper or the full scheduler protocol now
owned by k55. The duplicate-readiness adapter also found last-key-wins decoding
in readiness,
which the shared loader repairs.

| Retained condition | Result |
| --- | --- |
| `k54-red-01` | 6 failed, 156 passed: four closed-stream combinations, escaped nested-result recursion, and accepted duplicate readiness. Lint import-order failure retained; format and strict types pass. |
| `k54-fixed-01` | Ruff lint/format, strict Pyright and all 162 tests pass. Every hostile-publication control returns failure/unknown creation with the worker reaped inside its original allowance. |
| `k54-audit-01` | Frozen independent audit passes: 949 retained exact process identities absent and 270 fixture directories reconciled. |

Both self-checks retain matching before/after input hashes, checker reap receipts
and artifact hashes. The escaped-exception baseline separately retains the exact
worker owner state after shutdown. The auditor validates those envelopes, the
earlier k51 auditor's recorded evidence hashes, and live/mixed/invalid/missing
selector controls for its own absence predicate. Every earlier failed sample
remains unchanged. Later absence observations do not repair original unknown
creation; k51's missing close-error worker identity remains unknown.

Child-reported helper identity/reap remains conditional evidence. The persistent
worker, gated publication, independent helper reconciliation and shared scheduler
remain live in `complete-worker-scheduler-k55`. This repair neither closes an
ancestor nor changes the guest/app/privilege or acceptance result: **0/10,
unavailable**.

### k52: worker receipt timing validation

The live worker transport now validates its query interval against timestamps
observed by the parent: dispatch immediately before fork, followed by worker
start, worker finish, complete-channel receipt and the immutable query deadline.
Required times must be finite numbers. The capture cutoff must fall between
worker start and that deadline; optional creation, kill and reap times must lie
inside the query interval in causal order, with creation required before kill or
reap. The parent checks the remaining allowance after decoding before accepting
output. It retains the validated worker times separately from its own completion
time and outer shutdown deadline. Complete malformed frames also retain the
parent receipt timestamp. A rejected publication retains failure, empty output
and unknown helper creation.

Sixteen additional synthetic publications exercise predispatch/reversed/future
intervals, null required times, invalid cutoffs, impossible helper event times
and nonfinite numbers. Their child publisher creates no helper; fork, reset,
nonblocking channels, framing, validation and parent shutdown remain real. Raw
payload and frame bytes are retained as hex beside the failed outcome. Existing
real reset/capture controls now check the parent/worker interval, and a real
sleep helper is killed and reaped at its capture cutoff with its timing retained.
These checks establish interval consistency; they do not authenticate a child's
helper identity or independently prove that helper was reaped.

Review k56 found that the original kill-before-create and reap-before-kill
fixtures could move query start before parent dispatch. Their generic error
assertions therefore did not isolate the causal-order check: dispatch validation
could reject them first. The k52 results remain retained evidence of rejection,
but do not prove those two controls exercised the intended branch. The corrected
controls and their causal-loop-disabled run are recorded under k57 below.

| Retained condition | Result |
| --- | --- |
| `k52-red-01` | 12 failed, 166 passed: impossible timing publications were accepted. Ruff lint/format and strict types pass. |
| `k52-fixed-01` | 2 failed, 177 passed: timing publications are rejected, but oversized and partial complete frames lack the new parent receipt witness. Other checks pass. |
| `k52-fixed-02` | Ruff lint/format, strict Pyright and all 179 tests pass after recording the complete-channel witness before frame validation. |
| `k52-audit-01` | Frozen evidence/resource audit passes: 1,451 retained exact process identities absent and 455 fixture directories reconciled. |

The runner retains per-test JUnit outcomes, checker reap receipts, artifact hashes
and equal before/after subject digests for each condition. Archived source,
manifest, lock and Taskfile copies in `evidence/k52-inputs-01` match those recorded
digests. Copies were made after the runs; the first fixed source was reconstructed
by reversing only the receipt-witness move and checked against its runner digest.
The audit validates those copies, raw timing publications, the real helper timing
and earlier k54 evidence hashes. Its external process oracle is challenged with
live, mixed, invalid and known-reaped selectors before checking recorded exact
identities and reconciling allowlisted scratch paths. Earlier failed evidence
remains unchanged. Later absence cannot establish an unrecorded helper identity
or repair an original unknown-creation outcome.

This independently useful slice leaves the full original persistent-worker,
gated-publication and shared-scheduler contract live in
`complete-worker-scheduler-k55`; every ancestor remains open. Review k56
challenged this timing boundary, its evidence and ownership claims; k57 integrates
its findings below. The final whole-runtime review remains due
before retained evidence k21. No guest, app, privilege, stalled-storage or
acceptance claim changes: **0/10, unavailable**.

### k57: causal controls and receipt field consistency

The corrected hostile publisher samples query start and finish after dispatch,
then reverses only the helper event times. Every synthetic timing fault now
asserts its exact rejection reason. Four ordering controls fail when the causal
loop is disabled, including kill-before-create and reap-before-kill; the retained
payloads also show valid dispatch/receipt intervals and otherwise coherent
creation/reap fields. This separates causal ordering from the earlier dispatch
check without assuming a child scheduling delay.

The decoder also requires reap state and reap timestamp to agree, PID and
creation timestamp to agree, and a kill timestamp whenever a kill errno is
reported. It verifies the exact capture cutoff derived from query start and the
immutable query deadline. Six hostile controls cover both directions of the
first two relationships, the kill errno implication and the cutoff. Genuine
early-exit and missing-executable controls retain accepted worker publications;
the existing normal capture and real killed-helper controls also remain valid.

| Retained condition | Result |
| --- | --- |
| `k57-order-red-01` | 10 failed, 177 passed: disabling only the causal loop accepts four impossible event orders; the six new consistency cases are also accepted. Lint, format and strict types pass. |
| `k57-coupling-red-01` | 6 failed, 181 passed: the original decoder rejects the four ordering controls for the exact causal-order reason, but accepts the six inconsistent receipts. Other checks pass. |
| `k57-fixed-01` | Lint, format, strict Pyright and all 187 tests pass with field relationships and the exact cutoff checked. |
| `k57-audit-01` | Audit stops before resource inspection on an empty optional nested-readiness publication file. The parent had already rejected readiness; this failed audit remains unchanged. |
| `k57-audit-02` | Receipt validation passes; resource enumeration stops on the same optional empty witness. This failed audit also remains unchanged. |
| `k57-audit-03` | Corrected frozen audit passes: 1,452 recorded exact process identities absent and 480 fixture directories reconciled. All earlier evidence digests match. |

Each runner condition has the same complete JUnit case set, retained checker
reap receipts, artifact hashes and equal before/after input digests. All source,
manifest, lock and Taskfile copies in `evidence/k57-inputs-01` were frozen before
their corresponding runs. The prior-evidence manifest records every earlier
evidence file's digest; no earlier failed or successful run is overwritten.
The final audit verifies those frozen copies, per-test outcomes, exact new
rejection reasons, ordering-control payloads and accepted genuine publications.
Its live, mixed, invalid and known-reaped process selectors challenge the absence
oracle before resource inspection. It preserves the earlier audit failures,
reconciles only recorded fixture sessions and reports equal before/after subject
digests. An empty optional readiness witness is permitted only for the two
readiness-rejection controls; every timing publication remains required.

These repairs check consistency of the worker's report. They do not authenticate
helper identity or independently establish helper reap. The full persistent
worker, gated publication, shared scheduler and final whole-runtime review
remain live under k55 and its ancestors. Acceptance stays **0/10, unavailable**.

### k55: positive helper PIDs in worker receipts

The decoder now rejects a reported helper PID of zero or less with
`invalid worker helper pid`. Such values cannot identify one exact helper.
The existing integer check still rejects booleans and strings, and a missing PID
remains valid when helper creation failed and no creation timestamp is reported.
This is a protocol consistency check; a positive worker-reported PID still does
not authenticate helper ownership or independently prove reap.

Five added hostile publications carry zero, `-1`, `-123`, a boolean and a string
PID. Each has a matching creation timestamp, a valid query interval witnessed
by the parent and the exact derived capture cutoff. The synthetic publisher
creates no helper. The fork, reset, bounded channels, decoder and worker shutdown
remain real. Tests require the exact rejection reason, discarded output,
conservative unknown helper creation, worker reap and completion within the
original deadline. Existing genuine capture, early-exit, missing-executable and
killed-helper controls continue to exercise accepted publications.

| Retained condition | Result |
| --- | --- |
| `k55-red-01` | 3 failed, 189 passed: zero and both negative PIDs were accepted. Boolean and string controls already reject with `invalid worker integer`. Lint, format and strict types pass. |
| `k55-fixed-01` | All 192 tests, lint, format and strict Pyright pass after the nonpositive PID check. |
| `k55-audit-01` | Ruff E501 stops the task before audit execution. The original script and failure record are retained. |
| `k55-audit-02` | Frozen audit passes: 978 recorded positive PID selectors absent and 330 fixture directories reconciled. |

The two test conditions retain the same per-test JUnit case set, checker reap
receipts, artifact hashes and equal before/after source digests. Copies in
`evidence/k55-inputs-01/red` were made after RED launched and match its recorded
digests; copies in `final` were made before the final run. The audit checks
every copied source, manifest, lock and Taskfile against the corresponding
runner record. Its prior-evidence manifest covers 42,688 earlier evidence files;
those hashes remain unchanged.

The audit verifies the new publication values and their otherwise coherent
timestamps, exact rejection reasons, retained RED failures and final results.
It challenges its external process oracle with live, mixed, invalid and
known-reaped selectors before inspecting recorded process identities. Synthetic
nonpositive selectors are excluded before inspection. Scratch cleanup is confined
to the recorded pytest sessions; copied evidence and per-path removal records
are retained. The audit records equal before/after input hashes in
[`k55-audit-02/result.json`](../../experiments/exec-reliability/evidence/k55-audit-02/result.json).
Later absence cannot repair an unknown-creation outcome or identify an unrecorded
escaped helper.

The full original persistent-worker, gated-publication, independent ownership
and shared-scheduler contract has moved from k55 into the flat live sibling
`complete-worker-scheduler-k58`. Every ancestor remains open. A separate
`review-impl` of this PID-validation increment precedes k58; the final aggregate
review remains due before retained evidence k21. No guest, installed-app,
privilege, stalled-storage or acceptance claim changes: **0/10, unavailable**.

### k58: terminal helper receipt consistency

The interim worker decoder now requires a returncode exactly when it reports
helper reap. A reaped helper cannot also report a reap error or lost ownership.
These four lines reject internally contradictory evidence after the existing
field, timing, positive-PID and event-coupling guards. They do not authenticate
worker-reported ownership or independently establish that a helper was reaped.

Eight controls replace only the helper capture result inside the ordinary worker.
Four publish status without reap, reap without status, reap with a wait error,
and reap with lost ownership. Four coherent controls cover success, nonzero
status, an unreaped wait error and an unreaped lost-ownership error. Each helper
receipt is explicitly synthetic and uses the actual worker PID as its reported
helper selector; no helper is created. Worker admission, fork, reset, framing,
channels, parent decoding and worker shutdown are real. Rejections require exact
reasons, discarded output, conservative unknown creation and real worker reap.
The retained witness is the serialized receipt body before ordinary framing.
Existing genuine success, missing executable, early exit and killed-helper
controls still pass.

| Retained condition | Result |
| --- | --- |
| `k58-red-01` | Four new contradictory-receipt cases fail; the other 196 pass. Lint, format and strict types pass. |
| `k58-fixed-01` | All 200 tests, lint, format and strict Pyright pass. |
| `k58-audit-01` | Ruff rejects an unused variable before audit execution; original script and failure log retained. |
| `k58-audit-02` | Frozen audit passes: 994 recorded positive selectors absent; 346 owned fixture directories reconciled. |

Both conditions use identical tests and retain the same individual JUnit case
set. Frozen source copies were made before each run and match every recorded
source/manifest/lock/Taskfile digest. The audit verifies each condition's checker
reap receipts, artifact digests, unchanged before/after subjects, exact expected
failing cases, publication values and parent dispatch/receipt witnesses. Its
46,705 frozen input paths include 44,682 earlier evidence files, which remain
byte-for-byte unchanged. No subject was edited while its measurement ran.

The external process oracle is challenged with live, mixed, invalid and
known-reaped selectors. Explicit simulated selectors are excluded; no selector
is signalled. Cleanup is confined to fixture directories in sessions named by
retained argv or JUnit evidence, with bounded enumeration, copied creation
witnesses and per-path removal records. See
[`k58-audit-02/result.json`](../../experiments/exec-reliability/evidence/k58-audit-02/result.json).
Later absence cannot resolve the original unknown creation or identify an
unrecorded escaped helper. Graph coverage reported changed/untracked metadata;
material conclusions use the current source and retained execution evidence.

The complete original runtime charter remains live in flat sibling
`complete-worker-scheduler-k60`, including persistent bootstrap, gated helper
publication, independently tracked ownership, the shared scheduler, remaining
error/admission consistency checks and the full control inventory. All ancestors
remain open. A separate review of this increment precedes k60; the final aggregate
review remains due before k21. No guest, installed-app, privilege, stalled-storage
or ten-clone acceptance claim changes: **0/10, unavailable**.

### k60: admission and failure receipt consistency

The interim worker decoder now validates the complete fork-admission record:
exact fields, boolean decisions, positive thread counts and errnos, signal
states, refusal priority and native-observation failures. Coherent admission
refusals survive transport as known no-helper failures. A refusal cannot also
claim helper creation. Error, remedy and channel-close messages must contain
text, and failure states must carry an error. ECHILD must agree with lost
ownership; a remedy is required exactly when an unreaped helper remains owned.
These checks establish receipt consistency. Worker-reported identity and reap
still require independent authentication in the remaining runtime work.

Fifty new publication controls include 38 malformed receipts and 12 coherent
success or failure receipts. They replace only the helper-capture result inside
the ordinary worker; worker admission, fork, reset, framing, channels, decoding
and shutdown remain real. Each witness labels its helper evidence synthetic and
uses the actual worker PID or no PID; these controls create no helper. Rejection
requires the exact reason, discarded output, conservative unknown creation and
worker reap within the original allowance. The earlier ECHILD/retained-owner
positive control now uses EIO and a residual remedy; a separate negative control
retains the contradictory ECHILD state identified by review k61.

A further control starts a real waiting Python thread inside the worker and
calls the genuine helper-capture implementation. Both thread observations report
two threads, so helper admission refuses before creation. The final transport
preserves that refusal and the known no-helper outcome. Existing genuine success,
nonzero exit, missing executable and killed-helper controls continue to pass.

| Retained condition | Result |
| --- | --- |
| `k60-red-01` | 45 failed, 206 passed: 38 malformed publications, six coherent admission refusals and the genuine thread-refusal control expose the old decoder. Lint, format and strict types pass. |
| `k60-fixed-01` | All 251 tests pass; lint and format pass. Strict Pyright reports four narrowing errors in the new validator. This attempt remains unchanged. |
| `k60-fixed-02` | All 251 tests, lint, format and strict Pyright pass after the narrowing correction. |
| `k60-audit-01` | Lint and format pass; audit execution stops on an ambiguous filename-prefix match before resource reconciliation. The frozen script and failure record remain retained. |
| `k60-audit-02` | Exact witness-name matching fixes the audit. All checks pass: 1,644 recorded positive selectors absent and 672 fixture directories reconciled. |

All three test conditions retain the same individual JUnit case inventory.
Source, manifest, lock and Taskfile copies were frozen before each run and match
its recorded digests. The audit verifies checker reap, artifact hashes,
before/after subject equality, each expected RED failure, publication contents,
exact decoder reasons and parent dispatch/receipt timing. Its 50,043 input paths
include 46,714 earlier evidence files, all unchanged. No measurement subject was
edited during its run.

The external process oracle is challenged with live, mixed, invalid and
known-reaped selectors. Explicit simulated identities are excluded and no
selector is signalled. Scratch reconciliation stays within fixture directories
in retained pytest sessions, with bounded enumeration, copied creation evidence
and per-path removal records. See
[`k60-audit-02/result.json`](../../experiments/exec-reliability/evidence/k60-audit-02/result.json).
Later absence cannot resolve the original unknown creation or identify an
unrecorded escaped helper. Graph coverage for the changed runtime, tests, report
and final auditor has no recorded issue with matching metadata; this is a best
effort signal, and the conclusions use the source and retained execution records.

The entire original runtime and control charter remains live in flat sibling
`complete-worker-scheduler-k62`: persistent bootstrap, gated helper publication,
independent ownership, shared scheduling and the full real-control inventory.
An independent review of this increment precedes k62; the final aggregate review
remains due before k21. All aggregate ancestors stay open. No guest, installed-app,
privilege, stalled-storage or ten-clone acceptance claim changes: **0/10,
unavailable**.

## Raw-helper containment correction (k66)

The k65 review identified that the raw-helper reset introduced in k62 called
`setsid()` and placed each helper in a separate group and session. That
contradicted the worker-group contract and allowed a live helper to survive a
worker-group KILL. The retained earlier pre-exec receipt and a new failing real
group-signal control both reproduce the defect.

Helpers now retain the worker's group and session; the worker still creates its
own session. The reset API defaults to creating a session, and only the helper
path opts out. The corrected run passes all 259 tests plus lint, format and
strict types. Its real exec helper shares the worker's group/session, and a
kernel exit notification independently observes that helper exit after the
group KILL. Helper reap acknowledgement after worker death remains unavailable.

The [k66 report](../../experiments/exec-reliability/evidence/worker-group-k66/report.md)
separates actual runtime/kernel operations from fixture coordination, records
fixed bounds and exact RED/GREEN outcomes, and links source snapshots and
before/after digests. All 55,603 earlier evidence files remain byte-for-byte
unchanged. The current full contract remains live in
`complete-worker-scheduler-k64`; its original body and every aggregate ancestor
stay unchanged and open. Ten-clone acceptance remains **0/10, unavailable**.

## Native SIGCHLD auto-reap admission (k64)

The native observation now rejects a default SIGCHLD handler with Darwin's
`SA_NOCLDWAIT` flag. Python's cached `SIG_DFL` and the native handler alone do
not establish wait ownership: this flag can discard child status. The existing
controller acquisition and worker/helper fork admissions reject the distinct
native state. The worker receipt decoder accepts a coherent refusal and rejects
a ready claim carrying it. Existing `SIG_IGN` and custom-handler classifications
are preserved.

The [k64 report](../../experiments/exec-reliability/evidence/native-auto-reap-k64/report.md)
retains all three conditions: RED has 259 passes and ten new failures; the first
repair has 268 passes and one existing ignore-classification failure; the final
repair passes all 269 tests, lint, formatting and strict types. Real libc controls
cover acquisition and both fork boundaries with exact signal-state restoration;
two real workers transport native-state helper refusals. Two further decoder
publications are explicitly synthetic and create no helper.

The independent audit retains its own failed count assumption and corrected
result. It verifies unchanged inputs and the same individual case inventory,
57,834 unchanged prior evidence files and absence of 18 exact recorded checker
and fixture-worker PIDs. It does not claim helper reap acknowledgement or a
resource census of every existing test. The full original aggregate charter is
preserved in flat sibling `complete-worker-scheduler-k67`, preceded by a separate
review of this admission change. Every aggregate ancestor remains open;
ten-clone acceptance remains **0/10, unavailable**.


### k69 explicit helper fork-error handoff

The interim worker separately publishes the actual helper fork OSError with errno, request/deadline and monotonic timing. Validated no-child evidence survives terminal loss while the query remains failed; silence, setup failure, late/malformed messages and conflicting identity/terminal evidence remain conservative. The complete locked self-check passes 311 tests plus lint, formatting and strict types. The independent audit found all 1,808 retained exact positive PID selectors absent and confirmed 65,742 prior evidence files unchanged. These later observations do not resolve unknown creation or acknowledge helper reap. See the [k69 retained report](../../experiments/exec-reliability/evidence/helper-fork-error-k69/report.md) for every failed sample, injected-versus-real distinction and audit limits. The unchanged full scheduler charter continues in k71; aggregate ancestors remain open and ten-clone acceptance stays 0/10 unavailable.

### k75 incremental worker result framing

The interim per-query receiver validates the four-byte result header immediately,
limits later reads to the declared remainder plus one detection byte, and rejects
truncated result EOF even while the control pipe remains open. Terminal success
still requires both clean channel ends. Early-error cleanup preserves creation
conflicts, so partial contradictory data cannot grant no-child authority.

The [k75 report](../../experiments/exec-reliability/evidence/result-framing-k75/report.md)
retains five frozen conditions, including both RED runs and the failed first full
check. The final check passes all 357 tests, lint, formatting and strict types.
The independent audit reconciles each case, verifies 78,030 prior evidence files
unchanged, and observes all 1,397 recorded exact positive PID selectors absent.
It explicitly excludes an exploratory run's lost fixture receipts and does not
convert later absence into historical helper reap or resolved unknown creation.
The full persistent-worker/shared-scheduler charter remains live in k78, preceded
by a separate review of k75. All aggregate ancestors stay open; ten-clone
acceptance remains **0/10, unavailable**.

### k80 result framing review integration

The receiver now rejects zero and one-byte declarations at the completed header.
Independent testcases exercise framing errors, requested-read budgets, cumulative
byte counts, first TERM, early return and fixed deadlines. Retained RED runs show
each distinguishing property failing for its own reason. Exact byte counts and unchanged deadlines that already
passed on the original receiver remain controls. Valid fragmented delivery at
exactly FRAME_CAP also passes.

The [k80 report](../../experiments/exec-reliability/evidence/result-framing-k80/report.md)
records both review findings, six frozen conditions and the failed first full
run. Final verification passes all 401 tests, lint, formatting and strict types.
The auditor compares exact failure sets and reasons, confirms 80,336 earlier
evidence files unchanged and observes 2,440 recorded exact PID selectors absent.
Later absence grants no historical reap authority. The full scheduler charter
continues unchanged in k78. All aggregate ancestors remain open and ten-clone
acceptance remains **0/10, unavailable**.


### k85 workload termination cutoff admission

Workload termination fixes its outer cleanup deadline before the first TERM or
exit observation. Signal dispatch and exit-observation entry refuse an expired
cutoff, including equality; an observation returned at/after its cutoff cannot
authorize a final table. Skipped signals are explicit unsuccessful receipts.
The earlier PID cap and fixed group grace remain in force. These are sampled
admission checks, without a hard real-time guarantee across OS preemption.

The [k85 retained report](../../experiments/exec-reliability/evidence/termination-cutoff-k85/report.md)
records fourteen final controls, six mutations of three deadline comparisons
(signal admission, observer entry and observer return) and their equality
operators, real direct children with simulated delayed observer returns, every failed sample, and a
passing 448-test full replay plus lint, formatting and strict types. Earlier
full-run permission/empty-PID fixture failures remain unresolved; the replay
only repairs a frozen checker's metadata path, with runtime/tests unchanged.
The independent audit records its exact case, identity and prior-byte limits.
This slice does not establish the persistent worker or shared scheduler. The
full aggregate continues in flat sibling complete-worker-scheduler-k88 with a
separate required final review; k28/k17 remain open and ten-clone acceptance is
**0/10, unavailable**.


### k90 termination review integration

All workload signals now close at K-R, including equality, with
R=min(100ms,A/4). Each signal receipt records the same monotonic sample used
for admission. The original deadline, earlier PID cap, observation guards and
unsuccessful skipped-signal outcomes remain in force. Sampled checks still
cannot prevent OS preemption between a clock read and the syscall.

The [k90 report](../../experiments/exec-reliability/evidence/termination-cutoff-k90/report.md)
retains the original eight failing controls, twenty passing cutoff controls,
two independent defect-restoration conditions and three invalid-mode controls.
The reusable runner now rejects missing, unknown or extra modes before output
creation. The full self-check passes 454 tests, lint, formatting and strict
types. Historical k85 evidence stays unchanged. Its six comparison/equality
mutations do not independently test early deadline initialization, the
observed-exit short circuit or each sticky observation-failure assignment.
Earlier intermittent full-run fixture failures remain unresolved. The mandatory
reap attempt after deadline expiry belongs to complete-worker-scheduler-k88,
alongside the existing worker/scheduler charter and its final aggregate review.
k28/k17 remain open; ten-clone acceptance remains **0/10, unavailable**.

### k88 first workload-leader reap after deadline expiry

The first exact-child nonblocking reap now runs even at or after the original
reap deadline, after signal and process-query authority closes. Later polls
remain bounded by that deadline. A successful late reap records status and
releases ownership while retaining failed timing; incomplete/error outcomes
retain their residual receipt without renewing grace.

The [k88 report](../../experiments/exec-reliability/evidence/leader-reap-k88/report.md)
retains seven expected RED failures, two passing runs of the same 16 focused
cases, and a full self-check passing all 460 tests, lint, formatting and strict
types. The new cases use actual exited children with explicitly simulated
clocks, no-completion and ECHILD responses. Their independent exact-child owner
retains cleanup/absence evidence. The byte/receipt audit checks case identities
and all 103,182 earlier evidence files. Earlier intermittent full-run fixture
failures remain unresolved.

The aggregate persistent worker/shared scheduler charter and its required final
review continue in complete-worker-scheduler-k91, after the focused k88 review.
k28/k17 remain open; ten-clone acceptance remains **0/10, unavailable**.


### k101 declared inspection-worker bootstrap bounds

`Plan.bootstrap` freezes the immutable readiness, failed-bootstrap TERM and
KILL/reap allowances before evidence creation or subject hashing. Each allowance
must be a positive finite number, with ceilings of one, two and two seconds
respectively. Direct Worker startup validates the same declaration before
creating pipes or forking. The runtime uses those allowances without extending
an earlier owner deadline and keeps signal scope at the exact PID until
readiness confirms the private group.

Worker receipts now include declared bounds, the readiness deadline and outcome,
and failed-bootstrap shutdown phase deadlines, finish time, overrun and terminal
ownership outcome. A delayed syscall return can exceed the sampled deadline; it
records failure and residual ownership rather than authorizing later signals.
These bounds do not provide a hard real-time guarantee.

The [retained k101 report](../../experiments/exec-reliability/evidence/bootstrap-bounds-k101/report.md)
contains the failing baseline, real-child controls, independently restored
source defects and full-suite checks. Deferred observation and delayed syscall
return are explicitly simulated; TERM/KILL and exact-child cleanup are real.
The separate producer review precedes complete-worker-scheduler-k104, which
retains the complete aggregate charter: worker-failure and orderly-close phase
declarations, the shared scheduler, overlap controls and final k28 review.
k28/k17 stay open; ten-clone acceptance remains **0/10, unavailable**.

## First worker reap at a fixed cutoff (k107)

The worker owner receives one first exact-child nonblocking reap attempt over
its lifetime, even when its original shutdown cutoff has expired. Request polls
consume that exception; a new request or close does not reset the count. The attempt is recorded before
the syscall; no-result, errno and late returns all consume that exception.
Once any attempt exists, the original cutoff forbids later polling. Signal
authority and shutdown phase deadlines do not change. A successful late reap
releases ownership while retaining failed timing; live/EIO outcomes retain the
worker residual, and ECHILD records ownership loss separately.

The [k107 report](../../experiments/exec-reliability/evidence/worker-first-reap-k107/report.md)
retains the failing baseline, 42 new real-child controls, three rejected
mutations, a passing 754-case suite and configured static checks. Its audit
verifies 408 exact-child fixture cleanups and 163,383 unchanged earlier evidence
paths. Injected errno and delayed returns are simulated; fixture signals,
exits and exact-child status consumption are real. Read the separate k107
review before relying on this boundary. Complete-worker-scheduler-k114 retains
the complete original aggregate charter, shared scheduling, shutdown/teardown
overlap, remaining controls and final k28 review. k28/k17 remain open and
ten-clone acceptance stays **0/10, unavailable**.

## Worker reap scope clarification (k112)

The k111 review identified an untested lifetime boundary in k107. The counter
includes successful-request polling. A worker that exits after its last poll
and reaches close after its cutoff retains a failed residual without a final
poll, even when its status is waitable. The specification now states this
limitation explicitly. k114 owns any scheduler redesign needed to improve it
while preserving the same-cutoff guard. The [k112 report](../../experiments/exec-reliability/evidence/worker-reap-scope-k112/report.md)
records a real request/late-exit/expired-close control and its scope mutant.
All prior k107 measurements remain historical evidence; k28/k17 remain open
and ten-clone acceptance remains **0/10, unavailable**.

## Incremental worker shutdown (k110)

Worker shutdown exposes `begin_close()` to freeze the original authority and
request EOF without polling or waiting, and `advance_close()` for one
nonblocking owner turn. Each turn makes at most one exact-child reap attempt,
sends due worker escalation and returns an absolute next wake or terminal
`None`. Existing synchronous `close()` drives those same operations. Repeated
calls do not renew earlier cutoffs, signal attempts or the owner-lifetime
first-reap exception. Shutdown phase/reserve formulas and strict timing receipts
are preserved. The controller does not yet interleave this API with teardown.

The [k110 report](../../experiments/exec-reliability/evidence/incremental-shutdown-k110/report.md)
retains 42 expected pre-implementation failures, 151 passing focused controls,
a frozen 796/797 full result, a subsequent 797/797 required self-check and
238/238 review controls. Static checks pass. The frozen failure records a real
`30416 30416 ?E` ps row: strict parsing keeps group evidence unknown and fails
closed. Its signal ordering, leader reap and worker close succeed. Why ps
emitted that state and whether it repeats remain unresolved; the subsequent
pass does not repair the first sample. The audit verifies case identities,
453 named exact-child cleanup receipts and 167,784 unchanged prior evidence
paths. Deferred status, delayed returns and denied history are explicitly
simulated; real UID EPERM and setuid ps KILL are not established.

The independent k113 review follows this bounded slice. The full original
aggregate charter is preserved in complete-worker-scheduler-k114, including
shared safety-first scheduling, private-loop removal, actual teardown overlap,
remaining controls, repeatability limitations and final k28 review. k28/k17
remain open; ten-clone acceptance remains **0/10, unavailable**.

## Fork-failure channel EOF ordering (k116)

A timely valid helper fork failure could become unknown when the result pipe
closed first. The main loop rejected its empty result frame; the existing
bounded cleanup read then saw control EOF without recording it. Recording that
already observed EOF preserves the failure errno and no-child classification
when both channels closed cleanly. Late, partial, contradictory and open-channel
data retain uncertainty. The repair changes no deadlines, read caps or signals.

The [k116 report](../../experiments/exec-reliability/evidence/fork-channel-k116/report.md)
retains deterministic real-pipe RED evidence for each EOF order, 37 passing
focused controls, and a rejected removal of the repair. Two declared interleaved
full-suite pairs use genuinely different worker bytes: both candidates pass
854/854 tests and static checks; both baselines fail the result-first case.
One baseline also fails the existing blocking deferred-reap timing control,
with a probe 4.944 ms past its cutoff and successful exact-child fixture cleanup.
That failure remains unresolved; subsequent passes do not repair it. Helper
fork refusal and terminal-result loss are injected; worker transport, exit and
reaping are real. These observations do not identify the order in earlier
untraced lost-result samples.

The producer has a separate independent review. The complete original aggregate
charter remains in complete-worker-scheduler-k119: shared scheduling, removal of
private waits, teardown overlap, all retained timing/setup failures, comparisons
against the actual scheduler and final k28 review. k28/k17 remain open and
ten-clone acceptance remains **0/10, unavailable**.

## Incremental workload-leader reaping (k119)

The controller now exposes one exact-child reap attempt per nonblocking turn,
with an immutable cutoff and terminal idempotence. Its existing synchronous
consumer sleeps outside those turns. The retained [k119 comparison](../../experiments/exec-reliability/evidence/leader-turns-k119/report.md)
contains 22 passing focused controls, two rejected mutations, and one full
860-test/static pass, with all prior evidence preserved. The full aggregate
contract remains in complete-worker-scheduler-k122; shared scheduling and actual
teardown overlap are unfinished. Historical failures remain binding.

## Incremental active-workload waiting (k122)

The controller exposes one non-reaping workload exit observation per turn, with an immutable cutoff, deadline-capped wake and inert terminal turns. Its synchronous compatibility consumer sleeps outside those turns. The [k122 report](../../experiments/exec-reliability/evidence/workload-turns-k122/report.md) retains the RED baseline, first candidate sleep-rounding failure, corrected 18-control pass, six rejected mutations and one complete 866-test/static pass. All 60 new exact-child journals report absence; 193,709 prior evidence paths are unchanged. The full original aggregate charter and unresolved historical failures now belong to complete-worker-scheduler-k124. Shared scheduling and real teardown overlap remain unfinished, with separate seam review before continuation and final aggregate review still required. k28/k17 remain open; ten-clone acceptance is **0/10, unavailable**.

## Readiness wake dispatch (k132)

The common driver now accepts optional read/write interests and wakes bounded request and failure-retirement callbacks on pipe readiness, while rechecking due safety work and preserving owner deadlines and caps. The [k132 report](../../experiments/exec-reliability/evidence/readiness-wakes-k132/report.md) records nine added driver cases, two 106-case focused passes, six mutations rejected against both source conditions, and two final candidate runtime passes of 896 tests. Corrected frozen-parent and frozen-candidate Ruff, format and strict Pyright checks pass. Original inference failures and a later evidence-auditor import failure remain retained with the verified repairs.

The comparison gate remains rejected: one original parent suite has three workload timing failures, and a final parent suite reproduces the earlier expired-request `residual_remedy` None failure. Clean candidate runs do not resolve those observations. The auditor verifies all four pairs item by item, unchanged subject/artifact hashes, 94 recorded exact-fixture absence audits and all 232,416 prior evidence paths. This delivers a readiness seam; production owners still run sequentially. Independent review `complete-worker-scheduler-k135` precedes the intact aggregate continuation `complete-worker-scheduler-k134`, which owns shared production scheduling, actual teardown overlap, all outstanding controls/failures and final k28 review. k28/k17 remain open and ten-clone acceptance remains **0/10, unavailable**.
