# Memory

## Darwin pipe EOF requires all write-FD holders to exit
`readDataToEndOfFile()` blocks until every process holding the write end of the pipe exits, not just the direct child. Long-lived bash descendants inherit the write FD and prevent EOF from ever arriving.

## ProcessRunner uses temp-file capture, not pipes
`TestAnywareAgent.ProcessRunner` writes stdout/stderr to temp files instead of pipes. This removes the all-holders-must-exit invariant and is the correct pattern for capturing output from commands that spawn subprocesses.

## Foundation Process does not call setsid or setpgrp
`Process` leaves children in the same process group as the parent. When bash exits, its children are reparented to launchd; the process tree is unrecoverable at that point.

## pgrep -P snapshot must precede killing bash
Take a `pgrep -P <pid>` snapshot before terminating bash. Descendants reparented to launchd after bash exits cannot be found by PID tree traversal afterward.

## Process-tree kill after exec is best-effort
Descendants spawned between the `pgrep -P` snapshot and the kill signal can leak as orphans. Foundation `Process` cannot place children in their own process group, so there is no race-free solution.

## testanyware vm stop requires explicit VM id
`testanyware vm stop` takes a required VM id; no auto-discovery because QEMU has no registry. VNC viewer close is handled in Swift by `VMLifecycle.stop`.

## URLSession timeout must exceed agent exec deadline
`AgentTCPClient` passes `timeout + 10s` to URLSession for exec calls. If URLSession and the agent deadline are equal, URLSession can fire first and report a client-side timeout before the agent's `timedOut: true` response arrives.

## exec HTTP response carries timedOut field
The `/exec` response includes `timedOut: Bool` to distinguish a deadline-expired run from a non-zero exit code. A timed-out exec returns `timedOut: true` and `exitCode: -1`.

## OCR dispatch: Vision in-process on macOS, bridged on others
`TestAnywareServer` sends `/ocr` to `Vision.framework` in-process on macOS. On Linux and Windows it uses `OCRChildBridge`, a stdin/stdout JSON protocol to a Python EasyOCR daemon started with `--daemon`. Set `TESTANYWARE_OCR_FALLBACK=1` to force the bridge path on macOS.

## exec fix works for long-running commands in-VM
Long-running commands (e.g. package installs) exit cleanly with correct exit codes. Verified via `scp` + `launchctl unload/load` against a running tart VM — no golden rebuild needed for exec-only verification.

## Agent hot-swap does not re-grant TCC accessibility
Replacing `/usr/local/bin/testanyware-agent` and reloading the LaunchAgent leaves `/health` returning `accessible: false`. TCC pins the grant to the original binary's csreq (CDHash for adhoc-signed binaries). Exec, upload, and download still work; AX tests require a golden rebuild or explicit TCC re-grant.

## vm-start.sh writes XDG-compliant connect spec per VM
`vm-start.sh` writes spec + meta sidecar under `$XDG_STATE_HOME/testanyware/vms/<id>/`; `vm-stop.sh` removes it. Per-VM storage replaces the former single `~/.testanyware/connect.json` that was clobbered by a second `vm-start.sh`.

## VM ids use testanyware-<hex8> format
`vm-start.sh` generates `testanyware-<hex8>` identifiers for each VM clone. All lifecycle scripts and the Swift CLI use this id to address a specific VM.

## CLI VM resolution: --connect → --vm → flags → env → error
`ConnectionOptions.resolve()` tries in order: `--connect` (explicit spec path), `--vm` (named id), explicit host/port flags, `TESTANYWARE_VM_ID` env var, XDG connect-spec file, then error. Subshells and fresh terminals can pick up the running VM without explicit plumbing.

## _testanyware-paths.sh is shared by lifecycle scripts
All VM lifecycle shell scripts source `_testanyware-paths.sh` for consistent XDG path computation. Scripts are normal subprocesses, not sourced fragments.

## XDG state holds ephemeral VM files; data holds persistent files
VM spec and meta sidecar files are written under `$XDG_STATE_HOME/testanyware/vms/` and removed by `vm-stop.sh`. Clone directories and golden images are written under `$XDG_DATA_HOME/testanyware/` and persist across VM lifecycle.

## FindTextCommand delegates OCR to server
`FindTextCommand` calls the server's `/ocr` endpoint rather than Vision.framework in-process. The command has no platform-specific code; OCR platform selection happens entirely in `TestAnywareServer`.

## ConnectionSpec.namedSpecPath duplicates VMPaths logic
`ConnectionSpec.namedSpecPath(for:)` and `VMPaths` both compute named-spec paths under the XDG state directory. Consolidation is deferred; changing one without the other will create drift.

