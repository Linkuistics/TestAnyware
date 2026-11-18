import Darwin
import Foundation
import TestAnywareAgentProtocol
import XCTest
@testable import TestAnywareAgent

/// Cancellation through the lifecycle owner and the registry (spec, sections
/// "Arbitration", "Cancel endpoint" and "Termination after a timeout or a
/// cancellation", ADR-0025). These are new-behaviour controls: the old agent
/// had no cancellation. Leaf k221.
final class ExecCancellationTests: XCTestCase {

    /// The cancellation reply bound with `waitMs` 5000.
    static let cancelReplyBound: TimeInterval = 5
    /// The owner lateness bound at a deadline or a cancellation.
    static let ownerLatenessBoundMs = 500

    func testCancellationEndsTheGroupAndRepliesWithTheRecord() async throws {
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("child.pid")
        let registry = ExecRegistry()
        let running = try start(
            registry, "cancel-me",
            "printf 'before cancel\\n'; sleep 30 & echo $! > '\(pidFile)'; wait", timeout: 60)
        let childPid = try await waitForPidFile(pidFile)
        await fixture.adopt(session: getsid(childPid))
        let child = try await fixture.track("child.pid", command: "sleep")

        let started = Date()
        let (state, reply) = registry.cancel("cancel-me", waitNs: 5_000_000_000)
        let replyAfter = Date().timeIntervalSince(started)
        XCTAssertEqual(state, .terminal)
        let record = try XCTUnwrap(reply).record
        try checkValid(XCTUnwrap(reply), id: "cancel-me")
        XCTAssertEqual(record.outcome, .cancelled(beforeStart: false))
        let term = try XCTUnwrap(record.termination)
        XCTAssertEqual(term.groupEmpty, .yes)
        XCTAssertTrue(term.withinBounds)
        XCTAssertLessThanOrEqual(record.lifecycle.ownerLatenessMs, Self.ownerLatenessBoundMs)
        XCTAssertLessThan(replyAfter, Self.cancelReplyBound)
        XCTAssertNil(record.remainingGroup)
        XCTAssertFalse(child.isAlive, "the child outlived the cancellation")
        XCTAssertEqual(reply?.exitCode, -1)
        XCTAssertEqual(reply?.timedOut, false)
        XCTAssertEqual(reply?.stdout, "before cancel")
        XCTAssertEqual(reply?.stderr, "Process cancelled")
        _ = await running.value
    }

    func testTheCancellationReplyWaitsAtMostWaitMs() async throws {
        let registry = ExecRegistry()
        let running = try start(registry, "slow-end", "trap '' TERM; sleep 30", timeout: 60)
        try await Task.sleep(nanoseconds: 300_000_000)
        let started = Date()
        let (state, reply) = registry.cancel("slow-end", waitNs: 500_000_000)
        let elapsed = Date().timeIntervalSince(started)
        XCTAssertEqual(state, .terminating)
        XCTAssertNil(reply)
        XCTAssertGreaterThanOrEqual(elapsed, 0.49)
        XCTAssertLessThan(elapsed, 1.0)
        // SIGKILL follows at T0 + 1.9 s, so a wait of 5 s sees the record.
        let (again, record) = registry.cancel("slow-end", waitNs: 5_000_000_000)
        XCTAssertEqual(again, .terminal)
        XCTAssertEqual(record?.record.outcome, .cancelled(beforeStart: false))
        XCTAssertEqual(record?.record.termination?.leaderStatus, .signalled(signal: Int(SIGKILL)))
        _ = await running.value
    }

    func testARepeatedCancellationReturnsTheSameRecord() async throws {
        let registry = ExecRegistry()
        let running = try start(registry, "twice", "sleep 30", timeout: 60)
        try await Task.sleep(nanoseconds: 300_000_000)
        let first = registry.cancel("twice", waitNs: 5_000_000_000)
        let second = registry.cancel("twice", waitNs: 5_000_000_000)
        XCTAssertEqual(first.0, .terminal)
        XCTAssertEqual(second.0, .terminal)
        XCTAssertEqual(first.1, second.1)
        _ = await running.value
    }

