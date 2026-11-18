# macos-agent-cancellation-k237

**Reviews:** macos-agent-cancellation-k221

## Goal

Try to break the macOS agent change of macos-agent-cancellation-k221. The
Host CLI leaf host-cli-control-k222 builds on its wire shapes next. Find each
cancellation, exit or shutdown order that gives a record the kernel did not
produce. Find each request that a running command or a waiting cancellation
blocks. Find each download that sends a byte of another file version or a
byte past its declared length. Find each download that reports success for
wrong bytes.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Arbitration",
  "Termination after a timeout or a cancellation", "Agent shutdown",
  "Cancellation and CLI interruption", "Responsiveness", "Exec record",
  "Evidence transfer", "Bounds" and "Test seams and required controls".
- ADR-0024, ADR-0025 and ADR-0026.
- The contract of k221 is its `Done when` list and its notes.
- The code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`
  (the cancellation signal and the owner), `ExecRegistry.swift`,
  `SingleVersionFile.swift`, `ExecRecordMapping.swift`, and
  `agents/macos/Sources/testanyware-agent/AgentServer.swift` and `main.swift`
  (the cancel endpoint, the download and the SIGTERM path).
- The controls: `agents/macos/Tests/TestAnywareAgentTests/AgentProcessTests.swift`,
  `ExecCancellationTests.swift`, `ExecRegistryTests.swift`,
  `SingleVersionFileTests.swift` and the changed `ExecRecordValidity.swift`.
  `task agent:macos:test` runs them.
- Retained evidence: `experiments/exec-reliability/evidence/macos-agent-cancellation-k221`.
- Documents: `docs/architecture/agent-protocol.md` and
  `docs/components/agents-macos.md`.
- The Linux counterpart and its review: linux-agent-cancellation-k220,
  linux-agent-cancellation-k235 and linux-agent-cancellation-k236.

## Specific doubts

- One owner must handle the leader exit, the deadline and a cancellation in
  order. Check that a cancellation never wins over a leader that has exited.
  Check that a timeout never becomes a cancellation, and the reverse. Check
  the lateness of each path.
- The cancellation reaches the owner through a pipe that the kqueue waiter
  watches. Compare every path that closes the pipe with every path that
  writes to it or registers it. Check the termination waits, which drop the
  read source, and the fallback status poll without a source.
- Each waiting cancellation runs on a new thread. Check whether many waiting
  cancellations can delay health or another request.
- Retention: 300 s, 64 at most, the oldest first, never a running command.
  Check eviction, tombstones and duplicate ids across every registry entry
  point. Check the publish precondition.
- Shutdown at SIGTERM: check the dispatch signal source, the order of
  admission, cancellation and listener close, and the bound on the whole
  exit. The binary has no leader-wait seam, so the unavailable-status case
  runs `ExecRegistry.shutDown` in the test process. Judge whether that
  control covers the binary.
- Download: one handle, exactly the declared length, and a closed
  connection on a short read through Hummingbird 2.28.1. The `fileExists`
  check before the open stays. Check that the length never depends on it.
- The old-fail controls ran the transport tests against the shipped agent
  instead of the k13 host diagnostic. Judge whether that run shows the
  stated old failure. Every control must be able to fail on the new code.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings, not fixes.

## Notes

Review the producer commit of macos-agent-cancellation-k221 against the
current source. Do not replace a golden, and do not change the global CLI or
app.

## Decisions (running log)

- Review producer commit `45fb4da1` against its current source by inspection
  only. No test, build, lint, format or execution probe runs in this session.
  Both graph discovery and coverage checking failed before serving a query
  because an unverified graph generation is active. Direct source reads are
  the evidence; no graph coverage or freshness is established.
- A failed cancellation pipe or read-source registration leaves the working
  process source able to sleep without observing cancellation. Record the
  missing fallback separately from ordinary exit-versus-cancellation ordering.
- The download opens its path synchronously before checking that the handle
  is a regular file. A FIFO with no writer can therefore block a cooperative
  request worker. This is outside the producer's disclosed stalled-read limit,
  which concerns a read on the NIO file thread pool.
- A waiting cancellation follows the dictionary key after releasing the
  registry lock. Eviction and reuse can make it return a different command's
  record. Record that ordering separately from the otherwise correct oldest
  first eviction policy.
- An agent that starts but cannot serve health makes the transport fixture
  skip its tests. That can hide a startup regression from the required test
  task. Record it as a verification defect, without claiming the retained
  passing run had skips.
- These four findings identify risks to the existing acceptance conditions.
  Triage belongs in one flat integration sibling before host-cli-control-k222,
  which is the next live sibling and consumes this implementation. No new
  context node or further adversarial review is needed for this handoff.

## Findings

Coordinates refer to producer commit `45fb4da1`, which is this review's
parent. All wrong results below are derived from source. None is a fresh
runtime measurement, and this review changes no production or test code.

### R1 — P1: a missing cancellation wake source leaves the owner asleep

Code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:104-116`,
`:240-260` and `:278-285`; shutdown consumes the same signal at
`agents/macos/Sources/TestAnywareAgent/ExecRegistry.swift:193-205` and
`agents/macos/Sources/testanyware-agent/AgentServer.swift:637-644`.

