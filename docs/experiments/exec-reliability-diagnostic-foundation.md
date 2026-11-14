# Execution reliability diagnostic foundation

The local diagnostic controller passes its revised forced-failure self-check on a
macOS host. It now observes graceful leader exit, distinguishes PID and group
interruption with real child controls, and records group inspection before
reaping. Review integration corrects false success with live group members,
fixture cleanup and checker timeout reporting. Initial fork admission now rejects
extra Python or native threads, changed Python or kernel SIGCHLD disposition and
unavailable native observations before acquisition, retaining failed results.
Review integration k32 independently reproduced and repaired the Python signal
cache gap. The current measured condition is k47-final-01: workload leaders
are reaped with exact-PID nonblocking waits before a fixed deadline; failed
reaps retain an explicit owner and residual without destructor polling.
The controller now requests at most one final process table, after the final
signal and observed leader exit. Missing exit evidence leaves inspection
unknown without a query; a live or unknown final table cannot trigger a retry.
It preserves k37's strict process-table acceptance, k33's KILL ordering and
k36's PID-only deadline cap. The
complete worker/scheduler boundary remains k30's work, continued by k34's
worker-scheduler-k38 subtree, continued by worker-scheduler-k40 and its
worker-scheduler-k42 subtree, worker-scheduler-k44 subtree and remaining
worker-scheduler-k48 leaf.
A failed local inspection now remains failed throughout that execution.
Independent cleanup, disposal and public absence commands still run, but later
local observations cannot restore disposal certainty.
Independently supervised process inspection, retained-file identity,
stalled-storage disposal and live route
preparation still need the work named below before causal use. In particular,
helper creation remains synchronous and can still block disposal. Once created,
the helper now has bounded pipe capture and nonblocking reap attempts, with a
strong residual owner on failure; controller-local scheduling remains interim.
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