    func testACancellationAfterATimeoutReturnsTheTimedOutRecord() async throws {
        let registry = ExecRegistry()
        let running = try start(registry, "late-cancel", "sleep 30", timeout: 1)
        _ = await running.value
        let (state, reply) = registry.cancel("late-cancel", waitNs: 0)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.outcome, .timedOut(timeoutSeconds: 1))
    }

    func testACancellationDuringTimeoutTerminationKeepsTheTimeout() async throws {
        let registry = ExecRegistry()
        let running = try start(registry, "in-grace", "trap '' TERM; sleep 30", timeout: 1)
        // The deadline passes at 1 s, and the grace runs to about 2.9 s.
        try await Task.sleep(nanoseconds: 1_600_000_000)
        let (state, reply) = registry.cancel("in-grace", waitNs: 5_000_000_000)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.outcome, .timedOut(timeoutSeconds: 1))
        // A later cancellation does not shorten the grace.
        let term = try XCTUnwrap(reply?.record.termination)
        XCTAssertGreaterThanOrEqual(term.killSentAfterMs, 1850)
        _ = await running.value
    }

    func testACancellationAfterAnExitReturnsTheExitRecord() async throws {
        let registry = ExecRegistry()
        let running = try start(registry, "done", "printf finished; exit 3", timeout: 10)
        _ = await running.value
        let (state, reply) = registry.cancel("done", waitNs: 5_000_000_000)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.outcome, .exited(exitCode: 3))
        XCTAssertEqual(reply?.stdout, "finished")
    }

    /// The leader exits at once, but the waiter never reports it. The
    /// cancellation wakes the owner, and its non-blocking kernel wait finds
    /// the exit first (spec, "Arbitration", step 2).
    func testTheExitAndCancelRaceResolvesByKernelState() async throws {
        let registry = ExecRegistry()
        let seams = OwnerSeams(
            leaderWaiter: { ExitBlindWaiter(pid: $0) }, captureRead: OwnerSeams.live.captureRead)
        let running = try start(registry, "race", "printf 'finished'; exit 7", timeout: 30, seams: seams)
        try await Task.sleep(nanoseconds: 300_000_000)
        let (state, reply) = registry.cancel("race", waitNs: 5_000_000_000)
        XCTAssertEqual(state, .terminal)
        let record = try XCTUnwrap(reply).record
        try checkValid(XCTUnwrap(reply), id: "race")
        XCTAssertEqual(record.outcome, .exited(exitCode: 7))
        XCTAssertNil(record.termination)
        XCTAssertEqual(record.remainingGroup, ExecRemainingGroup.none)
        XCTAssertEqual(Data(base64Encoded: record.evidence.stdout.base64), Data("finished".utf8))
        XCTAssertLessThanOrEqual(record.lifecycle.ownerLatenessMs, Self.ownerLatenessBoundMs)
        _ = await running.value
    }

    /// A late owner can see a leader that exited after the deadline. The
    /// outcome is `exited`, and the record keeps the lateness (spec,
    /// "Arbitration"; review k235 R3 on Linux).
    func testALateOwnerKeepsItsLatenessWhenTheExitWins() async throws {
        let seams = OwnerSeams(
            leaderWaiter: { LateOwnerWaiter(pid: $0) }, captureRead: OwnerSeams.live.captureRead)
        let owned = await ProcessRunner.runOwned(command: "sleep 1.3", timeoutSeconds: 1, seams: seams)
        try checkValid(owned.reply(id: "late"), id: "late")
        XCTAssertEqual(owned.outcome, .exited(code: 0))
        XCTAssertNil(owned.termination)
        XCTAssertGreaterThanOrEqual(owned.ownerLatenessMs, 900)
    }

    func testACancellationBeforeTheSpawnStartsNothing() async throws {
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-k221-\(UUID().uuidString)").path
        let signal = CancelSignal()
        signal.request()
        let owned = await ProcessRunner.runOwned(
            command: "touch '\(marker)'", timeoutSeconds: 10, cancel: signal)
        let reply = owned.reply(id: "accepted")
        try checkValid(reply, id: "accepted")
        XCTAssertEqual(reply.record.outcome, .cancelled(beforeStart: true))
        XCTAssertNil(reply.record.lifecycle.leaderPid)
        XCTAssertNil(reply.record.lifecycle.durationMs)
        XCTAssertNil(reply.record.termination)
        XCTAssertEqual(reply.exitCode, -1)
        XCTAssertEqual(reply.stderr, "Process cancelled")
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertFalse(FileManager.default.fileExists(atPath: marker), "a cancelled command started")
    }

    func testATombstonedRecordHasNoLateness() throws {
        let reply = ProcessRunner.cancelledBeforeStart(requestedAt: ProcessRunner.uptime()).reply(id: "early")
        try checkValid(reply, id: "early")
        XCTAssertEqual(reply.record.outcome, .cancelled(beforeStart: true))
        XCTAssertEqual(reply.record.lifecycle.ownerLatenessMs, 0)
        XCTAssertEqual(reply.stdout, "")
        XCTAssertEqual(reply.stderr, "Process cancelled")
    }

    /// The deadline saturates, and a cancellation still ends the command.
    func testAHugeTimeoutRunsAndCancels() async throws {
        for (index, timeout) in [20_000_000_000, Int.max].enumerated() {
            let registry = ExecRegistry()
            let running = try start(registry, "huge-\(index)", "sleep 30", timeout: timeout)
            try await Task.sleep(nanoseconds: 300_000_000)
            let (state, reply) = registry.cancel("huge-\(index)", waitNs: 5_000_000_000)
            XCTAssertEqual(state, .terminal)
            XCTAssertEqual(reply?.record.outcome, .cancelled(beforeStart: false))
            _ = await running.value
        }
    }

    // MARK: - No cancellation wake source (review k237 R1)

    /// The pipe did not open. The exit source of the leader stays live.
    func testACancellationWithoutAPipeEndsTheCommandWithinTheLatenessBound() async throws {
        try await checkCancellationWithoutAWakeSource(CancelSignal(wakeFD: -1, writeFD: -1))
    }

    /// The pipe exists, but the queue refuses its read source. No descriptor
    /// has this number, so `kevent` refuses it with EBADF. The exit source of
    /// the leader stays live.
    func testACancellationWhoseReadSourceFailsEndsTheCommandWithinTheLatenessBound() async throws {
        try await checkCancellationWithoutAWakeSource(CancelSignal(wakeFD: Int32.max, writeFD: -1))
    }

    /// The deadline is 3 s. An owner that sleeps through the cancellation
    /// wakes only at the deadline, and its lateness is then about 2.7 s.
    private func checkCancellationWithoutAWakeSource(_ signal: CancelSignal) async throws {
        let canceller = Task.detached {
            try? await Task.sleep(nanoseconds: 300_000_000)
            signal.request()
        }
        let owned = await ProcessRunner.runOwned(command: "exec sleep 30", timeoutSeconds: 3, cancel: signal)
        await canceller.value
        XCTAssertEqual(owned.outcome, .cancelled(beforeStart: false))
        XCTAssertLessThanOrEqual(owned.ownerLatenessMs, Self.ownerLatenessBoundMs)
        XCTAssertEqual(owned.termination?.groupEmpty, "yes")
    }

    // MARK: - Helpers

    /// Admits `id` and runs its command in the background, as `/exec` does.
    private func start(
        _ registry: ExecRegistry, _ id: String, _ command: String, timeout: Int, seams: OwnerSeams = .live
    ) throws -> Task<Void, Never> {
        guard case .start(let signal) = registry.admit(id) else {
            throw FixtureFailure("the registry did not admit \(id)")
        }
        return Task.detached {
            let owned = await ProcessRunner.runOwned(
                command: command, timeoutSeconds: timeout, seams: seams, cancel: signal)
            registry.publish(id, reply: owned.reply(id: id))
        }
    }

    private func checkValid(_ reply: ExecReply, id: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let body = try JSONEncoder().encode(reply)
        XCTAssertEqual(ExecValidity.errors(reply: body, expectedID: id), [], file: file, line: line)
    }

    /// Waits for a PID file and returns the PID that it holds.
    private func waitForPidFile(_ path: String) async throws -> pid_t {
        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline {
            if let text = try? String(contentsOfFile: path, encoding: .utf8), text.hasSuffix("\n"),
               let pid = pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines)) {
                return pid
            }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        throw FixtureFailure("no PID in \(path)")
    }
}

