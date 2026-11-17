# Linux download consistency (k14)

Leaf `linux-download-consistency-k14` of issue #14. It asks how the Linux agent sends a file that changes during a download. It repeats the k7 macOS question on Linux, with the same classes and oracle. This leaf changes no production framing and no production finalisation.

## Result

The k7 macOS failure does not occur on Linux. On macOS, a replacement by rename between two path lookups gave a complete 25-byte frame with the wrong bytes. The Linux agent opens the path once and reads only that descriptor. A rename after the open does not change the open file. All four replacement cases sent the old version, complete and correct, to both clients.

Linux has its own failure. The agent takes Content-Length from `fstat`, but it reads to end of file, not to that length. Two results follow:

- A file that grows in place after `fstat` sends more body bytes than Content-Length. The raw client sees a framing violation. The Host CLI stops at Content-Length, keeps a prefix and exits 0. When the prefix is not a version that the file held, the copy is wrong and the CLI reports success.
- An in-place overwrite between two reads gives a torn body of the advertised length. Both clients accept it and both copies are wrong.

A file that becomes shorter after `fstat` sends fewer bytes than Content-Length. Both clients detect this. The immutable binary oracle, a 64 MiB file and the immutable publication protocol from k7 were correct in every read.

## Question and scope

The leaf asked these questions:

1. Which file changes during a Linux download give a complete, correct copy?
2. Which give a detected failure, and which give a wrong copy with success?
3. Does the k7 publication protocol also give correct copies on Linux? That protocol writes a temporary name and renames it to a fresh name.
4. Does the agent stream with constant memory within the 150 s transfer bound?

The leaf does not check lifecycle-record corruption. It does not reopen the macOS inquiry.

## Code under test

The Linux agent serves `POST /download` in two places:

- `agents/linux/testanyware_agent/system_endpoints.py:109` `handle_download` calls `open(path, "rb")` once and returns the file object.
- `agents/linux/testanyware_agent/server.py:84` `_handle_download` calls `os.fstat(fileobj.fileno()).st_size`, sends that size as Content-Length and then runs `while chunk := fileobj.read(CHUNK_SIZE): self.wfile.write(chunk)`. `CHUNK_SIZE` is 1 MiB.

The server is a plain `HTTPServer` that answers with HTTP/1.0 and closes each connection. The checkout files and the files that Homebrew installed for Host CLI 2.8.0 have the same SHA-256 digests (see Accounting).

## Fixture

One container runs the pinned k5 image `python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016`. That image is Python 3.12.15 on Linux 7.0.14-linuxkit, aarch64. The container runs as uid 1000 with 512 MiB of memory and a limit of 128 processes. Docker publishes each port on 127.0.0.1 only. The case files live on the container file system (overlay), not on a bind mount.

`download_k14_seam.py` is the supervisor in the container. It starts two agents from the checkout, both read-only mounts:

- The **instrumented** agent runs the unchanged `AgentRequestHandler` and the unchanged `handle_download`. Three observers record the evidence at module boundaries. `system_endpoints.open` records the identity of the opened file and wraps it. Each read then records its offset, length and digest. It also records the inodes of the descriptor and of the path. `server.os.fstat` records the size that becomes Content-Length. A handler subclass records each socket write and keeps the body bytes.
- The **unchanged** agent runs `run_server` with no observer. It serves the stress reads and the publication control.

A hook in the instrumented agent changes the file at one event inside the agent process: `after-open`, `after-fstat` or `before-read-N`. It can also pause there until the host resumes it. The hook runs once, and the trace records the run.

The host controller `download_k14.py` uses two clients for each case. The raw client from k7 records the status, the Content-Length, the received bytes and how the connection ended. Host CLI 2.8.0 runs `testanyware file download --agent 127.0.0.1:PORT --json`. The controller records its exit, output, destination and temporary files.

No VM was used. The handler is Python file I/O, so a container with a Linux kernel runs the same code. The section Limits states what this leaves untested.

## Case matrix

The matrix was frozen before the recorded run. Each case uses a fresh file. The pending and terminal records are the retained k7 records of 25 and 123 bytes. A 3 MiB pattern file needs four reads of 1 MiB at most.

"Allowed" lists the versions that the file held during the case. Version 0 is the first version.

