import Foundation

// The exec wire types (docs/specs/guest-command-lifecycle.md, "Exec record").
//
// Every record key is always present. A key that does not apply carries
// null, so these types encode optionals with `encode`, not `encodeIfPresent`.
// The only optional key is `reason`, which a stream carries only when its
// capture is incomplete. The decoders require every key, as the host reader
// in cli-rs/crates/testanyware-protocol/src/exec_record.rs does.

/// The body of `POST /exec`.
public struct ExecRequest: Codable, Sendable, Equatable {
    public var command: String
    public var timeout: Int?
    public var detach: Bool?
    public var id: String?

    public init(command: String, timeout: Int? = nil, detach: Bool? = nil, id: String? = nil) {
        self.command = command
        self.timeout = timeout
        self.detach = detach
        self.id = id
    }

    /// Reports whether an exec id has 1 to 64 characters from `A-Z a-z 0-9 . _ -`.
    public static func isValidID(_ id: String) -> Bool {
        let allowed = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-".unicodeScalars)
        return (1...64).contains(id.unicodeScalars.count) && id.unicodeScalars.allSatisfy(allowed.contains)
    }
}

/// The first and final outcome of a command.
public enum ExecOutcome: Codable, Sendable, Equatable {
    case exited(exitCode: Int)
    case signalled(signal: Int, signalName: String)
    case timedOut(timeoutSeconds: Int)
    case cancelled(beforeStart: Bool)
    case spawnFailed(errno: Int, errorName: String, message: String)

    private enum Keys: String, CodingKey {
        case kind, exitCode, signal, signalName, timeoutSeconds, beforeStart, errno, errorName, message
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "exited": self = .exited(exitCode: try c.decode(Int.self, forKey: .exitCode))
        case "signalled":
            self = .signalled(
                signal: try c.decode(Int.self, forKey: .signal),
                signalName: try c.decode(String.self, forKey: .signalName))
        case "timed_out": self = .timedOut(timeoutSeconds: try c.decode(Int.self, forKey: .timeoutSeconds))
        case "cancelled": self = .cancelled(beforeStart: try c.decode(Bool.self, forKey: .beforeStart))
        case "spawn_failed":
            self = .spawnFailed(
                errno: try c.decode(Int.self, forKey: .errno),
                errorName: try c.decode(String.self, forKey: .errorName),
                message: try c.decode(String.self, forKey: .message))
        case let kind:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: c, debugDescription: "unknown outcome \(kind)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .exited(let exitCode):
            try c.encode("exited", forKey: .kind)
            try c.encode(exitCode, forKey: .exitCode)
        case .signalled(let signal, let signalName):
            try c.encode("signalled", forKey: .kind)
            try c.encode(signal, forKey: .signal)
            try c.encode(signalName, forKey: .signalName)
        case .timedOut(let timeoutSeconds):
            try c.encode("timed_out", forKey: .kind)
            try c.encode(timeoutSeconds, forKey: .timeoutSeconds)
        case .cancelled(let beforeStart):
            try c.encode("cancelled", forKey: .kind)
            try c.encode(beforeStart, forKey: .beforeStart)
        case .spawnFailed(let errno, let errorName, let message):
            try c.encode("spawn_failed", forKey: .kind)
            try c.encode(errno, forKey: .errno)
            try c.encode(errorName, forKey: .errorName)
            try c.encode(message, forKey: .message)
        }
    }
}

public enum ExecCapture: String, Codable, Sendable {
    case complete, incomplete
}

/// The evidence of one output stream.
public struct ExecStreamEvidence: Codable, Sendable, Equatable {
    public var base64: String
    public var bytes: Int
    public var fileBytes: Int
    public var sha256: String
    public var capture: ExecCapture
    /// `deadline` or `read_error` for an incomplete stream, else nil.
    public var reason: String?

    public init(base64: String, bytes: Int, fileBytes: Int, sha256: String, capture: ExecCapture, reason: String?) {
        self.base64 = base64
        self.bytes = bytes
        self.fileBytes = fileBytes
        self.sha256 = sha256
        self.capture = capture
        self.reason = reason
    }
}

public struct ExecEvidence: Codable, Sendable, Equatable {
    public var stdout: ExecStreamEvidence
    public var stderr: ExecStreamEvidence

    public init(stdout: ExecStreamEvidence, stderr: ExecStreamEvidence) {
        self.stdout = stdout
        self.stderr = stderr
    }
}

