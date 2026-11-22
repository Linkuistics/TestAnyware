# Apple Vision text recognition hangs its process on a macOS 27 host

**Status:** measured on 2026-11-22 by leaf `vision-ocr-hang-k12` of the issue #13 grove. The
host is macOS 27.2.1 (26B131) on an Apple M4 Max.

**Audience:** whoever maintains the macOS OCR engine, and whoever checks whether a later macOS
fixes the fault.

**Bottom line:** a `VNRecognizeTextRequest` sometimes never returns. After that, every later
request of the same process waits too. No public setting of the request avoids the hang. No
request in that process recovers, and a new process works at once. So the macOS engine runs Vision
in a worker child, which it can stop and start again. ADR-0002 records this in its section
"Vision runs in a worker child".

## What the user saw

In leaf `macos-golden-refresh-run-k11`, the base build of macOS 27.2.1 hung in 3 of 5 runs. Each
time it was on the Setup Assistant screen "account creation in progress". A `sample` of the hung
process showed the stack below. The live runs and their logs are in the running log of that
leaf.

## The reproduction

A scratch harness sent the call sequence of `testanyware-ocr-client/src/vision.rs` many times in
one process, on one synthetic frame of 1024x768. The frame is
`cli-rs/crates/testanyware-ocr-client/tests/fixtures/vision-stress-1024x768.png`. A watchdog
stopped the run when one request took more than 30 s. A warm request took 50 ms to 250 ms.

| Request setting | Runs of up to 3000 requests | Hangs |
|---|---|---|
| The device that Vision selects | 5 | at requests 444, 205, 163 and 2287, and 1 clean run |
| The CPU on the main compute stage | 5 | at requests 1576, 1576 and 2157, and 2 clean runs |

So the hang does not need a special frame. It is a chance event for each request, at about 1 in
1500 requests here. In the live runs it was much more frequent. The screen "account creation in
progress" showed it most often because the driver polls that screen for about 30 s, with one
request each second.

The ignored test `repeated_requests_return` in `vision.rs` repeats this check. Run it with
`task ocr:vision-hang-repro`.

## The stack of a hung process

The offline stack and the live stack of k11 are the same:

- The serial queue `com.apple.VN.detectorSyncTasksQueue.VNCRImageReaderDetector` runs
  `-[VNCRImageReaderDetector processRegionOfInterest:...]`. It waits on a semaphore inside the
  private framework `TextRecognition`.
- Two `ANEServicesThread` threads wait for Mach messages.
- The unified log of the process shows the Neural Engine requests of the hung request, the same
  as those of a good request. Nothing follows them. `aned` serves other processes during the
  hang.

The cause inside `TextRecognition` is not known.

## What does not recover or avoid it

- **Another request in the same process.** After a hang, the harness sent five more requests,
  each bounded by 20 s. They used a minimum text height, a custom word, revision 2, no language
  correction, and the setting of the engine. All five hung. Before the stress, each setting had
  returned in less than 250 ms. So the hang wedges Vision for the whole process.
- **The CPU device.** `setComputeDevice(_:for:)` with `MLCPUComputeDevice` exists from macOS 14
  ([Vision](https://developer.apple.com/documentation/vision/vnrequest/setcomputedevice(_:for:)),
  [Core ML](https://developer.apple.com/documentation/coreml/mlcpucomputedevice)). The CPU device
  still hung, as the table shows. The deprecated `usesCPUOnly` did not stop the use of the Neural
  Engine either.
- **A deadline alone.** Leaf k11 bounded each request by 30 s. In its second refresh, all three
  attempts of a retry in the same process timed out.

## A second finding: the first request of a new binary

The first request of a binary that `aned` has not seen takes 28 s to 36 s. A sample shows
`TextRecognition` in `-[_ANEClient compileModel:options:qos:error:]`, a synchronous XPC call to
`aned`. The CPU device and `usesCPUOnly` do not stop this compile. The next process of the same
binary took 0.2 s from start to answer.

A copy of a binary at a new path compiled again, and so
did a rebuild of the CLI at its old path. A rebuild of the harness at its old path did not. So
the key of that cache is not known.

This compile can follow each install and each build. So the first request of a worker has 90 s,
and a later request has 30 s.

## Where the findings landed

- ADR-0002, the section "Vision runs in a worker child": the engine runs Vision in the hidden
  command `testanyware _vision-ocr` and retries a request once on a new worker.
- `testanyware-ocr-client/src/engine.rs`: the deadlines and the retry.