| Case | Change | Allowed |
|---|---|---|
| replace-longer-after-open | rename the 123-byte record over the 25-byte record after `open` | 25, 123 |
| replace-longer-after-fstat | the same, after `fstat` | 25, 123 |
| replace-shorter-after-fstat | rename the 25-byte record over the 123-byte record after `fstat` | 123, 25 |
| replace-between-reads | rename a 4 MiB pattern over a 3 MiB pattern before read 2 | 3 MiB, 4 MiB |
| truncate-after-open | truncate the 123-byte record to 25 bytes after `open` | 123, 25 |
| truncate-after-fstat | the same, after `fstat` | 123, 25 |
| truncate-between-reads | truncate the 3 MiB pattern to 1.5 MiB before read 2 | 3 MiB, 1.5 MiB |
| append-after-fstat | append the 123-byte record to the 25-byte record after `fstat` | 25, 148 |
| overwrite-longer-after-fstat | write the 123-byte record over the 25-byte record in place after `fstat` | 25, 123 |
| overwrite-between-reads | write another 3 MiB pattern in place before read 2 | 3 MiB, other 3 MiB |
| immutable-binary | none, the 5-byte oracle `00ff80410a` | oracle |
| missing-file | none, the path does not exist | none |
| agent-killed-mid-body | pause before read 2, then SIGKILL the agent | 3 MiB |
| client-abandons-mid-body | pause before read 2, then the client closes or receives SIGTERM | 3 MiB |
| large-immutable | none, a 64 MiB pattern | 64 MiB |

## Per-case evidence

Run `local-01`. "fstat" is the size that the agent advertised. "Sent" is the number of body bytes that the agent wrote to the socket. For each raw case, the bytes that the agent wrote equal the bytes that the raw client received.

| Case | fstat | Sent | Raw client | Host CLI |
|---|---|---|---|---|
| replace-longer-after-open | 25 | 25 | allowed-complete, version 0 | allowed-complete, version 0 |
| replace-longer-after-fstat | 25 | 25 | allowed-complete, version 0 | allowed-complete, version 0 |
| replace-shorter-after-fstat | 123 | 123 | allowed-complete, version 0 | allowed-complete, version 0 |
| replace-between-reads | 3,146,728 | 3,146,728 | allowed-complete, version 0 | allowed-complete, version 0 |
| truncate-after-open | 25 | 25 | allowed-complete, version 1 | allowed-complete, version 1 |
| truncate-after-fstat | 123 | 25 | detected-failure, end of file after 25 bytes | detected-failure, exit 1, `CONNECTION_DROPPED` |
| truncate-between-reads | 3,146,728 | 1,572,864 | detected-failure, end of file after 1,572,864 bytes | detected-failure, exit 1, `CONNECTION_DROPPED` |
| append-after-fstat | 25 | 148 | **framing-violation**, 148 bytes for a length of 25 | allowed-complete, the 25-byte prefix equals version 0 |
| overwrite-longer-after-fstat | 25 | 123 | **framing-violation**, 123 bytes for a length of 25 | **undetected-wrong**, exit 0, 25 bytes of version 1 |
| overwrite-between-reads | 3,146,728 | 3,146,728 | **undetected-wrong**, torn body | **undetected-wrong**, exit 0, torn body |
| immutable-binary | 5 | 5 | allowed-complete | allowed-complete |
| missing-file | none | none | detected-failure, status 400 | detected-failure, exit 1, `DOWNLOAD_FAILED` |
| agent-killed-mid-body | 3,146,728 | 1,048,576 | detected-failure, end of file after 1,048,576 bytes | detected-failure, exit 1, `CONNECTION_DROPPED` |
| client-abandons-mid-body | 3,146,728 | 2,097,152 | client-abandoned after 65,694 bytes | **interrupt-leak**, one temporary file |
| large-immutable | 67,108,864 | 67,108,864 | allowed-complete, 1.99 s | allowed-complete, 0.20 s |

The trace explains each class:

- **Replacement by rename.** The descriptor inode stayed the inode that `open` returned. The path inode changed after the rename. The agent read only the old inode, so it sent the old version with its correct length.
- **Truncation before `fstat`.** `fstat` saw the new size, so the agent advertised and sent version 1.
- **Truncation after `fstat`.** The read returned end of file early. The agent stopped and closed the connection before Content-Length. Both clients detected the short body.
- **Growth after `fstat`.** The read returned the new bytes up to end of file. The agent sent them all after a Content-Length of 25. The raw client received the extra bytes. The Host CLI kept the first 25 bytes and ignored the remainder.
- **In-place overwrite between reads.** Read 1 returned the first version and reads 2 to 4 returned the second version. The length did not change, so nothing on the wire showed the tear.
- **Agent killed.** The connection closed after 1 MiB. The CLI left no temporary file. A restarted agent answered health checks.
- **Client abandoned.** The resumed agent got `BrokenPipeError` on its third write and closed the file. The Host CLI, after SIGTERM while paused, left `.file.testanyware-42947.0.tmp` in the destination folder.

