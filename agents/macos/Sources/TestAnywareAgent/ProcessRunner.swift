import CryptoKit
import Darwin
import Foundation

// The lifecycle owner for one guest command (docs/specs/guest-command-lifecycle.md).
//
// This file compiles alone, with Foundation, Darwin and CryptoKit only. The
// k3 and k12 old-path detectors build it with `swiftc` next to a probe, so it
// must not use a type from another file. The wire types live in
// TestAnywareAgentProtocol, and ExecRecordMapping.swift copies an
// `OwnedExecResult` into them.

public struct ProcessRunResult: Sendable {
    public let exitCode: Int32
    public let stdout: String
    public let stderr: String
    public let timedOut: Bool

    public init(exitCode: Int32, stdout: String, stderr: String, timedOut: Bool) {
        self.exitCode = exitCode
        self.stdout = stdout
        self.stderr = stderr
        self.timedOut = timedOut
    }
}

// MARK: - Owner result

/// The first and final outcome of one command.
public enum OwnedOutcome: Sendable, Equatable {
    case exited(code: Int32)
    case signalled(signal: Int32)
    case timedOut(seconds: Int)
    case spawnFailed(errno: Int32)
}

/// The finalised evidence of one output stream.
public struct OwnedStream: Sendable {
    public let data: Data
    public let base64: String
    public let fileBytes: Int
    public let sha256: String
    public let complete: Bool
    /// `deadline` or `read_error` for an incomplete stream, else nil.
    public let reason: String?
}

/// The leader status that the termination sequence observed.
public enum OwnedLeaderStatus: Sendable, Equatable {
    case exited(code: Int32)
    case signalled(signal: Int32)
    case unavailable
}

/// The termination sequence. Every time is in milliseconds from T0.
public struct OwnedTermination: Sendable {
    public let termSentAfterMs: Int
    public let killSentAfterMs: Int
    public let leaderStatus: OwnedLeaderStatus
    public let leaderExitAfterMs: Int?
    /// `yes`, `no` or `unknown`.
    public let groupEmpty: String
    public let groupEmptyAfterMs: Int?
}

/// The whole owner result, before the wire mapping.
public struct OwnedExecResult: Sendable {
    public let outcome: OwnedOutcome
    public let stdout: OwnedStream
    public let stderr: OwnedStream
    public let leaderPid: Int32?
    public let leaderStartTime: String?
    public let spawnMs: Int
    public let durationMs: Int?
    public let ownerLatenessMs: Int
    public let finalisedAfterMs: Int
    /// `none`, `present` or `unknown` for an exited or signalled command, else nil.
    public let remainingGroup: String?
    public let termination: OwnedTermination?
    /// The macOS legacy projection of the outcome and the evidence bytes.
    public let legacy: ProcessRunResult
}

// MARK: - Seams

/// Waits for the leader. Only the owner calls it, and only `reap` reaps.
public protocol LeaderWaiter: AnyObject, Sendable {
    /// Waits until the leader exits or the uptime deadline passes. Never reaps.
    /// Returns true only when the exit status is waitable, so `reap` returns at once.
    func waitExit(until deadline: UInt64) -> Bool
    /// Reports whether the leader exited, without blocking or reaping.
    func hasExited() -> Bool
    /// Reaps the leader and returns its wait status. Blocks until the leader exits.
    func reap() -> Int32
}

/// The two narrow test seams: the leader waiter and the capture read.
public struct OwnerSeams: Sendable {
    public var leaderWaiter: @Sendable (pid_t) -> LeaderWaiter
    /// Reads up to `buffer.count` bytes of one stream at `offset`.
    /// Returns the byte count, 0 at end of file, or -1 with `errno` set.
    public var captureRead: @Sendable (
        _ stream: String, _ fd: Int32, _ buffer: UnsafeMutableRawBufferPointer, _ offset: off_t
    ) -> Int

    public init(
        leaderWaiter: @escaping @Sendable (pid_t) -> LeaderWaiter,
        captureRead: @escaping @Sendable (String, Int32, UnsafeMutableRawBufferPointer, off_t) -> Int
    ) {
        self.leaderWaiter = leaderWaiter
        self.captureRead = captureRead
    }

