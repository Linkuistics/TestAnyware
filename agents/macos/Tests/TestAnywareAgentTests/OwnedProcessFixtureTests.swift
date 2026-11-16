import Darwin
import Foundation
import XCTest

/// Checks that `OwnedProcessFixture` refuses processes that it cannot bind to
/// the test, and that it keeps responsibility for a child when `track` is
/// cancelled (review k188, findings R1 and R4).
///
/// Each test starts its children through `Process` and keeps the handle, so
/// the test can stop a child that the fixture correctly refuses to signal.
final class OwnedProcessFixtureTests: XCTestCase {

    /// A PID file that names the test process itself: the fixture refuses it,
    /// even when the test process started inside the clock allowance.
    func testRefusesTheTestProcess() async throws {
        let fixture = try OwnedProcessFixture()
        let path = fixture.pidFile("self.pid")
        try "\(getpid())\n".write(toFile: path, atomically: true, encoding: .utf8)

        await assertNotOwned { try await fixture.track("self.pid", command: "sleep") }

        let report = await fixture.cleanUp()
        XCTAssertTrue(report.outcomes.isEmpty, "cleanup signalled \(report.outcomes)")
        XCTAssertEqual(report.unresolved.count, 1, "\(report.unresolved)")
        try? FileManager.default.removeItem(at: fixture.directory)
    }

    /// A PID file that names a process of this session that started before
    /// the fixture: the fixture refuses it, records nothing and leaves it alone.
    func testRefusesProcessThatStartedBeforeTheFixture() async throws {
        let older = try launch("/bin/sleep", ["30"])
        defer { stop(older) }
        // Wait beyond the clock allowance, so the start time alone refuses it.
        let allowance = Double(OwnedProcessFixture.clockAllowanceMicroseconds) / 1_000_000
        try await Task.sleep(nanoseconds: UInt64((allowance + 0.2) * 1_000_000_000))
        let fixture = try OwnedProcessFixture()
        let path = fixture.pidFile("older.pid")
        try "\(older.processIdentifier)\n".write(toFile: path, atomically: true, encoding: .utf8)

        await assertNotOwned { try await fixture.track("older.pid", command: "sleep") }

        let report = await fixture.cleanUp()
        XCTAssertTrue(report.outcomes.isEmpty, "cleanup signalled \(report.outcomes)")
        XCTAssertEqual(report.unresolved.count, 1, "\(report.unresolved)")
        XCTAssertTrue(older.isRunning, "the fixture stopped a process that it does not own")
        try? FileManager.default.removeItem(at: fixture.directory)
    }

    /// A process that started after the fixture but in another session: the
    /// fixture refuses it and never signals it.
    func testRefusesProcessInAnotherSession() async throws {
        let fixture = try OwnedProcessFixture()
        let path = fixture.pidFile("other-session.pid")
        // Foundation makes each child a process group leader, and `setsid`
        // fails for a group leader. So the retained Perl parent forks, and
        // the forked child starts the new session. The parent forwards
        // SIGTERM, so `stop(other)` also stops the child.
        let script = """
            use POSIX;
            my $c = fork() // die "fork: $!";
            if ($c == 0) {
                setsid() >= 0 or die "setsid: $!";
                open(F, '>', $ARGV[0]) or die; print F "$$\\n"; close F;
                exec 'sleep', '30';
            }
            $SIG{TERM} = sub { kill 'TERM', $c; waitpid($c, 0); exit 0 };
            waitpid($c, 0);
            """
        let other = try launch("/usr/bin/perl", ["-e", script, path])
        defer { stop(other) }

        await assertNotOwned { try await fixture.track("other-session.pid", command: "sleep") }

        let pid = try XCTUnwrap(pid_t(
            String(contentsOfFile: path, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        ))
        XCTAssertNotEqual(getsid(pid), getsid(0), "the control did not start a new session")
        let report = await fixture.cleanUp()
        XCTAssertTrue(report.outcomes.isEmpty, "cleanup signalled \(report.outcomes)")
        XCTAssertEqual(report.unresolved.count, 1, "\(report.unresolved)")
        let seen = ProcessIdentity.observe(pid: pid)
        XCTAssertEqual(seen?.command, "sleep")
        XCTAssertEqual(seen?.zombie, false, "the fixture stopped a process that it does not own")
        try? FileManager.default.removeItem(at: fixture.directory)
    }

    /// `track` is cancelled before the child writes its PID file. `cleanUp`
    /// still binds that child and stops it.
    func testCleanupStopsChildWhenTrackIsCancelled() async throws {
        let fixture = try OwnedProcessFixture()
        let path = fixture.pidFile("late.pid")
        let child = try launch("/bin/bash", ["-c", "sleep 0.5; echo $$ > '\(path)'; exec sleep 30"])
        defer { stop(child) }

        let tracking = Task { try await fixture.track("late.pid", command: "sleep") }
        try await Task.sleep(nanoseconds: 100_000_000)
        tracking.cancel()
        let result = await tracking.result
        XCTAssertThrowsError(try result.get())

        let report = await fixture.cleanUp()
        XCTAssertEqual(report.unresolved, [])
        XCTAssertEqual(report.outcomes.count, 1, "\(report.outcomes)")
        XCTAssertEqual(report.outcomes.first?.0.pid, child.processIdentifier)
        XCTAssertEqual(report.outcomes.first?.1, .stoppedByTerm)
        child.waitUntilExit()
        XCTAssertFalse(child.isRunning)
    }

    // MARK: - Helpers

    private func assertNotOwned(
        _ body: () async throws -> Void, file: StaticString = #filePath, line: UInt = #line
    ) async {
        do {
            try await body()
            XCTFail("track accepted a process that it cannot bind", file: file, line: line)
        } catch OwnedProcessFixture.FixtureError.notOwned {
            // Expected.
        } catch {
            XCTFail("unexpected error \(error)", file: file, line: line)
        }
    }

    private func launch(_ executable: String, _ arguments: [String]) throws -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        return process
    }

    /// Stops a child through its retained handle and reaps it.
    private func stop(_ process: Process) {
        if process.isRunning { process.terminate() }
        process.waitUntilExit()
    }
}
