import Darwin
import Foundation
import TestAnywareAgentProtocol
import XCTest
@testable import TestAnywareAgent

/// The agent binary on loopback, through raw HTTP (leaf k221). These are the
/// transport controls of the spec's required-control table: health during a
/// command, cancellation, the tombstone order, a disconnect that changes
/// nothing, agent shutdown at SIGTERM and the download framing.
///
/// The tests start `testanyware-agent` from the build directory. The
/// environment variable `TESTANYWARE_AGENT_UNDER_TEST` names another binary,
/// such as the shipped agent for an old-fail run. An old agent ignores the
/// second argument and listens on every address, so that run needs a host
/// whose firewall does not prompt.
final class AgentProcessTests: XCTestCase {

    static let healthBound: TimeInterval = 1
    static let cancelReplyBound: TimeInterval = 5
    /// From SIGTERM to the agent exit: the 4 s publication wait and 0.5 s for
    /// the process exit.
    static let shutdownBound: TimeInterval = 4.5

    private var agent: AgentProcess!

    override func setUpWithError() throws {
        agent = try AgentProcess.start()
    }

    override func tearDown() {
        agent?.stop()
    }

    // MARK: - Responsiveness

    /// Corrected-pass. The old-fail case of this row is Linux.
    func testHealthRepliesWithin1SecondDuringACommand() throws {
        let agent = self.agent!
        let running = Background { try agent.post("/exec", ["command": "sleep 3", "timeout": 10]) }
        Thread.sleep(forTimeInterval: 0.3)
        for _ in 0..<5 {
            let start = Date()
            let reply = try agent.get("/health")
            XCTAssertEqual(reply.status, 200)
            XCTAssertLessThan(Date().timeIntervalSince(start), Self.healthBound)
        }
        XCTAssertEqual(try running.join(timeout: 10).status, 200)
    }

    // MARK: - Cancellation

    func testACancellationEndsTheCommandAndRetainsItsRecord() async throws {
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("leader.pid")
        let agent = self.agent!
        let running = Background {
            try agent.post("/exec", [
                "command": "echo $$ > '\(pidFile)'; exec sleep 30", "timeout": 60, "id": "http-cancel",
            ])
        }
        try await track(fixture, "leader.pid")
        let start = Date()
        let cancel = try agent.post("/exec/cancel", ["id": "http-cancel", "waitMs": 5000])
        XCTAssertLessThan(Date().timeIntervalSince(start), Self.cancelReplyBound)
        XCTAssertEqual(cancel.status, 200)
        let body = try cancel.object()
        XCTAssertEqual(body["state"] as? String, "terminal")
        let record = try XCTUnwrap(body["record"] as? [String: Any])
        XCTAssertEqual(try Self.validity(record, id: "http-cancel"), [])
        XCTAssertEqual(record["outcome"] as? NSDictionary, ["kind": "cancelled", "beforeStart": false])
        let termination = try XCTUnwrap(record["termination"] as? [String: Any])
        XCTAssertEqual(termination["groupEmpty"] as? String, "yes")
        XCTAssertEqual(termination["withinBounds"] as? Bool, true)

        // The /exec reply is the same record, and a repeat changes nothing.
        let exec = try running.join(timeout: 10)
        XCTAssertEqual(exec.status, 200)
        XCTAssertEqual(try exec.object() as NSDictionary, record as NSDictionary)
        let again = try agent.post("/exec/cancel", ["id": "http-cancel", "waitMs": 5000]).object()
        XCTAssertEqual(again as NSDictionary, body as NSDictionary)
    }

    func testAShortWaitRepliesTerminating() throws {
        let agent = self.agent!
        let running = Background {
            try agent.post("/exec", ["command": "trap '' TERM; sleep 30", "timeout": 60, "id": "short-wait"])
        }
        Thread.sleep(forTimeInterval: 0.3)
        let start = Date()
        let cancel = try agent.post("/exec/cancel", ["id": "short-wait"]).object()
        XCTAssertLessThan(Date().timeIntervalSince(start), Self.healthBound)
        XCTAssertEqual(cancel as NSDictionary, ["state": "terminating"])
        XCTAssertEqual(try running.join(timeout: 10).status, 200)
    }

