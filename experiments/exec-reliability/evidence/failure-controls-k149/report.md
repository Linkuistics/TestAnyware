# Diagnostic failure controls (k149)

This leaf classifies every retained process-control failure that blocked k17.
It repairs the demonstrated defects and runs the actual setuid `ps` KILL control.
It also runs the real UID-denial control with a separately authorised owner. Per-case verdicts are in [verdicts.json](verdicts.json).

All runs use Python 3.12 on macOS with 16 cores. Each condition has its own fresh
directory and a frozen input set made by `prepare.py`. The tools hash the frozen
subjects before and after each run. Every recorded hash pair is unchanged.

## Classification summary

| Class | Cases |
|---|---|
| Runtime defect, repaired here | k110 exiting `?E` table |
| Runtime defect, repaired earlier | k114 lost-result frame (k116 EOF order) |
| Fixture defect, repaired here | admission timing (k121, k139, k116, k129), late-created identity (11 failures) |
| Fixture defect, repaired earlier | k145 and k132 interrupted live group (k143), expired-request `None` (k137), k104 and k114 assertions, setup tooling |
| Unavoidable missed scheduling bound | k132 exited-leader grace, k106 prebuffer, helper capture deadline (eight observation groups), k85 empty PID, k77 checker ceiling |

No bound was raised to hide a stall. When a bound was missed, the runtime failed
closed. It kept the failed timing, the uncertain inspection and the separate
worker, helper and workload ownership.

## Admission timing (fixture defect)

Four historical failures report a probe 20 µs to 4.944 ms after the cutoff. The
runtime `_reap` and `_signal` read `now` and compare it with the cutoff. Only
then do they call the syscall. The tests timestamped the syscall entry inside
their wrapper, so a scheduler delay after admission looked like a late
admission.

`admission_timing.py` drives the real `_reap` against an owned, paused child. It
delays the call by 500 µs after the clock read. Against each historical frozen
input and the final inputs, the delayed call executes late but is admitted early.
The `reap-ignores-cutoff` mutant admits late. Each run kills and reaps its child,
then checks for `ECHILD`.

The tests now assert admission time, which they read from the runtime frame, and
they retain execution time as evidence. Each syscall must also follow its
admission by less than 50 ms, so a stale admission clock fails. The committed
inputs pass 94 timing cases. The four mutants fail as follows:

| Mutant | Failed of 94 |
|---|---|
| `reap-ignores-cutoff` | 22 |
| `signal-ignores-cutoff` | 8 |
| `stale-reap-admission` (clock 1 s old in `_reap`) | 88 |
| `stale-signal-admission` (clock 1 s old in `_signal`) | 70 |

## Interrupted live group (fixture defect, k143 repair)

The k145 and k132 receipts show PID TERM, PID KILL and leader return code −9.
The frozen peer ignored TERM until its child declared readiness.
`scope_readiness.py` declares readiness at set offsets against the k145 peer
and the final peer. With the k145 peer, early readiness gives return code 0. A
missing declaration reproduces −9 with PID KILL.

The final peer returns 0 in
every condition. All children are absent afterwards.

## Late-created identity (fixture defect)

Eleven failures in k87, k94, k106 and k140 share one mechanism. The worker wrote
its witness file after TERM, inside the runtime's 13.5 ms KILL window. Every
failure left a partial `late-created-shutdown-*.tmp` file. The worker now
records its TERM state in an inherited anonymous `mmap`. The parent publishes
the witness after capture.

The old fixture fails 13 cases with `KeyError: 'after_term'` when a 30 ms stall
follows the late write. The repaired fixture passes 27 cases with and without
the stall. The `late-identity-dropped` runtime mutant still fails 14 cases.

## Exiting group table (runtime defect)

k110's final table showed the leader as `?E` 2.8 ms after its observed exit. The
parser rejected the state, so termination failed. Apple's `ps` prints `E` for
`P_WEXIT` before `SZOMB` ([print.c](https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/print.c)).
It prints `?` when it reads no thread state ([tasks.c](https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/tasks.c)).
Kqueue refuses a `NOTE_EXIT` registration with `ESRCH` from exit start. The
runtime counts that refusal as exit.