public struct ExecLifecycle: Codable, Sendable, Equatable {
    public var leaderPid: Int?
    public var groupId: Int?
    public var leaderStartTime: String?
    public var spawnMs: Int
    public var durationMs: Int?
    public var ownerLatenessMs: Int
    public var finalisedAfterMs: Int

    public init(
        leaderPid: Int?, groupId: Int?, leaderStartTime: String?, spawnMs: Int,
        durationMs: Int?, ownerLatenessMs: Int, finalisedAfterMs: Int
    ) {
        self.leaderPid = leaderPid
        self.groupId = groupId
        self.leaderStartTime = leaderStartTime
        self.spawnMs = spawnMs
        self.durationMs = durationMs
        self.ownerLatenessMs = ownerLatenessMs
        self.finalisedAfterMs = finalisedAfterMs
    }

    private enum Keys: String, CodingKey {
        case leaderPid, groupId, leaderStartTime, spawnMs, durationMs, ownerLatenessMs, finalisedAfterMs
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        leaderPid = try c.decode(Int?.self, forKey: .leaderPid)
        groupId = try c.decode(Int?.self, forKey: .groupId)
        leaderStartTime = try c.decode(String?.self, forKey: .leaderStartTime)
        spawnMs = try c.decode(Int.self, forKey: .spawnMs)
        durationMs = try c.decode(Int?.self, forKey: .durationMs)
        ownerLatenessMs = try c.decode(Int.self, forKey: .ownerLatenessMs)
        finalisedAfterMs = try c.decode(Int.self, forKey: .finalisedAfterMs)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(leaderPid, forKey: .leaderPid)
        try c.encode(groupId, forKey: .groupId)
        try c.encode(leaderStartTime, forKey: .leaderStartTime)
        try c.encode(spawnMs, forKey: .spawnMs)
        try c.encode(durationMs, forKey: .durationMs)
        try c.encode(ownerLatenessMs, forKey: .ownerLatenessMs)
        try c.encode(finalisedAfterMs, forKey: .finalisedAfterMs)
    }
}

public enum ExecRemainingGroup: String, Codable, Sendable {
    case none, present, unknown
}

public enum ExecLeaderStatus: Codable, Sendable, Equatable {
    case exited(exitCode: Int)
    case signalled(signal: Int)
    case unavailable

    private enum Keys: String, CodingKey {
        case kind, exitCode, signal
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "exited": self = .exited(exitCode: try c.decode(Int.self, forKey: .exitCode))
        case "signalled": self = .signalled(signal: try c.decode(Int.self, forKey: .signal))
        case "unavailable": self = .unavailable
        case let kind:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: c, debugDescription: "unknown status \(kind)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .exited(let exitCode):
            try c.encode("exited", forKey: .kind)
            try c.encode(exitCode, forKey: .exitCode)
        case .signalled(let signal):
            try c.encode("signalled", forKey: .kind)
            try c.encode(signal, forKey: .signal)
        case .unavailable:
            try c.encode("unavailable", forKey: .kind)
        }
    }
}

public enum ExecGroupEmpty: String, Codable, Sendable {
    case yes, no, unknown
}

public struct ExecTermination: Codable, Sendable, Equatable {
    public var termSentAfterMs: Int
    public var killSentAfterMs: Int
    public var leaderStatus: ExecLeaderStatus
    public var leaderExitAfterMs: Int?
    public var groupEmpty: ExecGroupEmpty
    public var groupEmptyAfterMs: Int?
    public var withinBounds: Bool

    public init(
        termSentAfterMs: Int, killSentAfterMs: Int, leaderStatus: ExecLeaderStatus, leaderExitAfterMs: Int?,
        groupEmpty: ExecGroupEmpty, groupEmptyAfterMs: Int?, withinBounds: Bool
    ) {
        self.termSentAfterMs = termSentAfterMs
        self.killSentAfterMs = killSentAfterMs
        self.leaderStatus = leaderStatus
        self.leaderExitAfterMs = leaderExitAfterMs
        self.groupEmpty = groupEmpty
        self.groupEmptyAfterMs = groupEmptyAfterMs
        self.withinBounds = withinBounds
    }

