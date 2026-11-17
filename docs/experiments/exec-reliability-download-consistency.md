# macOS download consistency for mutable files (leaf download-consistency-k7)

This report asks one question for issue #14 on macOS. Failure D received a truncated
status record, and the Host CLI labelled the download successful. Which boundary
caused that result? The report also measures which file changes a download detects,
and it tests one stable publication protocol. It does not choose a repair. It makes
no claim about Linux or Windows.

## Result

The run reproduced the failure D bytes exactly, on the shipped agent and on an agent
built from this checkout. Both downloads had SHA-256
`ca41852314b967a327b0a4faf0240cac2f2e304387439e5518cf380da4b9a56e`. That digest is
the digest of the historical file `after-abrupt-03-state-002.json`. The raw HTTP
client received `Content-Length: 25` and 25 bytes. The Host CLI exited 0 and wrote a
receipt for 25 bytes.

The run shows a defect in the download handler of the macOS agent, through the
`FileIO` type of Hummingbird. The defect can produce the failure D bytes. The run does
not show that this defect produced failure D in the guest (see *Integration k210*).

The handler finds the file by path three times. The
second lookup supplies the length. The third lookup opens the file. Nothing checks
that the second and the third lookup found the same file.

The writer of failure D replaces a 25-byte pending record with a longer terminal
record. When the replacement falls between the second and the third lookup, the
agent sends 25 bytes of the new file. The framing is then valid, so no client can
detect the error. This run did not test the relay of the TestAnyware app. ADR-0004
describes how the relay forwards bytes, but no measurement shows what the relay
received or sent in failure D. A 25-byte prefix of the terminal record also fits a
truncation at another point on that route.

The stress cases gave the same result without a pause. The shim stayed loaded and
can change the timing, so these cases are not an uninstrumented control. A writer replaced the record
about 74,000 times in 10 s while the raw client read it. On the shipped agent, 1,887
of 53,031 reads returned the failure D bytes with a complete frame. On the checkout
agent, 2,794 of 48,535 reads did the same.

A stable publication protocol had no error in this run. Each record went to a new
name once and that name never changed. All 200 records of each agent arrived
complete.

## Causal chain

The checked-out source of Hummingbird is 2.28.1, revision
`341e11f65d8e24cf87e9aae427c040f0084ada4e`, as `agents/macos/Package.resolved` pins
it. The swift-nio source is 2.105.0, revision
`2f58d8ef84438d6517fc78bcea2f8527e3c72936`.

1. `handleDownload` in `agents/macos/Sources/testanyware-agent/AgentServer.swift`
   calls `FileManager.default.fileExists(atPath:)`. This is lookup 1.
2. `FileIO.loadFile` calls `fileSystem.info(forFileAt:)` by path. This is lookup 2.
   It sets `Content-Length` to the size that it finds.
3. `FileIO.readFile` returns a `ResponseBody`. When the server writes the body, it calls
   `fileSystem.withFileHandle(forReadingAt:)` by path. This is lookup 3. It reads the
   range that lookup 2 set.
4. `FileChunks` in swift-nio reads with `pread`. A short read ends the range as a
   normal end, not as an error.

The trace of the forced case shows the chain. The shim records the inode at each call.

| Call | Shipped agent | Checkout agent |
| --- | --- | --- |
| lookup 1 | `stat`, inode 632885858, size 25 | `stat`, inode 632960397, size 25 |
| lookup 2 | `lstat`, inode 632885858, size 25 | `stat`, inode 632960397, size 25 |
| pause, then rename of the new record | | |
| lookup 3 | `open`, inode 632885865, size 123 | `open`, inode 632960404, size 123 |
| read | `pread` of 25 bytes at offset 0 | `pread` of 25 bytes at offset 0 |

The two agents call different system functions for lookup 2. The shipped agent calls
`lstat`, and the checkout agent calls `stat`. The release tag `v2.8.0` has no
`Package.resolved` for the macOS agent, so the dependency versions of the shipped
agent are unknown. The result is the same for both agents.

## Cases

The controller starts a new agent process for each case on a loopback port. A shim,
loaded with `DYLD_INSERT_LIBRARIES`, traces `stat`, `lstat`, `open`, `pread` and
`close` below the case directory. It pauses one armed call until the controller
writes a resume file. A calibration download finds the lookup that supplies the
length, because the two agents differ.

Each case runs once with a raw HTTP client and once with the Host CLI. The raw client
reads to `Content-Length`. Then it waits 0.5 s for extra bytes or the close. The Host
CLI runs `testanyware file download --agent 127.0.0.1:PORT`.

