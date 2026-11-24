# Review of the macOS disconnect availability diagnostics

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

Review `macos-disconnect-availability-k202` inspects producer
`macos-disconnect-availability-k13`, commit
`308c3c4efecc058eadbe5dbfc2be535209dfe5fe`. Four findings require triage before
`lifecycle-evidence-k8` consumes the report's design inputs.

This was an inspection-only review of source, the committed diff, tests as text,
and retained `host-01` / `live-01` receipts. No diagnostic, test, build, lint or
format command was run. Codebase Memory refused to start because a
pre-coordination or unverified generation was active; source reads supplied the
evidence, and no graph coverage claim is made.

## P1 — Command substring matching permits kills of unowned processes

Anchors: `experiments/exec-reliability/disconnect_k13_host.py:115–124` and
`experiments/exec-reliability/disconnect_k13_seam.py:197–208`.

The build audit selects every same-user process whose command contains the
temporary build path or a case token, then sends SIGKILL after rechecking its
start time and command. The seam sweep uses the same rule with its token.
Rechecking identity prevents some PID races but does not establish ownership.
For example, an independently launched `tail -f <build-directory>/build.log`
matches the build audit and is killed despite never being created by the run.
A shell observing a case token has the corresponding seam-sweep exposure.

This violates the root brief's requirement to clean up positively identified
owned processes. The retained sweeps are empty, so the receipts do not show an
actual unowned kill. The defect is in what a future cleanup is authorized to
kill; a private path in an observer's arguments cannot grant that authority.

## P2 — Host polling can make a request held until handler exit look responsive

Anchors: `experiments/exec-reliability/disconnect_k13_seam.py:473–479` and
`experiments/exec-reliability/disconnect_k13_check.py:274–275,311–316`.

`run_case` stops polling the shell and child while each synchronous follow-up
runs. It polls again only after receiving the answer. `Watch` records the time
it observes disappearance, rather than the actual exit time. The checker then
accepts `answered < handler_end`, and obtains reference latencies from expected
concurrent replies without establishing that those replies were responsive.

Consider health requests held from 2 seconds until the shell exits at 20 seconds
in both the concurrent and disconnected cases. Each health answer is stamped
before the next process poll stamps disappearance. Both replies contain valid
health JSON, and each latency is about 18 seconds, so the disconnected reply is
within the reference plus 5 seconds. It is classified responsive even though it
was blocked for the entire remaining handler lifetime. The concurrent case can
also validate itself against its own latency. The 30-second host follow bound
allows this timeline.

The current HTTP receipts answer health in 0.0016–0.0059 seconds and ordinary
exec in 0.0676–0.0729 seconds; this finding does not reverse those observations.
It means the instrument cannot reliably falsify the blocked condition it was
commissioned to distinguish. The oracle tests supply disappearance timestamps
independently of the collector and therefore do not cover this observation gap.

## P2 — The live inspector cannot distinguish a reused PID from its payload

Anchors: `experiments/exec-reliability/disconnect_k13_burst.py:84–108` and
`experiments/exec-reliability/disconnect_k13_check.py:397–427,502–503,522–525`.

The inspector reads a numeric child PID and caches its current parent in
`shell.of.<child-pid>`. Neither record contains a process start identity. Later
inspections use `kill -0` and current `ps` output; the parser only checks the
current parent relation. Naming the cache by child PID prevents an ordinary
different-PID stale record, but does not prevent PID reuse.

If the recorded child exits and its PID is reused before the first inspection,
the inspector can cache an unrelated process's parent as the measured shell.
If reuse occurs after a correct first inspection, a later unrelated `sleep 120`
at that PID is accepted as `survived-handler`. A reused cached shell PID is
enough for `live_handler` to return `overran`, without validating its command.
These are concrete wrong-process lifecycle verdicts under the task's explicit
PID-reuse acceptance check. The current receipts show the expected payload shell
and child commands; no actual reuse is established in this run.

## P2 — Late absence does not prove child termination with the handler

Anchors: `experiments/exec-reliability/disconnect_k13_check.py:510–525` and
`docs/experiments/exec-reliability-macos-disconnect-availability.md:178–179`.

For concurrent and term modes, a shell present near 18 seconds and absent by
30 seconds is classified `deadline` without an upper bound near the 20-second
deadline. An absent child at the late sample is classified
`ended-with-handler`, without comparing its end to the handler's end. The report
then uses these labels to assert that the deadline already ends both live
children and that they ended with the handler.

The term receipt observes shell 918 and child 920 present during
18.005583–18.092285 seconds and both absent during 30.004036–30.122540 seconds.
Concurrent shell 760 and child 762 have the same sampling pattern. A shell ending
at 20 seconds followed by its child ending at 25 seconds produces these same
samples, but the child survived the handler for 5 seconds. A term shell itself
ending at 25 seconds is also compatible with its bracket. The report acknowledges
the latter uncertainty earlier, but the design-input claim still asserts the
stronger conclusion. This matters to the downstream lifecycle design, which must
not treat absence at 30 seconds as proof of deadline cleanup or simultaneous
termination.

## Evidence reconciliation

The loopback copy preserves the shipped `ProcessRunner.swift` and
`AgentServer.swift` bytes before building; its changed entry point binds the real
router to loopback. No mismatch was found in that seam.

Both checker modes recognize complete ordinary output reported as a timeout.
The host concurrent ordinary request retains `ordinary-ok`, guest exit -1 and
`timed_out: true` at 10.065137 seconds. It supplies no sound ordinary reference,
so host term ordinary remains unknown. Live cleanup likewise prints
`cleanup confirmed` while reporting a 9-second guest timeout; the report retains
that anomaly. These outcomes are not silently converted to successful latency
references. The original host judgement remains in `check-01.json`, SHA-256
`1da69d92c6f848a8dfdf02d285071b35ed2b3d6ea003b9f702cfdcf353a4e95d`.

Live term exits the local CLI by SIGTERM at 2.010089 seconds. Its follow-ups finish
by 2.281191 seconds, well before the shell is observed alive near 18 seconds.
All three download receipts report 8 bytes and retain hex `7061727469616c0a`.
These receipts support availability and exact partial-file transfer independently
of the lifecycle overstatement above.

The resource receipts match the report: acquire 15.314568 seconds; health before
0.084061 and after 0.072305; cleanup 9.117497 within its 26-second controller
budget; disposal 1.114373 within 41 seconds; post-disposal inspection 0.149308.
The clone is absent afterward, app PID 53925 is unchanged, tooling digests match,
and the controller reports no residuals. Host servers stop by SIGTERM, with empty
sweeps and removed temporary roots/build directory. These retained results do
not establish that the cleanup selection rules are safe for unowned observers.
