import Foundation
import TestAnywareAgentProtocol

// Copies the owner result into the exec wire types. ProcessRunner.swift
// cannot import the protocol module, because the old-path detectors compile
// it alone, so the copy lives here.

extension OwnedExecResult {

    /// The macOS exec reply for this result under the exec id `id`.
    public func reply(id: String) -> ExecReply {
        ExecReply(
            exitCode: legacy.exitCode, stdout: legacy.stdout, stderr: legacy.stderr,
            timedOut: legacy.timedOut, record: record(id: id)
        )
    }

    /// The typed exec record for this result under the exec id `id`.
    public func record(id: String) -> ExecRecord {
        ExecRecord(
            id: id,
            outcome: wireOutcome,
            evidence: ExecEvidence(stdout: stdout.wire, stderr: stderr.wire),
            lifecycle: ExecLifecycle(
                leaderPid: leaderPid.map(Int.init),
                // The leader leads a new session, so the group id is its pid.
                groupId: leaderPid.map(Int.init),
                leaderStartTime: leaderStartTime,
                spawnMs: spawnMs,
                durationMs: durationMs,
                ownerLatenessMs: ownerLatenessMs,
                finalisedAfterMs: finalisedAfterMs
            ),
            remainingGroup: remainingGroup.flatMap(ExecRemainingGroup.init(rawValue:)),
            termination: termination.map { $0.wire(finalisedAfterMs: finalisedAfterMs) },
            legacyProjection: "macos"
        )
    }

    var wireOutcome: ExecOutcome {
        switch outcome {
        case .exited(let code):
            return .exited(exitCode: Int(code))
        case .signalled(let signal):
            return .signalled(signal: Int(signal), signalName: ProcessRunner.signalName(signal))
        case .timedOut(let seconds):
            return .timedOut(timeoutSeconds: seconds)
        case .cancelled(let beforeStart):
            return .cancelled(beforeStart: beforeStart)
        case .spawnFailed(let code):
            return .spawnFailed(
                errno: Int(code), errorName: ProcessRunner.errorName(code), message: ProcessRunner.message(code)
            )
        }
    }
}

extension OwnedStream {
    var wire: ExecStreamEvidence {
        ExecStreamEvidence(
            base64: base64, bytes: data.count, fileBytes: fileBytes, sha256: sha256,
            capture: complete ? .complete : .incomplete, reason: reason
        )
    }
}

extension OwnedTermination {
    /// The termination bound from the spec: the group is empty, SIGKILL left
    /// by T0 + 2 s, and finalisation ended within 2 s of SIGKILL.
    static let boundMs = 2000

    func wire(finalisedAfterMs: Int) -> ExecTermination {
        let status: ExecLeaderStatus
        switch leaderStatus {
        case .exited(let code): status = .exited(exitCode: Int(code))
        case .signalled(let signal): status = .signalled(signal: Int(signal))
        case .unavailable: status = .unavailable
        }
        let empty = ExecGroupEmpty(rawValue: groupEmpty) ?? .unknown
        return ExecTermination(
            termSentAfterMs: termSentAfterMs,
            killSentAfterMs: killSentAfterMs,
            leaderStatus: status,
            leaderExitAfterMs: leaderExitAfterMs,
            groupEmpty: empty,
            groupEmptyAfterMs: groupEmptyAfterMs,
            withinBounds: empty == .yes
                && killSentAfterMs <= Self.boundMs
                && finalisedAfterMs - killSentAfterMs <= Self.boundMs
        )
    }
}
