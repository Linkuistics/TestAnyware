import Foundation
import TestAnywareAgentProtocol

// The exec registry: running commands, retained records and tombstones.
//
// The source of truth is docs/specs/guest-command-lifecycle.md, sections
// "Cancellation and CLI interruption", "Retention" and "Agent shutdown", and
// ADR-0025. The registry knows exec ids and retention. The owner in
// ProcessRunner.swift knows the processes. A cancellation passes from the
// registry to the owner only through the command's `CancelSignal`. The Linux
// agent has the same registry in `exec_registry.py`.
//
// Each wait in this type blocks its thread. The HTTP handlers call the
// waiting methods on a dedicated thread, so no wait holds a worker of the
// shared Swift concurrency pool.

public final class ExecRegistry: @unchecked Sendable {

    /// Each terminal record and each tombstone stays this long.
    public static let retentionNs: UInt64 = 300_000_000_000
    /// Terminal records and tombstones together, at most.
    public static let retentionLimit = 64
    /// The longest `waitMs` of a cancellation.
    public static let cancelWaitMaxMs = 10_000
    /// From SIGTERM to the last published record: the root grace bound of 2 s
    /// plus the root kill, drain and reap bound of 2 s. The owner schedule
    /// needs 3.8 s after its outcome.
    public static let shutdownWaitNs: UInt64 = 4_000_000_000

    /// The answer to an `/exec` id (spec, "Start", steps 2 and 3).
    public enum Admission: Sendable {
        /// Run the command with this signal.
        case start(CancelSignal)
        /// Answer a cancelled record and spawn nothing.
        case tombstoned
        /// The id names a running or retained command.
        case inUse
        /// The agent is shutting down.
        case closed
    }

    /// The state of an id, as the cancel endpoint reports it.
    public enum CancelState: String, Sendable {
        case terminating, terminal
        case unknownID = "unknown_id"
    }

    /// One admitted command. A waiting cancellation holds this object, so it
    /// reads the record of its own command even after eviction and reuse of
    /// the id (review k237 R3). The Linux registry waits on its `_Entry` in
    /// the same way. Read and written under the lock.
    private final class Run {
        let signal: CancelSignal
        var reply: ExecReply?
        init(_ signal: CancelSignal) { self.signal = signal }
    }

    private enum State {
        case running(Run)
        case terminal(ExecReply)
        case tombstone
    }

    private struct Entry {
        var state: State
        /// The retention clock: the admission, publication or tombstone time.
        var since: UInt64
        /// The order of that event. It breaks a tie between equal times.
        var sequence: UInt64
        var retained: Bool {
            if case .running = state { return false }
            return true
        }
    }

    /// The registry of this agent process.
    public static let shared = ExecRegistry()

    private let clock: @Sendable () -> UInt64
    private let retentionNs: UInt64
    private let limit: Int
    private let condition = NSCondition()
    private var entries: [String: Entry] = [:]
    private var closed = false
    private var sequence: UInt64 = 0

    /// `clock` measures retention ages in nanoseconds. A wait always uses the
    /// owner clock, `ProcessRunner.uptime()`.
    public init(
        clock: @escaping @Sendable () -> UInt64 = { ProcessRunner.uptime() },
        retentionNs: UInt64 = ExecRegistry.retentionNs,
        limit: Int = ExecRegistry.retentionLimit
    ) {
        self.clock = clock
        self.retentionNs = retentionNs
        self.limit = limit
    }

    // MARK: Retention, under the lock

    private func evict() {
        let now = clock()
        for (id, entry) in entries where entry.retained && now >= entry.since && now - entry.since >= retentionNs {
            entries[id] = nil
        }
        // A running command is never evicted. The oldest retained entry goes first.
        let retained = entries.filter { $0.value.retained }
            .sorted { ($0.value.since, $0.value.sequence) < ($1.value.since, $1.value.sequence) }
        for (id, _) in retained.prefix(max(0, retained.count - limit)) {
            entries[id] = nil
        }
    }

    /// A new entry, stamped now. Called under the lock.
    private func stamped(_ state: State) -> Entry {
        sequence += 1
        return Entry(state: state, since: clock(), sequence: sequence)
    }

