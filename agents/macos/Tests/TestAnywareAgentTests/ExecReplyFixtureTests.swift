import Foundation
import TestAnywareAgentProtocol
import XCTest
@testable import TestAnywareAgent

/// The recorded macOS exec replies in `agents/macos/Tests/fixtures/exec-records`.
///
/// `task agent:macos:record-exec-replies` writes them. Each body is the
/// owner result through the exec mapper and the plain `JSONEncoder` that
/// `jsonOK` in AgentServer.swift uses, so it is the wire body of `/exec`.
/// The Host CLI reader tests in
/// cli-rs/crates/testanyware-protocol/tests/exec_record.rs read the same
/// files. The round trip below is the encoding drift test of the exec types.
final class ExecReplyFixtureTests: XCTestCase {

    static let recordEnv = "TESTANYWARE_RECORD_EXEC_REPLIES"
    static let directory = ExecOwnerTests.repoRoot.appendingPathComponent("agents/macos/Tests/fixtures/exec-records")
    static let source = "agents/macos/Tests/TestAnywareAgentTests/ExecReplyFixtureTests.swift (k219)"

    struct Case {
        let name: String
        let request: ExecRequest
    }

    /// The legacy macOS cases with their recorded commands, plus cases that
    /// reach the record fields the legacy cases do not reach.
    static func cases() throws -> [Case] {
        let url = ExecOwnerTests.repoRoot.appendingPathComponent("cli-rs/tests/fixtures/exec-replies/legacy-macos.json")
        let legacy = try JSONDecoder().decode(ExecOwnerTests.LegacyFile.self, from: Data(contentsOf: url))
        var chosen = legacy.cases.map { item -> Case in
            // The spawn-failure child writes the id `spawn-failure`.
            let id = item.case == "spawn-failure" ? "spawn-failure" : nil
            return Case(
                name: item.case,
                request: ExecRequest(command: item.request.command, timeout: item.request.timeout, id: id))
        }
        chosen += [
            Case(
                name: "remaining-group",
                request: ExecRequest(command: "sleep 3 & printf 'leader done\\n'; exit 0", timeout: 10)),
            Case(name: "requested-id", request: ExecRequest(command: "printf 'with id'", timeout: 10, id: "k219.fixture-1")),
            Case(name: "timeout-child-kill", request: ExecRequest(command: "trap '' TERM; sleep 30 & wait", timeout: 2)),
            // Review k231, finding R3: Foundation removes a trailing vertical
            // tab and a trailing U+0085.
            Case(name: "trim-unicode", request: ExecRequest(command: "printf 'a\\v'; printf 'e\\302\\205' >&2", timeout: 10)),
        ]
        return chosen
    }

    // MARK: - Recorder

    /// Writes one fixture per case. It does nothing in a normal run.
    func testRecordExecReplies() async throws {
        guard let out = ProcessInfo.processInfo.environment[Self.recordEnv] else { return }
        let directory = URL(fileURLWithPath: out)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        for item in try Self.cases() {
            let body: Data
            if item.name == "spawn-failure" {
                body = try ExecOwnerTests.spawnFailureReply()
            } else {
                let owned = await ProcessRunner.runOwned(
                    command: item.request.command, timeoutSeconds: item.request.timeout ?? 30)
                // The id rule of handleExec in AgentServer.swift.
                let id = item.request.id ?? UUID().uuidString.lowercased()
                body = try JSONEncoder().encode(owned.reply(id: id))
            }
            XCTAssertEqual(ExecValidity.errors(reply: body, expectedID: item.request.id), [], item.name)
            let fixture = Fixture(
                case: item.name, source: Self.source, request: item.request, httpStatus: 200,
                body: try JSONDecoder().decode(ExecReply.self, from: body))
            var text = try encoder.encode(fixture)
            text.append(0x0a)
            try text.write(to: directory.appendingPathComponent("\(item.name).json"))
        }
    }

    // MARK: - Checks

    func testFixturesExist() throws {
        let names = Set(try Self.fixtures().map(\.name))
        XCTAssertEqual(names, Set(try Self.cases().map(\.name)))
    }

    func testEveryFixtureIsAValidReply() throws {
        for fixture in try Self.fixtures() {
            XCTAssertEqual(fixture.object["httpStatus"] as? Int, 200, fixture.name)
            XCTAssertEqual(
                ExecValidity.errors(reply: fixture.body, expectedID: fixture.sentID), [], fixture.name)
        }
    }

    /// The encoding drift test. Each recorded body decodes into `ExecReply`
    /// and encodes back to the same JSON value. A renamed key, a dropped key
    /// or a null key that the encoder leaves out makes the values differ.
    func testFixturesRoundTripThroughTheWireTypes() throws {
        for fixture in try Self.fixtures() {
            let reply = try JSONDecoder().decode(ExecReply.self, from: fixture.body)
            let again = try JSONSerialization.jsonObject(with: JSONEncoder().encode(reply)) as? NSDictionary
            let original = try JSONSerialization.jsonObject(with: fixture.body) as? NSDictionary
            XCTAssertNotNil(original, fixture.name)
            XCTAssertEqual(again, original, fixture.name)
        }
    }

