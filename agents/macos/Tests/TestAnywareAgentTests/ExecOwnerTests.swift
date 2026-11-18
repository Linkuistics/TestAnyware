import CryptoKit
import Darwin
import Foundation
import TestAnywareAgentProtocol
import XCTest
@testable import TestAnywareAgent

/// The lifecycle owner and its typed record (leaf k219). Each test checks
/// the reply against the validity rules, then the values of its case.
final class ExecOwnerTests: XCTestCase {

    // MARK: - Outcomes and evidence

    func testOracleTextKeepsExactBytes() async throws {
        let run = try await exec("printf 'Explain this change.'; printf 'separate stderr\\n' >&2")
        XCTAssertEqual(run.record.outcome, .exited(exitCode: 0))
        XCTAssertEqual(run.bytes(\.stdout), Data("Explain this change.".utf8))
        XCTAssertEqual(run.bytes(\.stderr), Data("separate stderr\n".utf8))
        XCTAssertEqual(run.record.remainingGroup, ExecRemainingGroup.none)
        XCTAssertNil(run.record.termination)
        XCTAssertEqual(run.reply.stdout, "Explain this change.")
        XCTAssertEqual(run.reply.stderr, "separate stderr")
    }

    func testBinaryOutputKeepsEveryByte() async throws {
        let run = try await exec("printf '\\000\\377\\200A\\n'")
        XCTAssertEqual(run.record.evidence.stdout.base64, "AP+AQQo=")
        // The old decode fails on invalid UTF-8, so the legacy field is empty.
        XCTAssertEqual(run.reply.stdout, "")
    }

    func testExitCodeAndPartialOutput() async throws {
        let run = try await exec("printf 'partial out\\n'; printf 'partial err\\n' >&2; exit 23")
        XCTAssertEqual(run.record.outcome, .exited(exitCode: 23))
        XCTAssertEqual(run.bytes(\.stdout), Data("partial out\n".utf8))
        XCTAssertEqual(run.bytes(\.stderr), Data("partial err\n".utf8))
        XCTAssertEqual(run.reply.exitCode, 23)
    }

    func testSignalDeathIsSignalled() async throws {
        let run = try await exec("printf 'before signal\\n'; kill -KILL $$")
        XCTAssertEqual(run.record.outcome, .signalled(signal: 9, signalName: "SIGKILL"))
        XCTAssertEqual(run.reply.exitCode, 9)
        XCTAssertEqual(run.reply.stdout, "before signal")
    }

    func testLargeCaptureIsComplete() async throws {
        let size = 8 * 1024 * 1024
        let run = try await exec("head -c \(size) /dev/zero")
        let stdout = run.record.evidence.stdout
        XCTAssertEqual(stdout.capture, .complete)
        XCTAssertEqual(stdout.bytes, size)
        XCTAssertEqual(stdout.sha256, Self.sha256(Data(count: size)))
    }

    /// A member that outlives the leader: the owner reports the group as
    /// present and leaves it alone, because the leader exited by itself.
    func testRemainingGroupIsReported() async throws {
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("remaining.pid")
        let run = try await exec("sleep 30 & echo $! > '\(pidFile)'; printf 'leader done\\n'; exit 0")
        XCTAssertEqual(run.record.outcome, .exited(exitCode: 0))
        XCTAssertEqual(run.record.remainingGroup, .present)
        await fixture.adopt(session: try XCTUnwrap(run.owned.leaderPid))
        try await fixture.track("remaining.pid", command: "sleep")
    }

    // MARK: - Deadline and termination

    func testDeadlineTerminatesTheGroupWithinBounds() async throws {
        let run = try await exec(
            "printf 'before deadline\\n'; printf 'deadline err\\n' >&2; sleep 30", timeout: 2)
        XCTAssertEqual(run.record.outcome, .timedOut(timeoutSeconds: 2))
        let term = try XCTUnwrap(run.record.termination)
        XCTAssertEqual(term.leaderStatus, .signalled(signal: Int(SIGTERM)))
        XCTAssertEqual(term.groupEmpty, .yes)
        XCTAssertTrue(term.withinBounds)
        // The leader died of SIGTERM, so SIGKILL followed at its exit.
        XCTAssertLessThan(term.killSentAfterMs, 1900)
        XCTAssertEqual(run.reply.exitCode, -1)
        XCTAssertTrue(run.reply.timedOut)
        XCTAssertEqual(run.reply.stdout, "before deadline")
        XCTAssertEqual(run.reply.stderr, "deadline err\n[Process timed out after 2s]")
    }

    /// Old path failure: the old runner waited 3 s after SIGTERM. The owner
    /// sends SIGKILL by T0 + 1.9 s.
    func testIgnoredTermGetsKillAfterGrace() async throws {
        let run = try await exec("trap '' TERM; printf 'ignoring term\\n'; sleep 30", timeout: 2)
        let term = try XCTUnwrap(run.record.termination)
        XCTAssertEqual(term.leaderStatus, .signalled(signal: Int(SIGKILL)))
        XCTAssertGreaterThanOrEqual(term.killSentAfterMs, 1850)
        XCTAssertLessThanOrEqual(term.killSentAfterMs, 2000)
        XCTAssertEqual(term.groupEmpty, .yes)
        XCTAssertTrue(term.withinBounds)
    }

