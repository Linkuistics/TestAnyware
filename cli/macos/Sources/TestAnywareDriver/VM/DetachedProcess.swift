import Darwin
import Foundation

/// Spawn a long-running process in its own session.
///
/// The child is immune to SIGHUP on the caller's terminal (via
/// `POSIX_SPAWN_SETSID`) and outlives the Swift parent. Stdout and stderr
/// are redirected to `logPath` (append mode, created if absent); stdin is
/// connected to `/dev/null`.
///
/// Foundation `Process` cannot call `setsid`/`setpgrp` — see memory note
/// "Foundation Process does not call setsid or setpgrp". This enum is the
/// replacement for `Process` in VM-lifecycle contexts where the child must
/// survive a parent exit without being reparented under launchd in an
/// un-killable way.
public enum DetachedProcess {

    public static func spawn(
        executable: String,
        arguments: [String],
        logPath: String
    ) throws -> Int32 {
        var fileActions: posix_spawn_file_actions_t? = nil
        posix_spawn_file_actions_init(&fileActions)
        defer { posix_spawn_file_actions_destroy(&fileActions) }

        posix_spawn_file_actions_addopen(&fileActions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_addopen(
            &fileActions, 1, logPath, O_WRONLY | O_CREAT | O_APPEND, 0o644
        )
        posix_spawn_file_actions_adddup2(&fileActions, 1, 2)

        var attrs: posix_spawnattr_t? = nil
        posix_spawnattr_init(&attrs)
        defer { posix_spawnattr_destroy(&attrs) }
        posix_spawnattr_setflags(&attrs, Int16(POSIX_SPAWN_SETSID))

        let argv: [UnsafeMutablePointer<CChar>?] =
            ([executable] + arguments).map { strdup($0) } + [nil]
        defer { argv.forEach { free($0) } }

        var pid: pid_t = 0
        let status = posix_spawn(&pid, executable, &fileActions, &attrs, argv, environ)
        if status != 0 {
            throw DetachedProcessError.spawnFailed(errno: status, executable: executable)
        }
        return Int32(pid)
    }
}

public enum DetachedProcessError: Error, CustomStringConvertible, Equatable {
    case spawnFailed(errno: Int32, executable: String)

    public var description: String {
        switch self {
        case .spawnFailed(let err, let exe):
            let message = String(cString: strerror(err))
            return "posix_spawn failed for \(exe): errno=\(err) (\(message))"
        }
    }
}
