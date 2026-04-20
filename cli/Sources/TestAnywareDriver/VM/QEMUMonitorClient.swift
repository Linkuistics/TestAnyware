import Foundation

/// Human Monitor Protocol (HMP) client for a QEMU monitor unix socket.
///
/// Communication goes through `/usr/bin/nc -U` launched via `/bin/sh`,
/// matching `scripts/macos/vm-start.sh` byte-for-byte: a subshell pipes
/// `command\n` to `nc`, sleeps briefly to let the response drain, then
/// closes the pipe so `nc` exits and returns what it read. Passing the
/// command, timeout, and socket path as positional `sh -c` arguments
/// keeps them out of the shell's parse stream.
///
/// Parsers are exposed as `static` functions so unit tests can exercise
/// them without spawning a process.
public struct QEMUMonitorClient: Sendable {
    public let socketPath: String

    public init(socketPath: String) {
        self.socketPath = socketPath
    }

    /// Send `command` over the monitor socket and return the raw response.
    public func send(_ command: String, timeoutSeconds: Double = 0.5) async throws -> String {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/sh")
        proc.arguments = [
            "-c",
            "(printf '%s\\n' \"$1\"; sleep \"$2\") | /usr/bin/nc -U \"$3\"",
            "testanyware-qmon",
            command,
            String(format: "%.2f", timeoutSeconds),
            socketPath,
        ]
        let outPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = Pipe()

        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            proc.terminationHandler = { _ in
                cont.resume(returning: ())
            }
            do {
                try proc.run()
            } catch {
                cont.resume(throwing: error)
            }
        }

        let data = outPipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8) ?? ""
    }

    /// Poll `info usernet` until the guest→host forward port is observable,
    /// or the attempt budget is exhausted. Returns `nil` on timeout.
    public func agentPort(attempts: Int, intervalSeconds: Double) async throws -> Int? {
        for attempt in 0..<attempts {
            let response = try await send("info usernet")
            if let port = Self.parseAgentPort(infoUsernet: response) {
                return port
            }
            if attempt < attempts - 1 {
                try await Task.sleep(nanoseconds: UInt64(intervalSeconds * 1_000_000_000))
            }
        }
        return nil
    }

    /// Poll `info vnc` until the listening VNC port is observable, or the
    /// attempt budget is exhausted. Returns `nil` on timeout.
    public func vncPort(attempts: Int, intervalSeconds: Double) async throws -> Int? {
        for attempt in 0..<attempts {
            let response = try await send("info vnc")
            if let port = Self.parseVNCPort(infoVnc: response) {
                return port
            }
            if attempt < attempts - 1 {
                try await Task.sleep(nanoseconds: UInt64(intervalSeconds * 1_000_000_000))
            }
        }
        return nil
    }

    /// Best-effort `set_password vnc <password>`. The monitor socket may
    /// not accept connections on the first try immediately after QEMU
    /// launch — retry up to `attempts` times and swallow send errors.
    public func setVNCPassword(_ password: String, attempts: Int) async {
        // Strip any newline a caller might accidentally embed — HMP is
        // line-terminated so a newline would inject a second command.
        let sanitised = password.replacingOccurrences(of: "\n", with: "")
            .replacingOccurrences(of: "\r", with: "")
        for attempt in 0..<attempts {
            _ = try? await send("set_password vnc \(sanitised)")
            if attempt < attempts - 1 {
                try? await Task.sleep(nanoseconds: 500_000_000)
            }
        }
    }

    // MARK: - Pure parsers

    /// Parse the host-forward port from `info usernet` output. The first
    /// `HOST_FORWARD` row wins; our QEMU invocations only ever declare one.
    ///
    /// Expected row: `TCP[HOST_FORWARD]  <fd>  *  <host_port>  <guest>  ...`
    ///
    /// Splits on `Character.isNewline` (not the literal `"\n"`) because
    /// QEMU monitor responses use CRLF line endings, and Swift's
    /// `Character` is a grapheme cluster — `\r\n` is a single `Character`,
    /// so `split(separator: "\n")` collapses the whole response into one
    /// logical line and the field-index parse fails. Surfaced via the
    /// Windows VM start smoke after the swtpm `sun_path` fix.
    public static func parseAgentPort(infoUsernet: String) -> Int? {
        for line in infoUsernet.split(whereSeparator: { $0.isNewline }) {
            guard line.contains("HOST_FORWARD") else { continue }
            let fields = line.split(separator: " ", omittingEmptySubsequences: true)
            if fields.count >= 4, let port = Int(fields[3]) {
                return port
            }
        }
        return nil
    }

    /// Parse the listening VNC port from `info vnc` output.
    ///
    /// Expected snippet: `Server: 127.0.0.1:<port>`.
    public static func parseVNCPort(infoVnc: String) -> Int? {
        let pattern = #"127\.0\.0\.1:(\d+)"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return nil }
        let range = NSRange(infoVnc.startIndex..<infoVnc.endIndex, in: infoVnc)
        guard let match = regex.firstMatch(in: infoVnc, range: range),
            match.numberOfRanges >= 2,
            let portRange = Range(match.range(at: 1), in: infoVnc)
        else {
            return nil
        }
        return Int(infoVnc[portRange])
    }
}