| Case | Change | Pause | Raw client | Host CLI |
| --- | --- | --- | --- | --- |
| replace-longer | 25-byte record replaced by a 123-byte record | after lookup 2 | undetected-wrong | undetected-wrong |
| replace-shorter | 123-byte record replaced by a 25-byte record | after lookup 2 | detected-failure | detected-failure |
| truncate-during-read | file cut to 64 KiB | before read 2 | detected-failure | detected-failure |
| overwrite-during-read | same-length content written in place | before read 2 | undetected-wrong | undetected-wrong |
| append-during-read | 128 KiB appended | before read 2 | allowed-complete | allowed-complete |
| immutable-binary | none, oracle `00ff80410a` | none | allowed-complete | allowed-complete |
| agent-killed-mid-body | agent receives SIGKILL | before read 2 | detected-failure | detected-failure |
| cli-killed-mid-body | Host CLI receives SIGTERM | before read 2 | not run | interrupt-leak |

The shipped agent and the checkout agent gave the same class in every row.

The large cases use 394,216 bytes, which is three 128 KiB chunks and 1,000 bytes.
Each version has distinct bytes, so a torn body shows its offsets. The torn body of
overwrite-during-read matches the old version up to offset 131,072. From there it
matches the new version. That offset is the end of the first chunk.

A shorter file gives fewer bytes than `Content-Length`. The agent then closes the
connection. The raw client sees the end before the length. The Host CLI exits 1 with
`CONNECTION_DROPPED` and the message `connection dropped: error reading a body from
connection: end of file before message length reached`. It leaves no destination and
no temporary file. An agent killed during the body gives the same result.

The append case is complete because the agent reads only the range that lookup 2 set. The body
equals the version before the append.

### Interrupted Host CLI

SIGTERM to the Host CLI during the body left a temporary file in the destination
directory, for example `.cli-killed-mid-body.bin.testanyware-38359.0.tmp`. The
destination did not exist. The Rust client in
`cli-rs/crates/testanyware-agent-client/src/lib.rs` removes its temporary file only
when the download returns an error. A signal ends the process first. This report
does not repair that path.

## Classes

`download_k7_check.py` gives each download one class. It compares the bytes by exact
equality with the versions that the file held during the case. It accepts no prefix,
no suffix and no trim.

- **allowed-complete**: the transfer reports success, and the bytes equal an allowed
  version.
- **detected-failure**: the transfer reports failure, with no destination and no
  temporary file.
- **detected-failure-leak**: the transfer reports failure and leaves a temporary file.
- **undetected-wrong**: the transfer reports success, and the bytes equal no allowed
  version.
- **framing-violation**: the raw client received bytes after `Content-Length`.
- **clean-interrupt** and **interrupt-leak**: the result of a Host CLI that received
  a signal.
- **unclassified**: the evidence is missing or does not fit.

An undetected-wrong class is a finding. The check passes with it. The check fails
when a record is missing or unclassified. It also fails when an agent did not listen
on loopback only or did not stop. It fails when the oracle controls or the
historical digests change.

## Oracle

The immutable binary oracle is `00ff80410a`. The checker writes three independent
copies to disk and reads each back. It accepts the exact copy. It rejects a copy with
one flipped bit and a copy cut to 4 bytes. The checker keeps its own copy of the
constants and does not import them from the controller. A test checks that the two
copies agree.

The historical failure D files are not changed. The controller records their
digests before and after the run, and the checker reads them again. All three lists
agree. The run keeps copies in
`experiments/exec-reliability/evidence/download-consistency-k7/historical/`, with
`SHA256SUMS`.

| File | Bytes | SHA-256 |
| --- | --- | --- |
| `after-abrupt-03-initial.json`, the pending record | 25 | `b95c9261fb8ff0ca68a9d8b06ebdcb4102a72f60b670c2d748cdf4eca454e7a4` |
| `after-abrupt-03-state-002.json`, the malformed prefix | 25 | `ca41852314b967a327b0a4faf0240cac2f2e304387439e5518cf380da4b9a56e` |
| `after-abrupt-03-collect-002.json`, the successful receipt | 827 | `6645b2e4b55f2b89a7e635bfa6bd17074f538c2978606e01852ce049cc3740a3` |
| `after-abrupt-03-launch.json`, the detached exec | 1216 | `eb9c539fccdae4d746627b159d754845a725782efe37d548b61f57de555338b7` |

