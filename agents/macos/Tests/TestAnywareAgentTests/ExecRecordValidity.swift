import CryptoKit
import Foundation
import TestAnywareAgentProtocol

/// The validity rules of an exec reply, from the "Exec record" section of
/// docs/specs/guest-command-lifecycle.md. The Host CLI reader in
/// cli-rs/crates/testanyware-protocol/src/exec_record.rs applies the same
/// rules. Times and process identities differ from run to run, so the rules
/// check relations, not values.
enum ExecValidity {

    static let recordKeys: Set<String> = [
        "id", "outcome", "evidence", "lifecycle", "remainingGroup", "termination", "legacyProjection",
    ]
    static let legacyKeys: Set<String> = ["exitCode", "stdout", "stderr", "timedOut"]
    static let streamKeys: Set<String> = ["base64", "bytes", "fileBytes", "sha256", "capture"]
    static let lifecycleKeys: Set<String> = [
        "leaderPid", "groupId", "leaderStartTime", "spawnMs", "durationMs", "ownerLatenessMs", "finalisedAfterMs",
    ]
    static let terminationKeys: Set<String> = [
        "termSentAfterMs", "killSentAfterMs", "leaderStatus", "leaderExitAfterMs",
        "groupEmpty", "groupEmptyAfterMs", "withinBounds",
    ]

