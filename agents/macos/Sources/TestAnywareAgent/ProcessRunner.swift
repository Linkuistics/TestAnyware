import Foundation

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

public enum ProcessRunner {

    /// Run a shell command via /bin/bash -c, capturing stdout/stderr to
    /// temp files (not pipes), with a wall-clock timeout.
    ///
    /// Pipes deadlock when the spawned shell forks long-lived descendants
    /// that inherit the pipe write FDs (e.g. `brew install` → curl/ruby/
    /// gcc): EOF never arrives on the read end, so a `readDataToEnd…`
    /// thread blocks indefinitely even after the immediate child has been
    /// terminated. Files have no EOF semantics — we read whatever has
    /// been written, whenever we want.
    ///
    /// On timeout: the descendant tree is snapshotted **before** killing
    /// bash (descendants get reparented to PID 1 once their parent exits,
    /// at which point `pgrep -P` can no longer recover them), then bash
    /// is sent SIGTERM, and any survivors are SIGKILL'd. The call
    /// returns whatever the children wrote to the temp files before
    /// being killed, with `timedOut == true`.
    public static func runShell(
        command: String,
        timeoutSeconds: Int
    ) async throws -> ProcessRunResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = ["-c", command]

        let tempDir = FileManager.default.temporaryDirectory
        let outURL = tempDir.appendingPathComponent("testanyware-exec-\(UUID().uuidString).out")
        let errURL = tempDir.appendingPathComponent("testanyware-exec-\(UUID().uuidString).err")
        FileManager.default.createFile(atPath: outURL.path, contents: nil)
        FileManager.default.createFile(atPath: errURL.path, contents: nil)
        defer {
            try? FileManager.default.removeItem(at: outURL)
            try? FileManager.default.removeItem(at: errURL)
        }
        let outHandle = try FileHandle(forWritingTo: outURL)
        let errHandle = try FileHandle(forWritingTo: errURL)
        defer {
            try? outHandle.close()
            try? errHandle.close()
        }
        process.standardOutput = outHandle
        process.standardError = errHandle

#if EXIT_TRACE
        let trace = ExitTrace.shared
        let call = trace.begin(command: command, timeout: timeoutSeconds)
        process.terminationHandler = { finished in
            trace.mark(call, "foundation-termination", [
                "status": "\(finished.terminationStatus)",
                "reason": "\(finished.terminationReason.rawValue)",
            ])
        }
#endif
        try process.run()
        let bashPid = process.processIdentifier
#if EXIT_TRACE
        trace.mark(call, "spawned", ["pid": "\(bashPid)"])
        trace.watch(call, pid: bashPid)
        let exited = await tracedWaitForExit(
            process: process, seconds: timeoutSeconds, call: call, phase: "deadline")
#else
        let exited = await waitForExit(process: process, seconds: timeoutSeconds)
#endif

        if !exited {
#if EXIT_TRACE
            trace.mark(call, "descendants-start")
#endif
            let descendants = collectDescendants(of: bashPid)
#if EXIT_TRACE
            trace.mark(call, "descendants-end", [
                "pids": descendants.map(String.init).joined(separator: " "),
            ])
            trace.mark(call, "terminate-sent", ["pid": "\(bashPid)"])
#endif
            process.terminate()
#if EXIT_TRACE
            let terminated = await tracedWaitForExit(
                process: process, seconds: 3, call: call, phase: "terminate")
#else
            let terminated = await waitForExit(process: process, seconds: 3)
#endif
            if !terminated {
#if EXIT_TRACE
                let killed = kill(bashPid, SIGKILL)
                trace.mark(call, "kill-sent", [
                    "pid": "\(bashPid)", "result": "\(killed)", "errno": "\(killed < 0 ? errno : 0)",
                ])
                process.waitUntilExit()
                trace.mark(call, "reap-return", ["pid": "\(bashPid)"])
#else
                kill(bashPid, SIGKILL)
                process.waitUntilExit()
#endif
            }
            for pid in descendants where pid > 1 {
#if EXIT_TRACE
                let killed = kill(pid, SIGKILL)
                trace.mark(call, "descendant-kill", [
                    "pid": "\(pid)", "result": "\(killed)", "errno": "\(killed < 0 ? errno : 0)",
                ])
#else
                kill(pid, SIGKILL)
#endif
            }
        }