    /// A cancellation that overtakes its own `/exec` leaves a tombstone. The
    /// later `/exec` ends as cancelled without a spawn, and the record stays.
    func testATombstoneCancelsALaterExecWithoutASpawn() throws {
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-k221-\(UUID().uuidString)").path
        XCTAssertEqual(try agent.post("/exec/cancel", ["id": "early"]).object() as NSDictionary, ["state": "unknown_id"])
        XCTAssertEqual(
            try agent.post("/exec/cancel", ["id": "early", "waitMs": 5000]).object() as NSDictionary,
            ["state": "unknown_id"])
        let exec = try agent.post("/exec", ["command": "touch '\(marker)'", "timeout": 10, "id": "early"])
        XCTAssertEqual(exec.status, 200)
        let record = try exec.object()
        XCTAssertEqual(try Self.validity(record, id: "early"), [])
        XCTAssertEqual(record["outcome"] as? NSDictionary, ["kind": "cancelled", "beforeStart": true])
        XCTAssertEqual(record["exitCode"] as? Int, -1)
        XCTAssertEqual(record["stderr"] as? String, "Process cancelled")
        Thread.sleep(forTimeInterval: 0.2)
        XCTAssertFalse(FileManager.default.fileExists(atPath: marker), "a tombstoned exec spawned its command")
        XCTAssertEqual(
            try agent.post("/exec/cancel", ["id": "early"]).object() as NSDictionary,
            ["state": "terminal", "record": record] as NSDictionary)
        let again = try agent.post("/exec", ["command": "true", "timeout": 10, "id": "early"])
        XCTAssertEqual(again.status, 409)
        XCTAssertEqual(try again.object()["error"] as? String, "exec_id_in_use")
    }

    func testADuplicateIDOfARunningCommandIsRefused() throws {
        let agent = self.agent!
        let running = Background {
            try agent.post("/exec", ["command": "sleep 30", "timeout": 60, "id": "dup"])
        }
        Thread.sleep(forTimeInterval: 0.3)
        let duplicate = try agent.post("/exec", ["command": "true", "timeout": 5, "id": "dup"])
        XCTAssertEqual(duplicate.status, 409)
        XCTAssertEqual(try duplicate.object()["error"] as? String, "exec_id_in_use")
        _ = try agent.post("/exec/cancel", ["id": "dup", "waitMs": 5000])
        let outcome = try running.join(timeout: 10).object()["outcome"] as? [String: Any]
        XCTAssertEqual(outcome?["kind"] as? String, "cancelled")
    }

    func testAMalformedCancellationIsRefused() throws {
        let bodies: [Any] = [
            [String: Any](), ["id": ""], ["id": "has space"], ["id": 7],
            ["id": "ok", "waitMs": -1], ["id": "ok", "waitMs": 10001], ["id": "ok", "waitMs": "5"], [1, 2],
        ]
        for body in bodies {
            let reply = try agent.post("/exec/cancel", body)
            XCTAssertEqual(reply.status, 400, "\(body)")
            XCTAssertEqual(try reply.object()["error"] as? String, "exec_failed", "\(body)")
        }
    }

    // MARK: - Connection loss (the k13 cases, extended)