## Old-path failure

The macOS failure from k7 is not reproduced on Linux. The cause is in the code, and the trace shows it. The macOS handler looks the path up three times. The Linux handler opens it once and uses the descriptor for `fstat` and for every read. A rename between those calls cannot change the file that the agent reads.

The natural stress agrees. A writer in the container swapped the 25-byte and 123-byte records by rename 781,097 times in 10 s. The raw client read the path 15,766 times from the unchanged agent. Every read was allowed-complete: 7,723 reads of the 123-byte record and 8,043 reads of the 25-byte record, and no wrong byte.

## Controls

The checker accepts the run only when every control behaves:

| Control | Result |
|---|---|
| File semantics without HTTP, in the container | A descriptor opened before a rename reads the old file. After a truncation to 25, `fstat` gave 123 and the read gave 25 bytes. After an append, `fstat` gave 25 and the read gave 148 bytes. An overwrite between two reads gave a torn result. |
| Oracle comparator, on the host | It accepts the exact oracle and rejects a copy with one flipped bit and a copy with 4 of the 5 bytes. |
| Hook | Each armed hook ran at its event, and no arm stayed unused. |
| Agent bytes and received bytes | For each raw case with a body, the digests are equal. |
| Retained pending record | SHA-256 `b95c9261fb8f…` is unchanged. |
| Natural stress | 15,766 reads, all classified, writer stopped with no error. |
| Immutable publication | 200 records, 200 complete reads, 0 mismatches. Before each rename, 1,185 polls in total received status 400. |
| Streaming | Both 64 MiB transfers ended within the 150 s bound. Peak RSS of the agent rose by 0 KiB during the transfers. |
| Loopback | Every published port was bound to 127.0.0.1. |
| Disposal | The container was removed, and no container with the run label remained. |

The tests in `test_download_k14.py` give the checker 18 synthetic records. The checker must accept the good records and reject the others. For example, it must reject a body with a changed byte, an unused hook and agent bytes that differ from the received bytes.

## Differences from macOS

| Item | macOS (k7) | Linux (k14) |
|---|---|---|
| Path lookups for one download | 3 | 1 |
| Replacement by rename during a download | A complete frame with the wrong bytes is possible. | The old version is sent, complete and correct. |
| Growth in place after the size lookup | Not in the k7 matrix | Bytes after Content-Length. The CLI keeps a prefix and exits 0. |
| In-place overwrite between reads | Torn body accepted | Torn body accepted |
| Shrink after the size lookup | Detected | Detected |
| Immutable publication protocol | 200 of 200 correct | 200 of 200 correct |
| Host CLI SIGTERM during the body | Temporary file left | Temporary file left |

## Design implications

These points are for the design leaf. This leaf changes no production code.

1. **The publication protocol is the safe contract on both systems.** The writer writes a temporary file and renames it to a fresh name. This gave correct copies on macOS and on Linux. A reader that polls the fresh name until it answers 200 never received a wrong byte.
2. **Replacement of a stable name is safe on Linux but not on macOS.** A design that depends on that behaviour would be correct on one guest only.
3. **The Linux agent breaks HTTP framing when a file grows.** It must stop at the advertised length, or the length must come from the bytes that it sends. Now the extra bytes are lost only because each connection closes. A client that reuses connections would read them as the next response.
4. **No framing check can detect an in-place overwrite.** Only a writer that does not change a published file, or a digest of the content, can detect it. The agent sends no digest now.
5. **The Host CLI accepts a body that ends at Content-Length while the peer sends more.** It exits 0 for a prefix that was never a file version. The CLI could reject a response that has bytes after the advertised length.
6. **The Host CLI leaves a temporary file after SIGTERM.** This is the k7 finding, now seen on Linux too.

## Limits

- The fixture is a container on Docker Desktop, not a TestAnyware Linux guest. The guest kernel, the guest file system and the relay from the host app to the guest are not tested. The overlay file system gave the POSIX results in the file semantics controls. A guest with ext4 has the same POSIX semantics, but this run does not prove it.
- The forced cases change the file inside the agent process at an exact point. A writer in another process can reach the same points only by chance. The stress shows the natural rate for rename only.
- The trace observes module boundaries in the agent. It does not observe the kernel socket buffer. The raw client's bytes are the evidence for what crossed the network.
- The Host CLI ran with `--agent` to the published port. The VM-id path through the TestAnyware app was not used.
- One recorded run was made. Trials 1 and 2 ran before it to correct the seam, and they were not retained.

