import XCTest
@testable import TestAnywareAgent

final class ProcessRunnerTests: XCTestCase {

    func testCapturesStdoutAndExitCode() async throws {
        let result = try await ProcessRunner.runShell(
            command: "echo hello", timeoutSeconds: 5
        )
        XCTAssertEqual(result.stdout, "hello")
        XCTAssertEqual(result.exitCode, 0)
        XCTAssertFalse(result.timedOut)
    }

    func testCapturesStderrSeparately() async throws {
        let result = try await ProcessRunner.runShell(
            command: "echo out; echo err >&2; exit 7", timeoutSeconds: 5
        )
        XCTAssertEqual(result.stdout, "out")
        XCTAssertEqual(result.stderr, "err")
        XCTAssertEqual(result.exitCode, 7)
        XCTAssertFalse(result.timedOut)
    }

    func testHandlesLargeOutputWithoutPipeDeadlock() async throws {
        // 256 KiB easily exceeds a typical 64 KiB pipe buffer; this would
        // deadlock the old pipe-based reader if it weren't draining
        // concurrently. With the file-based reader, size doesn't matter.
        let result = try await ProcessRunner.runShell(
            command: "head -c 262144 /dev/urandom | base64", timeoutSeconds: 10
        )
        XCTAssertEqual(result.exitCode, 0)
        XCTAssertFalse(result.timedOut)
        XCTAssertGreaterThan(result.stdout.count, 200_000)
    }

    func testTimeoutReturnsTimedOutFlag() async throws {
        let start = Date()
        let result = try await ProcessRunner.runShell(
            command: "sleep 30", timeoutSeconds: 1
        )
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertTrue(result.timedOut)
        XCTAssertEqual(result.exitCode, -1)
        XCTAssertTrue(result.stderr.contains("timed out"))
        // Allow generous overhead: must have terminated well before the
        // 30s sleep would naturally complete.
        XCTAssertLessThan(elapsed, 10.0)
    }

    func testDescendantHoldingFDDoesNotWedgeAfterParentExits() async throws {
        // The original wedge: bash forks a long-lived child that inherits
        // bash's stdout pipe; bash exits but the pipe never reaches EOF
        // because the grandchild still has the write end. With files this
        // works fine — the parent's exit is enough to consider the call
        // done, and any output from the lingering grandchild lands in
        // the file (which we read regardless).
        //
        // We simulate by having bash spawn a `sleep` that outlives bash:
        // the parent shell exits immediately while sleep hangs around
        // holding the inherited stdout FD.
        let start = Date()
        let result = try await ProcessRunner.runShell(
            command: "echo parent-done; sleep 30 &", timeoutSeconds: 5
        )
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertEqual(result.stdout, "parent-done")
        XCTAssertEqual(result.exitCode, 0)
        XCTAssertFalse(result.timedOut)
        // The bash itself exits in <100ms; we should not be stuck waiting
        // for the backgrounded sleep to drain its inherited FD.
        XCTAssertLessThan(elapsed, 4.0)
        // Best-effort cleanup of the lingering sleep so tests don't
        // accumulate background processes when run repeatedly.
        _ = try? await ProcessRunner.runShell(
            command: "pkill -f 'sleep 30' 2>/dev/null; true",
            timeoutSeconds: 2
        )
    }

    func testDetachedRunReturnsImmediately() throws {
        let start = Date()
        try ProcessRunner.runShellDetached(command: "sleep 30")
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 1.0)
        // Cleanup
        let cleanup = Process()
        cleanup.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        cleanup.arguments = ["-f", "sleep 30"]
        cleanup.standardOutput = FileHandle.nullDevice
        cleanup.standardError = FileHandle.nullDevice
        try? cleanup.run()
        cleanup.waitUntilExit()
    }
}
