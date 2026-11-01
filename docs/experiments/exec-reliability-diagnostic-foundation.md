# Execution reliability diagnostic foundation

The local diagnostic controller passes its forced-failure self-check on a
macOS host. This establishes an instrument for the causal inquiries in
[issue #14](https://github.com/Linkuistics/TestAnyware/issues/14), not a repair
of TestAnyware. No guest was booted, no guest command was run, and no app was
started or stopped. Live guest routes remain unavailable under the conditions
listed below. The historical acceptance result remains **0/10, unavailable**.

## Use and boundaries

From the repository root:

```sh
mise exec -- task diagnostic:self-check RUN=self-check-new-condition
mise exec -- task diagnostic:preflight OUT=experiments/exec-reliability/evidence/preflight-new-condition
```

The self-check uses the locked Python project in
[`experiments/exec-reliability`](../../experiments/exec-reliability/).
Its [`controller.py`](../../experiments/exec-reliability/controller.py) exposes
immutable `Call`, `Resource` and `Plan` declarations and `execute(plan, evidence_root)`.
The evidence directory is created exclusively from a fresh `uuid.uuid4().hex`.
An existing directory is rejected. Calls have unique names and explicit absolute
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

The controller creates a separate process session for each command. A call
deadline signals its recorded local process group with TERM, grants at most
two seconds, then sends KILL and bounds leader reaping by at most two seconds.
`interrupt_after` instead signals **only the command CLI PID**, matching the
issue's abrupt-interruption comparison. The outer controller survives. This
does not send a guest cancellation request or contain detached descendants.
macOS kqueue observes exit before reaping so the leader identity is retained
until signals finish. Observation errors fail the call and still attempt
bounded termination/reaping.

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

The first failed case stops the declared workload and later cases are listed
as unreached. Safety cleanup still runs. Controller crash, disk failure,
uninterruptible kernel operations, arbitrary descendant containment and guest
cleanup correctness are outside this instrument's demonstrated guarantee.
Normal local command completion alone proves neither absence of local helper
descendants nor guest process cleanup. Later fixtures must positively inspect
their known child identities. Mock disposal is not evidence of live VM disposal.

## Forced-failure evidence

`mise exec -- task diagnostic:self-check RUN=self-check-final` passed **13 tests**
on 1 November 2026, including Ruff lint/format and strict Pyright. The interpreter
was CPython 3.12.15; the lock selects pytest 9.2.0, Ruff 0.17.3 and Pyright
1.1.414. Raw declarations, streams and receipts are retained in
[`self-check-final`](../../experiments/exec-reliability/evidence/self-check-final/).
The [item index](../../experiments/exec-reliability/evidence/self-check-index.json)
also includes the earlier 12-test condition under `self-check/`; those receipts
describe their own earlier source digest and are not relabelled as the final build.

The rows below name final-condition run directories. Their `result.json` may
correctly say `ok: false`: the **test passes because the forced failure was
detected**, not because that command succeeded.

| Control | Run directory under `self-check-final/` | Observation |
| --- | --- | --- |
| Exact binary stdout and separate stderr | `b744181d8c2d42309bc146c6aeb2f7a5` | `00ff80410a` and `separate stderr\n` preserved exactly; subject hashes equal |
| Overlong call | `30fe9008e3954abc97d23ec8cf9e5957` | Deadline, exit -15, partial stdout retained; next case unreached; 0.352 s |
| CLI ignores TERM | `b22264e5d64f49099e3045929ea7db5e` | PID-only TERM then KILL, exit -9, reaped; controller survived; 0.460 s |
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

| Guest OS and route | Established fact | Live condition unavailable / next action |
| --- | --- | --- |
| macOS, unchanged installed CLI/app/agent | Installed CLI/app identities and stopped `testanyware-golden-macos-27` selector found | No owned clone or fresh agent identity/health. Declare boot/slot/export bounds, ledger a new ID, start a clone through `vm start --platform macos --base testanyware-golden-macos-27 --id <run-id> --json`, then one health/identity probe by VM ID. Preserve the stopped golden. |
| Linux, unchanged installed CLI/app/agent | Same installed host route; stopped `testanyware-golden-linux-24.04` found | Same missing owned clone and fresh agent proof. Use `--platform linux --base testanyware-golden-linux-24.04` with a new ID; verify the selected backend and answering source hashes. |
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
| Final diagnostic controller | `907f73196a76588b43825eec365e6232247705794df8f36e8531b65e6f3503b4` |
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
returned exit 1 and empty stdout, so none of those identities was present at
audit time. It removed the remaining fake-VM marker
`diag-444a6cccc6b546599ef421539c5a01a9-vm` and positively checked absence.
Original uncertain-disposal receipts remain unchanged. Earlier pytest-owned
scratch directories may already have been pruned by pytest; this is not live
VM disposal. The diagnostic venv and retained raw evidence are intentional
workspace artifacts. No VM, image, archive or test app was acquired by this
leaf, so there is no live VM/image/app teardown to report.

The graph supplied Tier 2 discovery for the app, CLI and golden provisioning
boundaries. Coverage checks found no recorded parse gaps on the relied-on
source paths; this is a best-effort signal, not a completeness proof. Changed
Taskfile and untracked lockfile metadata were checked directly. Relevant source
is in `testanyware-cli/src/{main,resolve,commands/vm}.rs`,
`testanyware-app/src/main.rs`, `testanyware-vm/src/golden_linux.rs`,
`scripts/build-app.sh`, and the two `vm:create-golden-*` Taskfile entries.

The next macOS inquiry can scope the unsafe process-test cleanup and run
owned local process/HTTP controls. The unchanged live macOS/Linux inquiries
have an identified installed host route but must still create their bounded
owned clone and verify fresh answering identities. Checkout agent/app live
conditions cannot be claimed until the table's build/install/grant facts are
established. If that preparation requires its own session, insert it before
measurement; do not absorb an unbounded setup into a causal sample. The
controller's scheduled implementation review precedes these consumers.