    // MARK: The owner side

    /// Registers an `/exec` id.
    public func admit(_ id: String) -> Admission {
        condition.lock()
        defer { condition.unlock() }
        if closed { return .closed }
        evict()
        if let entry = entries[id] {
            guard case .tombstone = entry.state else { return .inUse }
            // The tombstone becomes the running entry of a command that never
            // starts. `publish` then retains its cancelled record.
            entries[id] = stamped(.running(Run(CancelSignal())))
            return .tombstoned
        }
        let signal = CancelSignal()
        entries[id] = stamped(.running(Run(signal)))
        return .start(signal)
    }

    /// Makes the record of a running command final and retains it.
    public func publish(_ id: String, reply: ExecReply) {
        condition.lock()
        defer { condition.unlock() }
        guard let entry = entries[id], case .running(let run) = entry.state else {
            preconditionFailure("publish of an exec id that is not running: \(id)")
        }
        run.signal.close()
        run.reply = reply
        entries[id] = stamped(.terminal(reply))
        condition.broadcast()
        evict()
    }

    /// `running`, `terminal`, `tombstone`, or nil for an id that it does not hold.
    public func state(_ id: String) -> String? {
        condition.lock()
        defer { condition.unlock() }
        evict()
        switch entries[id]?.state {
        case .running: return "running"
        case .terminal: return "terminal"
        case .tombstone: return "tombstone"
        case nil: return nil
        }
    }

    // MARK: The client side

    /// Cancels by id and waits up to `waitNs` for the record.
    ///
    /// Returns `terminal` with the reply, `terminating` without it, or
    /// `unknown_id` without it. A repeat never changes a terminal outcome. An
    /// unknown id becomes a tombstone. A repeat on a tombstone keeps its
    /// first time, because no command ran under that id.
    public func cancel(_ id: String, waitNs: UInt64) -> (CancelState, ExecReply?) {
        condition.lock()
        defer { condition.unlock() }
        evict()
        guard let entry = entries[id] else {
            entries[id] = stamped(.tombstone)
            evict()
            return (.unknownID, nil)
        }
        switch entry.state {
        case .tombstone:
            return (.unknownID, nil)
        case .terminal(let reply):
            return (.terminal, reply)
        case .running(let run):
            run.signal.request()
            let end = ProcessRunner.uptime() &+ waitNs
            while true {
                // Only this command's own record. The id can hold another
                // command by now.
                if let reply = run.reply { return (.terminal, reply) }
                let now = ProcessRunner.uptime()
                if now >= end { return (.terminating, nil) }
                _ = condition.wait(until: Date(timeIntervalSinceNow: Double(end - now) / 1e9))
            }
        }
    }

    // MARK: Agent shutdown

    /// Stops admission and cancels every running command. Returns their ids.
    public func close() -> [String] {
        condition.lock()
        defer { condition.unlock() }
        closed = true
        let now = ProcessRunner.uptime()
        var running: [String] = []
        for (id, entry) in entries {
            if case .running(let run) = entry.state {
                run.signal.request(at: now)
                running.append(id)
            }
        }
        return running.sorted()
    }

    /// Waits until each command is published or the uptime `until` passes.
    /// Returns the ids that are still running.
    public func waitPublished(_ ids: [String], until: UInt64) -> [String] {
        condition.lock()
        defer { condition.unlock() }
        func pending() -> [String] {
            ids.filter { id in
                if case .running? = entries[id]?.state { return true }
                return false
            }
        }
        while true {
            let left = pending()
            let now = ProcessRunner.uptime()
            if left.isEmpty || now >= until { return left }
            _ = condition.wait(until: Date(timeIntervalSinceNow: Double(until - now) / 1e9))
        }
    }

    /// The agent shutdown of the spec: stop admission, cancel every running
    /// command, run `afterClose` and wait for publication until `until`. A
    /// leader whose status stays unavailable does not delay this, because its
    /// owner publishes on schedule. Returns the ids still unpublished.
    public func shutDown(until: UInt64, afterClose: () -> Void = {}) -> [String] {
        let running = close()
        afterClose()
        return waitPublished(running, until: until)
    }
}