Input and ordering: let `pipe` fail during admission, for example with EMFILE
under temporary descriptor pressure. `CancelSignal` deliberately survives
with `wakeFD == -1`. Release that pressure before the asynchronous owner
creates its capture files, then run `sleep 120` with timeout 300. Let the
process source register successfully and the owner enter `kevent`. Request
cancellation after that wait starts. The same failure follows if the pipe
exists but adding its read filter fails while the process filter remains live.

Derived wrong result: `watch` cannot install a wake source, but this does not
disable `sourceLive`. That flag describes the process filter alone. The
10 ms fallback poll is therefore bypassed, and `kevent` can sleep for 60 s
without checking `requestedAt`. A five-second cancellation returns
`terminating` while the command still runs, and owner lateness can exceed
500 ms by many seconds. If SIGTERM starts shutdown in that interval, its
four-second wait can expire and the agent can exit while its admitted command
still runs. The pipe failure is an explicit supported branch of this new code,
not an error that refuses the exec request.

The current failed-registration test substitutes failure of the **process**
source, which sets `sourceLive` false and takes the working poll path. The
normal cancellation controls have a working pipe and read filter. Neither
exercises a live process filter without a cancellation filter. This breaks
the cancellation/cleanup and responsive-owner acceptance conditions.

### R2 — P2: downloading a FIFO blocks before the non-file rejection

Code: `agents/macos/Sources/TestAnywareAgent/SingleVersionFile.swift:40-56`
and `agents/macos/Sources/testanyware-agent/AgentServer.swift:585-601`.

Input: an existing FIFO with no writer at `/tmp/evidence.pipe`, followed by
`POST /download?path=/tmp/evidence.pipe`. A regular path replaced by a FIFO
between the preliminary path check and the open reaches the same code.

Derived wrong result: `fileExists` accepts the FIFO as an existing
non-directory. `open(O_RDONLY | O_CLOEXEC)` then waits for a writer. The
`S_IFREG` check that would return not-found is after that blocking open, so it
is never reached. This call runs directly in the async request handler;
only later `pread` calls run on the NIO thread pool. The request has no reply
or cancellation boundary, and enough such requests hold the cooperative
workers needed by health, exec and cancel handlers. A client disconnect does
not release a thread blocked in the open.

This is a regression from the previous `FileIO` path, whose file operations
were asynchronous and whose zero-size case returned without opening a body
handle. It is also distinct from D22's accepted limit of a stalled download
**read** holding a NIO file worker. The file tests cover directories and
missing paths, but no FIFO; the transport download test uses a regular file.
The new implementation's intended rejection of non-regular files must not
block the request service before that rejection.

### R3 — P2: eviction and id reuse can substitute a waiting cancel's record

Code: `agents/macos/Sources/TestAnywareAgent/ExecRegistry.swift:91-100`,
`:113-127`, `:131-140` and `:178-185`.

Input and ordering: cancel running command A with id `reuse` and
`waitMs: 10000`. Its cancel thread signals A and releases the condition lock
to wait. Publish A's cancelled record, then let 64 cancellations of other,
unknown ids create tombstones before that waiter reacquires the lock. This
legally evicts A's record as the oldest of 65 retained entries. Admit command
B under the now-free id `reuse`, let B run `exit 23`, and publish B's record
before the original waiter resumes. All steps can occur within its wait
budget; no running command needs to be evicted.

Derived wrong result: the original cancellation returns `terminal` with B's
`exited(23)` record. It requested cancellation of A, not B. The local
`entry`/`signal` identifies A before the wait, but the loop subsequently
accepts any terminal value at `entries[id]`. It neither retains A's
completion nor checks that the entry is still the same command. Without
reuse, eviction alone makes it wait to the deadline and report `terminating`
after A already finished. The permitted retention cap explains loss of a
lookup; it does not justify attributing another execution's outcome and
evidence to an already waiting cancellation.