    /// The leader dies of SIGTERM, but a member ignores it. SIGKILL at the
    /// leader's exit empties the group long before the grace ends.
    func testMemberIgnoringTermIsKilledAtLeaderExit() async throws {
        let run = try await exec("(trap '' TERM; exec sleep 30) & sleep 30", timeout: 1)
        let term = try XCTUnwrap(run.record.termination)
        XCTAssertEqual(term.leaderStatus, .signalled(signal: Int(SIGTERM)))
        XCTAssertLessThan(term.killSentAfterMs, 1000)
        XCTAssertEqual(term.groupEmpty, .yes)
        XCTAssertTrue(term.withinBounds)
    }

    /// The leader status seam never reports the exit. The record says
    /// `unavailable` at K + 1.4 s, and the owner reaps the leader later.
    func testUnavailableLeaderStatusIsHeldAndReapedLate() async throws {
        let seams = OwnerSeams(
            leaderWaiter: { BlindLeaderWaiter(pid: $0) }, captureRead: OwnerSeams.live.captureRead)
        let run = try await exec("sleep 30", timeout: 1, seams: seams)
        let term = try XCTUnwrap(run.record.termination)
        XCTAssertEqual(term.leaderStatus, .unavailable)
        XCTAssertNil(term.leaderExitAfterMs)
        XCTAssertEqual(term.groupEmpty, .no)
        XCTAssertFalse(term.withinBounds)
        XCTAssertGreaterThanOrEqual(term.killSentAfterMs, 1850)
        XCTAssertLessThanOrEqual(run.record.lifecycle.finalisedAfterMs - term.killSentAfterMs, 2100)

        // The hold thread reaps the leader. Then the PID is no child of ours.
        let pid = try XCTUnwrap(run.owned.leaderPid)
        let deadline = Date().addingTimeInterval(5)
        var status: Int32 = 0
        while waitpid(pid, &status, WNOHANG | WNOWAIT) >= 0, Date() < deadline {
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        XCTAssertEqual(waitpid(pid, &status, WNOHANG), -1, "the owner did not reap the held leader")
    }

    // MARK: - Capture

    func testStalledCaptureEndsAtTheFinaliseDeadline() async throws {
        let seams = OwnerSeams(
            leaderWaiter: OwnerSeams.live.leaderWaiter,
            captureRead: { stream, fd, buffer, offset in
                if stream == "stdout" { sleep(4) }
                return pread(fd, buffer.baseAddress, buffer.count, offset)
            })
        let run = try await exec("printf hello", seams: seams)
        let stdout = run.record.evidence.stdout
        XCTAssertEqual(stdout.capture, .incomplete)
        XCTAssertEqual(stdout.reason, "deadline")
        XCTAssertEqual(stdout.fileBytes, 5)
        XCTAssertEqual(run.record.evidence.stderr.capture, .complete)
        XCTAssertGreaterThanOrEqual(run.record.lifecycle.finalisedAfterMs, 1850)
        XCTAssertLessThanOrEqual(run.record.lifecycle.finalisedAfterMs, 2300)
    }

    func testReadErrorMarksTheStreamIncomplete() async throws {
        let seams = OwnerSeams(
            leaderWaiter: OwnerSeams.live.leaderWaiter,
            captureRead: { stream, fd, buffer, offset in
                if stream == "stderr" {
                    errno = EIO
                    return -1
                }
                return pread(fd, buffer.baseAddress, buffer.count, offset)
            })
        let run = try await exec("printf out; printf err >&2", seams: seams)
        XCTAssertEqual(run.record.evidence.stdout.capture, .complete)
        XCTAssertEqual(run.record.evidence.stderr.capture, .incomplete)
        XCTAssertEqual(run.record.evidence.stderr.reason, "read_error")
    }

    // MARK: - Spawn failure

    static let spawnChildEnv = "TESTANYWARE_SPAWN_FAILURE_OUT"

    /// The spawn failure runs in a fresh `xctest` process under a process
    /// limit of 1, so the limit never touches this process.
    func testSpawnFailureIsAnOutcome() async throws {
        let body = try Self.spawnFailureReply()
        XCTAssertEqual(ExecValidity.errors(reply: body, expectedID: "spawn-failure"), [])
        let reply = try JSONDecoder().decode(ExecReply.self, from: body)
        XCTAssertEqual(
            reply.record.outcome,
            .spawnFailed(errno: Int(EAGAIN), errorName: "EAGAIN", message: String(cString: strerror(EAGAIN))))
        XCTAssertEqual(reply.record.evidence.stdout.bytes, 0)
        XCTAssertEqual(reply.record.evidence.stdout.capture, .complete)
        XCTAssertEqual(reply.exitCode, -1)
        XCTAssertEqual(reply.stderr, "Resource temporarily unavailable")
    }

    /// Runs `testSpawnFailureChildProcess` in a fresh `xctest` process under
    /// a process limit of 1 and returns the reply body that it wrote.
    static func spawnFailureReply() throws -> Data {
        let out = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-spawn-failure-\(UUID().uuidString).json").path
        defer { try? FileManager.default.removeItem(atPath: out) }
        let xctest = ProcessInfo.processInfo.arguments[0]
        let bundle = Bundle(for: Self.self).bundlePath
        let child = try ControlChild(
            "/bin/bash",
            [
                "-c",
                "ulimit -S -u 1 && exec \"$0\" -XCTest "
                    + "TestAnywareAgentTests.ExecOwnerTests/testSpawnFailureChildProcess \"$1\"",
                xctest, bundle,
            ],
            environment: [spawnChildEnv: out])
        child.waitUntilExit()
        return try Data(contentsOf: URL(fileURLWithPath: out))
    }

    /// The child half of `spawnFailureReply`. It does nothing in a normal run.
    func testSpawnFailureChildProcess() async throws {
        guard let out = ProcessInfo.processInfo.environment[Self.spawnChildEnv] else { return }
        let owned = await ProcessRunner.runOwned(command: "printf 'never runs\\n'", timeoutSeconds: 10)
        try JSONEncoder().encode(owned.reply(id: "spawn-failure")).write(to: URL(fileURLWithPath: out))
    }

    // MARK: - Legacy projection

    /// The legacy fields equal the replies of the old agent, recorded by the
    /// k12 matrix. The spawn-failure reply was HTTP 500, so it has no fields.
    func testLegacyFieldsEqualTheOldReplies() async throws {
        let url = Self.repoRoot.appendingPathComponent("cli-rs/tests/fixtures/exec-replies/legacy-macos.json")
        let file = try JSONDecoder().decode(LegacyFile.self, from: Data(contentsOf: url))
        var checked = 0
        for item in file.cases where item.httpStatus == 200 {
            let old = try JSONDecoder().decode(LegacyReply.self, from: Data(item.bodyText.utf8))
            let new = await ProcessRunner.runOwned(command: item.request.command, timeoutSeconds: item.request.timeout)
            XCTAssertEqual(new.legacy.exitCode, old.exitCode, item.case)
            XCTAssertEqual(new.legacy.stdout, old.stdout, item.case)
            XCTAssertEqual(new.legacy.stderr, old.stderr, item.case)
            XCTAssertEqual(new.legacy.timedOut, old.timedOut, item.case)
            checked += 1
        }
        XCTAssertEqual(checked, 7)
    }

    // MARK: - Exec ids

    func testExecIDRule() {
        XCTAssertTrue(ExecRequest.isValidID("k217.fixture-1"))
        XCTAssertTrue(ExecRequest.isValidID(String(repeating: "a", count: 64)))
        XCTAssertFalse(ExecRequest.isValidID(""))
        XCTAssertFalse(ExecRequest.isValidID(String(repeating: "a", count: 65)))
        XCTAssertFalse(ExecRequest.isValidID("has space"))
        XCTAssertFalse(ExecRequest.isValidID("é"))
    }

    // MARK: - Helpers

    struct Run {
        let owned: OwnedExecResult
        let reply: ExecReply
        var record: ExecRecord { reply.record }

        func bytes(_ stream: KeyPath<ExecEvidence, ExecStreamEvidence>) -> Data? {
            Data(base64Encoded: record.evidence[keyPath: stream].base64)
        }
    }

    /// Runs one command, encodes its reply and checks the validity rules.
    private func exec(
        _ command: String, timeout: Int = 10, seams: OwnerSeams = .live,
        file: StaticString = #filePath, line: UInt = #line
    ) async throws -> Run {
        let owned = await ProcessRunner.runOwned(command: command, timeoutSeconds: timeout, seams: seams)
        let body = try JSONEncoder().encode(owned.reply(id: "k219.test"))
        XCTAssertEqual(ExecValidity.errors(reply: body, expectedID: "k219.test"), [], file: file, line: line)
        return Run(owned: owned, reply: try JSONDecoder().decode(ExecReply.self, from: body))
    }

    static func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    static let repoRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

    struct LegacyFile: Decodable {
        struct Case: Decodable {
            let `case`: String
            let request: Request
            let httpStatus: Int
            let bodyText: String
        }
        struct Request: Decodable {
            let command: String
            let timeout: Int
        }
        let cases: [Case]
    }

    struct LegacyReply: Decodable {
        let exitCode: Int32
        let stdout: String
        let stderr: String
        let timedOut: Bool
    }
}

/// A leader waiter that never sees the exit. It reaps through the real
/// `waitpid`, so the owner's late reap still works.
final class BlindLeaderWaiter: LeaderWaiter, @unchecked Sendable {
    private let real: KqueueLeaderWaiter

    init(pid: pid_t) { real = KqueueLeaderWaiter(pid: pid) }

    func waitExit(until deadline: UInt64) -> Bool {
        let now = DispatchTime.now().uptimeNanoseconds
        if deadline > now { usleep(useconds_t(min((deadline - now) / 1000, UInt64(UInt32.max)))) }
        return false
    }

    func hasExited() -> Bool { false }

    func reap() -> Int32 { real.reap() }
}