The malformed prefix is `{"state": "terminal", "re`. It has the length of the
pending record and the first bytes of a terminal record. The receipt reports
`bytes: 25` and `ok: true`. The case replace-longer gives the same digest from a
terminal record of 123 bytes, because only the first 25 bytes matter. This report
uses the malformed JSON as evidence only, not as a schema.

## Consistency contract

The current `/download` path supports no consistency contract for a file that
changes during the download. The run measured three results.

- A file replaced by a longer file can give a complete frame with wrong bytes.
- A file overwritten in place can give a complete frame with a torn body.
- A file that becomes shorter gives a detected failure, if the body is shorter than
  the length.

An atomic rename by the writer does not make the read coherent. The reader finds the
file by path more than once.

A stable publication protocol is a tested alternative, not a selected repair. The writer
writes each record to a temporary file and renames it to a new name. It never
replaces that name. The reader asks for the next name and accepts the first 200
response.

The run tested 200 records on each agent with exact equality. There were 10,264 and
10,266 responses of 404 before publication, and no mismatch. This test ran on the
host file system, not in a guest. The relay was not used.

## Integration k210

The review `download-consistency-k209` reported five findings in
`docs/experiments/exec-reliability-download-consistency-review-k209.md`.
Integration k210 found each finding valid against the source.

| finding | change in k210 | limit that stays |
|---|---|---|
| R1. The checker accepted a stress record with missing, empty or unclassified reads. | The immutable summary needs 200 publications and 200 classified reads. The mutable summary needs at least one read, a class for each read, a total equal to `reads` and a stopped writer. A stress record with an error is incomplete. | The retained run keeps the aggregates of the immutable reads, not each body. |
| R2. No step had an independent deadline, and a failure could leave a writer, the Host CLI or an agent running. | `download_k7_run.sh` runs each step under GNU `timeout`. The build has 600 s, the probe 300 s, the check 60 s and each inspection 30 s. An exit trap ends each listed agent whose command still names its binary, then removes the build tree. The raw client bounds the whole download. The CLI and agents get 2 s of grace, then SIGKILL and 2 s to reap. A failed `ps` or `pgrep` reads as unknown, not as absence. A failed stress read stops its writer and is kept as an error. | A process that does not end after SIGKILL stays. The record names it as `unreaped`, and the run fails. |
| R3. The checker judged `immutable-binary` by the allowed file that the controller wrote. | The checker compares that case with its own copy of the oracle. | None found. |
| R4. *Result* named the agent handler as the boundary that caused failure D, and called the unpaused stress natural timing. | *Result* and *Limits* now state a host agent defect that can produce failure D. The guest route and the relay stay unproved. The stress cases are instrumented. | A stronger attribution needs evidence from the guest and relay route. No such run is planned in this grove. |
| R5. The digests before and after omitted the agent sources, the executables and the compiled shim. | The runner hashes every file under `agents/macos`, the shipped agent, the Host CLI and `timeout` before the build and after the check. The controller hashes the Host CLI, the shim and both agents again after the cases. The checker fails when one changes. | The `local-01` freeze stays incomplete. A digest taken now cannot stand for its after-state. |

### Run `local-02`

The k210 tooling ran once as `local-02`, from source commit
`6cdfefc4e058de9efdb188124d0ac1fcb7168ff9`, with no retry. The workloads and counts
are those of `local-01`. The runner exited 0.

The build, probe and check exited 0, and
both digest pairs were equal before and after. The outer directory holds 31 subject
digests. The cleanup ended no agent, `survivors.txt` is empty, and the build tree was
removed. The probe took 32 s.

Every forced case gave the class it gave in `local-01`, on both agents. The SIGTERM
of the CLI ended it within the grace on both agents.

| Agent | Wrong complete frame (failure D bytes) | Premature EOF | Total reads | Replacements | Immutable complete |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shipped | 1,775 | 1,700 | 52,922 | 71,867 | 200 of 200 |
| Checkout | 2,249 | 2,168 | 48,532 | 70,319 | 200 of 200 |

The shipped agent and the Host CLI had the digests of `local-01`. The checkout agent
had SHA-256 `6d33935a978cdd97988538235a488d20801dd966cec7e0fbbaea4e0e5e7b5d0c`. The
`local-01` digest was `02cb30175b43f04f60bbd80b65e9a0545c5fb71bf3888cbf2c55a1500a20dd4d`.
The agent sources did not change between the two source commits. The cause of the
different digest was not examined.

### Re-judgement of `local-01`