    /// Old-fail: the shipped agent has no `/exec/cancel`, so the record of a
    /// disconnected command is lost. A close, a reset or a half-close changes
    /// nothing: the command runs to its own exit, and the agent retains the
    /// record (spec, "Connection loss").
    ///
    /// The control reads the record with a cancellation only after the kernel
    /// no longer holds the leader, not even as a zombie. The owner reaps the
    /// leader only after it chose the outcome, so that cancellation cannot
    /// change the record. A marker file written by the command is not enough:
    /// the shell can still run after it (repeat-03 of leaf k221).
    func testADisconnectChangesNothingAndTheRecordIsRetained() throws {
        for mode in Connection.Loss.allCases {
            let pidPath = FileManager.default.temporaryDirectory
                .appendingPathComponent("testanyware-k221-\(UUID().uuidString).pid").path
            defer { try? FileManager.default.removeItem(atPath: pidPath) }
            let id = "disconnect-\(mode)"
            let connection = try Connection(port: agent.port)
            try connection.send(Request.post("/exec", [
                "command": "echo $$ > '\(pidPath)'; printf start; sleep 2; printf done", "timeout": 30, "id": id,
            ]))
            Thread.sleep(forTimeInterval: 0.5)
            connection.lose(mode)
            let leader = try XCTUnwrap(Self.readPid(pidPath), "\(mode): no leader PID")
            let seen = try XCTUnwrap(ProcessIdentity.observe(pid: leader)?.identity, "\(mode): no leader")
            XCTAssertTrue(
                waitFor(seconds: 6) { ProcessIdentity.observe(pid: leader)?.identity != seen },
                "\(mode): the leader was not reaped")
            connection.close()
            let cancel = try agent.post("/exec/cancel", ["id": id, "waitMs": 5000]).object()
            XCTAssertEqual(cancel["state"] as? String, "terminal", "\(mode)")
            let record = try XCTUnwrap(cancel["record"] as? [String: Any], "\(mode)")
            XCTAssertEqual(try Self.validity(record, id: id), [], "\(mode)")
            XCTAssertEqual(record["outcome"] as? NSDictionary, ["kind": "exited", "exitCode": 0], "\(mode)")
            XCTAssertEqual(record["stdout"] as? String, "startdone", "\(mode)")
        }
    }

    /// The cancellation case of the extended k13 control: after a close, an
    /// explicit cancellation on a new connection ends the command at once.
    func testACancellationAfterADisconnectEndsTheCommand() async throws {
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("leader.pid")
        let connection = try Connection(port: agent.port)
        try connection.send(Request.post("/exec", [
            "command": "echo $$ > '\(pidFile)'; exec sleep 30", "timeout": 60, "id": "closed-then-cancelled",
        ]))
        let leader = try await track(fixture, "leader.pid")
        connection.lose(.close)
        try await Task.sleep(nanoseconds: 500_000_000)
        XCTAssertTrue(leader.isAlive, "the close ended the command")
        let start = Date()
        let cancel = try agent.post("/exec/cancel", ["id": "closed-then-cancelled", "waitMs": 5000]).object()
        XCTAssertLessThan(Date().timeIntervalSince(start), Self.cancelReplyBound)
        XCTAssertEqual(cancel["state"] as? String, "terminal")
        let record = try XCTUnwrap(cancel["record"] as? [String: Any])
        XCTAssertEqual(record["outcome"] as? NSDictionary, ["kind": "cancelled", "beforeStart": false])
        XCTAssertFalse(leader.isAlive, "the cancelled leader survived")
    }

    // MARK: - Agent shutdown

    /// New behaviour: at SIGTERM during a command, the group ends and the
    /// agent exits with status 0 within the bound (spec, "Agent shutdown").
    func testSigtermDuringACommandEndsTheGroupAndExitsInBounds() async throws {
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("child.pid")
        let agent = self.agent!
        let running = Background {
            try agent.post("/exec", [
                "command": "sleep 30 & echo $! > '\(pidFile)'; wait", "timeout": 60, "id": "at-sigterm",
            ])
        }
        let child = try await track(fixture, "child.pid")
        let start = Date()
        agent.signal(SIGTERM)
        let status = agent.waitExit(seconds: 10)
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertEqual(status, 0, "the agent did not exit with status 0")
        XCTAssertLessThanOrEqual(elapsed, Self.shutdownBound)
        XCTAssertFalse(child.isAlive, "the child outlived the agent")
        _ = try? running.join(timeout: 10)
    }

    // MARK: - Download

