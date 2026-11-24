# Review of the k7 download consistency investigation

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

The retained host run demonstrates a metadata/open race and torn reads. Five
material findings remain before the tooling is reused by
`linux-download-consistency-k14` and its conclusions become inputs to
`lifecycle-evidence-k8`. This review does not select a consistency repair.

The producer reviewed is commit `8d8087fa`,
`download-consistency-k7: locate the macOS boundary that sends a truncated record as complete`.
This was an inspection of source and retained evidence only. No tests, builds,
lint, formatting or diagnostic workloads were run. The graph CLI could not start:
`codebase-memory-mcp: CBM CLI could not start because a pre-coordination or unverified CBM generation is active; close all CBM sessions and commands, then retry`.
Source and evidence were read directly; graph generation and index coverage
could not be verified.

## R1 — P1: the stress checker accepts missing successful reads

Location: `experiments/exec-reliability/download_k7_check.py:185`.
Related: `experiments/exec-reliability/download_k7_check.py:209`,
`experiments/exec-reliability/download_k7_check.py:270`,
`experiments/exec-reliability/download_k7.py:493`.

The immutable branch of `stress_summary` returns `complete: true` whenever
`outcomes` and `publications` exist. It does not require a successful read of
each publication. The controller can exhaust a record's five-second polling
window and move on without recording that missing read as a failed item. For
example, 200 publications with only 404 responses, zero complete reads and a
stopped loopback agent pass the stress completeness check. A nonempty mismatch
list also leaves completeness true. The mutable branch likewise returns true
for empty outcomes or unclassified outcomes and does not reconcile outcome
counts with `reads`.

This matters because the stable publication control is evidence for a design
option, and the checker is intended for reuse. A completed diagnostic that
observes a classified bad download is legitimate; a missing observation is not
evidence of a completed comparison, and completion alone must not imply that
the stable control passed. The current checker cannot enforce that distinction.
The retained run records 200 complete immutable reads per agent and no
mismatches, so this is a defect in the reusable instrument, not evidence that
those recorded totals were false. Individual immutable response bodies are not
retained; those totals can be traced to the controller's aggregates and the 200
published files per agent, rather than independently recomputed from every
received response.

## R2 — P1: the diagnostic has no independent invocation deadline or reliable failure cleanup

Location: `experiments/exec-reliability/download_k7_run.sh:44`.
Related: `experiments/exec-reliability/download_k7.py:191`,
`experiments/exec-reliability/download_k7.py:222`,
`experiments/exec-reliability/download_k7.py:429`,
`experiments/exec-reliability/download_k7.py:464`.

The wrapper waits directly for the build, probe and checker and has no
independent controller deadline or cleanup trap. A socket timeout in
`raw_download` applies separately to each receive. A peer that keeps sending
before each timeout can keep that operation running indefinitely. Worker joins
do not cancel the worker. More directly, an exception from `raw_download` in
mutable stress bypasses `stop.set()` and `thread.join()`: its non-daemon writer
continues replacing the file and prevents Python from exiting. The wrapper then
waits indefinitely and never reaches the resource audit or build removal.

Other failure paths compound the same problem. CLI interruption uses
`communicate(timeout=30)` without a kill-and-reap fallback if that wait expires.
Agent startup and shutdown inspection commands have no timeout. `Agent.stop`
allows ten seconds after TERM and another ten after KILL; a failed `ps`
inspection can be recorded as absence rather than an unknown cleanup result.

The root brief explicitly requires independently bounded whole invocations,
including failed wait/inspection cleanup (`.grove/_BRIEF.md:294`), and specifies
two-second termination and kill/drain/reap budgets. The producer also agreed to
reuse k11's controller and preserve bounded transfer diagnostics. The observed
32.113-second probe and 36 stopped agents show that this invocation returned;
they do not establish those failure bounds. Reuse needs a bounded failure path,
not a larger comparison ceiling.

## R3 — P2: the independent oracle is disconnected from the transferred immutable case

Location: `experiments/exec-reliability/download_k7_check.py:252`.
Related: `experiments/exec-reliability/download_k7_check.py:76`,
`experiments/exec-reliability/download_k7_check.py:99`,
`experiments/exec-reliability/download_k7_check.py:123`.

The checker has an independent constant, `ORACLE_HEX = "00ff80410a"`, but uses it
only to create and compare its own disk copies. Every actual case, including
`immutable-binary`, is classified against the controller's `allowed-*.bin`
files. If the controller supplies a flipped or truncated immutable fixture and
the same bytes as its allowed version, a complete transfer is classified
`allowed-complete` while the independent oracle controls still pass. There is
no connection between the independently specified bytes and that transfer's
acceptance decision.

This fails the producer's agreed independent immutable-byte control. Exact
comparison is present, but the source of the expected bytes is material. The
retained raw and CLI immutable transfers do contain `00ff80410a`; this finding
concerns what the checker would incorrectly certify on reuse, not a corruption
observed in this run.

## R4 — P2: the report attributes the historical guest result more strongly than the evidence permits

Location: `docs/experiments/exec-reliability-download-consistency.md:18`.
Related: `docs/experiments/exec-reliability-download-consistency.md:23`,
`docs/experiments/exec-reliability-download-consistency.md:193`,
`experiments/exec-reliability/download_k7.py:145`,
`experiments/exec-reliability/download_k7_shim.c:95`.

