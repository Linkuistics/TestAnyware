#if EXIT_TRACE
// Diagnostic seam for the k195 exit-observation inquiry. It exists only when
// the agent is built with `-Xswiftc -DEXIT_TRACE`. The default build compiles
// none of this file and none of the marks in ProcessRunner.
//
// The trace records two sources of one exec request on one clock,
// CLOCK_UPTIME_RAW. Kernel events come from a kqueue on a dedicated thread
// outside Grand Central Dispatch. Agent marks come from each step of
// `ProcessRunner.runShell`. A starved dispatch pool therefore delays only the
// agent marks.
//
// A kernel record carries the time when the trace thread received its event
// batch. That time is an upper bound on the kernel event, not its time. An
// agent mark takes its time under the record lock, so the sequence order and
// the time order of agent marks agree.
//
// kqueue process notes, NOTE_EXITSTATUS and the knote removal at exit:
// https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kqueue.2.html
// (checked against the MacOSX SDK header sys/event.h of Xcode Swift 6.4.2).

import Darwin
import Foundation

public final class ExitTrace: @unchecked Sendable {
    public static let shared = ExitTrace()

    private let lock = NSLock()
    private let fd: Int32
    private let kq: Int32
    private var sequence: UInt64 = 0
    private var calls: UInt64 = 0
    // One registration of one process incarnation. The kqueue carries the
    // token in udata, so each event resolves to the registration that made
    // it, also after the PID is reused.
    private struct Watch {
        let call: UInt64
        let pid: pid_t
        let start: UInt64
    }

    private var tokens: UInt = 0
    private var watches: [UInt: Watch] = [:]
    private var live: [pid_t: UInt] = [:]

    private init() {
        let env = ProcessInfo.processInfo.environment["TESTANYWARE_EXIT_TRACE"]
        let path = env ?? "/tmp/testanyware-exit-trace.jsonl"
        fd = open(path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0o644)
        kq = kqueue()
        let thread = Thread { [self] in self.loop() }
        thread.name = "testanyware-exit-trace"
        thread.start()
        emit(call: 0, event: "trace-open", fields: ["kq": "\(kq)", "fd": "\(fd)"])
    }

    /// Returns a new call number for one runShell invocation.
    public func begin(command: String, timeout: Int) -> UInt64 {
        lock.lock()
        calls += 1
        let call = calls
        lock.unlock()
        emit(call: call, event: "begin", fields: [
            "timeout": "\(timeout)",
            "command": Self.quote(String(command.prefix(160))),
        ])
        return call
    }

    /// Records one agent step.
    public func mark(_ call: UInt64, _ event: String, _ fields: [String: String] = [:]) {
        emit(call: call, event: event, fields: fields)
    }

    /// Starts kernel observation of one spawned shell.
    public func watch(_ call: UInt64, pid: pid_t) {
        register(call: call, pid: pid, parent: 0)
    }

    // MARK: - Kernel observation

    /// Returns true when this call made a new registration.
    @discardableResult
    private func register(call: UInt64, pid: pid_t, parent: pid_t) -> Bool {
        let start = Self.start(of: pid)
        lock.lock()
        if let old = live[pid], let watch = watches[old], start != 0, watch.start == start {
            // This incarnation is already registered.
            lock.unlock()
            return false
        }
        tokens += 1
        let token = tokens
        live[pid] = token
        watches[token] = Watch(call: call, pid: pid, start: start)
        lock.unlock()
        var change = kevent(
            ident: UInt(pid),
            filter: Int16(EVFILT_PROC),
            flags: UInt16(EV_ADD | EV_CLEAR),
            fflags: UInt32(NOTE_EXIT) | UInt32(NOTE_EXITSTATUS)
                | UInt32(NOTE_FORK) | UInt32(NOTE_EXEC),
            data: 0,
            udata: UnsafeMutableRawPointer(bitPattern: token)
        )
        let result = kevent(kq, &change, 1, nil, 0, nil)
        let error = result < 0 ? errno : 0
        if result < 0 {
            // Undo the failed registration, so a later process with this PID
            // can register.
            lock.lock()
            if live[pid] == token { live.removeValue(forKey: pid) }
            watches.removeValue(forKey: token)
            lock.unlock()
        }
        emit(call: call, event: result < 0 ? "watch-missed" : "watch", fields: [
            "pid": "\(pid)",
            "parent": "\(parent)",
            "token": "\(token)",
            "start": "\(start)",
            "errno": "\(error)",
            "path": Self.quote(Self.path(of: pid)),
        ])
        if result == 0 {
            // A child forked before this registration has no fork note.
            adopt(call: call, parent: pid)
        }
        return result == 0
    }

