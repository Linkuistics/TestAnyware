import Foundation
import TestAnywareAgentProtocol
import XCTest
@testable import TestAnywareAgent

/// Retention, admission and shutdown of the exec registry (spec, sections
/// "Retention", "Cancel endpoint" and "Agent shutdown"). A fake clock moves
/// the retention ages. Leaf k221.
final class ExecRegistryTests: XCTestCase {

    func testTheSpecValuesAre300SecondsAnd64() {
        XCTAssertEqual(ExecRegistry.retentionNs, 300_000_000_000)
        XCTAssertEqual(ExecRegistry.retentionLimit, 64)
        XCTAssertEqual(ExecRegistry.cancelWaitMaxMs, 10_000)
        XCTAssertEqual(ExecRegistry.shutdownWaitNs, 4_000_000_000)
    }

    func testATerminalRecordIsRetainedFor300Seconds() {
        let clock = FakeClock()
        let registry = ExecRegistry(clock: clock.read)
        publishCancelled(registry, "kept")
        clock.advance(seconds: 299)
        XCTAssertEqual(registry.state("kept"), "terminal")
        clock.advance(seconds: 1)
        XCTAssertNil(registry.state("kept"))
    }

    func testATombstoneIsRetainedFor300SecondsFromItsFirstCancellation() {
        let clock = FakeClock()
        let registry = ExecRegistry(clock: clock.read)
        XCTAssertEqual(registry.cancel("early", waitNs: 0).0, .unknownID)
        clock.advance(seconds: 200)
        // A repeat keeps the first time, because no command ran under the id.
        XCTAssertEqual(registry.cancel("early", waitNs: 0).0, .unknownID)
        clock.advance(seconds: 99)
        XCTAssertEqual(registry.state("early"), "tombstone")
        clock.advance(seconds: 1)
        XCTAssertNil(registry.state("early"))
    }

    func testTheOldestRetainedEntryGoesFirstBeyond64() {
        let clock = FakeClock()
        let registry = ExecRegistry(clock: clock.read)
        for index in 0..<64 {
            publishCancelled(registry, "r\(index)")
            clock.advance(seconds: 1)
        }
        XCTAssertEqual(registry.cancel("t64", waitNs: 0).0, .unknownID)
        XCTAssertNil(registry.state("r0"))
        XCTAssertEqual(registry.state("r1"), "terminal")
        XCTAssertEqual(registry.state("t64"), "tombstone")
    }

    func testARunningCommandIsNeverEvicted() {
        let clock = FakeClock()
        let registry = ExecRegistry(clock: clock.read, retentionNs: 1_000_000_000, limit: 1)
        guard case .start = registry.admit("running") else { return XCTFail("not admitted") }
        clock.advance(seconds: 10)
        publishCancelled(registry, "a")
        publishCancelled(registry, "b")
        XCTAssertEqual(registry.state("running"), "running")
        XCTAssertNil(registry.state("a"))
        XCTAssertEqual(registry.state("b"), "terminal")
    }

    func testADuplicateIDOfARunningOrRetainedCommandIsRefused() {
        let registry = ExecRegistry()
        guard case .start = registry.admit("dup") else { return XCTFail("not admitted") }
        guard case .inUse = registry.admit("dup") else { return XCTFail("a running id was admitted again") }
        registry.publish("dup", reply: Self.cancelledReply("dup"))
        guard case .inUse = registry.admit("dup") else { return XCTFail("a retained id was admitted again") }
    }

    /// The tombstone order of the spec: a cancellation of an unknown id, then
    /// an `/exec` with that id, gives a cancelled record without a spawn.
    func testATombstonedIDIsAdmittedAsCancelledAndThenRetained() {
        let registry = ExecRegistry()
        XCTAssertEqual(registry.cancel("early", waitNs: 0).0, .unknownID)
        guard case .tombstoned = registry.admit("early") else { return XCTFail("the tombstone was ignored") }
        let reply = ProcessRunner.cancelledBeforeStart(requestedAt: ProcessRunner.uptime()).reply(id: "early")
        registry.publish("early", reply: reply)
        let (state, retained) = registry.cancel("early", waitNs: 0)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(retained, reply)
        guard case .inUse = registry.admit("early") else { return XCTFail("a retained id was admitted again") }
    }

    func testTheCancelSignalClosesAtPublication() {
        let registry = ExecRegistry()
        guard case .start(let signal) = registry.admit("closing") else { return XCTFail("not admitted") }
        registry.publish("closing", reply: Self.cancelledReply("closing"))
        signal.request()
        XCTAssertNil(signal.requestedAt, "a request after publication changed the signal")
        XCTAssertEqual(registry.cancel("closing", waitNs: 0).0, .terminal)
    }

    func testARepeatedCancellationKeepsTheFirstRequestTime() {
        let signal = CancelSignal()
        signal.request(at: 100)
        signal.request(at: 200)
        XCTAssertEqual(signal.requestedAt, 100)
    }

    func testCloseStopsAdmissionAndCancelsEveryRunningCommand() {
        let registry = ExecRegistry()
        guard case .start(let one) = registry.admit("one"),
              case .start(let two) = registry.admit("two") else { return XCTFail("not admitted") }
        XCTAssertEqual(registry.close(), ["one", "two"])
        XCTAssertNotNil(one.requestedAt)
        XCTAssertNotNil(two.requestedAt)
        guard case .closed = registry.admit("three") else { return XCTFail("admission stayed open") }
    }