    func testADownloadSendsTheExactBytesWithTheirLength() throws {
        let data = Data((0..<300_000).map { UInt8(truncatingIfNeeded: $0 &* 31) })
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("testanyware-k221-\(UUID().uuidString)")
        try data.write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }
        let reply = try agent.post("/download?path=\(Self.escape(url.path))", nil)
        XCTAssertEqual(reply.status, 200)
        XCTAssertEqual(reply.declared, data.count)
        XCTAssertEqual(reply.body, data)
        XCTAssertEqual(reply.extra, Data())

        let missing = try agent.post("/download?path=\(Self.escape(url.path + ".absent"))", nil)
        XCTAssertEqual(missing.status, 404)
        XCTAssertEqual(try missing.object()["error"] as? String, "download_failed")
    }

    /// Review k237 R2. A FIFO without a writer gets the 404 of a non-file at
    /// once, and health still answers. On failure, a writer ends the wait.
    func testADownloadOfAFIFOAnswers404AtOnce() throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("testanyware-k238-\(UUID().uuidString).pipe").path
        XCTAssertEqual(mkfifo(path, 0o600), 0)
        defer { unlink(path) }
        let agent = self.agent!
        let request = Background { try agent.post("/download?path=\(Self.escape(path))", nil) }
        let reply: HTTPReply
        do {
            reply = try request.join(timeout: Self.healthBound)
        } catch {
            let writer = open(path, O_WRONLY | O_NONBLOCK)
            _ = try? request.join(timeout: 5)
            if writer >= 0 { close(writer) }
            return XCTFail("the download of a FIFO did not answer within 1 s: \(error)")
        }
        XCTAssertEqual(reply.status, 404)
        XCTAssertEqual(try reply.object()["error"] as? String, "download_failed")
        XCTAssertEqual(try agent.get("/health").status, 200)
    }

    // MARK: - Helpers

    /// Waits for the PID file `name`, adopts the session of its process and
    /// binds that process to the fixture, so cleanup can end it.
    @discardableResult
    private func track(_ fixture: OwnedProcessFixture, _ name: String) async throws -> ProcessIdentity {
        let path = fixture.pidFile(name)
        var pid: pid_t?
        let deadline = Date().addingTimeInterval(5)
        while pid == nil, Date() < deadline {
            if let text = try? String(contentsOfFile: path, encoding: .utf8), text.hasSuffix("\n") {
                pid = pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines))
            } else {
                try await Task.sleep(nanoseconds: 10_000_000)
            }
        }
        let found = try XCTUnwrap(pid, "no PID in \(path)")
        await fixture.adopt(session: getsid(found))
        return try await fixture.track(name, command: "sleep")
    }

    static func readPid(_ path: String) -> pid_t? {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8), text.hasSuffix("\n") else { return nil }
        return pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    static func validity(_ record: [String: Any], id: String) throws -> [String] {
        ExecValidity.errors(reply: try JSONSerialization.data(withJSONObject: record), expectedID: id)
    }

    static func escape(_ path: String) -> String {
        path.addingPercentEncoding(withAllowedCharacters: .alphanumerics.union(CharacterSet(charactersIn: "/._-")))!
    }

    private func waitFor(seconds: TimeInterval, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if condition() { return true }
            Thread.sleep(forTimeInterval: 0.02)
        }
        return condition()
    }
}

/// The agent shutdown routine with a leader whose status stays unavailable.
/// The binary has no leader-wait seam, so this control runs the routine that
/// the SIGTERM handler calls, `ExecRegistry.shutDown`, in this process.
final class AgentShutdownTests: XCTestCase {