## VM spec/meta writers use temp-file, chmod 0600, replaceItemAt
`VMMeta.writeAtomic(to:)` and `VMSpec.writeAtomic(to:)` write JSON to a sibling `.tmp` file, chmod it to 0600, then call `replaceItemAt` with a `moveItem` fallback. All VM-owned files are readable only by the owner.

## VMMeta.pid and VMSpec.platform are non-optional
`VMMeta.pid` is `Int` (not `Int?`) and `VMSpec.platform` is `Platform` (not `Platform?`). bash-written files with null fields get rewritten on next run; Swift-side creation always supplies real values.

## VMSpec wraps VNCSpec, AgentSpec, Platform directly
`VMSpec` holds `vnc: VNCSpec`, `agent: AgentSpec?`, and `platform: Platform` without duplicating those types. Changes to `VNCSpec`/`AgentSpec` propagate automatically; do not introduce parallel wrapper types.

## All VM tests use swift-testing, not XCTest
Unit tests (`VMPathsTests`, `VMMetaTests`, `VMSpecTests`) and integration tests (`VMLifecycleTests`) use swift-testing (`@Test`, `#expect`). XCTest and swift-testing cannot coexist in the same target. Plan sketches referencing XCTest for this suite are incorrect.

## VirtioFS can serve stale cached content
VirtioFS does not always propagate host-side file updates to the guest immediately. Workaround: transfer files via `testanyware scp` (base64 over SSH) or restart the VM. Do not rely on VirtioFS for files that are edited on the host and read back in the VM within the same session.

## Menu bar accessibility tree is lazy
`testanyware agent snapshot --window "Menu Bar"` only exposes top-level menu bar items. Submenus do not appear in the tree until the menu is open. Verifying or interacting with submenu items requires opening the menu first (e.g. via VNC click), then re-snapshotting.

## VMTypes.swift extends Platform instead of defining VMPlatform
VM lifecycle code uses the existing `Platform` enum, extended in `VM/VMTypes.swift` with `defaultBase: String` and `backend: VMBackend` properties. No separate `VMPlatform` type exists — the plan's sketch of `VMPlatform` was superseded to match the same "no parallel wrapper types" precedent that governs VMSpec. Downstream plan text referring to `VMPlatform(rawValue:)` should be read as `Platform(rawValue:)`.

## OCRChildBridge tempFileCleanedUpAfterCall is flaky under parallel tests
`OCRChildBridgeTests.tempFileCleanedUpAfterCall` scans `NSTemporaryDirectory()` for leftover `testanyware-ocr-*.png` files, so a parallel OCR test writing its own temp PNG can cause a false failure. Passes in isolation. Not a regression; do not chase.

## TartRunner swallows external-schema errors at the boundary
`TartRunner.parseList` returns `[]` on malformed JSON or schema mismatch, and `runList` returns `[]` on non-zero exit or a missing `tart` binary. Deliberate: a future `tart list --format json` upgrade must not break `testanyware vm list`. The leniency lives only at the tart boundary — internal Swift parsers stay strict. `parseVNCURL`, by contrast, throws `TartRunnerError.vncURLMalformed` because a bad VNC URL means we cannot connect at all, so the caller must know.

## TartRunner.TartVM uses CodingKeys for PascalCase JSON
The private `TartVM` Decodable struct in `TartRunner.swift` declares properties in camelCase (`name`, `state`, `disk`) and maps them to tart's `Name`/`State`/`Disk` keys via `CodingKeys`. Mirrors VMMeta's `clone_dir` → `cloneDir` mapping. Do not reintroduce PascalCase Swift properties to match the JSON verbatim; it triggers Swift linter warnings.

## VMListFormatter column widths are byte-for-byte parity with vm-list.sh
`VMListFormatter.render` reproduces `scripts/macos/vm-list.sh`'s printf format strings exactly: goldens `"  %-8s %-40s %-8s %s"`, running `"  %-20s %-8s %-30s %-24s PID %s"`. Nil `sizeGB` renders as `"? GB"`; nil `pid` renders as `"?"`. Changing widths breaks downstream grep/awk consumers.

## vm list calls TartRunner.runList once and partitions
`VMCommand.List.run()` invokes `TartRunner.runList()` once and partitions the returned `[VMListEntry]` into goldens/running by `kind`. The plan sketch showed two separate calls; the single call avoids a redundant `tart list --format json` subprocess. Do not revert.

## vm-list/start/stop/delete.sh are thin exec wrappers
`vm-list.sh`, `vm-start.sh`, `vm-stop.sh`, and `vm-delete.sh` all `exec testanyware vm <subcommand> "$@"`. The bash implementations are gone; logic lives in `VM/QEMURunner.swift`, `VM/TartRunner.swift`, and `VM/VMListFormatter.swift`. Column-width parity preserved so existing grep/awk consumers still work.

