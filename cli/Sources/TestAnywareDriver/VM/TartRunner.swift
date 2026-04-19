import Foundation

/// Static helpers for driving the `tart` CLI.
///
/// Pure parsers (`parseList`, `parseVNCURL`) are testable without invoking
/// `tart`; side-effectful operations (`runList`, `start`, `stop`, `delete`)
/// invoke the binary and are added by later plan tasks.
public enum TartRunner {

    public struct VNCURL: Equatable, Sendable {
        public var host: String
        public var port: Int
        public var password: String?

        public init(host: String, port: Int, password: String?) {
            self.host = host
            self.port = port
            self.password = password
        }
    }

    private struct TartVM: Decodable {
        var name: String
        var state: String?
        var disk: Int?

        enum CodingKeys: String, CodingKey {
            case name = "Name"
            case state = "State"
            case disk = "Disk"
        }
    }

    /// Parse `tart list --format json` output into list entries.
    ///
    /// Classification:
    /// - names prefixed `testanyware-golden-` → `.golden` (regardless of state)
    /// - other names prefixed `testanyware-` with `state == "running"` → `.running`
    /// - anything else is dropped
    ///
    /// Returns `[]` on malformed JSON or unexpected schema so a future tart
    /// upgrade cannot break `testanyware vm list`. The boundary to an
    /// externally-owned tool is where we accept schema drift gracefully.
    public static func parseList(tartJSON: String) throws -> [VMListEntry] {
        guard let data = tartJSON.data(using: .utf8), !tartJSON.isEmpty else { return [] }
        let vms: [TartVM]
        do {
            vms = try JSONDecoder().decode([TartVM].self, from: data)
        } catch {
            return []
        }
        var out: [VMListEntry] = []
        for vm in vms {
            if vm.name.hasPrefix("testanyware-golden-") {
                out.append(VMListEntry(
                    kind: .golden,
                    name: vm.name,
                    platform: platformFromName(vm.name),
                    backend: "tart",
                    sizeGB: vm.disk.map { "\($0) GB" }
                ))
            } else if vm.state == "running" && vm.name.hasPrefix("testanyware-") {
                out.append(VMListEntry(
                    kind: .running,
                    name: vm.name,
                    platform: platformFromName(vm.name),
                    backend: "tart"
                ))
            }
        }
        return out
    }

    /// Parse a `vnc://[:password@]host:port[...]` URL as emitted by
    /// `tart run --vnc-experimental`.
    ///
    /// tart appends a trailing `...` as a progress marker; strip it first.
    /// `URL(string:)` does not reliably handle the password-only credential
    /// form (`:pw@host`), so the parser is hand-rolled.
    public static func parseVNCURL(_ raw: String) throws -> VNCURL {
        var s = raw.trimmingCharacters(in: .whitespaces)
        while s.hasSuffix(".") { s.removeLast() }
        guard s.hasPrefix("vnc://") else {
            throw TartRunnerError.vncURLMalformed(raw)
        }
        var rest = String(s.dropFirst("vnc://".count))
        var password: String? = nil
        if let atIdx = rest.firstIndex(of: "@") {
            var cred = String(rest[..<atIdx])
            if cred.hasPrefix(":") { cred.removeFirst() }
            password = cred.isEmpty ? nil : cred
            rest = String(rest[rest.index(after: atIdx)...])
        }
        let parts = rest.split(separator: ":", maxSplits: 1).map(String.init)
        guard parts.count == 2, let port = Int(parts[1]) else {
            throw TartRunnerError.vncURLMalformed(raw)
        }
        return VNCURL(host: parts[0], port: port, password: password)
    }

    private static func platformFromName(_ name: String) -> String {
        if name.contains("macos") || name.contains("tahoe") { return "macos" }
        if name.contains("linux") || name.contains("ubuntu") { return "linux" }
        if name.contains("windows") { return "windows" }
        return "unknown"
    }

