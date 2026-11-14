# Late helper identity: k76 integration (k77)

The overflow repair retains complete valid identity lines inside the admitted
control prefix, even when the same read contains the overflow-detection byte.
The final diagnostic suite passed all 350 tests. This is a bounded repair to the
k71 slice; the persistent worker/shared scheduler and ten-clone acceptance remain
open under k75 and its ancestors. Ten-clone acceptance remains **0/10 unavailable**.

## Findings and resulting behavior

All three findings from review commit
`cf9ba1da0b6e634470108e25e7da5235e8f1f8c5` were verified against the source and
classified as real issues. The second concerns verification and the third concerns
the scope stated for the evidence.

1. **Identity before overflow.** Previously, the cap exception preceded parsing,
   and cleanup refused to parse once the cumulative byte count exceeded 4096.
   The parent now stores only the first 4096 control bytes and parses their complete
   lines before rejecting overflow. Byte 4097 detects overflow and cannot complete
   a protocol line. Both timely and cleanup reads share the cumulative 4096+1
   ceiling; cleanup reads remain nonblocking. Identity lines in an overflowing read
   grant cleanup obligations only. The query cutoff and fixed shutdown deadline
   remain unchanged, and late terminal/reap results are never decoded.
2. **Controls that can fail.** A frozen permissive mutant bypasses `_created`
   validation and deliberately completes an unterminated cleanup line. All seven
   invalid-field/framing controls fail against it: token, boolean PID, stale time,
   future time, mismatched deadline, extra field and partial line. A separate mutant
   lets late readiness set `owner.ready`; all six select controls then fail their
   `signal_scope == "pid"` assertion. This is explicitly a **readiness-flag test**.
   It does not observe a live worker sending a PID-versus-group signal.
3. **Production signal disposition.** The shutdown adapter installs a SIGTERM
   handler and publishes held bytes after TERM. It demonstrates cleanup parsing
   through real pipes, but that publication mechanism differs from production.
   Production resets SIGTERM to `SIG_DFL`, so late bytes available after TERM are
   bytes already buffered before termination; the worker cannot publish after it
   dies. Two added controls prebuffer valid/distinct identities, verify the default
   disposition, then observe a recorded TERM and worker exit `-SIGTERM`. The parent
   retains those identities without accepting late terminal/reap authority.

The six overflow controls put a valid identity, or two distinct identities, before
padding in one actual parent read across read, select and shutdown delays. Their
retained read records include the exact bytes. Final read/shutdown chunks were
4001 bytes after the earlier 96 control bytes; select chunks were 4097 bytes.
The existing overflow-before-identity controls continue to reject identities
outside the admitted prefix. Distinct synthetic PID `987654322` is test data.

## Frozen conditions and outcomes

Each condition has its own source copy, raw streams, per-case JUnit where produced,
artifact hashes, and before/after subject digests. See
[frozen inputs](../k77-inputs-01/) and each condition's verdict. The fixed checker
limits were lint/format/types 30 seconds each, pytest 60 seconds, followed by
TERM/KILL waits of 2 seconds each. No limit was increased.

| Condition | Retained outcome |
| --- | --- |
| [k77-red-01](../k77-red-01/verdict.json) | Six intended same-read failures; three original overflow controls pass |
| [k77-invalid-mutant-01](../k77-invalid-mutant-01/verdict.json) | All seven invalid controls fail at the identity assertion |
| [k77-scope-mutant-01](../k77-scope-mutant-01/verdict.json) | All six select controls fail at the scope flag assertion |
| [k77-green-01](../k77-green-01/verdict.json) | Failed: comment lint error and one witness-file race; 349 tests pass |
| [k77-green-02](../k77-green-02/verdict.json) | Failed: static checks pass, pytest reaches its 60-second limit; exit -15, reaped, no JUnit |
| [k77-red-02](../k77-red-02/verdict.json) | Final controls: six intended failures and three passes; checker 5.707 seconds |
| [k77-invalid-mutant-02](../k77-invalid-mutant-02/verdict.json) | Final controls: all seven intended failures; checker 5.182 seconds |
| [k77-scope-mutant-02](../k77-scope-mutant-02/verdict.json) | Final controls: all six intended failures; checker 4.323 seconds |
| [k77-green-03](../k77-green-03/verdict.json) | Lint, format, strict types and all 350 tests pass; pytest 54.39 seconds, checker 55.046 seconds |

The witness race was repaired with atomic temporary-file replacement. The final
fixtures use a 0.4-second allowance instead of 0.6 seconds, with the same
0.05-second shutdown reserve and delay across the query cutoff. All RED and mutant
conditions were repeated with these final fixtures. The shorter allowance reduces
intentional waiting; it does not renew a deadline or relax the checker cap.
Earlier failures remain failures, including the incomplete timeout transcript.

The final root runtime SHA256 is
`fb7e7a31ad0d8682098bd5503c6003e5519d5f7add95099522025fb3b7b0b84e`;
the final test SHA256 is
`e7e808bf3186252258b64d869e1f55f4ae7140f085e09f8969b522c471ca25c5`.
The final RED runtime SHA256 is
`b1f57d4158354c44a5f8eb3f76a00345f7edcb01684b6b2cdeae04a1f121e1a4`.
Its bytes were compared directly with the parent commit's runtime and match.
The audit checks frozen source digests and confirms that final live sources match
the successful condition.

## Integrity and resources

The [frozen audit](../k77-audit-01/audit.py) passed through the repository's
`diagnostic:resource-audit` task. Its [verdict](../k77-audit-01/verdict.json) records:

- 3,938 audit subjects unchanged before/after, and all **74,051 earlier evidence
  files** matching the pre-edit SHA256 manifest before and after the audit.
- Exact named-case comparison for all complete conditions: 39 failed-case
  occurrences become GREEN in the final condition. This includes repeated RED
  and mutation conditions and the witness race; matching totals alone is not used.
- All 27 final late controls checked for identity obligations, failed-capture
  semantics and original deadlines. The six same-read records and both
  production-disposition witnesses are checked explicitly.
- All 2,119 recorded nonsynthetic fixture PIDs absent under the exact-PID `ps`
  oracle. Live, absent, mixed and invalid-input controls distinguish a clean
  absence from a failed observation.
- 748 scratch directories checked. Two exact plan-owned mock markers containing
  `owned` were removed only after absence and exact plan/byte checks. The audit
  records their paths, run identities, bytes and hashes. Two empty nested-readiness
  witnesses are retained as expected negative-fixture output.

Later PID absence is a resource observation, not historical proof of reap and not
authority to retire an unknown helper. Failed captures retain their cleanup
obligations; the test harness releases only its evidence entries after assertions.
No privilege, guest or installed application was changed. Prior reports, including
k71's original adapter wording, remain byte-for-byte intact; this report supplies
the corrected interpretation. k75 and every aggregate ancestor remain live.