    public static let live = OwnerSeams(
        leaderWaiter: { KqueueLeaderWaiter(pid: $0) },
        captureRead: { _, fd, buffer, offset in pread(fd, buffer.baseAddress, buffer.count, offset) }
    )
}

/// Waits on a kqueue `EVFILT_PROC` `NOTE_EXIT` source. The `kevent` timeout
/// is the timer source. The waiter never blocks a shared dispatch worker.
///
/// Only a `waitid` status check decides an exit. XNU posts `NOTE_EXIT` before
/// the leader becomes a zombie, so the event alone does not make the status
/// waitable. After an unconfirmed event, a failed registration or a queue
/// error, the waiter polls the status until the deadline.
public final class KqueueLeaderWaiter: LeaderWaiter, @unchecked Sendable {
    /// Registers the exit source of `pid` on `queue`. Returns 0 or an errno.
    /// A test can replace it to make the source fail or fire early.
    typealias Registration = @Sendable (_ queue: Int32, _ pid: pid_t) -> Int32

    static let registerExit: Registration = { queue, pid in
        var change = kevent(
            ident: UInt(pid), filter: Int16(EVFILT_PROC), flags: UInt16(EV_ADD | EV_ONESHOT),
            fflags: NOTE_EXIT, data: 0, udata: nil
        )
        return kevent(queue, &change, 1, nil, 0, nil) < 0 ? errno : 0
    }

    /// The status poll interval after an event that the status did not confirm.
    static let confirmPollUs: useconds_t = 1_000
    /// The status poll interval without a working source.
    static let fallbackPollUs: useconds_t = 10_000
    /// The longest single `kevent` wait, so a huge deadline never reaches the kernel.
    static let longestWaitNs: UInt64 = 60_000_000_000

    private let pid: pid_t
    private let queue: Int32
    private var exited = false
    /// True while the queue can still deliver the exit event.
    private var sourceLive: Bool

    public convenience init(pid: pid_t) {
        self.init(pid: pid, register: Self.registerExit)
    }

    init(pid: pid_t, register: Registration) {
        self.pid = pid
        queue = kqueue()
        // A leader that is already exiting refuses the registration with
        // ESRCH. That case and every other failure use the status poll.
        sourceLive = queue >= 0 && register(queue, pid) == 0
    }

    deinit {
        if queue >= 0 { close(queue) }
    }

    public func waitExit(until deadline: UInt64) -> Bool {
        while true {
            if exited || hasExited() {
                exited = true
                return true
            }
            let now = ProcessRunner.uptime()
            if now >= deadline { return false }
            let left = deadline - now
            guard sourceLive else {
                usleep(useconds_t(min(left / 1_000, UInt64(Self.fallbackPollUs))))
                continue
            }
            let wait = min(left, Self.longestWaitNs)
            var timeout = timespec(tv_sec: Int(wait / 1_000_000_000), tv_nsec: Int(wait % 1_000_000_000))
            var event = kevent()
            let count = kevent(queue, nil, 0, &event, 1, &timeout)
            if count > 0 {
                // The one-shot source is spent. The status check at the top
                // decides, and the poll covers the gap before the zombie state.
                sourceLive = false
                if !hasExited() { usleep(useconds_t(min(left / 1_000, UInt64(Self.confirmPollUs)))) }
            } else if count < 0, errno != EINTR {
                sourceLive = false
            }
        }
    }

    public func hasExited() -> Bool {
        var info = siginfo_t()
        let result = waitid(P_PID, id_t(pid), &info, WEXITED | WNOHANG | WNOWAIT)
        return result == 0 && info.si_pid == pid
    }

    public func reap() -> Int32 {
        var status: Int32 = 0
        while waitpid(pid, &status, 0) < 0, errno == EINTR {}
        return status
    }
}

// MARK: - Runner

public enum ProcessRunner {

