# Linux timeout lifecycle implementation review

Review handle: `linux-timeout-lifecycle-k205`.
Producer: `linux-timeout-lifecycle-k5`, commit
`5045154d03692231fabd682c1f302f69a30edcbd`.
Measured source freeze: `5e828cb6da55afe266064b81a022aa63472da697`.

The retained container evidence supports the reported exec defects. Three
material gaps remain in cleanup ownership, measurement provenance and evidence
replay. They need disposition before the evidence consumer treats this as a
fully accepted measurement. This review makes no production-code changes.

## Findings

### P2 — The identity check does not make the cleanup signal safe against PID reuse

Anchor: `experiments/exec-reliability/linux_k5_seam.py:259–267`.

`kill_here` takes one `/proc` snapshot, compares a target's start ticks with that
snapshot, then calls `os.kill(pid, signal.SIGKILL)`. A target can exit after the
snapshot and its PID can be assigned to another process before the signal. The
numeric PID then addresses the replacement, despite the successful start-tick
comparison. A process that the case did not create can be killed. The snapshot
also ages while preceding targets are processed.

The reuse control in `experiments/exec-reliability/test_linux_k5.py:86` supplies
a replacement identity before the snapshot. It checks that earlier reuse is
rejected; it does not cover the check-to-signal interval. The ownership claim in
`docs/experiments/exec-reliability-linux-timeout-lifecycle.md:76–85` is therefore
stronger than the implementation guarantees. A stable process handle, with its
identity verified before signalling through that handle, would address this
specific race. Another `/proc` read followed by numeric `kill` leaves a window.

The container boundary limits the reach of this issue. The retained run shows
no reused PID or wrong-process kill; this is an unmet ownership guarantee, not
an allegation that such a kill occurred in run-03.

### P2 — Imported measurement inputs lack the required before/after hashes

Anchor: `experiments/exec-reliability/linux_k5.py:174–194`.

The plan's `subjects()` lists the agent's `server.py` and `system_endpoints.py`,
but omits the other imported agent modules and all three accessibility stubs.
`sources()` inventories those files only before execution. In run-03,
`subjects_before.json` and `result.json`'s `subjects_after` do not cover:

- `agents/linux/testanyware_agent/{__init__,__main__,accessibility,models,query_resolver,role_mapper,tree_walker}.py`;
- `experiments/exec-reliability/linux_k5_stub/gi/__init__.py`;
- `experiments/exec-reliability/linux_k5_stub/gi/repository/__init__.py`;
- `experiments/exec-reliability/linux_k5_stub/pyatspi/__init__.py`.

The checker also imports `experiments/exec-reliability/outcomes_k12_check.py`,
which is absent from both subject hashes and the source inventory. The brief's
measurement contract requires every read subject to be hashed before and after
the run (`.grove/_BRIEF.md:295–305`). A read-only container mount prevents guest
writes but does not prevent a host edit from changing a mounted input during
measurement. Such an edit can escape the current provenance check.

The initial inventory is useful evidence and the stubs are narrow import
substitutes; neither supplies the missing post-run hashes. This review found no
evidence that these inputs actually changed. The existing run cannot acquire
contemporaneous after-hashes retrospectively: retain its limits when deciding
whether a newly frozen measurement is required.

### P2 — Evidence replay follows the original worktree instead of the supplied directory

Anchor: `experiments/exec-reliability/linux_k5_check.py:453–463`.

`load(out)` opens `out/summary.json`, then takes the raw-evidence directory from
the absolute `summary["run"]` path. The committed run-03 value points into
`/Users/antony/Development/AbstractingComputers/TestAnyware.gh-issue-14/`.

If another checkout or a copied evidence directory contains the complete
run-03, checking that directory still reads the original worktree's raw files
when they exist. If they do not exist, loading fails despite the supplied
directory containing the retained evidence. This affects both checker modes
and prevents the retained bundle from serving as an independent replay source.
Resolve the selected run within the supplied evidence bundle while retaining
the original absolute path as provenance metadata.

## Report precision

P3, `docs/experiments/exec-reliability-linux-timeout-lifecycle.md:132,149,156,245`:
the reference deadline's raw timestamps put TERM at 2.000837292 seconds and end
at 2.003695334 seconds. The reported 2.004 seconds is the end time, not the TERM
time. Also, the trace wrapper delegates the original arguments and results, but
one separate run per seam does not establish that its timing effect is only
microseconds. Describe that overhead as unmeasured. These points do not change
the observed timeout and survivor defects.

## Evidence reconciliation and limits

The inspected evidence is under
`experiments/exec-reliability/evidence/linux-timeout-lifecycle-k5/run-03/`, with
raw records in `b9ecb57852d04eb1a11c41a8a02a5577/`. Inspection of the frozen
`REQUIRED` declaration against the retained `check.json` finds all 63 required
classifications among 66 records: 11 cases across six seams. Reading both
checker branches confirms that normal mode checks measurement soundness and
`--expect-defects` additionally requires those classifications. Old-path
defects are intentionally accepted by normal mode.

Raw stream bytes support the newline stripping, binary decode failure and
partial-output loss. The five old-path fork/wait records show a live child at
their late observation; recorded cleanup takes approximately 0.106–0.378
seconds. Those observations occur at or after the faithful bound, rather than
at exactly six seconds. Host durations use host timestamps and container
durations use container timestamps; no cross-clock subtraction is needed.

The spawn-failure containers run the agent as PID 1 under UID 1001, reach health
before the limit is applied, and retain `RLIMIT_NPROC` changing to `(1, 1)`.
The single-threaded HTTP handler then reports EAGAIN from command spawn. Root
observers are outside that UID's limit. The stubs do not replace the exec
handler. These records support the intended spawn-failure interpretation.

The reference timeout records support TERM near two seconds, and the ignored
TERM case supports KILL after a further 2.001883418 seconds. Final inside
snapshots retain only the container init. The host record shows the app
terminating with status 0, container disposal returning 0 in 0.8378577079856768
seconds, a successful empty labelled-container query, and removal of the work
directory. This remains container evidence; it establishes no live-VM repair.

This was an inspection-only review of committed source and retained evidence.
No tests, checker runs, build, lint, formatting or new measurements were run.
Read-only JSON and AST inspection reconciled the retained classifications and
timestamps. Graph coverage could not be verified: the CBM CLI refused to start
because a pre-coordination or unverified generation was active. Direct source
inspection supplied the review evidence.
