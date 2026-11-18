import Darwin
import Foundation
import os
import XCTest

/// The kernel identity of one process: its PID and its start time.
///
/// A PID alone can name a different process after the first one exits. The
/// kernel start time does not change across `exec`, so the pair names one
/// process for its whole life. The command name is kept for verification and
/// reports. It is not part of the identity, because it changes at `exec`.
struct ProcessIdentity: Equatable, Sendable, CustomStringConvertible {
    let pid: pid_t
    let startSeconds: UInt64
    let startMicroseconds: UInt64

    var description: String {
        "pid \(pid) started \(startSeconds).\(String(format: "%06llu", startMicroseconds))"
    }

    /// The state of a process, read from `proc_pidinfo(PROC_PIDTBSDINFO)`.
    struct Observation: Sendable {
        let identity: ProcessIdentity
        let command: String
        let zombie: Bool
        let parent: pid_t
    }

    /// Reads the process that holds `pid` now, or nil when no process holds it.
    static func observe(pid: pid_t) -> Observation? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.stride)
        guard proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size else {
            return nil
        }
        let command = withUnsafeBytes(of: info.pbi_comm) {
            String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self)
        }
        return Observation(
            identity: ProcessIdentity(
                pid: pid,
                startSeconds: info.pbi_start_tvsec,
                startMicroseconds: info.pbi_start_tvusec
            ),
            command: command,
            zombie: info.pbi_status == UInt32(SZOMB),
            parent: pid_t(info.pbi_ppid)
        )
    }

    /// True when this exact process exists and has not exited. A zombie has
    /// exited and runs no code, so it does not count as alive.
    var isAlive: Bool {
        guard let now = Self.observe(pid: pid) else { return false }
        return now.identity == self && !now.zombie
    }
}