## Resource audit

| Resource | Bound | Observed |
|---|---|---|
| Whole controller | 600 s, GNU timeout | 19.5 s |
| Each transfer | 150 s | 1.99 s at most (64 MiB raw) |
| Each Host CLI | 150 s | 0.20 s at most |
| Container start and agent readiness | 60 s and 30 s | 0.10 s and 0.65 s |
| Container stop (guest cleanup) | 30 s | 0.22 s |
| Container removal (disposal) | 45 s | 0.02 s |
| Container memory and processes | 512 MiB, 128 | peak agent RSS unchanged during 64 MiB |
| Ports | 127.0.0.1 only | 3 ports, all on 127.0.0.1 |
| Host CLI temporary files | none expected | one, in client-abandons-mid-body, kept as evidence |
| Containers after the run | none | none, `containers-after.txt` is empty |

## Accounting

Source commit `950cf8829eebd0ac9b7c5208a75edfd4273da75d`, with the k14 files as the dirty diff in `local-01-outer/source-dirty.diff`. Host macOS 27.0.1. Docker 29.9.3. Host CLI 2.8.0. The tooling digests and the subject digests were the same before the probe and after the check.

After the run, one change went into `download_k14_check.py`. A relative RESULT now goes into OUT, as the task text states. Before, it went into the working directory. The recorded check used the default path, so this change does not affect it. The corrected checker re-judged `local-01` into `local-01/rejudge-k14.json`, and it gave the same verdict as `local-01/check.json`.

| Item | SHA-256 |
|---|---|
| `agents/linux/testanyware_agent/server.py`, checkout and installed | `b62de750472930c628dd4ae8a5bd118928f4efefb373c8365568acb3d0102a95` |
| `agents/linux/testanyware_agent/system_endpoints.py`, checkout and installed | `217770b9151037568b54683338c5cbcb2ad84e14170b357ba5f54cb2c35f81cd` |
| Host CLI `/opt/homebrew/Cellar/testanyware/2.8.0_1/bin/testanyware` | `44b08089cd01ebed55a786f9b86ca1bb80b53cd6b3778bd24a3e574c3030a340` |
| GNU timeout `/opt/homebrew/Cellar/coreutils/9.12_1/bin/gtimeout` | `6a7b0620313d337d70366cd4d3b969b49854abfeb1db602bdf3328122e223c6d` |
| `local-01/check.json` | `6df0f9d75e544246d2ef1e315424f7c4b6b196b380efd5efb6d74fc14c65696f` |
| `local-01/inventory.json` | `61962c9a87824257a5650d519ed0a9ccbcd6e7c5d3ecac01cd7506a835c9f914` |
| `local-01/controls.json` | `827b983ddebb79278d38d02f13b5c7e52716c0ffa578cb4816c7fb9601fcd3b1` |
| `local-01/stress.json` | `8d9ede15a10440bf89c2820d7eccfcef35a3e1484b943108b51368c8d80410ca` |
| `local-01/immutable.json` | `270fac162e3ccf83a5cf6c111445a5ee7d56f54eea90838e19eba2c696a5ad9b` |
| `local-01-outer/tooling-before.sha256` | `01b7e18cbcde408d4c7d83265c2c375a0823bba8f3065155f07433a42e7d14a4` |
| `local-01-outer/subjects-before.sha256` | `eaeda7f879b5f5eeeb5ede716aa02a0d9c94777e5d10ba7054aef28ce8a669ce` |

The evidence is in `experiments/exec-reliability/evidence/linux-download-consistency-k14/`. `local-01` holds the controller output, each case record with its allowed, received and emitted bytes, and the agent trace. The container log and the four agent logs were empty, and the `*.log` rule in `.gitignore` keeps them out of the repository. Bodies up to 8 MiB are kept as files. The 64 MiB body is kept by digest only. `local-01-outer` holds the identities, the digests and the check output.

## Commands

```sh
mise exec -- task diagnostic:linux-download-consistency-tools
mise exec -- task diagnostic:linux-download-consistency-container OUT=experiments/exec-reliability/evidence/linux-download-consistency-k14/local-02
mise exec -- task diagnostic:linux-download-consistency-check OUT=experiments/exec-reliability/evidence/linux-download-consistency-k14/local-01 RESULT=rejudge.json
```

The container task refuses an OUT that exists. A new run needs a new OUT.