    func testShutdownDoesNotWaitForALeaderWhoseStatusStaysUnavailable() async throws {
        let registry = ExecRegistry()
        guard case .start(let signal) = registry.admit("held") else { return XCTFail("not admitted") }
        let seams = OwnerSeams(
            leaderWaiter: { BlindLeaderWaiter(pid: $0) }, captureRead: OwnerSeams.live.captureRead)
        let running = Task.detached {
            let owned = await ProcessRunner.runOwned(
                command: "sleep 30", timeoutSeconds: 60, seams: seams, cancel: signal)
            registry.publish("held", reply: owned.reply(id: "held"))
        }
        try await Task.sleep(nanoseconds: 300_000_000)
        let start = ProcessRunner.uptime()
        let pending = await Self.onThread { registry.shutDown(until: start + ExecRegistry.shutdownWaitNs) }
        let elapsed = Double(ProcessRunner.uptime() - start) / 1e9
        XCTAssertEqual(pending, [])
        XCTAssertLessThanOrEqual(elapsed, 4.0)
        let (state, reply) = registry.cancel("held", waitNs: 0)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.outcome, .cancelled(beforeStart: false))
        XCTAssertEqual(reply?.record.termination?.leaderStatus, .unavailable)
        guard case .closed = registry.admit("after") else { return XCTFail("admission stayed open") }
        _ = await running.value
        // The hold thread reaps the leader after the record. Then it is no child of ours.
        let pid = try XCTUnwrap(reply?.record.lifecycle.leaderPid)
        let deadline = Date().addingTimeInterval(5)
        var status: Int32 = 0
        while waitpid(pid_t(pid), &status, WNOHANG | WNOWAIT) >= 0, Date() < deadline {
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        XCTAssertEqual(waitpid(pid_t(pid), &status, WNOHANG), -1, "the owner did not reap the held leader")
    }

    static func onThread<T: Sendable>(_ body: @escaping @Sendable () -> T) async -> T {
        await withCheckedContinuation { continuation in
            Thread { continuation.resume(returning: body()) }.start()
        }
    }
}

/// Review k237 R4. A built agent that cannot serve health fails the
/// transport controls. It must not skip them, and the failure keeps the
/// agent's own output.
final class AgentProcessStartTests: XCTestCase {

    func testAnAgentThatExitsBeforeHealthIsAFailureWithItsOutput() throws {
        let script = FileManager.default.temporaryDirectory.appendingPathComponent("testanyware-k238-\(UUID().uuidString).sh")
        try Data("#!/bin/sh\necho 'k238 startup diagnostic' >&2\nexit 3\n".utf8).write(to: script)
        defer { try? FileManager.default.removeItem(at: script) }
        XCTAssertEqual(chmod(script.path, 0o700), 0)
        XCTAssertThrowsError(try AgentProcess.start(binary: script.path)) { error in
            XCTAssertFalse(error is XCTSkip, "a startup failure skipped the controls")
            let failure = error as? AgentProcess.NotHealthy
            XCTAssertNotNil(failure, "unexpected error: \(error)")
            XCTAssertTrue(failure?.output.contains("k238 startup diagnostic") == true, "the log was lost: \(error)")
        }
    }
}

// MARK: - Agent process

/// One agent binary on a loopback port, started with `posix_spawn` and
/// reaped by its exact PID. Its output goes to a log file.
final class AgentProcess: @unchecked Sendable {
    let pid: pid_t
    let port: Int
    let log: URL
    private var reaped = false

    static var binary: String {
        if let path = ProcessInfo.processInfo.environment["TESTANYWARE_AGENT_UNDER_TEST"], !path.isEmpty {
            return path
        }
        // The test bundle lies in the build directory beside the agent.
        return Bundle(for: AgentProcess.self).bundleURL.deletingLastPathComponent()
            .appendingPathComponent("testanyware-agent").path
    }

    /// A built agent that did not serve health. This is a failure, not a
    /// skip, so a startup regression fails `agent:macos:test` (review k237 R4).
    struct NotHealthy: Error, CustomStringConvertible {
        let binary: String
        let output: String
        var description: String { "the agent \(binary) did not answer /health: \(output)" }
    }

    static func start(binary: String = AgentProcess.binary) throws -> AgentProcess {
        let process = try AgentProcess(binary: binary, port: try freePort())
        let deadline = Date().addingTimeInterval(15)
        while Date() < deadline {
            if (try? Connection.exchange(port: process.port, Request.get("/health")).status) == 200 {
                return process
            }
            guard process.isRunning else { break }
            Thread.sleep(forTimeInterval: 0.05)
        }
        // `stop` removes the log, so the diagnostic is read first.
        process.terminate()
        let output = (try? String(contentsOf: process.log, encoding: .utf8)) ?? ""
        process.stop()
        throw NotHealthy(binary: binary, output: output)
    }