    /// The termination grace: SIGTERM at T0, SIGKILL by T0 + 1.9 s.
    static let graceNs: UInt64 = 1_900_000_000
    /// The leader-status wait after SIGKILL, and the end of the group probe.
    static let statusWaitNs: UInt64 = 1_400_000_000
    /// The capture finalisation deadline after SIGKILL, or after the outcome.
    static let finaliseNs: UInt64 = 1_900_000_000
    /// The interval between two signal-0 group probes.
    static let probeIntervalUs: useconds_t = 10_000

    static func uptime() -> UInt64 { DispatchTime.now().uptimeNanoseconds }

    static func ms(_ start: UInt64, _ end: UInt64) -> Int {
        end > start ? Int((end - start) / 1_000_000) : 0
    }

    /// Runs one command under its lifecycle owner and returns the owner result.
    ///
    /// The owner runs on its own thread, so no shared dispatch worker waits
    /// for the command.
    public static func runOwned(
        command: String, timeoutSeconds: Int, seams: OwnerSeams = .live
    ) async -> OwnedExecResult {
        let owner = ExecOwner(
            command: command, timeoutSeconds: timeoutSeconds, seams: seams, requestedAt: uptime()
        )
        return await withCheckedContinuation { continuation in
            let thread = Thread {
                continuation.resume(returning: owner.run())
            }
            thread.name = "testanyware-exec-owner"
            thread.start()
        }
    }

    /// Runs one command and returns its macOS legacy projection.
    public static func runShell(command: String, timeoutSeconds: Int) async throws -> ProcessRunResult {
        await runOwned(command: command, timeoutSeconds: timeoutSeconds).legacy
    }