    /// Registers the current children of one process. Returns the number of
    /// new registrations.
    @discardableResult
    private func adopt(call: UInt64, parent: pid_t) -> Int {
        var buffer = [pid_t](repeating: 0, count: 256)
        let count = buffer.withUnsafeMutableBytes {
            proc_listchildpids(parent, $0.baseAddress, Int32($0.count))
        }
        guard count > 0 else { return 0 }
        if Int(count) >= buffer.count {
            // A full snapshot can omit children.
            emit(call: call, event: "adopt-truncated", fields: [
                "parent": "\(parent)",
                "count": "\(count)",
            ])
        }
        var added = 0
        for child in buffer.prefix(min(Int(count), buffer.count)) where child > 0 {
            if register(call: call, pid: child, parent: parent) { added += 1 }
        }
        return added
    }

    private func loop() {
        let empty = kevent(ident: 0, filter: 0, flags: 0, fflags: 0, data: 0, udata: nil)
        var events = Array(repeating: empty, count: 16)
        while true {
            let count = kevent(kq, nil, 0, &events, Int32(events.count), nil)
            let now = Self.now()
            if count < 0 {
                if errno == EINTR { continue }
                emit(call: 0, event: "trace-loop-error", fields: ["errno": "\(errno)"], at: now)
                return
            }
            for event in events.prefix(Int(count)) {
                handle(event, at: now)
            }
        }
    }

    private func handle(_ event: kevent, at now: UInt64) {
        let pid = pid_t(event.ident)
        let token = UInt(bitPattern: event.udata)
        lock.lock()
        let call = watches[token]?.call ?? 0
        lock.unlock()
        let flags = event.fflags
        if flags & UInt32(NOTE_FORK) != 0 {
            emit(call: call, event: "kernel-fork", fields: [
                "pid": "\(pid)",
                "token": "\(token)",
            ], at: now)
            if adopt(call: call, parent: pid) == 0 {
                // The child exited before the snapshot, or an earlier snapshot
                // took it. The descendant set is therefore a lower bound.
                emit(call: call, event: "fork-unadopted", fields: ["pid": "\(pid)"], at: now)
            }
        }
        if flags & UInt32(NOTE_EXEC) != 0 {
            emit(call: call, event: "kernel-exec", fields: [
                "pid": "\(pid)",
                "token": "\(token)",
                "path": Self.quote(Self.path(of: pid)),
            ], at: now)
        }
        if flags & UInt32(NOTE_EXIT) != 0 {
            lock.lock()
            watches.removeValue(forKey: token)
            if live[pid] == token { live.removeValue(forKey: pid) }
            lock.unlock()
            emit(call: call, event: "kernel-exit", fields: [
                "pid": "\(pid)",
                "token": "\(token)",
                "status": "\(event.data)",
            ], at: now)
        }
    }

    // MARK: - Records

    private func emit(
        call: UInt64, event: String, fields: [String: String], at time: UInt64? = nil
    ) {
        lock.lock()
        // An agent mark takes its time under the lock, so its sequence order
        // and its time order agree. A kernel record keeps its batch time.
        let stamp = time ?? Self.now()
        sequence += 1
        let seq = sequence
        lock.unlock()
        var line = "{\"seq\":\(seq),\"t_ns\":\(stamp),\"agent\":\(getpid())"
        line += ",\"threads\":\(Self.threads()),\"call\":\(call)"
        line += ",\"event\":\"\(event)\""
        for key in fields.keys.sorted() {
            let value = fields[key] ?? ""
            let quoted = value.hasPrefix("\"") || Int64(value) != nil
            line += ",\"\(key)\":" + (quoted ? value : Self.quote(value))
        }
        line += "}\n"
        guard fd >= 0 else { return }
        line.utf8CString.withUnsafeBufferPointer { buffer in
            // One write per record; O_APPEND keeps records whole.
            _ = write(fd, buffer.baseAddress, buffer.count - 1)
        }
    }

    public static func now() -> UInt64 {
        clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
    }

    private static func threads() -> Int32 {
        var info = proc_taskinfo()
        let size = Int32(MemoryLayout<proc_taskinfo>.size)
        let result = proc_pidinfo(getpid(), PROC_PIDTASKINFO, 0, &info, size)
        return result == size ? info.pti_threadnum : -1
    }

    /// Returns the start time of one process in microseconds, or 0 when the
    /// process is gone. The PID and this time name one incarnation.
    private static func start(of pid: pid_t) -> UInt64 {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        let result = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size)
        guard result == size else { return 0 }
        return info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
    }

    private static func path(of pid: pid_t) -> String {
        var buffer = [CChar](repeating: 0, count: 4096)
        let length = proc_pidpath(pid, &buffer, UInt32(buffer.count))
        guard length > 0 else { return "" }
        let bytes = buffer.prefix(Int(length)).map { UInt8(bitPattern: $0) }
        return String(decoding: bytes, as: UTF8.self)
    }

    public static func quote(_ text: String) -> String {
        var out = "\""
        for scalar in text.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 {
                    out += String(format: "\\u%04x", scalar.value)
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        return out + "\""
    }
}
#endif
