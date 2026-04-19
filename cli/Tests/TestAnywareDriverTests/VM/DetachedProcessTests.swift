import Testing
import Foundation
import Darwin
@testable import TestAnywareDriver

@Suite("DetachedProcess")
struct DetachedProcessTests {

    private func tempLogURL() -> URL {
        URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("detached-\(UUID().uuidString).log")
    }

    @Test func spawnsAProcessInItsOwnSession() throws {
        let pid = try DetachedProcess.spawn(
            executable: "/bin/sh",
            arguments: ["-c", "sleep 0.3"],
            logPath: "/dev/null"
        )
        #expect(pid > 0)

        let sid = getsid(pid_t(pid))
        #expect(
            sid == pid_t(pid),
            "SID (\(sid)) should equal PID (\(pid)) when POSIX_SPAWN_SETSID has made the child a session leader"
        )
        let pgid = getpgid(pid_t(pid))
        #expect(
            pgid == pid_t(pid),
            "PGID (\(pgid)) should equal PID (\(pid)) — setsid also makes the child a process-group leader"
        )

        var status: Int32 = 0
        _ = waitpid(pid_t(pid), &status, 0)
    }

    @Test func redirectsStdoutStderrToLogFile() throws {
        let log = tempLogURL()
        defer { try? FileManager.default.removeItem(at: log) }
        let pid = try DetachedProcess.spawn(
            executable: "/bin/sh",
            arguments: ["-c", "echo out; echo err >&2"],
            logPath: log.path
        )
        var status: Int32 = 0
        _ = waitpid(pid_t(pid), &status, 0)
        let content = try String(contentsOf: log, encoding: .utf8)
        #expect(content.contains("out"))
        #expect(content.contains("err"))
    }

    @Test func appendModePreservesExistingContent() throws {
        let log = tempLogURL()
        defer { try? FileManager.default.removeItem(at: log) }
        try "preamble\n".write(to: log, atomically: true, encoding: .utf8)

        let pid = try DetachedProcess.spawn(
            executable: "/bin/sh",
            arguments: ["-c", "echo appended"],
            logPath: log.path
        )
        var status: Int32 = 0
        _ = waitpid(pid_t(pid), &status, 0)

        let content = try String(contentsOf: log, encoding: .utf8)
        #expect(content.contains("preamble"))
        #expect(content.contains("appended"))
    }

    @Test func throwsForMissingExecutable() {
        #expect(throws: DetachedProcessError.self) {
            _ = try DetachedProcess.spawn(
                executable: "/does/not/exist",
                arguments: [],
                logPath: "/dev/null"
            )
        }
    }

}