    func testFixturesHoldTheNamedOutcomes() throws {
        var replies: [String: ExecReply] = [:]
        for fixture in try Self.fixtures() {
            replies[fixture.name] = try JSONDecoder().decode(ExecReply.self, from: fixture.body)
        }
        func reply(_ name: String) throws -> ExecReply { try XCTUnwrap(replies[name], name) }

        XCTAssertEqual(try reply("exit-23-partial").record.outcome, .exited(exitCode: 23))
        XCTAssertEqual(try reply("binary").record.evidence.stdout.base64, "AP+AQQo=")
        XCTAssertEqual(
            Data(base64Encoded: try reply("oracle-text").record.evidence.stdout.base64),
            Data("Explain this change.".utf8))
        XCTAssertEqual(try reply("signal-kill").record.outcome, .signalled(signal: 9, signalName: "SIGKILL"))
        XCTAssertEqual(try reply("exit-9").record.outcome, .exited(exitCode: 9))
        guard case .spawnFailed(_, let errorName, _) = try reply("spawn-failure").record.outcome else {
            return XCTFail("spawn-failure is not a spawn failure")
        }
        XCTAssertEqual(errorName, "EAGAIN")
        XCTAssertEqual(try reply("remaining-group").record.remainingGroup, .present)
        XCTAssertEqual(try reply("requested-id").record.id, "k219.fixture-1")

        let deadline = try XCTUnwrap(try reply("deadline").record.termination)
        XCTAssertEqual(deadline.leaderStatus, .signalled(signal: Int(SIGTERM)))
        let ignored = try XCTUnwrap(try reply("deadline-term-ignored").record.termination)
        XCTAssertEqual(ignored.leaderStatus, .signalled(signal: Int(SIGKILL)))
        let killed = try XCTUnwrap(try reply("timeout-child-kill").record.termination)
        XCTAssertEqual(killed.leaderStatus, .signalled(signal: Int(SIGKILL)))
        XCTAssertEqual(killed.groupEmpty, .yes)

        let trimmed = try reply("trim-unicode")
        XCTAssertEqual(trimmed.stdout, "a")
        XCTAssertEqual(trimmed.stderr, "e")
        XCTAssertEqual(Data(base64Encoded: trimmed.record.evidence.stdout.base64), Data("a\u{0B}".utf8))
        XCTAssertEqual(Data(base64Encoded: trimmed.record.evidence.stderr.base64), Data("e\u{85}".utf8))
    }

    /// The legacy fields of each recorded legacy case equal the old reply.
    /// The old spawn-failure reply was HTTP 500, so it has no fields.
    func testFixtureLegacyFieldsEqualTheOldReplies() throws {
        let url = ExecOwnerTests.repoRoot.appendingPathComponent("cli-rs/tests/fixtures/exec-replies/legacy-macos.json")
        let legacy = try JSONDecoder().decode(ExecOwnerTests.LegacyFile.self, from: Data(contentsOf: url))
        let fixtures = Dictionary(uniqueKeysWithValues: try Self.fixtures().map { ($0.name, $0) })
        var checked = 0
        for item in legacy.cases where item.httpStatus == 200 {
            let old = try JSONDecoder().decode(ExecOwnerTests.LegacyReply.self, from: Data(item.bodyText.utf8))
            let body = try XCTUnwrap(fixtures[item.case], item.case).body
            let new = try JSONDecoder().decode(ExecReply.self, from: body)
            XCTAssertEqual(new.exitCode, old.exitCode, item.case)
            XCTAssertEqual(new.stdout, old.stdout, item.case)
            XCTAssertEqual(new.stderr, old.stderr, item.case)
            XCTAssertEqual(new.timedOut, old.timedOut, item.case)
            checked += 1
        }
        XCTAssertEqual(checked, 7)
    }

    // MARK: - Helpers

    struct Fixture: Encodable {
        let `case`: String
        let source: String
        let request: ExecRequest
        let httpStatus: Int
        let body: ExecReply
    }

    struct Recorded {
        let name: String
        let object: [String: Any]
        /// The raw body, re-serialised from the fixture's JSON value.
        let body: Data
        /// The id of the request, else the id of the reply.
        let sentID: String?
    }

    static func fixtures() throws -> [Recorded] {
        let names = try FileManager.default.contentsOfDirectory(atPath: directory.path)
            .filter { $0.hasSuffix(".json") }.sorted()
        return try names.map { name in
            let data = try Data(contentsOf: directory.appendingPathComponent(name))
            let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any], name)
            let body = try XCTUnwrap(object["body"] as? [String: Any], name)
            let request = try XCTUnwrap(object["request"] as? [String: Any], name)
            return Recorded(
                name: try XCTUnwrap(object["case"] as? String, name),
                object: object,
                body: try JSONSerialization.data(withJSONObject: body),
                sentID: (request["id"] as? String) ?? (body["id"] as? String))
        }
    }
}