    private enum Keys: String, CodingKey {
        case termSentAfterMs, killSentAfterMs, leaderStatus, leaderExitAfterMs
        case groupEmpty, groupEmptyAfterMs, withinBounds
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        termSentAfterMs = try c.decode(Int.self, forKey: .termSentAfterMs)
        killSentAfterMs = try c.decode(Int.self, forKey: .killSentAfterMs)
        leaderStatus = try c.decode(ExecLeaderStatus.self, forKey: .leaderStatus)
        leaderExitAfterMs = try c.decode(Int?.self, forKey: .leaderExitAfterMs)
        groupEmpty = try c.decode(ExecGroupEmpty.self, forKey: .groupEmpty)
        groupEmptyAfterMs = try c.decode(Int?.self, forKey: .groupEmptyAfterMs)
        withinBounds = try c.decode(Bool.self, forKey: .withinBounds)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(termSentAfterMs, forKey: .termSentAfterMs)
        try c.encode(killSentAfterMs, forKey: .killSentAfterMs)
        try c.encode(leaderStatus, forKey: .leaderStatus)
        try c.encode(leaderExitAfterMs, forKey: .leaderExitAfterMs)
        try c.encode(groupEmpty, forKey: .groupEmpty)
        try c.encode(groupEmptyAfterMs, forKey: .groupEmptyAfterMs)
        try c.encode(withinBounds, forKey: .withinBounds)
    }
}

/// The added record fields, without the legacy fields. The Linux and the
/// macOS agents send the same record shape.
public struct ExecRecord: Codable, Sendable, Equatable {
    public var id: String
    public var outcome: ExecOutcome
    public var evidence: ExecEvidence
    public var lifecycle: ExecLifecycle
    public var remainingGroup: ExecRemainingGroup?
    public var termination: ExecTermination?
    public var legacyProjection: String

    public init(
        id: String, outcome: ExecOutcome, evidence: ExecEvidence, lifecycle: ExecLifecycle,
        remainingGroup: ExecRemainingGroup?, termination: ExecTermination?, legacyProjection: String
    ) {
        self.id = id
        self.outcome = outcome
        self.evidence = evidence
        self.lifecycle = lifecycle
        self.remainingGroup = remainingGroup
        self.termination = termination
        self.legacyProjection = legacyProjection
    }

    private enum Keys: String, CodingKey {
        case id, outcome, evidence, lifecycle, remainingGroup, termination, legacyProjection
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        id = try c.decode(String.self, forKey: .id)
        outcome = try c.decode(ExecOutcome.self, forKey: .outcome)
        evidence = try c.decode(ExecEvidence.self, forKey: .evidence)
        lifecycle = try c.decode(ExecLifecycle.self, forKey: .lifecycle)
        remainingGroup = try c.decode(ExecRemainingGroup?.self, forKey: .remainingGroup)
        termination = try c.decode(ExecTermination?.self, forKey: .termination)
        legacyProjection = try c.decode(String.self, forKey: .legacyProjection)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(id, forKey: .id)
        try c.encode(outcome, forKey: .outcome)
        try c.encode(evidence, forKey: .evidence)
        try c.encode(lifecycle, forKey: .lifecycle)
        try c.encode(remainingGroup, forKey: .remainingGroup)
        try c.encode(termination, forKey: .termination)
        try c.encode(legacyProjection, forKey: .legacyProjection)
    }
}

/// The macOS exec reply: the four legacy fields, then the record fields at
/// the same level.
public struct ExecReply: Codable, Sendable, Equatable {
    public var exitCode: Int32
    public var stdout: String
    public var stderr: String
    public var timedOut: Bool
    public var record: ExecRecord

    public init(exitCode: Int32, stdout: String, stderr: String, timedOut: Bool, record: ExecRecord) {
        self.exitCode = exitCode
        self.stdout = stdout
        self.stderr = stderr
        self.timedOut = timedOut
        self.record = record
    }

    private enum Keys: String, CodingKey {
        case exitCode, stdout, stderr, timedOut
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        exitCode = try c.decode(Int32.self, forKey: .exitCode)
        stdout = try c.decode(String.self, forKey: .stdout)
        stderr = try c.decode(String.self, forKey: .stderr)
        timedOut = try c.decode(Bool.self, forKey: .timedOut)
        record = try ExecRecord(from: decoder)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(exitCode, forKey: .exitCode)
        try c.encode(stdout, forKey: .stdout)
        try c.encode(stderr, forKey: .stderr)
        try c.encode(timedOut, forKey: .timedOut)
        try record.encode(to: encoder)
    }
}

/// The legacy reply of a detached command, unchanged.
public struct ExecDetachedReply: Codable, Sendable, Equatable {
    public var exitCode: Int32
    public var stdout: String
    public var stderr: String
    public var timedOut: Bool

    public init() {
        exitCode = 0
        stdout = ""
        stderr = ""
        timedOut = false
    }
}