    private init(binary: String, port: Int) throws {
        self.port = port
        log = FileManager.default.temporaryDirectory.appendingPathComponent("testanyware-agent-\(UUID().uuidString).log")
        var actions: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_addopen(&actions, 1, log.path, O_WRONLY | O_CREAT | O_TRUNC, 0o600)
        posix_spawn_file_actions_adddup2(&actions, 1, 2)
        // A Swift concurrency thread blocks every signal, and a child inherits
        // the mask. The agent must see SIGTERM, so the mask is reset.
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
        let argv: [UnsafeMutablePointer<CChar>?] = [binary, String(port), "127.0.0.1"].map { strdup($0) } + [nil]
        let envp: [UnsafeMutablePointer<CChar>?] =
            ProcessInfo.processInfo.environment.map { strdup("\($0.key)=\($0.value)") } + [nil]
        defer {
            argv.forEach { free($0) }
            envp.forEach { free($0) }
        }
        var pid: pid_t = 0
        let result = posix_spawn(&pid, binary, &actions, &attributes, argv, envp)
        guard result == 0 else { throw POSIXError(POSIXErrorCode(rawValue: result) ?? .EIO) }
        self.pid = pid
    }

    var isRunning: Bool {
        if reaped { return false }
        var info = siginfo_t()
        let result = waitid(P_PID, id_t(pid), &info, WEXITED | WNOHANG | WNOWAIT)
        return !(result == 0 && info.si_pid == pid)
    }

    func signal(_ signal: Int32) {
        if !reaped { kill(pid, signal) }
    }

    /// Waits up to `seconds` for the exit and reaps the agent. Returns the
    /// exit status, the negative signal number, or nil when it still runs.
    func waitExit(seconds: TimeInterval) -> Int32? {
        let deadline = Date().addingTimeInterval(seconds)
        while isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
        guard !reaped, !isRunning else { return nil }
        var status: Int32 = 0
        while waitpid(pid, &status, 0) < 0, errno == EINTR {}
        reaped = true
        let low = status & 0x7f
        return low == 0 ? (status >> 8) & 0xff : -low
    }

    /// Ends the agent: SIGTERM, then SIGKILL after 5 s. Keeps the log.
    func terminate() {
        if !reaped {
            signal(SIGTERM)
            if waitExit(seconds: 5) == nil {
                signal(SIGKILL)
                _ = waitExit(seconds: 5)
            }
        }
    }

    /// Ends the agent as `terminate` does, then removes the log.
    func stop() {
        terminate()
        try? FileManager.default.removeItem(at: log)
    }

    private static func freePort() throws -> Int {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(.EIO) }
        defer { close(fd) }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        address.sin_port = 0
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, length) }
        }
        guard bound == 0 else { throw POSIXError(.EADDRINUSE) }
        _ = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) }
        }
        return Int(UInt16(bigEndian: address.sin_port))
    }
}

extension AgentProcess {
    func get(_ path: String) throws -> HTTPReply {
        try Connection.exchange(port: port, Request.get(path))
    }

    func post(_ path: String, _ json: Any?) throws -> HTTPReply {
        try Connection.exchange(port: port, Request.post(path, json))
    }
}

// MARK: - Raw HTTP

enum Request {
    static func get(_ path: String) -> Data {
        Data("GET \(path) HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".utf8)
    }

    static func post(_ path: String, _ json: Any?) -> Data {
        let body = json.map { (try? JSONSerialization.data(withJSONObject: $0)) ?? Data() } ?? Data()
        var head = "POST \(path) HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n"
        head += "Content-Type: application/json\r\nContent-Length: \(body.count)\r\n\r\n"
        return Data(head.utf8) + body
    }
}