    /// Run a shell command without waiting for it to exit. stdout/stderr
    /// are sent to /dev/null. Returns immediately after spawn.
    public static func runShellDetached(command: String) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = ["-c", command]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
    }

    // MARK: - Legacy projection

    /// The macOS legacy projection. The decode and the trim are the code of
    /// the old Foundation runner, unchanged.
    static func project(_ outcome: OwnedOutcome, stdout: Data, stderr: Data) -> ProcessRunResult {
        if case .spawnFailed(let code) = outcome {
            return ProcessRunResult(exitCode: -1, stdout: "", stderr: message(code), timedOut: false)
        }
        let out = decode(stdout)
        var err = decode(stderr)
        switch outcome {
        case .exited(let code):
            return ProcessRunResult(exitCode: code, stdout: out, stderr: err, timedOut: false)
        case .signalled(let signal):
            return ProcessRunResult(exitCode: signal, stdout: out, stderr: err, timedOut: false)
        case .timedOut(let seconds):
            let note = "Process timed out after \(seconds)s"
            err = err.isEmpty ? note : err + "\n[\(note)]"
            return ProcessRunResult(exitCode: -1, stdout: out, stderr: err, timedOut: true)
        case .spawnFailed:
            preconditionFailure("handled above")
        }
    }

    static func decode(_ data: Data) -> String {
        let text = String(data: data, encoding: .utf8) ?? ""
        return text.replacingOccurrences(of: "\\s+$", with: "", options: .regularExpression)
    }

    // MARK: - Names

    static func message(_ code: Int32) -> String { String(cString: strerror(code)) }

    // Interpolating a POSIXErrorCode gives "POSIXErrorCode(rawValue: 35)",
    // not the case name, so the names come from this table. Its order is
    // errno 1 to 107 in the macOS 27 SDK header usr/include/sys/errno.h.
    private static let errorNames = [
        "EPERM", "ENOENT", "ESRCH", "EINTR", "EIO", "ENXIO", "E2BIG", "ENOEXEC", "EBADF", "ECHILD",
        "EDEADLK", "ENOMEM", "EACCES", "EFAULT", "ENOTBLK", "EBUSY", "EEXIST", "EXDEV", "ENODEV", "ENOTDIR",
        "EISDIR", "EINVAL", "ENFILE", "EMFILE", "ENOTTY", "ETXTBSY", "EFBIG", "ENOSPC", "ESPIPE", "EROFS",
        "EMLINK", "EPIPE", "EDOM", "ERANGE", "EAGAIN", "EINPROGRESS", "EALREADY", "ENOTSOCK", "EDESTADDRREQ",
        "EMSGSIZE", "EPROTOTYPE", "ENOPROTOOPT", "EPROTONOSUPPORT", "ESOCKTNOSUPPORT", "ENOTSUP",
        "EPFNOSUPPORT", "EAFNOSUPPORT", "EADDRINUSE", "EADDRNOTAVAIL", "ENETDOWN", "ENETUNREACH", "ENETRESET",
        "ECONNABORTED", "ECONNRESET", "ENOBUFS", "EISCONN", "ENOTCONN", "ESHUTDOWN", "ETOOMANYREFS",
        "ETIMEDOUT", "ECONNREFUSED", "ELOOP", "ENAMETOOLONG", "EHOSTDOWN", "EHOSTUNREACH", "ENOTEMPTY",
        "EPROCLIM", "EUSERS", "EDQUOT", "ESTALE", "EREMOTE", "EBADRPC", "ERPCMISMATCH", "EPROGUNAVAIL",
        "EPROGMISMATCH", "EPROCUNAVAIL", "ENOLCK", "ENOSYS", "EFTYPE", "EAUTH", "ENEEDAUTH", "EPWROFF",
        "EDEVERR", "EOVERFLOW", "EBADEXEC", "EBADARCH", "ESHLIBVERS", "EBADMACHO", "ECANCELED", "EIDRM",
        "ENOMSG", "EILSEQ", "ENOATTR", "EBADMSG", "EMULTIHOP", "ENODATA", "ENOLINK", "ENOSR", "ENOSTR",
        "EPROTO", "ETIME", "EOPNOTSUPP", "ENOPOLICY", "ENOTRECOVERABLE", "EOWNERDEAD", "EQFULL", "ENOTCAPABLE",
    ]

    /// The Darwin name of an errno, such as `EAGAIN`.
    static func errorName(_ code: Int32) -> String {
        (1...errorNames.count).contains(Int(code)) ? errorNames[Int(code) - 1] : "E\(code)"
    }

    private static let signalNames = [
        "HUP", "INT", "QUIT", "ILL", "TRAP", "ABRT", "EMT", "FPE", "KILL", "BUS", "SEGV",
        "SYS", "PIPE", "ALRM", "TERM", "URG", "STOP", "TSTP", "CONT", "CHLD", "TTIN",
        "TTOU", "IO", "XCPU", "XFSZ", "VTALRM", "PROF", "WINCH", "INFO", "USR1", "USR2",
    ]

    /// The Darwin name of a signal number, such as `SIGKILL`.
    public static func signalName(_ signal: Int32) -> String {
        (1...signalNames.count).contains(Int(signal)) ? "SIG" + signalNames[Int(signal) - 1] : "SIG\(signal)"
    }

    /// Decodes a `waitpid` status into an exit code or a signal number.
    static func decodeStatus(_ status: Int32) -> OwnedLeaderStatus {
        let low = status & 0x7f
        return low == 0 ? .exited(code: (status >> 8) & 0xff) : .signalled(signal: low)
    }
}

// MARK: - Capture files

/// The agent-owned capture directory of one exec, with its four descriptors.
private struct CaptureFiles {
    let directory: String
    let outWrite: Int32
    let errWrite: Int32
    let outRead: Int32
    let errRead: Int32

