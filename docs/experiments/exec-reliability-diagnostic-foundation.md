# Execution reliability diagnostic foundation

The local diagnostic controller passes its revised forced-failure self-check on a
macOS host. It now observes graceful leader exit, distinguishes PID and group
interruption with real child controls, and records group inspection before
reaping. Review integration corrects false success with live group members,
fixture cleanup and checker timeout reporting. Independently supervised process
inspection, retained-file identity, stalled-storage disposal and live route
preparation still need the work named below before causal use. In particular,
the present `ps` helper can outlive its timeout and block disposal.
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
automatically hashes itself, the interpreter and explicit command executables;
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

TERM now records a monotonic leader-exit observation. PID-only interruption
skips KILL when the leader has exited; group deadlines still escalate when
executable members remain after leader exit. Each group snapshot retains its
monotonic start/end, raw `ps` stdout/stderr (hex in JSON), status, member
PID/PGID/state and signal-0 result. The leader stays unreaped through every
snapshot and signal. Only a clean table containing that leader can classify
the observed group as live or zombie-only. A missing leader, usage error,
timeout, malformed table or other failed inspection stays unknown and fails
the call. EPERM with observed live members is permission failure; observed
zombie-only EPERM is distinct; EPERM with failed inspection remains unknown.
These observations do not establish the kernel's historical cause for an
earlier EPERM or containment of arbitrary descendants.

The group poll currently reserves a 10 ms sampling window **inside** the declared phase;
kqueue polling continues to the actual deadline. This reserve is shorter than
some retained `ps` observations, so it does not prevent false instrument failures.
Do not interpret the passing controls as a proved inspection bound. KILL, further inspection and
leader reaping share one deadline, capped by the original TERM deadline plus
`kill_wait`. The receipt's `within_process_bounds` must be true for success.
There is no fresh reap allowance after inspection. Snapshots describe only
their recorded intervals, and group probes stop before reaping permits identity
reuse. The known-child fixtures perform separate exact-PID observations and
cooperative cleanup. A live final group now fails termination after normal exit
or PID-only interruption too, retaining observed members, PGID and a remedy.
PID-only interruption still sends no group signal. The caller must independently
revalidate and clean known children; no group signal is safe after leader reap.
Normal exit can still leave a writer: stream/result digests and demonstrated
late-write rejection remain the next increment.

The timing statements above are conditional on process creation and inspection
returning. Python 3.12's `subprocess.run(timeout=...)` kills a timed-out helper
and then waits without a timeout. Its setuid `/bin/ps` child therefore remains
an unbounded path to disposal, separate from blocked evidence storage. The
bounded-inspection design and reviewed implementation must resolve this before
causal use; increasing a sampling reserve cannot establish that property.

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
| [k23-final-01](../../experiments/exec-reliability/evidence/k23-final-01/verdict.json) | Delivered source condition: all four checkers and 54 tests pass; failed checker fixture artifacts are now retained with other run artifacts. |

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
