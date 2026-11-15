# Worker request transport turns (k126)

Parent-side worker requests now yield through the shared turn driver while
waiting for channel data and retiring a failed or standalone worker. Each
callback performs bounded work and returns its next absolute wake. The driver
owns sleeping. Request work uses ordinary priority; retirement uses safety
priority.

This delivers a bounded transport seam. Production owners still run
sequentially. Bootstrap, resource cleanup and disposal, workload waits, and
worker close do not yet share one concurrent production scheduler. The complete
aggregate charter and original history remain in complete-worker-scheduler-k129.
The k28 and k17 ancestors remain open. Guest acceptance remains 0/10 unavailable.

## Runtime boundary

`worker_capture.capture_helper` registers `advance_query` and
`advance_retirement` through `run_turns`. Query polling uses a zero-timeout
`select`. A ready query returns an immediate wake; an idle query returns at most
5 ms later. Retirement returns at most 1 ms later and retains its fixed KILL
wake. A terminal owner cannot keep a stale KILL wake active.

The query deadline, shutdown reserve, capped channel consumption, result
validation, late cleanup-only publication, helper obligations and sticky
failure policy retain their existing authority. The owner-lifetime signal and
WNOHANG admission guards remain unchanged. The final guarded reap attempt and
persistent worker close retain their existing placement. There is no new retry,
replacement or deadline extension.

## Controls and retained failures

Four real controls cover standalone and persistent transport, each with a
successful 128 KiB output or stalled helper creation. Each control inserts a
competing ready callback into the actual transport driver registration. They
check priority order, zero-timeout polling, requested reads of at most 64 KiB per
turn, and repeated terminal callbacks without more channel polling or reading.
Successful requests require multiple owner turns. Failed creation retains its
unknown-creation obligation. Exact worker identities are journaled for the
external checker.

The original RED sample has four failures because transport bypasses the shared
driver. The persistent stalled fixture was then corrected to install its fault
before worker fork. Success fixtures use sh/dd to exercise multiple result
reads. All inputs and results remain retained.

The stricter requested-read control initially failed four cases in
`controls-ready`. `debug-read-budget` traced 50,000 bytes from subprocess setup,
three 32,768-byte observer reads and a 4,097-byte transport request. The external
ps observer caused the 152,401-byte total. The corrected test excludes only that
observer from runtime accounting. It still accounts for the real reap call.
Production code did not change for this correction. Both failed samples remain.

The final frozen `inputs-verified` controls pass 90 cases. Earlier green and
failed samples remain separate. Each mutation starts from this final input:

| Mutation | Observed rejection |
| --- | --- |
| Query marked safety | Competing safety callback loses the expected order |
| Retirement marked ordinary | Ordinary competitor runs before retirement |
| Query blocks for 5 ms | Zero-timeout assertion fails |
| Result read uses frame cap | Requested read exceeds 64 KiB |
| Terminal query polls again | Repeated terminal callback increments polling |

## Measurement and preservation

`comparison-plan.json` declares the order and interpretation. `run_condition.py`
bounds controls at 60 seconds, full tests at 120 seconds and static checks at
30 seconds, with TERM 2 seconds then KILL/reap 2 seconds for checker shutdown.
Each output has one writer. Inputs, runner scripts, plan, deviations, manifest,
lockfile, Taskfiles, diagnostic spec, interpreter and helper binaries are hashed
before and after. Final measurements include `/bin/dd` in that subject set.
Earlier samples predate that addition and carry their actual declarations.

The external checker audits journaled exact identities after pytest. It signals
only an identity that still matches the recorded fixture. Passing audits require
absence. A worker reap does not discharge unknown helper creation.

`prior-after.json` compares every earlier evidence path with `prior-before.json`.
All 206,230 paths remain byte-for-byte unchanged. The inventory excludes only
this new evidence directory and named caches. `condition-audit.json` records
per-case outcomes, comparisons, subject stability and artifact digests.

## Results

| Sample | Tests | Static checks |
| --- | --- | --- |
| Final focused controls | 90 passed | Covered by final frozen static run |
| Parent full 1 | 875 passed, 4 new controls failed | All passed |
| Candidate full 1 | 879 passed | All passed |
| Parent full 2 | 875 passed, 4 new controls failed | All passed |
| Candidate full 2 | 879 passed | All passed |
| Final frozen static | No tests requested | Ruff, format and strict Pyright passed |

The parent measurements restore only the parent worker_capture.py revision
21842c58 and retain the candidate tests. Both pairs contain the same 879 case
identities. The only changed outcomes are the four new controls failing on the
parent and passing on the candidate. All 875 previously existing cases pass in
both pairs. Each full run has nine owned fixture journals; every recorded
identity is absent after the run, with no external cleanup signal needed.

The consolidated audit passed for every retained sample: artifact digests and
before/after subjects match, checkers were reaped without timeout, exact fixture
identities are absent, intended mutants have testcase failures, final samples
are green, and per-case comparisons preserve the declared identities. The
retained failed samples remain failures in that audit.

## Remaining acceptance

Historical failures from k85, k106, k114, k116, k121 and k122 remain aggregate
obligations. These samples neither reproduce nor resolve every historical
failure. They remain available in their original directories. Real privileged
ps KILL and UID EPERM controls remain unavailable without separately authorised
fixture prerequisites. No guests, installed application or golden images changed.

The next producer must compose competing production owners, verify their
priority registrations and teardown overlap, and satisfy the full retained
charter. A separate review-impl leaf owns independent challenge of this seam.