/// A leader waiter that never reports the exit, but wakes at a cancellation.
/// Its status check and its reap are real, so the owner's arbitration decides.
final class ExitBlindWaiter: LeaderWaiter, @unchecked Sendable {
    private let real: KqueueLeaderWaiter

    init(pid: pid_t) { real = KqueueLeaderWaiter(pid: pid) }

    func waitExit(until deadline: UInt64, wake: CancelSignal?) -> Bool {
        while true {
            let now = DispatchTime.now().uptimeNanoseconds
            if deadline <= now || wake?.requestedAt != nil { return false }
            usleep(useconds_t(min((deadline - now) / 1000, 10_000)))
        }
    }

    func hasExited() -> Bool { real.hasExited() }

    func reap() -> Int32 { real.reap() }
}

/// The owner resumes 2 s after the spawn. By then the deadline of 1 s has
/// passed and the leader has exited, so the first real wait sees the exit.
final class LateOwnerWaiter: LeaderWaiter, @unchecked Sendable {
    private let real: KqueueLeaderWaiter
    private var first = true

    init(pid: pid_t) { real = KqueueLeaderWaiter(pid: pid) }

    func waitExit(until deadline: UInt64, wake: CancelSignal?) -> Bool {
        if first {
            first = false
            sleep(2)
        }
        return real.waitExit(until: deadline, wake: wake)
    }

    func hasExited() -> Bool { real.hasExited() }

    func reap() -> Int32 { real.reap() }
}

/// A fixture step that did not happen. The test fails, because a skip would
/// hide a regression from `agent:macos:test` (review k237 R4).
struct FixtureFailure: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}
