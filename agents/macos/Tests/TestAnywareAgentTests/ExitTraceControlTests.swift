#if EXIT_TRACE
// Deterministic controls for the k195 exit-observation inquiry. They exist
// only in a build with `-Xswiftc -DEXIT_TRACE`. Each control forces one
// suspected ordering and then reads the exit trace to confirm the ordering.
//
// - Normal exit: the kernel exit comes before the agent sees it.
// - Pool starvation (candidate 1): the shell exits, but the inner wait block
//   cannot get a dispatch worker, so the agent reports a timeout.
// - Late exit with TERM (candidate 3, 30 s shape): the shell outlives the
//   deadline and exits on TERM inside the 3 s wait.
// - TERM ignored (candidate 3, 33 s shape): the 3 s TERM wait times out and
//   the agent sends SIGKILL.
//
// The runner sets TESTANYWARE_EXIT_TRACE to a fresh file before the run.

import Darwin
import Foundation
import XCTest
@testable import TestAnywareAgent

final class ExitTraceControlTests: XCTestCase {

    struct Record {
        let seq: Int
        let time: UInt64
        let call: Int
        let event: String
        let fields: [String: Any]

        func string(_ key: String) -> String? { fields[key] as? String }
        func int(_ key: String) -> Int? { (fields[key] as? NSNumber)?.intValue }
    }

    enum ControlError: Error {
        case noTracePath
        case noBegin(String)
    }

    // MARK: - Controls

    func testNormalExitIsObservedAfterKernelExit() async throws {
        let marker = Self.marker()
        let result = try await ProcessRunner.runShell(
            command: ": \(marker); true", timeoutSeconds: 5)
        XCTAssertFalse(result.timedOut)
        XCTAssertEqual(result.exitCode, 0)

        let records = try await trace(marker: marker)
        let shell = try XCTUnwrap(first(records, "spawned")?.int("pid"))
        let exit = try XCTUnwrap(kernelExit(records, pid: shell), "no kernel exit")
        let waiter = try XCTUnwrap(first(records, "waiter-return", phase: "deadline"))
        let outcome = try XCTUnwrap(first(records, "wait-result", phase: "deadline"))
        XCTAssertEqual(outcome.string("result"), "success")
        XCTAssertLessThan(exit.time, waiter.time)
        XCTAssertLessThan(waiter.time, outcome.time)
        report("normal", records)
    }

    func testStarvedPoolMissesAnExitedShell() async throws {
        let marker = Self.marker()
        let pool = Pool()
        defer { pool.releaseAll() }
        let running = try XCTUnwrap(
            pool.saturate(limit: 512, settle: .seconds(1)),
            "the global pool did not saturate within 512 blockers")
        // One free worker: the outer wait block takes it, the inner cannot.
        pool.releaseOne()
        // A blocking pause keeps this thread off the free worker.
        usleep(300_000)
        let watchdog = Thread { [pool] in
            Thread.sleep(forTimeInterval: 5)
            pool.releaseAll()
        }
        watchdog.start()

        let result = try await ProcessRunner.runShell(
            command: ": \(marker); true", timeoutSeconds: 2)
        pool.releaseAll()
        XCTAssertTrue(result.timedOut, "the starved pool did not hide the exit")

        let records = try await trace(marker: marker)
        let shell = try XCTUnwrap(first(records, "spawned")?.int("pid"))
        let exit = try XCTUnwrap(kernelExit(records, pid: shell), "no kernel exit")
        let outcome = try XCTUnwrap(first(records, "wait-result", phase: "deadline"))
        XCTAssertEqual(outcome.string("result"), "timed-out")
        XCTAssertLessThan(exit.time + 1_500_000_000, outcome.time,
                          "the shell did not exit well before the deadline")
        if let waiter = first(records, "waiter-start", phase: "deadline") {
            XCTAssertGreaterThan(waiter.seq, outcome.seq,
                                 "the inner wait block started before the deadline")
        }
        report("starved running=\(running)", records)
    }

    func testLateExitEndsOnTerm() async throws {
        let marker = Self.marker()
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("sleeper")
        let result = try await ProcessRunner.runShell(
            command: ": \(marker); sleep 6 & echo $! > '\(pidFile)'; wait",
            timeoutSeconds: 2)
        XCTAssertTrue(result.timedOut)

        let records = try await trace(marker: marker)
        let shell = try XCTUnwrap(first(records, "spawned")?.int("pid"))
        let deadline = try XCTUnwrap(first(records, "wait-result", phase: "deadline"))
        let term = try XCTUnwrap(first(records, "terminate-sent"))
        let exit = try XCTUnwrap(kernelExit(records, pid: shell), "no kernel exit")
        let waited = try XCTUnwrap(first(records, "wait-result", phase: "terminate"))
        XCTAssertEqual(deadline.string("result"), "timed-out")
        XCTAssertLessThan(deadline.time, exit.time)
        XCTAssertLessThan(term.time, exit.time)
        XCTAssertEqual(waited.string("result"), "success")
        XCTAssertNil(first(records, "kill-sent"))
        report("late-term", records)
    }