/// One parsed reply. `body` holds at most the declared length, and `extra`
/// holds every byte after it.
struct HTTPReply {
    let status: Int
    let headers: [String: String]
    let declared: Int?
    let body: Data
    let extra: Data

    func object() throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any], "not a JSON object: \(body)")
    }
}

/// A blocking TCP connection to the agent with a receive deadline.
final class Connection {
    enum Loss: CaseIterable, CustomStringConvertible {
        case close, reset, halfClose
        var description: String {
            switch self {
            case .close: return "close"
            case .reset: return "reset"
            case .halfClose: return "half-close"
            }
        }
    }

    private var fd: Int32

    init(port: Int, timeout: TimeInterval = 60) throws {
        fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(.EIO) }
        var deadline = timeval(tv_sec: Int(timeout), tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &deadline, socklen_t(MemoryLayout<timeval>.size))
        var noSigpipe: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noSigpipe, socklen_t(MemoryLayout<Int32>.size))
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        address.sin_port = UInt16(port).bigEndian
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard connected == 0 else {
            let code = errno
            Darwin.close(fd)
            throw POSIXError(POSIXErrorCode(rawValue: code) ?? .ECONNREFUSED)
        }
    }

    deinit { close() }

    static func exchange(port: Int, _ request: Data) throws -> HTTPReply {
        let connection = try Connection(port: port)
        try connection.send(request)
        return try connection.reply()
    }

    func send(_ data: Data) throws {
        let sent = data.withUnsafeBytes { Darwin.send(fd, $0.baseAddress, $0.count, 0) }
        guard sent == data.count else { throw POSIXError(.EPIPE) }
    }

    /// Reads to end of file and parses one reply.
    func reply() throws -> HTTPReply {
        var raw = Data()
        var buffer = [UInt8](repeating: 0, count: 65_536)
        while true {
            let count = recv(fd, &buffer, buffer.count, 0)
            if count < 0, errno == EINTR { continue }
            guard count > 0 else { break }
            raw.append(contentsOf: buffer[0..<count])
        }
        guard let split = raw.range(of: Data("\r\n\r\n".utf8)) else {
            throw POSIXError(.ECONNRESET)
        }
        let head = String(decoding: raw[..<split.lowerBound], as: UTF8.self).components(separatedBy: "\r\n")
        let status = Int(head[0].split(separator: " ")[1]) ?? 0
        var headers: [String: String] = [:]
        for line in head.dropFirst() {
            guard let colon = line.firstIndex(of: ":") else { continue }
            headers[line[..<colon].lowercased()] = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
        }
        let rest = raw[split.upperBound...]
        let declared = headers["content-length"].flatMap(Int.init)
        let length = min(declared ?? rest.count, rest.count)
        return HTTPReply(
            status: status, headers: headers, declared: declared,
            body: Data(rest.prefix(length)), extra: Data(rest.dropFirst(length)))
    }

    /// Ends the client side of the connection in one of the k13 ways.
    func lose(_ loss: Loss) {
        switch loss {
        case .close:
            close()
        case .reset:
            var linger = Darwin.linger(l_onoff: 1, l_linger: 0)
            setsockopt(fd, SOL_SOCKET, SO_LINGER, &linger, socklen_t(MemoryLayout<Darwin.linger>.size))
            close()
        case .halfClose:
            shutdown(fd, SHUT_WR)
        }
    }

    func close() {
        if fd >= 0 {
            Darwin.close(fd)
            fd = -1
        }
    }
}

/// One blocking call on its own thread.
final class Background: @unchecked Sendable {
    private let done = DispatchSemaphore(value: 0)
    private var result: Result<HTTPReply, Error>?

    init(_ body: @escaping @Sendable () throws -> HTTPReply) {
        Thread { [self] in
            result = Result(catching: body)
            done.signal()
        }.start()
    }

    func join(timeout: TimeInterval) throws -> HTTPReply {
        guard done.wait(timeout: .now() + timeout) == .success, let result else {
            throw POSIXError(.ETIMEDOUT)
        }
        return try result.get()
    }
}