The registry controls test oldest-first eviction, duplicate admission and
waiting for publication separately. They do not cover publication followed
by eviction/reuse before the waiter resumes. This violates the cancel
endpoint's promise to wait for the cancelled command's final record.

### R4 — P2: an agent startup regression skips the required transport gate

Code: `agents/macos/Tests/TestAnywareAgentTests/AgentProcessTests.swift:25-27`
and `:359-371`; `Taskfile.yml:1367-1372`.

Input: a successfully built test agent that starts and exits before serving
health, or one whose router no longer serves `/health`. An immediate exit
in `runAgent` is sufficient; a selected `TESTANYWARE_AGENT_UNDER_TEST` binary
that exits immediately reaches the same fixture branch.

Derived wrong result: `AgentProcess.start` turns failure to obtain health
into `XCTSkip`. Because every `AgentProcessTests` test calls it from setup,
all ten required HTTP controls are skipped instead of failing. The task runs
`swift build` and `swift test`, with no assertion that these controls actually
ran. It can therefore return success even though no command, cancellation,
download or SIGTERM behavior was tested and the built agent cannot serve.
`stop()` also removes the agent log before this error path reads it, so the
skip loses the startup diagnostic.

The retained mutants fail after a healthy agent starts; none challenges this
startup branch. This finding concerns future discrimination of the required
gate, not the producer's recorded passing run. A broken executable is a
product failure at this seam; treating all its controls as unavailable must
not produce the same successful gate as a working executable.

## Evidence and limits

- Read the producer's committed source diff, current owner, registry,
  mapping, download and server paths, package manifest, cancellation,
  retention, file, owner, shutdown and HTTP controls, validity checker,
  cited specification and ADRs, and protocol/component documentation.
  The source has not changed since the producer commit.
- The normal owner path checks kernel state before choosing timeout or
  cancellation and keeps the earliest elapsed cancellation/deadline event.
  Termination waits drop the readable cancellation source. Publication
  closes the signal after the owner has returned; request and close share
  its lock. The inspected production path does not establish an additional
  write-to-reused-descriptor finding.
- A cancel wait has its own thread and releases the registry condition while
  asleep. No shared-pool wait is established by an ordinary waiting cancel.
  Unbounded numbers of threads remain a disclosed resource limit, rather
  than a new demand for a concurrency policy in this review.
- The single-handle reader measures with `fstat` and caps each `pread` at
  the remaining declared length. The locally inspected Hummingbird
  `HTTPChannelHandler.swift` lets an error in the response body leave
  `executeThenClose`; a short read therefore closes the connection.
  The recorded k7 `check.json` reports both corrected rename cases as whole
  versions, 57,995 whole-version stress reads, and a short body for truncation.
  Its shipped condition retains the wrong-version bytes and 2,157 wrong
  stress reads. In-place overwrite remains expressly excluded; its torn bytes
  in both conditions do not add a finding.
- `old-fail-02/output.log` shows health and ordinary download passing on the
  shipped binary, a 404 at cancellation/retention, and the shutdown control
  exceeding ten seconds with a surviving child. The close/reset/half-close
  controls are a valid direct agent-transport extension for the new retention
  contract. A missing old endpoint demonstrates the missing capability; it
  does not remeasure the k13 app/Host CLI path or establish a cause for its
  earlier intermittent stalls. No such causal claim follows here.
- `corrected-pass-02/exit` is 0. Its output records the producer's 77 passing
  XCTest controls, including ten transport controls, not new verification
  by this reviewer. The five retained mutant
  receipts report failure for ignoring cancellation, skipping kernel
  arbitration, omitting shutdown cancellation, reopening the download path
  and ignoring the retention limit. None covers R1-R4's conditions.
- The unavailable-status shutdown control exercises the registry routine
  called by the binary with a blind leader waiter. The separate binary
  SIGTERM test exercises a cooperative child. Together they support those
  inspected boundaries; they are not an unavailable-status binary run or a
  live guest shutdown measurement. R1 supplies a separate derived binary
  cleanup failure without treating that test-seam limit alone as a defect.
- `list_projects` and the attempted `check_index_coverage` both failed before
  serving a query: `CBM CLI could not start because a pre-coordination or
  unverified CBM generation is active; close all CBM sessions and commands,
  then retry`. No graph generation, freshness or coverage was available.
  Conclusions are bounded to the directly inspected sources, not a
  repository-wide or exhaustive call-graph claim.
- No tests, builds, lint, formatting, live agents or guest commands ran.
  Guest acceptance remains **0/10, unavailable** pending the separate trial.