    func testIgnoredTermWaitsThreeSecondsThenKills() async throws {
        let marker = Self.marker()
        let fixture = try OwnedProcessFixture.make(for: self)
        let pidFile = fixture.pidFile("sleeper")
        let result = try await ProcessRunner.runShell(
            command: ": \(marker); trap '' TERM; sleep 6 & echo $! > '\(pidFile)'; wait",
            timeoutSeconds: 2)
        XCTAssertTrue(result.timedOut)

        let records = try await trace(marker: marker)
        let shell = try XCTUnwrap(first(records, "spawned")?.int("pid"))
        let waited = try XCTUnwrap(first(records, "wait-result", phase: "terminate"))
        let killed = try XCTUnwrap(first(records, "kill-sent"))
        let exit = try XCTUnwrap(kernelExit(records, pid: shell), "no kernel exit")
        let term = try XCTUnwrap(first(records, "terminate-sent"))
        XCTAssertEqual(waited.string("result"), "timed-out")
        XCTAssertLessThan(killed.time, exit.time)
        XCTAssertGreaterThanOrEqual(killed.time - term.time, 2_900_000_000)
        report("ignored-term", records)
    }

    // MARK: - Trace reading

    private static func marker() -> String {
        "k195-\(UUID().uuidString.lowercased())"
    }

    private func tracePath() throws -> String {
        guard let path = ProcessInfo.processInfo.environment["TESTANYWARE_EXIT_TRACE"],
              !path.isEmpty else {
            XCTFail("TESTANYWARE_EXIT_TRACE is not set")
            throw ControlError.noTracePath
        }
        return path
    }

    private func load() throws -> [Record] {
        let text = (try? String(contentsOfFile: try tracePath(), encoding: .utf8)) ?? ""
        return text.split(separator: "\n").compactMap { line in
            guard let data = line.data(using: .utf8),
                  let object = try? JSONSerialization.jsonObject(with: data),
                  let fields = object as? [String: Any],
                  let seq = (fields["seq"] as? NSNumber)?.intValue,
                  let time = (fields["t_ns"] as? NSNumber)?.uint64Value,
                  let call = (fields["call"] as? NSNumber)?.intValue,
                  let event = fields["event"] as? String else { return nil }
            return Record(seq: seq, time: time, call: call, event: event, fields: fields)
        }
    }

    /// Returns the records of the call whose command holds `marker`. Waits up
    /// to 3 s for the kernel exit of the shell, which the trace thread writes.
    private func trace(marker: String) async throws -> [Record] {
        let deadline = Date().addingTimeInterval(3)
        while true {
            let all = try load()
            guard let begin = all.first(where: {
                $0.event == "begin" && ($0.string("command") ?? "").contains(marker)
            }) else { throw ControlError.noBegin(marker) }
            let records = all.filter { $0.call == begin.call }.sorted { $0.seq < $1.seq }
            let shell = first(records, "spawned")?.int("pid")
            if let shell, kernelExit(records, pid: shell) != nil { return records }
            if Date() >= deadline { return records }
            try await Task.sleep(nanoseconds: 50_000_000)
        }
    }

    private func first(_ records: [Record], _ event: String, phase: String? = nil) -> Record? {
        records.first { $0.event == event && (phase == nil || $0.string("phase") == phase) }
    }

    private func kernelExit(_ records: [Record], pid: Int) -> Record? {
        records.first { $0.event == "kernel-exit" && $0.int("pid") == pid }
    }

    /// Writes the ordering to stderr, so the run log keeps it beside the trace.
    private func report(_ name: String, _ records: [Record]) {
        guard let start = records.first?.time else { return }
        let steps = records.map { record -> String in
            let ms = Double(record.time - start) / 1_000_000
            let phase = record.string("phase").map { "/\($0)" } ?? ""
            return String(format: "%@%@@%.1f", record.event, phase, ms)
        }
        FileHandle.standardError.write(Data(
            "k195-control \(name): \(steps.joined(separator: " "))\n".utf8))
    }
}

/// Blocks global-queue workers until the pool has no free worker.
final class Pool: @unchecked Sendable {
    private let lock = NSLock()
    private var gates: [DispatchSemaphore] = []
    private var skipFrom = Int.max

    /// Adds blockers one at a time. When a blocker does not start within
    /// `settle`, the pool is full. That blocker then returns at once when it
    /// runs. Returns the number of running blockers, or nil at `limit`.
    func saturate(limit: Int, settle: DispatchTimeInterval) -> Int? {
        for index in 0..<limit {
            let gate = DispatchSemaphore(value: 0)
            let started = DispatchSemaphore(value: 0)
            lock.withLock { gates.append(gate) }
            DispatchQueue.global().async { [self] in
                let skip = lock.withLock { index >= skipFrom }
                if skip { return }
                started.signal()
                gate.wait()
            }
            if started.wait(timeout: .now() + settle) == .success { continue }
            lock.withLock { skipFrom = index }
            // The blocker can start between the timeout and the flag.
            if started.wait(timeout: .now()) == .success {
                lock.withLock { skipFrom = Int.max }
                continue
            }
            return index
        }
        return nil
    }

    func releaseOne() {
        lock.withLock { gates.first }?.signal()
    }

    func releaseAll() {
        let all = lock.withLock { gates }
        for gate in all { gate.signal() }
    }
}
#endif