    /// Creates the directory and opens each file for the command and for the owner.
    /// Throws the errno of the first failed step.
    static func make() throws -> CaptureFiles {
        let base = FileManager.default.temporaryDirectory.path
        let directory = (base as NSString).appendingPathComponent("testanyware-exec-\(UUID().uuidString)")
        guard mkdir(directory, 0o700) == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        var opened: [Int32] = []
        func open(_ name: String, _ flags: Int32) throws -> Int32 {
            let fd = Darwin.open((directory as NSString).appendingPathComponent(name), flags | O_CLOEXEC, 0o600)
            guard fd >= 0 else {
                let code = errno
                opened.forEach { close($0) }
                remove(directory)
                throw POSIXError(POSIXErrorCode(rawValue: code) ?? .EIO)
            }
            opened.append(fd)
            return fd
        }
        let outWrite = try open("stdout", O_WRONLY | O_CREAT | O_EXCL)
        let errWrite = try open("stderr", O_WRONLY | O_CREAT | O_EXCL)
        // The owner opens its read descriptors before the spawn, so the
        // command cannot hide its output by removing the files.
        let outRead = try open("stdout", O_RDONLY)
        let errRead = try open("stderr", O_RDONLY)
        return CaptureFiles(
            directory: directory, outWrite: outWrite, errWrite: errWrite, outRead: outRead, errRead: errRead
        )
    }

    func closeWriters() {
        close(outWrite)
        close(errWrite)
    }

    func closeReaders() {
        close(outRead)
        close(errRead)
    }

    /// Removes the files and the directory. Open descriptors stay valid.
    func remove() { Self.remove(directory) }

    static func remove(_ directory: String) {
        unlink((directory as NSString).appendingPathComponent("stdout"))
        unlink((directory as NSString).appendingPathComponent("stderr"))
        rmdir(directory)
    }
}

/// The shared state of one capture worker. The lock guards every field.
private final class CaptureProgress: @unchecked Sendable {
    let lock = NSLock()
    let done = DispatchSemaphore(value: 0)
    var abandoned = false
    var data = [Data(), Data()]
    var hashers = [SHA256(), SHA256()]
    var failed = [false, false]
}

// MARK: - Owner

private final class ExecOwner: @unchecked Sendable {
    let command: String
    let timeoutSeconds: Int
    let seams: OwnerSeams
    let requestedAt: UInt64
#if EXIT_TRACE
    let trace = ExitTrace.shared
    var call: UInt64 = 0
#endif

    init(command: String, timeoutSeconds: Int, seams: OwnerSeams, requestedAt: UInt64) {
        self.command = command
        self.timeoutSeconds = timeoutSeconds
        self.seams = seams
        self.requestedAt = requestedAt
    }

    func run() -> OwnedExecResult {
#if EXIT_TRACE
        call = trace.begin(command: command, timeout: timeoutSeconds)
#endif
        let files: CaptureFiles
        do {
            files = try CaptureFiles.make()
        } catch {
            return spawnFailure((error as? POSIXError)?.code.rawValue ?? EIO, spawnedAt: ProcessRunner.uptime())
        }
        var pid: pid_t = 0
        let spawnError = spawn(&pid, files)
        let spawnedAt = ProcessRunner.uptime()
        files.closeWriters()
        guard spawnError == 0 else {
            files.closeReaders()
            files.remove()
            return spawnFailure(spawnError, spawnedAt: spawnedAt)
        }
        let startTime = Self.startTime(of: pid)
#if EXIT_TRACE
        trace.mark(call, "spawned", ["pid": "\(pid)"])
        trace.watch(call, pid: pid)
#endif
        let waiter = seams.leaderWaiter(pid)
        let deadline = Self.deadline(after: spawnedAt, seconds: timeoutSeconds)

        let outcome: OwnedOutcome
        let outcomeAt: UInt64
        var lateness = 0
        var remaining: String?
        var termination: OwnedTermination?
        let finaliseBy: UInt64
        if waiter.waitExit(until: deadline) {
            outcomeAt = ProcessRunner.uptime()
            if outcomeAt >= deadline { lateness = ProcessRunner.ms(deadline, outcomeAt) }
            (outcome, remaining) = settleExit(waiter, pid)
            finaliseBy = outcomeAt + ProcessRunner.finaliseNs
        } else {
            // The deadline passed. One non-blocking check chooses between a
            // late exit and a timeout.
            outcomeAt = ProcessRunner.uptime()
            lateness = ProcessRunner.ms(deadline, outcomeAt)
            if waiter.hasExited() {
                (outcome, remaining) = settleExit(waiter, pid)
                finaliseBy = outcomeAt + ProcessRunner.finaliseNs
            } else {
                outcome = .timedOut(seconds: timeoutSeconds)
                let (record, killAt) = terminate(waiter, pid, t0: outcomeAt)
                termination = record
                finaliseBy = killAt + ProcessRunner.finaliseNs
            }
        }

        let (out, err) = finalise(files, by: finaliseBy)
        let legacy = ProcessRunner.project(outcome, stdout: out.data, stderr: err.data)
        let result = OwnedExecResult(
            outcome: outcome, stdout: out, stderr: err,
            leaderPid: pid, leaderStartTime: startTime,
            spawnMs: ProcessRunner.ms(requestedAt, spawnedAt),
            durationMs: ProcessRunner.ms(spawnedAt, outcomeAt),
            ownerLatenessMs: lateness,
            finalisedAfterMs: ProcessRunner.ms(outcomeAt, ProcessRunner.uptime()),
            remainingGroup: remaining, termination: termination, legacy: legacy
        )
        files.remove()
#if EXIT_TRACE
        trace.mark(call, "return", [
            "exit": "\(legacy.exitCode)",
            "timed_out": legacy.timedOut ? "true" : "false",
            "stdout_bytes": "\(out.data.count)",
            "stderr_bytes": "\(err.data.count)",
        ])
#endif
        return result
    }

