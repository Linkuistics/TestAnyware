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
        // holding the inherited stdout FD. The shell writes the sleep's
        // PID to a fixture file, so cleanup stops that process only.
        //
        // The owner starts the command in a new session, so the sleep runs
        // in the session of the exec leader. The test adopts that session
        // from the owner's result.
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("lingering-sleep.pid")
        let start = Date()
        let owned = await ProcessRunner.runOwned(
            command: "echo parent-done; sleep 30 & echo $! > '\(pidFile)'",
            timeoutSeconds: 5
        )
        let elapsed = Date().timeIntervalSince(start)
        let leaderPid = try XCTUnwrap(owned.leaderPid)
        await fixture.adopt(session: leaderPid)
        XCTAssertEqual(owned.remainingGroup, "present")
        let result = owned.legacy
        XCTAssertEqual(result.stdout, "parent-done")
        XCTAssertEqual(result.exitCode, 0)
        XCTAssertFalse(result.timedOut)
        // The bash itself exits in <100ms; we should not be stuck waiting
        // for the backgrounded sleep to drain its inherited FD.
        XCTAssertLessThan(elapsed, 4.0)
        // The teardown block stops the tracked sleep after the test.
        try await fixture.track("lingering-sleep.pid", command: "sleep")
    }

    func testDetachedRunReturnsImmediately() async throws {
        // The detached shell writes its own PID, then becomes the sleep
        // through exec. The fixture stops that process only.
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("detached-sleep.pid")
        let start = Date()
        try ProcessRunner.runShellDetached(
            command: "echo $$ > '\(pidFile)'; exec sleep 30"
        )
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 1.0)
        try await fixture.track("detached-sleep.pid", command: "sleep")
    }
}
