# Linux download consistency implementation review — k211

Reviewed producer `linux-download-consistency-k14`, commit
`10e3b96e8025ea09a88e575afd67eb654fa9f306`, against its task, the common
evidence requirements in `.grove/_BRIEF.md`, ADR-0001 and ADR-0004.

Disposition: **five material findings; integration required before
`lifecycle-evidence-k8` consumes the evidence.** P1 denotes a resource-ownership
risk; P2 denotes a gap in a required measurement or its independent validation.

## Findings

### 1. P1 — Outer cleanup can delete containers owned by another run

Location: `experiments/exec-reliability/download_k14_run.sh:49`.

The trap enumerates `docker ps -a -q --filter "label=$LABEL"`, where
`LABEL=testanyware.k14.run`, and forcibly removes every returned container.
This selects the shared label key, rather than this invocation's unique label
value. The controller's disposal does scope its removal to the run value
(`experiments/exec-reliability/download_k14.py:232`), but the outer EXIT/INT/TERM/HUP
trap independently widens that scope again.

Concrete failure: run A and run B have different values of
`testanyware.k14.run`; when A exits or is interrupted, its trap removes B's
container too. A pre-existing container bearing the key is also selected. The
before inventory does not exclude those containers from deletion. This violates
the unique-run ownership requirement and can destroy another run's live
measurement. The retained local-01 inventories are empty, so they show no
observed collateral deletion; they do not make the cleanup scope safe.

### 2. P2 — The checker does not establish that the hook ran at its named event

Locations: `experiments/exec-reliability/download_k14_check.py:180` and
`experiments/exec-reliability/download_k14_check.py:279`.

`agent_evidence` reduces hook evidence to the existence of any `action` or
`paused` entry in the selected request. `judge_case` combines that Boolean with
`arm_unused == False`. Neither retains or compares the entry's event with the
planned event, nor checks its position relative to open, fstat and reads.

Concrete failure: a record planned for `after-fstat` can contain an action
labelled `after-open`, or omit the action's event altogether, and still be
complete when its transfer is otherwise classified. The existing synthetic
trace does omit the event (`experiments/exec-reliability/test_download_k14.py:28`),
and `test_unused_arm_makes_the_case_incomplete` expects that record to pass at
`experiments/exec-reliability/test_download_k14.py:147`.

This does not support the report's claim that the checker requires the hook at
exactly its event (`docs/experiments/exec-reliability-linux-download-consistency.md:121`).
I inspected every retained armed local-01 trace: the event names and ordering
match the intended seams. Thus the recorded interleavings have direct inspection
support, but the independent gate does not detect the wrong-event failure that
the review task explicitly requires it to detect.

### 3. P2 — Missing agent observations can still produce a complete case

Locations: `experiments/exec-reliability/download_k14_check.py:164`,
`experiments/exec-reliability/download_k14_check.py:281` and
`experiments/exec-reliability/download_k14_check.py:291`.

When the selected request has no `finish` entry, its emitted-byte summary becomes
an empty dictionary. The raw emitted/received comparison runs only if both
digests are present; otherwise `wire_ok` stays `None`. Completeness accepts
`wire_ok is not False`. Required descriptor, framing and read observations are
also not prerequisites for completeness.

Concrete failure: remove the `finish` entry from an otherwise valid ordinary
armed raw download, leaving its open/action and received body intact. The body
can still classify as `allowed-complete`, the arm check still passes, and the
case remains complete with no established emitted/received equality. Removing
its fstat, headers and read entries likewise does not invalidate it. An unarmed
ordinary case can lack the entire agent trace and still be complete.

The task requires descriptor identity, metadata, advertised framing and emitted
versus received bytes, not just a classified host result. Missing observations
must remain missing in the acceptance result. This finding concerns ordinary
completed transfers: intentional SIGKILL cases legitimately lack `finish` and
need to be assessed using the partial observations available for that case.
No retained ordinary local-01 trace was found to be missing its finish; this is
a fail-open validation defect derived from the checker, not a claim that the
retained normal transfer bytes differ.

### 4. P2 — Aggregate stress/publication output loses required per-item evidence

Locations: `experiments/exec-reliability/download_k14.py:413` and
`experiments/exec-reliability/download_k14.py:450`.

