#if EXIT_TRACE
// Diagnostic seam for the k195 exit-observation inquiry. It exists only when
// the agent is built with `-Xswiftc -DEXIT_TRACE`. The default build compiles
// none of this file and none of the marks in ProcessRunner.
//
// The trace records two clocks of one exec request. The kernel clock comes
// from a kqueue on a dedicated thread outside Grand Central Dispatch. The
// agent clock comes from marks at each step of `ProcessRunner.runShell`.
// A starved dispatch pool therefore delays only the agent marks.
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
    private var owners: [pid_t: UInt64] = [:]
    private var known: Set<pid_t> = []

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

    private func register(call: UInt64, pid: pid_t, parent: pid_t) {
        lock.lock()
        let fresh = known.insert(pid).inserted
        if fresh { owners[pid] = call }
        lock.unlock()
        guard fresh else { return }
        var change = kevent(
            ident: UInt(pid),
            filter: Int16(EVFILT_PROC),
            flags: UInt16(EV_ADD | EV_CLEAR),
            fflags: UInt32(NOTE_EXIT) | UInt32(NOTE_EXITSTATUS)
                | UInt32(NOTE_FORK) | UInt32(NOTE_EXEC),
            data: 0,
            udata: nil
        )
        let result = kevent(kq, &change, 1, nil, 0, nil)
        let error = result < 0 ? errno : 0
        emit(call: call, event: result < 0 ? "watch-missed" : "watch", fields: [
            "pid": "\(pid)",
            "parent": "\(parent)",
            "errno": "\(error)",
            "path": Self.quote(Self.path(of: pid)),
        ])
        if result == 0 {
            // A child forked before this registration has no fork note.
            adopt(call: call, parent: pid)
        }
    }

    private func adopt(call: UInt64, parent: pid_t) {
        var buffer = [pid_t](repeating: 0, count: 256)
        let count = buffer.withUnsafeMutableBytes {
            proc_listchildpids(parent, $0.baseAddress, Int32($0.count))
        }
        guard count > 0 else { return }
        for child in buffer.prefix(min(Int(count), buffer.count)) where child > 0 {
            register(call: call, pid: child, parent: parent)
        }
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
        lock.lock()
        let call = owners[pid] ?? 0
        lock.unlock()
        let flags = event.fflags
        if flags & UInt32(NOTE_FORK) != 0 {
            emit(call: call, event: "kernel-fork", fields: ["pid": "\(pid)"], at: now)
            adopt(call: call, parent: pid)
        }
        if flags & UInt32(NOTE_EXEC) != 0 {
            emit(call: call, event: "kernel-exec", fields: [
                "pid": "\(pid)",
                "path": Self.quote(Self.path(of: pid)),
            ], at: now)
        }
        if flags & UInt32(NOTE_EXIT) != 0 {
            lock.lock()
            known.remove(pid)
            lock.unlock()
            emit(call: call, event: "kernel-exit", fields: [
                "pid": "\(pid)",
                "status": "\(event.data)",
            ], at: now)
        }
    }

    // MARK: - Records

    private func emit(
        call: UInt64, event: String, fields: [String: String], at time: UInt64? = nil
    ) {
        let stamp = time ?? Self.now()
        lock.lock()
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