    func testWaitPublishedReturnsAtItsBoundWithThePendingIDs() {
        let registry = ExecRegistry()
        guard case .start = registry.admit("slow"), case .start = registry.admit("quick") else {
            return XCTFail("not admitted")
        }
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.1) {
            registry.publish("quick", reply: Self.cancelledReply("quick"))
        }
        let start = ProcessRunner.uptime()
        let pending = registry.waitPublished(["slow", "quick"], until: start + 500_000_000)
        let elapsed = Double(ProcessRunner.uptime() - start) / 1e9
        XCTAssertEqual(pending, ["slow"])
        XCTAssertGreaterThanOrEqual(elapsed, 0.49)
        XCTAssertLessThan(elapsed, 1.0)
    }

    func testACancellationOfARunningCommandWaitsForItsRecord() {
        let registry = ExecRegistry()
        guard case .start(let signal) = registry.admit("waited") else { return XCTFail("not admitted") }
        DispatchQueue.global().async {
            while signal.requestedAt == nil { usleep(1_000) }
            usleep(200_000)
            registry.publish("waited", reply: Self.cancelledReply("waited"))
        }
        let (state, reply) = registry.cancel("waited", waitNs: 5_000_000_000)
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.id, "waited")
    }

    /// Review k237 R3. The waiting thread is suspended. Meanwhile command A
    /// publishes, a tombstone evicts A's record, and command B reuses the id
    /// and publishes `exited(23)`. The waiter then wakes and must return A's
    /// record, not B's.
    func testAWaitingCancellationReturnsItsOwnRecordAfterEvictionAndReuse() async throws {
        let replyB = await ProcessRunner.runOwned(command: "exit 23", timeoutSeconds: 5).reply(id: "reuse")
        XCTAssertEqual(replyB.record.outcome, .exited(exitCode: 23))
        let registry = ExecRegistry(limit: 1)
        guard case .start(let signalA) = registry.admit("reuse") else { return XCTFail("A not admitted") }
        let waiter = SuspendableWaiter { registry.cancel("reuse", waitNs: 10_000_000_000) }
        let deadline = Date().addingTimeInterval(5)
        while signalA.requestedAt == nil, Date() < deadline { usleep(1_000) }
        // The request happens under the registry lock, and the waiter keeps
        // the lock until it waits. This call takes the lock after the request,
        // so the waiter now waits.
        XCTAssertEqual(registry.state("reuse"), "running")
        try waiter.suspend()
        defer { waiter.resume() }

        registry.publish("reuse", reply: Self.cancelledReply("reuse"))
        XCTAssertEqual(registry.cancel("evicts-a", waitNs: 0).0, .unknownID)
        XCTAssertNil(registry.state("reuse"), "A's record was not evicted")
        guard case .start = registry.admit("reuse") else { return XCTFail("B not admitted") }
        registry.publish("reuse", reply: replyB)

        waiter.resume()
        let (state, reply) = try XCTUnwrap(waiter.join(timeout: 5), "the waiting cancellation did not return")
        XCTAssertEqual(state, .terminal)
        XCTAssertEqual(reply?.record.outcome, .cancelled(beforeStart: true))
    }

    // MARK: - Helpers

    static func cancelledReply(_ id: String) -> ExecReply {
        ProcessRunner.cancelledBeforeStart(requestedAt: ProcessRunner.uptime()).reply(id: id)
    }

    private func publishCancelled(_ registry: ExecRegistry, _ id: String) {
        guard case .start = registry.admit(id) else { return XCTFail("\(id) not admitted") }
        registry.publish(id, reply: Self.cancelledReply(id))
    }
}

/// A retention clock that a test moves by hand.
final class FakeClock: @unchecked Sendable {
    private let lock = NSLock()
    private var now: UInt64 = 1_000_000_000_000

    var read: @Sendable () -> UInt64 {
        { [self] in
            lock.lock()
            defer { lock.unlock() }
            return now
        }
    }

    func advance(seconds: UInt64) {
        lock.lock()
        now += seconds * 1_000_000_000
        lock.unlock()
    }
}

/// A cancellation that waits on its own thread, which a test can suspend
/// with `thread_suspend` while it waits.
final class SuspendableWaiter: @unchecked Sendable {
    private let lock = NSCondition()
    private var thread: thread_act_t?
    private var result: (ExecRegistry.CancelState, ExecReply?)?
    private var suspended = false

    init(_ body: @escaping @Sendable () -> (ExecRegistry.CancelState, ExecReply?)) {
        Thread { [self] in
            lock.lock()
            thread = mach_thread_self()
            lock.broadcast()
            lock.unlock()
            let value = body()
            lock.lock()
            result = value
            lock.broadcast()
            lock.unlock()
        }.start()
    }

    deinit {
        if let thread { mach_port_deallocate(mach_task_self_, thread) }
    }

    func suspend() throws {
        lock.lock()
        defer { lock.unlock() }
        while thread == nil { lock.wait() }
        XCTAssertEqual(thread_suspend(thread!), KERN_SUCCESS)
        suspended = true
    }

    /// Resumes a suspended thread. A repeat does nothing.
    func resume() {
        lock.lock()
        defer { lock.unlock() }
        guard suspended, let thread else { return }
        suspended = false
        XCTAssertEqual(thread_resume(thread), KERN_SUCCESS)
    }

    func join(timeout: TimeInterval) -> (ExecRegistry.CancelState, ExecReply?)? {
        lock.lock()
        defer { lock.unlock() }
        let end = Date().addingTimeInterval(timeout)
        while result == nil, lock.wait(until: end) {}
        return result
    }
}