The stress controller collapses reads with the same status, length, end state
and digest into one count and retains only the first result in each group. Its
reported `seconds` is the configured duration, not the elapsed duration of each
read. The publication controller retains successful reads only as increments
to `outcomes["complete"]`; the record name/index, expected and received bytes
or digests, framing and timing of each success are discarded. Pre-publication
failures are reduced to status counts.

The retained `local-01/stress.json` therefore represents 15,766 reads by two
aggregate entries. `local-01/immutable.json` contains 200 complete outcomes,
1,185 HTTP 400 outcomes and no mismatches, but no receipts for the 200 distinct
record bodies. `publication_summary` accepts those counters
(`experiments/exec-reliability/download_k14_check.py:331`); it cannot
independently establish which record supplied each success or recheck its exact
byte oracle. The controller does compare bytes before incrementing, so this is
a retention/independence gap, not evidence of an observed incorrect body.

This falls short of `.grove/_BRIEF.md:70`'s raw receipts, monotonic timings and
item-by-item results. The counts support the stated controller observations,
but cannot serve as the required independently auditable evidence for repeated
rename reads or immutable publication. The discarded receipts cannot be
recovered by rejudging the aggregates; meeting that evidence requirement needs
new retained per-item measurements, distinct from the historical run.

### 5. P2 — A lifetime RSS peak difference does not establish the transfer memory bound

Locations: `experiments/exec-reliability/download_k14_seam.py:276`,
`experiments/exec-reliability/download_k14_seam.py:288` and
`experiments/exec-reliability/download_k14_check.py:371`.

The request and finish samples use `resource.getrusage(...).ru_maxrss`, a
process-lifetime peak. The instrumented agent handles earlier cases before the
large transfer. The checker subtracts the minimum from maximum of these peak
samples and calls a rise of at most 16,384 KiB `fixed_memory`.

Concrete failure: a transfer starts with current RSS below a previous process
peak and grows by more than 16 MiB without exceeding that peak. Both recorded
values remain identical, so this check reports zero growth and passes the bound.
The local-01 64 MiB raw request has 38,664 KiB at both boundaries
(`experiments/exec-reliability/evidence/linux-download-consistency-k14/local-01/cases/large-immutable/raw/record.json:21`
and the finish sample at line 2074); the subsequent CLI request also records
38,664 KiB at both boundaries. These observations establish no new lifetime
peak, rather than the claimed per-transfer RSS rise.

The observed transfer sizes and durations remain supported. Source inspection
also shows the unchanged handler's 1 MiB read loop. Neither fact supplies the
missing measurement of the stated memory-rise bound, and the observer itself
retains and hashes data. Before treating the fixed-memory gate as verified,
integration needs evidence that measures the claimed property, or an explicit
acceptance-level disposition of that limitation. Reinterpreting the existing
zero peak delta alone cannot establish it.

## Inspection scope and retained-evidence limits

This was an inspection-only review. I read source, synthetic tests and retained
records, compared retained bodies with their allowed versions, and recomputed
the report's evidence digests. I did not run tests, builds, lint, formatters or
the diagnostics, and made no production or test-code changes. The recorded
check and rejudge files are identical; retained before/after subject and tooling
hash lists are identical.

The Linux descriptor-open/fstat/read sequence supports the report's distinction
from macOS. Retained cases support the old-version rename results, append's
25-byte allowed Host CLI prefix, wrong overwrite prefixes, torn bodies and
detected truncation. The two abandon lanes differ: the raw lane emitted
2,097,152 bytes, while the CLI lane emitted all 3,146,728 bytes and retained a
temporary file. Those are separate observations, not one shared cutoff.

These are Linux-container measurements, with observer overhead on forced cases.
They do not establish the guest kernel/filesystem or the app's VM-id relay path;
the report states that boundary and guest acceptance remains unavailable, 0/10.
The producer also discloses that scratch trials 1 and 2 were not retained.
Their failures cannot be independently audited; nothing in this review converts
them into retained successful samples.

Graph verification was unavailable: no graph tools were exposed, and the CLI
refused to start because a pre-coordination or unverified generation was active.
All material findings above were derived from the cited source and retained
evidence directly; no graph coverage or exhaustive call-chain claim is made.