The forced host cases establish a sufficient agent-side mechanism for failure
D's exact bytes. They do not establish which inode/length interleaving occurred
in the historical guest, or what the historical relay received and emitted.
Nevertheless, the result names *the* responsible boundary for that failure and
states that the relay copies the agent's bytes, while acknowledging that this
run did not test the relay. Identical 25-byte prefixes are compatible with the
demonstrated race and with truncation at another point. The direct host route
does not discriminate between those historical possibilities. ADR-0004's relay
contract is architecture evidence, not a measurement of that failed execution.

The unpaused stress strengthens the case that the race does not require the
explicit gate, but remains instrumented: both agents retain the injected shim
and gate environment. Each targeted operation still performs mutex, counter
and gate-file work, and an open performs extra metadata inspection. The
wrappers preserve underlying results and do not fabricate the body; they can
still change timing. Thus this is an unpaused reproduction, not an independent
uninstrumented timing control.

For k8, the supported conclusion is a demonstrated host agent defect capable
of producing failure D, with the historical guest/relay attribution still
unproved. A stronger attribution needs discriminating evidence from that route;
the digest match alone does not supply it. This does not invalidate the host
control or require a full guest campaign merely to establish that host defect.

## R5 — P2: the recorded freeze does not cover the executable and source subjects

Location: `experiments/exec-reliability/download_k7_run.sh:18`.
Related: `experiments/exec-reliability/download_k7_run.sh:59`,
`experiments/exec-reliability/download_k7.py:533`, `.grove/_BRIEF.md:302`.

The before/after digest list covers the diagnostic scripts, Python dependency
files and Taskfile. Agent source, Swift dependency inputs, the Host CLI and
agent executables, and the compiled shim are outside that comparison. Source
identity and `Package.resolved` are captured once; executable hashes are also
captured only before probing. The run therefore cannot detect a changed binary
between the forced cases and stress, or changed source/dependency inputs during
the checkout build. A source commit plus one dirty diff does not establish an
unchanged working tree throughout the measurement.

The root brief requires before/after hashes of every read subject. This gap
limits attribution of the aggregate results to frozen executable/source
identities, which matters before comparing operating systems. The retained
tool lists do match, the dependency lockfile is present, and the recorded
executable hashes agree with the report. There is no evidence that omitted
subjects actually changed; the missing evidence must remain explicit rather
than be treated as a completed freeze.

## Evidence that limits the findings

The calibration and forced-case records support the reported mechanism. The
shipped trace is `stat`, `lstat`, `open`, and selects the first `lstat`; the
checkout trace is `stat`, `stat`, `open`, and selects the second `stat`. In the
replace-longer cases, the selected lookup sees a 25-byte inode and the later
open sees a different 123-byte inode. Raw and CLI received files are exactly
25 bytes with SHA-256
`ca41852314b967a327b0a4faf0240cac2f2e304387439e5518cf380da4b9a56e`.
The raw response advertises 25 bytes and completes; the CLI exits zero with a
25-byte receipt. The four retained historical artifacts match their recorded
digests.

The checkout source matches that sequence:
`agents/macos/Sources/testanyware-agent/AgentServer.swift:507` checks the path
before calling `FileIO.loadFile`;
`agents/macos/.build/checkouts/hummingbird/Sources/Hummingbird/Files/FileIO.swift:49`
gets path metadata and constructs a fixed range;
`agents/macos/.build/checkouts/hummingbird/Sources/Hummingbird/Files/FileIO.swift:135`
constructs the response length before the later path open. The NIO chunk reader
at `agents/macos/.build/checkouts/swift-nio/Sources/_NIOFileSystem/FileChunks.swift:241`
clamps reads to that range. The retained lockfile pins Hummingbird 2.28.1 at
`341e11f65d8e24cf87e9aae427c040f0084ada4e` and swift-nio 2.105.0 at
`2f58d8ef84438d6517fc78bcea2f8527e3c72936`; the inspected checkout revisions agree.
These pins do not establish the shipped agent's dependency source, which the
producer correctly leaves unknown.

The retained mutable stress outcome bodies and their digests reconcile with
the report:

| Agent | Terminal version | Pending version | Wrong complete frame | Premature EOF | Total reads | Replacements |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Shipped | 24,780 | 24,594 | 1,887 | 1,770 | 53,031 | 74,044 |
| Checkout | 21,563 | 21,413 | 2,794 | 2,765 | 48,535 | 74,033 |

All wrong complete bodies have the historical failure D digest. The immutable
aggregates record 200 complete reads each, with 10,264 and 10,266 prepublication
404 responses respectively. The forced overwrite bodies are exactly 131,072
old bytes followed by new bytes, totaling 394,216 bytes. Append returns the
allowed old version. Shorter replacement, truncation and agent termination
remain detected failures: raw EOF before the advertised length and CLI exit
one with `CONNECTION_DROPPED`, no destination and no temporary. CLI SIGTERM
leaves the reported temporary and is classified as a leak. These observations
were not normalized into successful transfers.

All 36 recorded agents report loopback-only listening and absence after stop;
the four deliberately killed agents exit with signal 9. The outer evidence
records successful build/probe/check exits, matching tool digests and build
directory removal. These are observations of the retained run, subject to R2's
failure-path limitations and R5's provenance gap. They do not resolve the
unmeasured guest and relay route in R4.