A 2 GB leader keeps `?E` for about 10 ms of teardown. `exiting_leader.py` runs
such leaders through the real controller. Before the repair, 2 of 10 tables were
`?E` and both runs failed as unknown. After the repair, 8 of 10 tables were `?Es`
and all 10 runs passed. The parser now accepts `?` only with `E`, and it counts
each `E` member as non-executable. The full suite passes 975 tests on the
committed frozen inputs.

## Lost-result frame (runtime defect, k116 repair)

k114's lost-result case reported `incomplete worker frame length`. The
`omit-cleanup-eof` mutant removes k116's EOF record and reproduces the exact
message. The final inputs pass all three EOF-order cases.

## Missed scheduling bounds

The k132 exited-leader test needs leader exit within the 150 ms TERM grace. The
historical exits took 158 ms and 168 ms. `exit_latency.py` measures 33–50 ms,
also with 48 busy processes. The cause of the historical stall is unidentified.

Eight observation groups show a real `ps` helper past its 128 ms capture
deadline. The latency control ran 340 group queries, and none exceeded 128 ms.
The idle median was 1.85 milliseconds.
The maximum was 62.1 milliseconds, with 16 busy processes.
In each historical case, the runtime killed the helper and failed closed.

k85's empty-PID errors occurred because TERM at 0.3 s arrived before the peer
printed its child PID. The `peer-startup-stall` mutant reproduces the exact
error on the final inputs. The fixture keeps this 0.3 s startup dependency.

## Real controls

**Setuid `ps` KILL: pass.** `setuid_ps_kill.py` drives the real capture path
against a `/bin/ps` table larger than the 256 KiB cap. The runtime kills its own
unreaped child. In 10 trials, the child ran with UID 0 and real UID 501. The
live window before KILL was 4.9–6.7 ms. Reap followed 11.8–13.8 ms later with
return code −9. The `suppress-kill` mutant fails only the KILL checks.

**Real UID denial: pass.** A fixture with foreign real, effective and saved UIDs
needs a separately authorised privileged owner. This leaf does not acquire
privilege. The human ran `foreign_owner.py` with `sudo` on 2026-11-16. The owner
started `/bin/sleep` as PID 43433 in its own group, with every UID set to
`nobody`. Its receipt is `foreign-owner/owner-receipt.json`.

`foreign_denial.py` drove the real controller from UID 501 against the
committed inputs. In group scope, TERM and KILL both failed with EPERM. No exit
was observed, so the final table was missing and group KILL was
`permission_unknown`. In PID scope, both signals failed with EPERM as
`signal_error`. Both terminations failed, and the fixture identity did not
change. All 16 checks in `foreign-denial/foreign-denial.json` are true.

The owner then sent SIGKILL and reaped PID 43433 by its exact PID with signal 9.
Afterwards, `ps` found no such process, and the owner had no children. The
owned plumbing check in `foreign-denial-self` fails as designed, because its
signals succeed.

## Independent review

An independent reviewer read the runtime and test diff against xnu and adv_cmds.
It accepted the `?E` parser and the late-identity fixture. It asked for three
changes, which this leaf applied:

- The parser comment now says that kqueue reports `ESRCH` once `proc_find`
  fails. That happens after the last thread ends and before `SZOMB`.
- The admission assertions now bound the gap between admission and syscall.
  Without that bound, a runtime that read a stale clock passed.
- The final-reap test adapter now branches on admission time. Before, a
  scheduler delay could let it reap early.

The reviewed inputs pass 94 timing cases and 27 late-identity cases. The first
full run in `full-reviewed` missed 5 bootstrap timing bounds by 80 to 129 milliseconds. An
external `jj` process used about 11 cores at that moment, with a load average
of 21.3. This leaf counts that run as load, and it changed no bound. The rerun in
`full-reviewed-2` passes 975 tests. `inputs-committed` adds only one Ruff blank
line in two test files, and it passes the same three runs.

## Preservation and tooling

`preserve.py` recorded 311,586 earlier evidence paths before and after the work,
and found no change. `prior-after-1.json` is the first after-snapshot. `prior-after.json` is the final one. The Taskfile tasks named `diagnostic:*` in this report rerun
every condition. `diagnostic:failure-tools` passes Ruff, formatting and strict
pyright on the evidence tools. The three malformed exit-latency outputs in
`malformed-exit-latency/` ran against live sources, so they are not evidence.