    /// Returns each broken rule of a macOS exec reply body.
    static func errors(reply body: Data, expectedID: String? = nil) -> [String] {
        guard let object = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
            return ["the body is not a JSON object"]
        }
        var errors = keyErrors(object, expected: recordKeys.union(legacyKeys), at: "reply")
        let reply: ExecReply
        do {
            reply = try JSONDecoder().decode(ExecReply.self, from: body)
        } catch {
            return errors + ["the body does not decode: \(error)"]
        }
        errors += shapeErrors(object)
        errors += recordErrors(reply.record, expectedID: expectedID, projection: "macos")
        errors += legacyErrors(reply)
        return errors
    }

    /// Returns each broken rule of the record fields alone. The Linux reply
    /// carries the same record next to its own legacy fields.
    static func errors(record body: Data, projection: String) -> [String] {
        guard let object = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
            return ["the body is not a JSON object"]
        }
        let missing = recordKeys.subtracting(object.keys)
        var errors = missing.isEmpty ? [] : ["reply lacks \(missing.sorted())"]
        do {
            let record = try JSONDecoder().decode(ExecRecord.self, from: body)
            errors += shapeErrors(object)
            errors += recordErrors(record, expectedID: nil, projection: projection)
        } catch {
            errors.append("the body does not decode: \(error)")
        }
        return errors
    }

    private static func keyErrors(_ object: [String: Any], expected: Set<String>, at path: String) -> [String] {
        let keys = Set(object.keys)
        var errors: [String] = []
        let missing = expected.subtracting(keys)
        let extra = keys.subtracting(expected)
        if !missing.isEmpty { errors.append("\(path) lacks \(missing.sorted())") }
        if !extra.isEmpty { errors.append("\(path) has unknown \(extra.sorted())") }
        return errors
    }

    /// Checks that every key is present, with null where it does not apply.
    private static func shapeErrors(_ object: [String: Any]) -> [String] {
        var errors: [String] = []
        if let evidence = object["evidence"] as? [String: Any] {
            for name in ["stdout", "stderr"] {
                guard let stream = evidence[name] as? [String: Any] else {
                    errors.append("evidence lacks \(name)")
                    continue
                }
                let incomplete = (stream["capture"] as? String) == "incomplete"
                let expected = incomplete ? streamKeys.union(["reason"]) : streamKeys
                errors += keyErrors(stream, expected: expected, at: "evidence.\(name)")
            }
        }
        if let lifecycle = object["lifecycle"] as? [String: Any] {
            errors += keyErrors(lifecycle, expected: lifecycleKeys, at: "lifecycle")
        }
        if let termination = object["termination"] as? [String: Any] {
            errors += keyErrors(termination, expected: terminationKeys, at: "termination")
        }
        return errors
    }

    static func recordErrors(_ record: ExecRecord, expectedID: String?, projection: String) -> [String] {
        var errors: [String] = []
        if !ExecRequest.isValidID(record.id) { errors.append("id \(record.id) is not a valid exec id") }
        if let expectedID, record.id != expectedID { errors.append("id \(record.id) is not \(expectedID)") }
        if record.legacyProjection != projection {
            errors.append("legacyProjection is \(record.legacyProjection), not \(projection)")
        }
        for (name, stream) in [("stdout", record.evidence.stdout), ("stderr", record.evidence.stderr)] {
            errors += streamErrors(stream, name: name)
        }

        let life = record.lifecycle
        let times = [life.spawnMs, life.ownerLatenessMs, life.finalisedAfterMs] + [life.durationMs].compactMap { $0 }
        if times.contains(where: { $0 < 0 }) { errors.append("lifecycle has a negative time") }
        var started = true
        var mayRemain = false
        var terminated = false
        switch record.outcome {
        case .spawnFailed(let code, let name, let message):
            started = false
            if code <= 0 || name.isEmpty || message.isEmpty { errors.append("spawn_failed lacks its error") }
        case .exited(let code):
            mayRemain = true
            if !(0...255).contains(code) { errors.append("exit code \(code) is out of range") }
        case .signalled(let signal, let name):
            mayRemain = true
            if signal <= 0 || !name.hasPrefix("SIG") { errors.append("signal \(signal) \(name) is invalid") }
        case .timedOut(let seconds):
            terminated = true
            if seconds <= 0 { errors.append("timeoutSeconds \(seconds) is not positive") }
        case .cancelled:
            terminated = true
        }
        if started {
            if life.leaderPid == nil || life.leaderStartTime == nil || life.durationMs == nil {
                errors.append("a started command lacks its leader identity or duration")
            }
            if life.groupId != life.leaderPid { errors.append("groupId is not the leader PID") }
        } else if life.leaderPid != nil || life.groupId != nil || life.leaderStartTime != nil
                    || life.durationMs != nil {
            errors.append("a spawn failure carries a leader identity")
        }
        if mayRemain != (record.remainingGroup != nil) {
            errors.append("remainingGroup is present only after exited or signalled")
        }
        if terminated != (record.termination != nil) {
            errors.append("termination is present only after timed_out or cancelled")
        }
        if let term = record.termination {
            errors += terminationErrors(term, finalisedAfterMs: life.finalisedAfterMs)
        }
        return errors
    }

    private static func streamErrors(_ stream: ExecStreamEvidence, name: String) -> [String] {
        var errors: [String] = []
        guard let bytes = Data(base64Encoded: stream.base64) else {
            return ["\(name) base64 does not decode"]
        }
        if bytes.count != stream.bytes { errors.append("\(name) bytes \(stream.bytes) is not \(bytes.count)") }
        let digest = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        if digest != stream.sha256 { errors.append("\(name) sha256 does not match its bytes") }
        switch stream.capture {
        case .complete:
            if stream.bytes != stream.fileBytes { errors.append("complete \(name) has bytes != fileBytes") }
            if stream.reason != nil { errors.append("complete \(name) carries a reason") }
        case .incomplete:
            if stream.bytes >= stream.fileBytes { errors.append("incomplete \(name) has bytes >= fileBytes") }
            if !["deadline", "read_error"].contains(stream.reason ?? "") {
                errors.append("incomplete \(name) has reason \(stream.reason ?? "nil")")
            }
        }
        return errors
    }

    private static func terminationErrors(_ term: ExecTermination, finalisedAfterMs: Int) -> [String] {
        var errors: [String] = []
        if !(0 <= term.termSentAfterMs && term.termSentAfterMs <= term.killSentAfterMs
             && term.killSentAfterMs <= finalisedAfterMs) {
            errors.append("termination times are out of order")
        }
        let unavailable = term.leaderStatus == .unavailable
        if unavailable != (term.leaderExitAfterMs == nil) {
            errors.append("leaderExitAfterMs is null only when the leader status is unavailable")
        }
        if (term.groupEmpty == .yes) != (term.groupEmptyAfterMs != nil) {
            errors.append("groupEmptyAfterMs is present only when groupEmpty is yes")
        }
        let bound = term.groupEmpty == .yes && term.killSentAfterMs <= 2000
            && finalisedAfterMs - term.killSentAfterMs <= 2000
        if term.withinBounds != bound { errors.append("withinBounds is \(term.withinBounds), not \(bound)") }
        return errors
    }

    /// The macOS legacy fields follow from the outcome.
    private static func legacyErrors(_ reply: ExecReply) -> [String] {
        switch reply.record.outcome {
        case .exited(let code):
            return reply.exitCode == code && !reply.timedOut ? [] : ["legacy fields do not match exited \(code)"]
        case .signalled(let signal, _):
            return reply.exitCode == signal && !reply.timedOut ? [] : ["legacy fields do not match signal \(signal)"]
        case .timedOut(let seconds):
            let note = "Process timed out after \(seconds)s"
            let noted = reply.stderr == note || reply.stderr.hasSuffix("\n[\(note)]")
            return reply.exitCode == -1 && reply.timedOut && noted ? [] : ["legacy fields do not match timed_out"]
        case .spawnFailed(_, _, let message):
            let ok = reply.exitCode == -1 && reply.stdout.isEmpty && reply.stderr == message && !reply.timedOut
            return ok ? [] : ["legacy fields do not match spawn_failed"]
        case .cancelled:
            return ["the macOS agent does not cancel before k221"]
        }
    }
}
