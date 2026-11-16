// k3 old-path probe. Build it together with the unchanged agent source
// agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift, so that the probe
// calls the real `ProcessRunner.runShell`.
//
// Usage: probe CONDITION TIMEOUT_SECONDS
//   CONDITION is one of:
//     unsaturated  no blockers in the global queue
//     free-2       saturate the global queue, then free exactly two workers
//     free-1       saturate the global queue, then free exactly one worker
//
// The probe prints one JSON object on stdout. It always releases its blockers
// before it exits.

import Foundation

let maxBlockers = 512
let startWaitMilliseconds = 500
let surplusWaitSeconds = 5
let settleMilliseconds = 200

final class Calibration: @unchecked Sendable {
    private let lock = NSLock()
    private var surplus = false
    private(set) var running = 0
    private(set) var surplusRan = false

    /// Called by each blocker when GCD gives it a worker. Returns true when
    /// the blocker must hold its worker, false when it arrived after
    /// saturation and must return at once.
    func arrive() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if surplus {
            surplusRan = true
            return false
        }
        running += 1
        return true
    }

    /// Called after a blocker did not start in time. Returns true when the
    /// late blocker has not arrived, so the pool is saturated at `running`.
    func markSaturated(expected: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if running != expected { return false }
        surplus = true
        return true
    }

    func didSurplusRun() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return surplusRan
    }
}

func monotonicSeconds() -> Double {
    Double(DispatchTime.now().uptimeNanoseconds) / 1e9
}

func emit(_ object: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

let arguments = CommandLine.arguments
guard arguments.count == 3,
      ["unsaturated", "free-2", "free-1"].contains(arguments[1]),
      let timeoutSeconds = Int(arguments[2]), timeoutSeconds > 0
else {
    FileHandle.standardError.write("usage: probe unsaturated|free-2|free-1 TIMEOUT_SECONDS\n".data(using: .utf8)!)
    exit(64)
}
let condition = arguments[1]

let calibration = Calibration()
let gate = DispatchSemaphore(value: 0)
let started = DispatchSemaphore(value: 0)
var saturatedAt: Int? = nil
var freed = 0
var surplusRan = false
let calibrationStart = monotonicSeconds()

if condition != "unsaturated" {
    var dispatched = 0
    while dispatched < maxBlockers {
        DispatchQueue.global().async {
            guard calibration.arrive() else { return }
            started.signal()
            gate.wait()
        }
        dispatched += 1
        if started.wait(timeout: .now() + .milliseconds(startWaitMilliseconds)) == .timedOut {
            if calibration.markSaturated(expected: dispatched - 1) {
                saturatedAt = dispatched - 1
                break
            }
            // The late blocker arrived between the timeout and the check.
            // Consume its start signal and keep calibrating.
            started.wait()
        }
    }
    if let running = saturatedAt {
        freed = condition == "free-2" ? 2 : 1
        for _ in 0..<min(freed, running) { gate.signal() }
        let deadline = monotonicSeconds() + Double(surplusWaitSeconds)
        while !calibration.didSurplusRun() && monotonicSeconds() < deadline {
            usleep(10_000)
        }
        surplusRan = calibration.didSurplusRun()
        usleep(useconds_t(settleMilliseconds * 1000))
    }
}
let calibrationSeconds = monotonicSeconds() - calibrationStart

var record: [String: Any] = [
    "condition": condition,
    "timeout_seconds": timeoutSeconds,
    "max_blockers": maxBlockers,
    "saturated_at": saturatedAt.map { $0 as Any } ?? NSNull(),
    "freed": freed,
    "surplus_ran": surplusRan,
    "calibration_seconds": calibrationSeconds,
    "cpu_count": ProcessInfo.processInfo.activeProcessorCount,
]

if condition != "unsaturated" && (saturatedAt == nil || !surplusRan) {
    record["outcome"] = "calibration-failed"
} else {
    let runStart = monotonicSeconds()
    do {
        let result = try await ProcessRunner.runShell(
            command: "printf ordinary-ok", timeoutSeconds: timeoutSeconds
        )
        record["outcome"] = "returned"
        record["exit_code"] = Int(result.exitCode)
        record["stdout"] = result.stdout
        record["stderr"] = result.stderr
        record["timed_out"] = result.timedOut
    } catch {
        record["outcome"] = "threw"
        record["error"] = "\(error)"
    }
    record["run_seconds"] = monotonicSeconds() - runStart
}

let remaining = (saturatedAt ?? 0) - min(freed, saturatedAt ?? 0)
for _ in 0..<remaining { gate.signal() }
record["released"] = remaining
emit(record)
