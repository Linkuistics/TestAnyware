import Darwin
import Foundation
import XCTest

/// Checks that `OwnedProcessFixture` refuses processes that it cannot bind to
/// the test, and that it keeps responsibility for a child when `track` is
/// cancelled (review k188, findings R1 and R4).
///
/// Each test starts its children as `ControlChild` values and keeps them, so
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
        // The retained Perl parent forks, and the forked child starts the
        // new session. A fork is never a group leader, so its `setsid`
        // cannot fail for that reason. The parent forwards SIGTERM, so
        // `stop(other)` also stops the child.
        //
        // Perl marks the pipe close-on-exec. So the parent reads end of file
        // when the child runs `sleep`, and only then writes the PID file.
        // The test never sees the PID before the child is `sleep`.
        let script = """
            use POSIX;
            pipe(my $r, my $w) or die "pipe: $!";
            my $c = fork() // die "fork: $!";
            if ($c == 0) {
                close $r;
                setsid() >= 0 or die "setsid: $!";
                exec 'sleep', '30';
            }
            close $w;
            $SIG{TERM} = sub { kill 'TERM', $c; waitpid($c, 0); exit 0 };
            my $b; sysread($r, $b, 1);
            open(F, '>', $ARGV[0]) or die; print F "$c\\n"; close F;
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

    private func launch(_ executable: String, _ arguments: [String]) throws -> ControlChild {
        try ControlChild(executable, arguments)
    }

    /// Stops a child through its retained handle and reaps it.
    private func stop(_ child: ControlChild) {
        if child.isRunning { child.terminate() }
        child.waitUntilExit()
    }
}

/// A control child that a test starts with `posix_spawn` and reaps by its
/// exact PID.
///
/// The controls do not use `Process`. At 27053381d8, `Process.waitUntilExit()`
/// blocked in 5 of 12 runs of these tests after the child had exited and
/// been reaped. Foundation reaps a `Process` child itself, so no process
/// remained to signal, and the wait never ended.
final class ControlChild {
    let processIdentifier: pid_t
    private var reaped = false

    init(_ executable: String, _ arguments: [String], environment extra: [String: String] = [:]) throws {
        var actions: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_addopen(&actions, 1, "/dev/null", O_WRONLY, 0)
        posix_spawn_file_actions_addopen(&actions, 2, "/dev/null", O_WRONLY, 0)
        let argv: [UnsafeMutablePointer<CChar>?] = ([executable] + arguments).map { strdup($0) } + [nil]
        let envp: [UnsafeMutablePointer<CChar>?] =
            ProcessInfo.processInfo.environment.merging(extra) { $1 }.map { strdup("\($0.key)=\($0.value)") } + [nil]
        defer {
            argv.forEach { free($0) }
            envp.forEach { free($0) }
        }
        // A Swift concurrency worker thread blocks every signal, and a
        // spawned child inherits that mask. Without these flags, the Perl
        // control never ran its SIGTERM handler.
        var attributes: posix_spawnattr_t?
        posix_spawnattr_init(&attributes)
        defer { posix_spawnattr_destroy(&attributes) }
        var none = sigset_t()
        var all = sigset_t()
        sigemptyset(&none)
        sigfillset(&all)
        posix_spawnattr_setsigmask(&attributes, &none)
        posix_spawnattr_setsigdefault(&attributes, &all)
        posix_spawnattr_setflags(&attributes, Int16(POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF))
        var pid: pid_t = 0
        let result = posix_spawn(&pid, executable, &actions, &attributes, argv, envp)
        guard result == 0 else { throw POSIXError(POSIXErrorCode(rawValue: result) ?? .EIO) }
        processIdentifier = pid
    }

    /// Reports whether the child has not exited. The check leaves an exited
    /// child unreaped.
    var isRunning: Bool {
        if reaped { return false }
        var info = siginfo_t()
        let result = waitid(P_PID, id_t(processIdentifier), &info, WEXITED | WNOHANG | WNOWAIT)
        return !(result == 0 && info.si_pid == processIdentifier)
    }

    func terminate() {
        if !reaped { kill(processIdentifier, SIGTERM) }
    }

    /// Waits for the child to exit, then reaps it.
    func waitUntilExit() {
        guard !reaped else { return }
        var status: Int32 = 0
        while waitpid(processIdentifier, &status, 0) < 0, errno == EINTR {}
        reaped = true
    }
}