    /// The uptime deadline of a timeout. The request accepts any Int, so the
    /// arithmetic saturates at `UInt64.max` instead of trapping after the spawn.
    static func deadline(after start: UInt64, seconds: Int) -> UInt64 {
        let (budget, overflow) = UInt64(max(seconds, 0)).multipliedReportingOverflow(by: 1_000_000_000)
        if overflow { return .max }
        let (sum, late) = start.addingReportingOverflow(budget)
        return late ? .max : sum
    }

    // MARK: Spawn

    /// Spawns `/bin/bash -c COMMAND` as the leader of a new session and group.
    /// Returns 0 or the errno of the spawn.
    private func spawn(_ pid: inout pid_t, _ files: CaptureFiles) -> Int32 {
        var actions: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_adddup2(&actions, files.outWrite, 1)
        posix_spawn_file_actions_adddup2(&actions, files.errWrite, 2)

        var attributes: posix_spawnattr_t?
        posix_spawnattr_init(&attributes)
        defer { posix_spawnattr_destroy(&attributes) }
        // POSIX_SPAWN_SETSID (0x0400) and POSIX_SPAWN_CLOEXEC_DEFAULT (0x4000)
        // are Apple extensions. Verified in the macOS 27 SDK header
        // usr/include/sys/spawn.h, lines 61-62, and in the xnu source:
        // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/spawn.h
        // CLOEXEC_DEFAULT passes only descriptors 0-2, so the agent sockets
        // stay out of the command. SETSIGDEF and SETSIGMASK reset every
        // signal, as the old Foundation runner did.
        let flags = POSIX_SPAWN_SETSID | POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_SETSIGMASK
        posix_spawnattr_setflags(&attributes, Int16(flags))
        var everySignal: sigset_t = ~0
        var noSignal: sigset_t = 0
        posix_spawnattr_setsigdefault(&attributes, &everySignal)
        posix_spawnattr_setsigmask(&attributes, &noSignal)

        // The command inherits the agent environment, as with Foundation Process.
        let arguments = ["/bin/bash", "-c", command].map { strdup($0) }
        let environment = ProcessInfo.processInfo.environment.map { strdup("\($0.key)=\($0.value)") }
        defer { (arguments + environment).forEach { free($0) } }
        var argv = arguments + [nil]
        var envp = environment + [nil]
        return posix_spawn(&pid, "/bin/bash", &actions, &attributes, &argv, &envp)
    }