    /// Invoke `tart list --format json` and parse the result.
    ///
    /// Returns `[]` if `tart` is not on `PATH` (hosts that only run qemu
    /// builds) or if the invocation itself fails — same leniency as
    /// `parseList`. A non-zero exit or unreadable output is treated as
    /// "no entries," not as an error to propagate.
    public static func runList() throws -> [VMListEntry] {
        guard let tartPath = which("tart") else { return [] }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: tartPath)
        proc.arguments = ["list", "--format", "json"]
        let stdout = Pipe()
        proc.standardOutput = stdout
        proc.standardError = Pipe()
        try proc.run()
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return [] }
        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        let json = String(data: data, encoding: .utf8) ?? ""
        return try parseList(tartJSON: json)
    }

    /// Locate an executable on `PATH` via `/usr/bin/which`.
    ///
    /// Returns `nil` if the binary is absent, `which` cannot run, or its
    /// output is empty. This is the gate that makes `runList` a no-op on
    /// qemu-only hosts.
    public static func which(_ name: String) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        proc.arguments = [name]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let path = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (path?.isEmpty ?? true) ? nil : path
    }

    // MARK: - VM lifecycle helpers

    /// Composite return for a successful `tart run` startup.
    public struct StartInfo: Equatable, Sendable {
        public var pid: Int32
        public var vnc: VNCURL
        public var ip: String?

        public init(pid: Int32, vnc: VNCURL, ip: String?) {
            self.pid = pid
            self.vnc = vnc
            self.ip = ip
        }
    }

    /// Best-effort `tart stop` followed by `tart delete`. Both are
    /// no-ops if the VM is absent, so a non-zero exit is ignored.
    public static func removeExisting(id: String) {
        _ = runTart(arguments: ["stop", id])
        _ = runTart(arguments: ["delete", id])
    }

    /// `tart delete <name>` for a golden image. Returns `true` on
    /// success. Missing binary or a non-zero exit yields `false` so
    /// callers can surface a single "deletion failed" message without
    /// inspecting internal process details.
    @discardableResult
    public static func deleteGolden(name: String) -> Bool {
        runTart(arguments: ["delete", name]).exitCode == 0
    }

    /// Names of currently-running tart VMs that are not golden images.
    /// Used by `vm delete` to warn about clones that may depend on the
    /// image about to be removed.
    public static func runningClones() -> [String] {
        let entries = (try? runList()) ?? []
        return entries.filter { $0.kind == .running }.map { $0.name }
    }

    /// `tart clone <base> <id>`.
    public static func clone(base: String, id: String) throws {
        let result = runTart(arguments: ["clone", base, id])
        if result.exitCode != 0 {
            throw TartRunnerError.commandFailed(
                "tart clone \(base) \(id) (exit \(result.exitCode)): \(result.stderr)"
            )
        }
    }

    /// `tart set <id> --display <WxH>`.
    public static func setDisplay(id: String, display: String) throws {
        let result = runTart(arguments: ["set", id, "--display", display])
        if result.exitCode != 0 {
            throw TartRunnerError.commandFailed(
                "tart set \(id) --display \(display) (exit \(result.exitCode)): \(result.stderr)"
            )
        }
    }

    /// Spawn `tart run <id> --no-graphics --vnc-experimental` detached
    /// (own session via `posix_spawn` + SETSID), append-redirecting
    /// stdout+stderr to `<logDir>/<id>.tart.log`. Returns the detached
    /// pid and the log path so callers can poll for the VNC URL.
    public static func runDetached(id: String, logDir: String) throws -> (pid: Int32, logPath: String) {
        try FileManager.default.createDirectory(
            atPath: logDir,
            withIntermediateDirectories: true
        )
        let logPath = "\(logDir)/\(id).tart.log"
        if !FileManager.default.fileExists(atPath: logPath) {
            FileManager.default.createFile(atPath: logPath, contents: nil)
        }
        guard let tartPath = which("tart") else {
            throw TartRunnerError.commandFailed("tart not found on PATH")
        }
        let pid = try DetachedProcess.spawn(
            executable: tartPath,
            arguments: ["run", id, "--no-graphics", "--vnc-experimental"],
            logPath: logPath
        )
        return (pid, logPath)
    }

    /// Poll `logPath` for the first `vnc://...` URL. Returns the parsed
    /// URL on success, or `nil` after `attempts * intervalSeconds`
    /// elapse without finding one. A missing log file is treated as
    /// "not yet"; the loop keeps polling until the deadline.
    public static func pollVNCURL(
        logPath: String,
        attempts: Int,
        intervalSeconds: Double
    ) -> VNCURL? {
        let pattern = #"vnc://\S+"#
        for attempt in 0..<attempts {
            if let text = try? String(contentsOfFile: logPath, encoding: .utf8),
               let range = text.range(of: pattern, options: .regularExpression),
               let parsed = try? parseVNCURL(String(text[range])) {
                return parsed
            }
            if attempt < attempts - 1 {
                Thread.sleep(forTimeInterval: intervalSeconds)
            }
        }
        return nil
    }

    /// Poll `tart ip <id>` until it returns a non-empty address or
    /// `attempts` are exhausted. Returns `nil` on timeout — callers
    /// treat IP-unavailable as a benign degradation (agent endpoint
    /// becomes unset in the spec).
    public static func pollIP(
        id: String,
        attempts: Int,
        intervalSeconds: Double
    ) -> String? {
        for attempt in 0..<attempts {
            let result = runTart(arguments: ["ip", id])
            if result.exitCode == 0 {
                let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty { return trimmed }
            }
            if attempt < attempts - 1 {
                Thread.sleep(forTimeInterval: intervalSeconds)
            }
        }
        return nil
    }

    // MARK: - private process plumbing

    private struct ProcessResult {
        var exitCode: Int32
        var stdout: String
        var stderr: String
    }

    private static func runTart(arguments: [String]) -> ProcessResult {
        guard let tartPath = which("tart") else {
            return ProcessResult(exitCode: -1, stdout: "", stderr: "tart not found on PATH")
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: tartPath)
        proc.arguments = arguments
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        do {
            try proc.run()
        } catch {
            return ProcessResult(exitCode: -1, stdout: "", stderr: "\(error)")
        }
        proc.waitUntilExit()
        let stdout = String(
            data: outPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        ) ?? ""
        let stderr = String(
            data: errPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        ) ?? ""
        return ProcessResult(exitCode: proc.terminationStatus, stdout: stdout, stderr: stderr)
    }
}

public enum TartRunnerError: Error, CustomStringConvertible, Equatable {
    case vncURLMalformed(String)
    case commandFailed(String)

    public var description: String {
        switch self {
        case .vncURLMalformed(let raw):
            return "Malformed VNC URL from tart: \(raw)"
        case .commandFailed(let detail):
            return "tart command failed: \(detail)"
        }
    }
}