## Task.sleep cross-executor cancel crashes in -O builds
Cancelling `Task.sleep(for: Duration)` from a different executor in optimised builds triggers a `swift_task_dealloc` LIFO violation → SIGABRT. Fix: use a monotonic epoch counter; bump it to invalidate in-flight sleeps; each fire-and-forget task checks epoch on wake and no-ops if stale.

## recordingTask uses cancel-sleep pattern; latent crash risk
`recordingTask` cancels a sleeping `Task` — the same pattern that caused the idle-timer SIGABRT. Apply the epoch-counter pattern (see above) before enabling recording under load.

## macOS ps does not support the sid keyword
`ps -o sid=` works on Linux but not on macOS — `ps` rejects it with "keyword not found" (valid alternatives: `sess`, `pgid`, `tpgid`). For session-ID assertions in Swift, call `Darwin.getsid(pid_t(pid))` on the child while it's still alive. The `DetachedProcessTests.spawnsAProcessInItsOwnSession` test uses this pattern. Do not reintroduce `ps -o sid`.

## DetachedProcess is the setsid escape hatch
`VM/DetachedProcess.swift` uses `posix_spawn` + `POSIX_SPAWN_SETSID` to spawn children that are session + process-group leaders. Memory notes "Foundation Process does not call setsid or setpgrp" and "pgrep -P snapshot must precede killing bash" describe the problem this solves. Use `DetachedProcess.spawn` for any VM-lifecycle child that must outlive the Swift parent. Do not substitute Foundation `Process` for long-running VM processes.

## VMLifecycle.delete auto-detects backend; refuses if clones present
`VMLifecycle.delete(name:force:)` picks tart if the name appears in `TartRunner.runList()`, else QEMU if a `.qcow2` exists. Refuses with `runningClonesPresent` when live clones are detected unless `--force` is passed. Error cases: `goldenNotFound`, `runningClonesPresent`, `tartDeleteFailed`. No `.notFound` case — mirror `VMLifecycleError.stopFailed` precedent.

## backingFile uses qemu-img info JSON, not python3
`QEMURunner.backingFile(ofQcow2:)` runs `qemu-img info --output=json` and parses the result with `JSONSerialization`. Replaces the bash approach of `python3 -c`. Path to `qemu-img` discovered via `TartRunner.which("qemu-img")` with homebrew fallback — consistent with `QEMURunner.start` pattern.

## 2 QEMU integration tests fail from swtpm sun_path limit
`QEMURunnerIntegrationTests` has 2 pre-existing failures caused by macOS `sun_path` length limit for the swtpm socket path. Tracked as backlog Task 12; independent of all other work. A passing suite shows 270/272; treat 2 QEMU integration failures as expected baseline.

## QEMURunner discovers VMs by directory scan
`QEMURunner` finds golden images by scanning `$XDG_DATA_HOME/testanyware/goldens/` for `.qcow2` files, and running clones by scanning `$XDG_DATA_HOME/testanyware/clones/` subdirectories for a `monitor.sock` file. No registry; presence of the socket means the VM is live.

## tart vnc-url appends trailing ellipsis
`tart`'s `vnc-url` subcommand appends `...` to URLs to indicate truncation. `TartRunner.parseVNCURL` strips the trailing `...` before handing the string to `URL(string:)`. Raw tart output is not a valid URL.

## VNCViewer uses AppleScript; no unit tests
`VM/VNCViewer.swift` opens, captures a window reference, and closes VNC Viewer via AppleScript. Unit tests are explicitly excluded from the plan for this file — AppleScript UI automation cannot be exercised without a live display.

## TestHealthServer uses raw BSD sockets
`TestHealthServer` in `AgentHealthWaiterTests.swift` binds and accepts connections using BSD socket calls directly (no `URLSession` or `NWListener`). Pattern for serving minimal HTTP responses in tests without introducing framework-level dependencies.

## kill(pid, 0) probes pid ownership before SIGTERM
`VMLifecycle.stop` calls `kill(pid, 0)` before sending `SIGTERM`. A zero-signal kill fails if the pid has been recycled, preventing an unintended signal to a wrong process. Omitting this step is a correctness bug.

## P3 plan code sketches use outdated constructors
VNCSpec and AgentSpec init signatures in P3 plan sketches differ from the landed Swift implementations. Cross-reference: "VMTypes.swift extends Platform instead of defining VMPlatform" and "VMSpec wraps VNCSpec, AgentSpec, Platform directly". Treat all P3/P4 plan code as guidance; compile-check against landed types before use.

## VMLifecycleError has stopFailed, not notFound
`VMLifecycle.stop` throws `VMLifecycleError.stopFailed` for absent tart VMs. No `.notFound` case exists. Plan sketches expecting `.notFound` are incorrect.