/// Long-lived children that one test starts, identified positively.
///
/// The test's shell command writes the child's PID into a file in a
/// directory that only this fixture uses (`echo $$` before `exec`, or `echo $!`
/// after `&`). `pidFile` registers the file's name. `track` reads the PID and
/// binds the process to this test only when all three conditions hold:
///
/// - the process started after the fixture was created,
/// - the process runs in the session of the test process, or in a session
///   that the test adopted with `adopt(session:)`,
/// - the process is neither the test process nor one of its ancestors, and
/// - its PID and start time stay the same across the check.
///
/// A process that fails the binding is refused and never signalled. A bound
/// child is recorded at once, so cancellation of `track` cannot drop it.
/// `cleanUp` signals a recorded child only while its PID still carries the
/// recorded start time. It also resolves each registered name that `track`
/// did not bind. The fixture never selects a process by name.
///
/// Limits: the kernel can reuse a PID between the identity check and `kill`.
/// That window is a few microseconds, and Darwin has no process handle that
/// would close it. A process of the same session that starts after the
/// fixture and takes the child's PID before `track` reads it also passes the
/// binding. That needs the child to exit and its PID to come back inside one
/// test. Foundation gives each child its own process group, so the group
/// cannot narrow the binding further.
///
/// The exec owner starts each command in a new session, so a child that the
/// command leaves runs in that session, not in the test session. The test
/// adopts that session through the leader PID in the owner's result. The
/// session ID outlives the reaped leader while members remain. A new session
/// that reuses the ID inside one test also passes the session check, but its
/// processes must still start after the fixture and appear in a PID file.
actor OwnedProcessFixture {
    enum FixtureError: Error, CustomStringConvertible {
        case noPidFile(String)
        case notOwned(String)

        var description: String {
            switch self {
            case .noPidFile(let detail), .notOwned(let detail): return detail
            }
        }
    }

    /// What `cleanUp` did with one recorded child.
    enum CleanupOutcome: Equatable, Sendable {
        case alreadyGone
        case stoppedByTerm
        case stoppedByKill
        case survived
    }

    /// What `cleanUp` did: one outcome for each child that it could identify,
    /// and one explanation for each registered PID file that it could not
    /// resolve. An unresolved file can leave a launched child running.
    struct CleanupReport: Sendable {
        var outcomes: [(ProcessIdentity, CleanupOutcome)] = []
        var unresolved: [String] = []
    }

    /// Bound for a child to write its PID file and finish its `exec`.
    static let trackBound: TimeInterval = 5
    /// Grace after SIGTERM, then the bound after SIGKILL (the brief's 2 s each).
    static let termGrace: TimeInterval = 2
    static let killBound: TimeInterval = 2
    /// Allowance for the wall clock between this process and the kernel.
    static let clockAllowanceMicroseconds: UInt64 = 1_000_000

    let directory: URL
    private let notBeforeMicroseconds: UInt64
    /// The session of the test process. Its children inherit it.
    private let session: pid_t
    /// Names that `pidFile` handed out. It is nonisolated, so a lock guards it.
    private let registered = OSAllocatedUnfairLock(initialState: Set<String>())
    /// Exec sessions that the test adopted from the owner's result.
    private var adoptedSessions: Set<pid_t> = []
    private var tracked: [ProcessIdentity] = []
    private var trackedNames: Set<String> = []

    init() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-process-fixture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        notBeforeMicroseconds = Self.wallClockMicroseconds()
        session = getsid(0)
    }

    /// Creates a fixture and registers its cleanup as a teardown block of
    /// `testCase`, so the cleanup runs even when the test fails or throws.
    static func make(for testCase: XCTestCase) throws -> OwnedProcessFixture {
        let fixture = try OwnedProcessFixture()
        testCase.addTeardownBlock {
            let report = await fixture.cleanUp()
            for (identity, outcome) in report.outcomes {
                FileHandle.standardError.write(Data(
                    "owned-process-fixture: \(identity) \(outcome)\n".utf8
                ))
                XCTAssertNotEqual(outcome, .survived, "owned child \(identity) survived cleanup")
            }
            for detail in report.unresolved {
                FileHandle.standardError.write(Data("owned-process-fixture: unresolved \(detail)\n".utf8))
                XCTFail("owned-process-fixture could not resolve a launched child: \(detail)")
            }
        }
        return fixture
    }

    /// The path of the PID file `name` in this fixture's directory, which
    /// `cleanUp` then resolves. The directory name holds only a UUID, so it is
    /// safe inside single quotes. Call it before the child starts.
    nonisolated func pidFile(_ name: String) -> String {
        registered.withLock { _ = $0.insert(name) }
        return directory.appendingPathComponent(name).path
    }

    /// Accepts children in the session that `leaderPid` leads. The value comes
    /// from the exec owner's result for a command that this test ran.
    func adopt(session leaderPid: pid_t) {
        adoptedSessions.insert(leaderPid)
    }

    /// Reads the PID that the child wrote to `pidFile(name)`, binds the child
    /// to this test and records it. Waits up to `trackBound` for the file and
    /// for the child's command to become `command`.
    @discardableResult
    func track(_ name: String, command: String) async throws -> ProcessIdentity {
        let path = pidFile(name)
        let deadline = Date().addingTimeInterval(Self.trackBound)
        var pid: pid_t?
        var bound: ProcessIdentity?
        var lastCommand = ""
        while Date() < deadline {
            if pid == nil { pid = Self.readPid(path) }
            if let pid {
                switch sight(pid) {
                case .gone:
                    throw FixtureError.notOwned("pid \(pid) from \(path) exited before it ran \(command)")
                case .foreign(let reason):
                    throw FixtureError.notOwned("\(reason), so it is not this test's child")
                case .owned(let seen):
                    if let bound, seen.identity != bound {
                        throw FixtureError.notOwned("pid \(pid) changed from \(bound) to \(seen.identity)")
                    }
                    if bound == nil {
                        // The binding holds now, so the fixture owns this child
                        // whatever happens to `track` from here on.
                        bound = seen.identity
                        record(seen.identity, name: name)
                    }
                    if seen.command == command { return seen.identity }
                    lastCommand = seen.command
                }
            }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        guard let pid else {
            throw FixtureError.noPidFile("no PID in \(path) within \(Self.trackBound) s")
        }
        throw FixtureError.notOwned("pid \(pid) runs \(lastCommand), not \(command)")
    }

    /// Stops every recorded child that is still alive, resolves and stops
    /// each registered child that `track` did not bind, then removes the
    /// directory. SIGTERM first, SIGKILL after `termGrace`.
    func cleanUp() async -> CleanupReport {
        var report = CleanupReport()
        let pending = registered.withLock { $0 }.subtracting(trackedNames).sorted()
        for name in pending {
            if let detail = await resolve(name) { report.unresolved.append(detail) }
        }
        for identity in tracked {
            report.outcomes.append((identity, await Self.stop(identity)))
        }
        tracked.removeAll()
        trackedNames.removeAll()
        if report.unresolved.isEmpty {
            try? FileManager.default.removeItem(at: directory)
        }
        return report
    }

    /// Signals `identity` only while its PID still carries its start time.
    static func stop(_ identity: ProcessIdentity) async -> CleanupOutcome {
        guard identity.isAlive else { return .alreadyGone }
        kill(identity.pid, SIGTERM)
        if await waitForExit(identity, seconds: termGrace) { return .stoppedByTerm }
        guard identity.isAlive else { return .stoppedByTerm }
        kill(identity.pid, SIGKILL)
        if await waitForExit(identity, seconds: killBound) { return .stoppedByKill }
        return .survived
    }

    // MARK: - Binding

    private enum Sighting {
        /// No live process holds the PID, so the child has exited.
        case gone
        /// A live process holds the PID, but nothing binds it to this test.
        case foreign(String)
        /// The process holding the PID is bound to this test.
        case owned(ProcessIdentity.Observation)
    }

    /// Checks whether the process that holds `pid` now is this test's child.
    private func sight(_ pid: pid_t) -> Sighting {
        // The test process and its ancestors can share the session and start
        // inside the clock allowance, but a child of the test is never one
        // of them.
        guard !Self.selfAndAncestors().contains(pid) else {
            return .foreign("pid \(pid) is the test process or one of its ancestors")
        }
        guard let before = ProcessIdentity.observe(pid: pid), !before.zombie else { return .gone }
        let started = before.identity.startSeconds * 1_000_000 + before.identity.startMicroseconds
        guard started + Self.clockAllowanceMicroseconds >= notBeforeMicroseconds else {
            return .foreign("pid \(pid) started before this fixture")
        }
        let processSession = getsid(pid)
        // A second observation shows that the session belongs to the same
        // process. A changed identity means the first process exited.
        guard let after = ProcessIdentity.observe(pid: pid), !after.zombie,
              after.identity == before.identity else { return .gone }
        guard processSession == session || adoptedSessions.contains(processSession) else {
            return .foreign("pid \(pid) runs in session \(processSession), not test session \(session)")
        }
        return .owned(after)
    }

    private func record(_ identity: ProcessIdentity, name: String) {
        tracked.append(identity)
        trackedNames.insert(name)
    }

    /// Binds and records the child of a registered name that `track` did not
    /// bind. Returns an explanation when no child can be identified.
    private func resolve(_ name: String) async -> String? {
        let path = pidFile(name)
        let deadline = Date().addingTimeInterval(Self.trackBound)
        var pid = Self.readPid(path)
        while pid == nil, Date() < deadline {
            try? await Task.sleep(nanoseconds: 10_000_000)
            pid = Self.readPid(path)
        }
        guard let pid else {
            return "no PID in \(path) within \(Self.trackBound) s, so a launched child may remain"
        }
        switch sight(pid) {
        case .gone:
            return nil
        case .foreign(let reason):
            return "\(reason), so cleanup did not signal it and the child from \(path) is unknown"
        case .owned(let seen):
            record(seen.identity, name: name)
            return nil
        }
    }

    /// The PIDs from this process up its parent chain to launchd.
    private static func selfAndAncestors() -> Set<pid_t> {
        var chain: Set<pid_t> = []
        var current = getpid()
        while current > 0, chain.insert(current).inserted {
            guard let seen = ProcessIdentity.observe(pid: current) else { break }
            current = seen.parent
        }
        return chain
    }

    private static func readPid(_ path: String) -> pid_t? {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8), text.hasSuffix("\n") else {
            return nil
        }
        return pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    private static func waitForExit(_ identity: ProcessIdentity, seconds: TimeInterval) async -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if !identity.isAlive { return true }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        return !identity.isAlive
    }

    private static func wallClockMicroseconds() -> UInt64 {
        var now = timeval()
        gettimeofday(&now, nil)
        return UInt64(now.tv_sec) * 1_000_000 + UInt64(now.tv_usec)
    }
}