The k210 checker re-judged `local-01`. The result is `check-k210.json` beside
the retained `check.json`. It exited 0 and gave the same rows as the first check.
`executables_unchanged` is null, because `local-01` has no digests after the cases.
The record is in `rejudge-k210/`, with the source commit, the dirty diff, the tool
digests and the digests of the 703 retained files. The two digest lists were equal.

### What consumers may rely on

- On macOS, the agent handler takes the length and the bytes from two different path
  lookups. A replacement between them gives a complete frame with wrong bytes. The
  shipped agent and the checkout agent both show this.
- Replacement with a shorter file, truncation and agent termination give a premature
  EOF. The Host CLI exits 1 for each.
- Publication to a new name once gave 200 complete reads of 200 on each agent, in
  both runs.

### What remains unproved

- That this defect produced failure D in the guest, and what the relay did then.
- The rate of the race without the shim.
- The freeze of the executables and sources during `local-01`.

The leaf `linux-download-consistency-k14` uses the k210 tooling. The design leaf
`lifecycle-evidence-k8` consumes this report and must treat these items as open.

## Limits

- All cases ran on the host, with the agent on loopback. No VM, guest, relay or
  Windows agent was used. A guest route was not necessary, because the host route
  reproduced the exact bytes.
- The Host CLI used the direct route with `--agent`. Failure D used `--vm`, which
  goes through the relay of the TestAnyware app on macOS hosts.
- The stress cases ran with the shim loaded and no pause. They do not measure the
  rate of an agent without the shim.
- The historical guest route is not measured. Which interleaving occurred in failure
  D, and what the relay received and sent, stay unproved.
- The forced cases ran once each. The stress cases ran 10 s and 200 records on each
  agent. The rates are samples, not bounds.
- The run did not test a handler that takes the length from the open handle. Such a
  handler would remove the replace-longer error. It would not remove the
  overwrite-during-read error.
- The Linux comparison is the work of leaf `linux-download-consistency-k14`. The
  controller, the classes and the checker do not depend on macOS, except the shim
  and the agent binaries.

## Design inputs

These are inputs for the design leaf `lifecycle-evidence-k8`. They are not decisions.

- A status record that the host reads while the guest can change it needs a read
  that detects change. Atomic replacement alone is not sufficient.
- Publication to a new name once gave complete reads in this run.
- The Host CLI leaves a temporary file when it receives SIGTERM during the body.

## Resource audit

| Resource | Result |
| --- | --- |
| agent processes | 36, each on a new loopback port, each stopped. `agents-after.txt` is empty. In `local-01`, a failed `ps` would also have read as stopped (see *Integration k210*). |
| listeners | each agent listened on `127.0.0.1` only. The controller checked this with `lsof`. |
| agent build tree | built in a temporary directory outside the repository, then removed. `build-dir-after` says `removed`. |
| Host CLI temporary files | one in each cli-killed-mid-body case, kept as evidence in the case directory |
| tooling digests | equal before and after. `local-01` did not hash the agent sources or the executables after the run (see *Integration k210*). |
| historical files | digests unchanged before, after and at check time |

## Accounting

Leaf k7 had one recorded run, `local-01`. It had no retry. Integration k210 added one
run with the k210 tooling, `local-02`, with no retry. Development trials ran in
the scratchpad and are not evidence. One development run reached the evidence
directory with its build tree in the outer directory. That run is not
kept.

The source commit of the run is `b921f918784e2ff1a0f745d3d7b4f5dcfc67f234`. The
`-outer` directory holds the dirty diff, the tool digests, the host identity, the
tool versions and `Package.resolved`. `meta.json` holds the digests of the Host CLI,
the shim and both agent binaries.

| Item | SHA-256 |
| --- | --- |
| Host CLI 2.8.0 | `44b08089cd01ebed55a786f9b86ca1bb80b53cd6b3778bd24a3e574c3030a340` |
| shipped agent | `22dd56cec883a9c471420172d0261617ed02a4fea218f949ee598a5311bab9f5` |
| checkout agent | `02cb30175b43f04f60bbd80b65e9a0545c5fb71bf3888cbf2c55a1500a20dd4d` |
| shim source | `521565834b4d03b78d74e02ec6e8c575db9ad57099f04410464e696c58a46bc1` |

## Commands

Run each command from the repository root.

```sh
task diagnostic:download-consistency-tools
task diagnostic:download-consistency-local OUT=experiments/exec-reliability/evidence/download-consistency-k7/local-03
task diagnostic:download-consistency-check OUT=experiments/exec-reliability/evidence/download-consistency-k7/local-02 RESULT=recheck.json
```

The evidence is in
`experiments/exec-reliability/evidence/download-consistency-k7/`.