## swift-testing lifecycle tests use do/cleanup-on-throw guard
swift-testing has no `setUp`/`tearDown` hooks. `VMLifecycleTests` uses a do block: start the VM, assert, then run cleanup on both the success path and any throw path before re-throwing. Works uniformly across all lifecycle tests without per-test teardown.

## Linux tart VMs boot ~38% faster than macOS
Smaller disk + lighter desktop: Linux round-trip is ~11.7 s vs macOS ~19 s. Set integration test time limits accordingly; the current 5-minute limit has headroom on both platforms.

## SSH wait dominates macOS VM start latency
SSH readiness polling accounts for ~11s of the ~26s macOS start round-trip. Disabling SSH in the golden image (backlog Task 5) would eliminate this cost entirely.

## VMLifecycle.stop runs VNC-viewer close for missing ids
`VMLifecycle.stop` executes the VNCViewer close prelude even when the tart VM id does not exist. The TCC gate short-circuits the rest; no user-visible failure, but the path is longer than necessary.

## AX walker descends through NSStackView correctly
`AXElementWrapper` traverses NSStackView children without barriers. Anonymous `NSTextField` instances inside NSStackView lack AX labels; `label()` falls through to `kAXPlaceholderValueAttribute` when both title and description are empty. The unlabelled-field symptom ("Multiple elements matched") is fixed; the traversal was never the problem.

## set-value --index is 1-based; 0 returns .notFound
`--index 0` always returns `.notFound`. Users encountering "Multiple elements matched" must pass `--index 1` (or higher). Zero is not a valid index value.

## `--window` flag includes drop-shadow in AX origin
On Tahoe, the AX-reported window origin includes the drop-shadow inset. Coords passed with `--window <name>` to `testanyware input click` land ~40 px below the intended position. Use screen-absolute coords from a full-screen `testanyware screenshot` instead of window-relative coords when click precision matters. Documented in `--window` help text and README Gotchas.

## swift-testing .enabled(if:) evaluates at discovery time
`.enabled(if:)` conditions on a `@Test` or `@Suite` are evaluated when the test runner discovers tests, not at execution time. Use it for TCC checks and env-var gates; do not rely on runtime guards inside the test body for skipping.

## testanyware-agent lacks TCC Documents access on Tahoe golden
`testanyware exec ls /Users/admin/Documents/` triggers a TCC privacy dialog on the Tahoe VM. The agent binary does not have a Documents folder or full-disk access TCC grant on Tahoe. Automated scripts that inspect file-save results in Documents will stall waiting for the dialog. Backlog Task 10 tracks the fix (TCC grant or entitlement during golden build).

## UnifiedRole.swift is duplicated; both copies must stay in sync
`UnifiedRole.swift` exists in both the CLI target and the agent-side target. Adding or removing a `UnifiedRole` case requires updating both files; `UnifiedRoleTests` checks `allCases.count` and will fail with a stale expected value if only one copy is updated.

## QEMUMonitorClient speaks HMP via nc -U, not NWConnection
`QEMUMonitorClient.send` pipes `command\n` through `/bin/sh -c "(printf ...; sleep) | /usr/bin/nc -U $sock"`, with the command, timeout, and socket path passed as positional `sh -c` args (`$1`, `$2`, `$3`). Matches the original bash byte-for-byte and avoids `Network.framework`'s idiosyncratic Unix-socket semantics. When the subshell's sleep expires, the pipe EOFs, nc closes its end, QEMU sees EOF and flushes — giving a clean response terminus without prompt sniffing. Pure parsers (`parseAgentPort`, `parseVNCPort`) are `static` for unit testability.

## QEMU backend uses DetachedProcess for qemu, Foundation Process for swtpm
`QEMURunner.start` spawns the `qemu-system-aarch64` binary via `DetachedProcess` (posix_spawn + SETSID) so the VM outlives the parent cleanly. swtpm, by contrast, goes via a plain Foundation `Process` run synchronously because it daemonises itself (`--daemon`) and exits — there's nothing for the parent to hold open.

## swtpm teardown path is under clone/<id>-tpm, not clone/tpm
`QEMURunner.stop` locates swtpm via `pgrep -f "swtpm.*<cloneDir>/<cloneName>-tpm"`. Older `vm-stop.sh` used `<cloneDir>/tpm/sock` which never matched the path `vm-start.sh` actually writes — so the bash stop path had been silently leaking an orphan swtpm process per run. The Swift port fixed this as part of the P4 migration.

## Swift 6 disallows Thread.sleep in async contexts
All three in-`async`-function delays in `QEMURunner.start` use `try? await Task.sleep(nanoseconds:)`. `Thread.sleep(forTimeInterval:)` emits `Class method 'sleep' is unavailable from asynchronous contexts` under Swift 6. Remains valid in synchronous functions like `QEMURunner.stop`.