    /// The leader start time from the kernel, as `seconds.microseconds`.
    /// The sysctl also answers for a zombie leader.
    static func startTime(of pid: pid_t) -> String? {
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, pid]
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0, size > 0 else { return nil }
        let start = info.kp_proc.p_un.__p_starttime
        return String(format: "%ld.%06d", start.tv_sec, start.tv_usec)
    }

    // MARK: Arbitration

    /// Reaps an exited leader and probes the group once with signal 0.
    private func settleExit(_ waiter: LeaderWaiter, _ pid: pid_t) -> (OwnedOutcome, String) {
        let outcome: OwnedOutcome
        switch ProcessRunner.decodeStatus(waiter.reap()) {
        case .exited(let code): outcome = .exited(code: code)
        case .signalled(let signal): outcome = .signalled(signal: signal)
        case .unavailable: preconditionFailure("a reaped status is never unavailable")
        }
        if killpg(pid, 0) == 0 { return (outcome, "present") }
        return (outcome, errno == ESRCH ? "none" : "unknown")
    }

    /// Runs the termination sequence from T0 and returns its record and the
    /// SIGKILL time.
    private func terminate(_ waiter: LeaderWaiter, _ pid: pid_t, t0: UInt64) -> (OwnedTermination, UInt64) {
        // The leader is unreaped here, so the group id is still this group.
        killpg(pid, SIGTERM)
        let termAt = ProcessRunner.uptime()
#if EXIT_TRACE
        trace.mark(call, "terminate-sent", ["pid": "\(pid)"])
#endif
        var exitAt: UInt64?
        if waiter.waitExit(until: t0 + ProcessRunner.graceNs) { exitAt = ProcessRunner.uptime() }
#if EXIT_TRACE
        trace.mark(call, "wait-result", ["phase": "terminate", "result": exitAt == nil ? "timed-out" : "success"])
#endif
        killpg(pid, SIGKILL)
        let killAt = ProcessRunner.uptime()
#if EXIT_TRACE
        trace.mark(call, "kill-sent", ["pid": "\(pid)"])
#endif
        let statusBy = killAt + ProcessRunner.statusWaitNs
        if exitAt == nil, waiter.waitExit(until: statusBy) { exitAt = ProcessRunner.uptime() }

        guard let exitAt else {
            hold(waiter, pid)
            let record = OwnedTermination(
                termSentAfterMs: ProcessRunner.ms(t0, termAt), killSentAfterMs: ProcessRunner.ms(t0, killAt),
                leaderStatus: .unavailable, leaderExitAfterMs: nil, groupEmpty: "no", groupEmptyAfterMs: nil
            )
            return (record, killAt)
        }
        let status = ProcessRunner.decodeStatus(waiter.reap())
#if EXIT_TRACE
        trace.mark(call, "reap-return", ["pid": "\(pid)"])
#endif
        // Only ESRCH ends the probe early. Right after SIGKILL, a member that
        // is still exiting can make the probe fail with EPERM for about 1 ms
        // (11 and 25 of 60 host trials in two runs of leaf k219), so EPERM
        // gives `unknown` only when it is still the result at K + 1.4 s.
        var groupEmpty = "no"
        var emptyAt: UInt64?
        while true {
            if killpg(pid, 0) == 0 {
                groupEmpty = "no"
            } else if errno == ESRCH {
                groupEmpty = "yes"
                emptyAt = ProcessRunner.uptime()
                break
            } else {
                groupEmpty = "unknown"
            }
            if ProcessRunner.uptime() >= statusBy { break }
            usleep(ProcessRunner.probeIntervalUs)
        }
        let record = OwnedTermination(
            termSentAfterMs: ProcessRunner.ms(t0, termAt), killSentAfterMs: ProcessRunner.ms(t0, killAt),
            leaderStatus: status, leaderExitAfterMs: ProcessRunner.ms(t0, exitAt),
            groupEmpty: groupEmpty, groupEmptyAfterMs: emptyAt.map { ProcessRunner.ms(t0, $0) }
        )
        return (record, killAt)
    }

    /// Keeps holding a leader whose status did not arrive. A thread reaps it
    /// later and logs the late reap. The owner sends no more signals.
    private func hold(_ waiter: LeaderWaiter, _ pid: pid_t) {
        let thread = Thread {
            let status = waiter.reap()
            let line = "testanyware-agent: late reap of leader \(pid), wait status \(status)\n"
            FileHandle.standardError.write(Data(line.utf8))
        }
        thread.name = "testanyware-exec-late-reap"
        thread.start()
    }

    // MARK: Capture

    /// Notes each file size, then reads both streams on one worker until the
    /// deadline. An unfinished stream is incomplete.
    private func finalise(_ files: CaptureFiles, by deadline: UInt64) -> (OwnedStream, OwnedStream) {
        let fds = [files.outRead, files.errRead]
        let sizes = fds.map { fd -> Int in
            var info = stat()
            return fstat(fd, &info) == 0 ? Int(info.st_size) : 0
        }
        let progress = CaptureProgress()
        let read = seams.captureRead
        let thread = Thread {
            // The worker owns the read descriptors, so an abandoned worker
            // never reads a reused descriptor.
            defer {
                fds.forEach { close($0) }
                progress.done.signal()
            }
            let chunk = 1 << 20
            let buffer = UnsafeMutableRawBufferPointer.allocate(byteCount: chunk, alignment: 1)
            defer { buffer.deallocate() }
            for (index, name) in ["stdout", "stderr"].enumerated() {
                var offset = 0
                while offset < sizes[index] {
                    let want = min(chunk, sizes[index] - offset)
                    let slice = UnsafeMutableRawBufferPointer(rebasing: buffer[0..<want])
                    let count = read(name, fds[index], slice, off_t(offset))
                    if count < 0, errno == EINTR { continue }
                    progress.lock.lock()
                    if progress.abandoned {
                        progress.lock.unlock()
                        return
                    }
                    if count <= 0 {
                        progress.failed[index] = true
                        progress.lock.unlock()
                        break
                    }
                    let bytes = UnsafeRawBufferPointer(rebasing: buffer[0..<count])
                    progress.data[index].append(contentsOf: bytes)
                    progress.hashers[index].update(bufferPointer: bytes)
                    progress.lock.unlock()
                    offset += count
                }
            }
        }
        thread.name = "testanyware-exec-capture"
        thread.start()
        _ = progress.done.wait(timeout: DispatchTime(uptimeNanoseconds: deadline))

        progress.lock.lock()
        progress.abandoned = true
        let data = progress.data
        let hashers = progress.hashers
        let failed = progress.failed
        progress.lock.unlock()

        let streams = (0..<2).map { index -> OwnedStream in
            let complete = data[index].count == sizes[index]
            return OwnedStream(
                data: data[index],
                base64: data[index].base64EncodedString(),
                fileBytes: sizes[index],
                sha256: hashers[index].finalize().map { String(format: "%02x", $0) }.joined(),
                complete: complete,
                reason: complete ? nil : (failed[index] ? "read_error" : "deadline")
            )
        }
        return (streams[0], streams[1])
    }

    // MARK: Spawn failure

    private func spawnFailure(_ code: Int32, spawnedAt: UInt64) -> OwnedExecResult {
        let empty = OwnedStream(
            data: Data(), base64: "", fileBytes: 0,
            sha256: SHA256.hash(data: Data()).map { String(format: "%02x", $0) }.joined(),
            complete: true, reason: nil
        )
        let outcome = OwnedOutcome.spawnFailed(errno: code)
        let legacy = ProcessRunner.project(outcome, stdout: Data(), stderr: Data())
#if EXIT_TRACE
        trace.mark(call, "spawn-error", ["errno": "\(code)"])
#endif
        return OwnedExecResult(
            outcome: outcome, stdout: empty, stderr: empty,
            leaderPid: nil, leaderStartTime: nil,
            spawnMs: ProcessRunner.ms(requestedAt, spawnedAt), durationMs: nil,
            ownerLatenessMs: 0, finalisedAfterMs: ProcessRunner.ms(spawnedAt, ProcessRunner.uptime()),
            remainingGroup: nil, termination: nil, legacy: legacy
        )
    }
}
