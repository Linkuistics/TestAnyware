import Darwin
import Foundation
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
            zombie: info.pbi_status == UInt32(SZOMB)
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
/// after `&`). `track` reads that PID and accepts the process only when it
/// started after the fixture was created and runs the expected command.
/// `cleanUp` signals a tracked child only while its PID still carries the
/// recorded start time. The fixture never selects a process by name.
///
/// Limit: the kernel can reuse a PID between the identity check and `kill`.
/// That window is a few microseconds, and Darwin has no process handle
/// that would close it.
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

    /// What `cleanUp` did with one tracked child.
    enum CleanupOutcome: Equatable, Sendable {
        case alreadyGone
        case stoppedByTerm
        case stoppedByKill
        case survived
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
    private var tracked: [ProcessIdentity] = []

    init() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-process-fixture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        notBeforeMicroseconds = Self.wallClockMicroseconds()
    }

    /// Creates a fixture and registers its cleanup as a teardown block of
    /// `testCase`, so the cleanup runs even when the test fails or throws.
    static func make(for testCase: XCTestCase) throws -> OwnedProcessFixture {
        let fixture = try OwnedProcessFixture()
        testCase.addTeardownBlock {
            let outcomes = await fixture.cleanUp()
            for (identity, outcome) in outcomes {
                FileHandle.standardError.write(Data(
                    "owned-process-fixture: \(identity) \(outcome)\n".utf8
                ))
                XCTAssertNotEqual(outcome, .survived, "owned child \(identity) survived cleanup")
            }
        }
        return fixture
    }

    /// The path of the PID file `name` in this fixture's directory. The
    /// directory name holds only a UUID, so it is safe inside single quotes.
    nonisolated func pidFile(_ name: String) -> String {
        directory.appendingPathComponent(name).path
    }

    /// Reads the PID that the child wrote to `pidFile(name)` and records the
    /// child's identity. Waits up to `trackBound` for the file and for the
    /// child's command to become `command`.
    @discardableResult
    func track(_ name: String, command: String) async throws -> ProcessIdentity {
        let path = pidFile(name)
        let deadline = Date().addingTimeInterval(Self.trackBound)
        var pid: pid_t?
        var last: ProcessIdentity.Observation?
        while Date() < deadline {
            if pid == nil, let text = try? String(contentsOfFile: path, encoding: .utf8),
               text.hasSuffix("\n"),
               let value = pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines)) {
                pid = value
            }
            if let pid {
                last = ProcessIdentity.observe(pid: pid)
                if let last, last.command == command { break }
            }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        guard let pid else {
            throw FixtureError.noPidFile("no PID in \(path) within \(Self.trackBound) s")
        }
        guard let last, !last.zombie else {
            throw FixtureError.notOwned("pid \(pid) from \(path) exited before it was identified")
        }
        let started = last.identity.startSeconds * 1_000_000 + last.identity.startMicroseconds
        guard started + Self.clockAllowanceMicroseconds >= notBeforeMicroseconds else {
            throw FixtureError.notOwned("pid \(pid) started before this fixture, so it is not this test's child")
        }
        // The start time shows that the test created this process, so the
        // fixture tracks it even when the command check below fails.
        tracked.append(last.identity)
        guard last.command == command else {
            throw FixtureError.notOwned("pid \(pid) runs \(last.command), not \(command)")
        }
        return last.identity
    }

    /// Stops every tracked child that is still alive, then removes the
    /// directory. SIGTERM first, SIGKILL after `termGrace`.
    func cleanUp() async -> [(ProcessIdentity, CleanupOutcome)] {
        var outcomes: [(ProcessIdentity, CleanupOutcome)] = []
        for identity in tracked {
            outcomes.append((identity, await Self.stop(identity)))
        }
        tracked.removeAll()
        try? FileManager.default.removeItem(at: directory)
        return outcomes
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