        let outData = (try? Data(contentsOf: outURL)) ?? Data()
        let errData = (try? Data(contentsOf: errURL)) ?? Data()
        var stdout = String(data: outData, encoding: .utf8) ?? ""
        var stderr = String(data: errData, encoding: .utf8) ?? ""
        stdout = stdout.replacingOccurrences(of: "\\s+$", with: "", options: .regularExpression)
        stderr = stderr.replacingOccurrences(of: "\\s+$", with: "", options: .regularExpression)

        let exitCode: Int32 = exited ? process.terminationStatus : -1
#if EXIT_TRACE
        trace.mark(call, "return", [
            "exit": "\(exitCode)",
            "timed_out": exited ? "false" : "true",
            "stdout_bytes": "\(outData.count)",
            "stderr_bytes": "\(errData.count)",
        ])
#endif

        if !exited {
            let note = "Process timed out after \(timeoutSeconds)s"
            stderr = stderr.isEmpty ? note : stderr + "\n[\(note)]"
        }

        return ProcessRunResult(
            exitCode: exitCode,
            stdout: stdout,
            stderr: stderr,
            timedOut: !exited
        )
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

    // MARK: - Helpers

#if EXIT_TRACE
    /// Mirrors `waitForExit` step for step and marks each step.
    private static func tracedWaitForExit(
        process: Process, seconds: Int, call: UInt64, phase: String
    ) async -> Bool {
        let trace = ExitTrace.shared
        trace.mark(call, "wait-enter", ["phase": phase])
        return await withCheckedContinuation { continuation in
            DispatchQueue.global().async {
                trace.mark(call, "outer-start", ["phase": phase])
                let semaphore = DispatchSemaphore(value: 0)
                DispatchQueue.global().async {
                    trace.mark(call, "waiter-start", ["phase": phase])
                    process.waitUntilExit()
                    trace.mark(call, "waiter-return", ["phase": phase])
                    semaphore.signal()
                }
                let result = semaphore.wait(timeout: .now() + .seconds(seconds))
                trace.mark(call, "wait-result", [
                    "phase": phase,
                    "result": result == .success ? "success" : "timed-out",
                ])
                continuation.resume(returning: result == .success)
            }
        }
    }
#endif

    private static func waitForExit(process: Process, seconds: Int) async -> Bool {
        await withCheckedContinuation { continuation in
            DispatchQueue.global().async {
                let semaphore = DispatchSemaphore(value: 0)
                DispatchQueue.global().async {
                    process.waitUntilExit()
                    semaphore.signal()
                }
                let result = semaphore.wait(timeout: .now() + .seconds(seconds))
                continuation.resume(returning: result == .success)
            }
        }
    }

    /// BFS-collect all descendants of `root` via `pgrep -P`. Snapshot must
    /// happen before the root is killed — once it dies, descendants get
    /// reparented to launchd (PID 1) and the tree can no longer be
    /// recovered through parent-PID lookups.
    private static func collectDescendants(of root: pid_t) -> [pid_t] {
        var result: [pid_t] = []
        var stack: [pid_t] = [root]
        while let pid = stack.popLast() {
            for child in directChildren(of: pid) where !result.contains(child) {
                result.append(child)
                stack.append(child)
            }
        }
        return result
    }

    private static func directChildren(of pid: pid_t) -> [pid_t] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        proc.arguments = ["-P", "\(pid)"]
        let outPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
        } catch {
            return []
        }
        proc.waitUntilExit()
        // pgrep output is bounded (one short line per child); a blocking
        // read here cannot deadlock the way the bash pipes do.
        let data = outPipe.fileHandleForReading.readDataToEndOfFile()
        let str = String(data: data, encoding: .utf8) ?? ""
        return str.split(separator: "\n").compactMap {
            pid_t($0.trimmingCharacters(in: .whitespaces))
        }
    }
}
