import Foundation

/// Diagnoses how the user's shell will resolve `testanyware`. Catches the
/// common contributor hazard of a stale `/usr/local/bin/testanyware`
/// dev-symlink shadowing the Homebrew install at `/opt/homebrew/bin/testanyware`.
public enum InstallPathCheck {

    public enum Verdict: Equatable {
        /// `which testanyware` resolves under the Homebrew prefix.
        case homebrewInstall(path: String, brewPrefix: String)

        /// `which testanyware` resolves to a path outside the Homebrew prefix
        /// while a Homebrew prefix exists. This is the dev-symlink-shadows-brew
        /// case the doctor is built to catch.
        case shadowed(path: String, brewPrefix: String)

        /// Homebrew is not installed (no prefix). Whatever is on PATH is what
        /// the user gets; nothing to compare against.
        case noHomebrew(path: String)

        /// Nothing on PATH resolves to `testanyware`. Either the binary is
        /// gone or the shell session is missing the right PATH entries.
        case notOnPath(brewPrefix: String?)
    }

    /// Pure classifier. Takes the path that `which testanyware` printed (the
    /// on-PATH symlink, not its target — the user's remediation needs to point
    /// at the symlink itself) and the Homebrew prefix; returns the verdict.
    /// Both inputs may be `nil`. Path comparison uses a trailing-slash guard
    /// so that `/opt/homebrew` does not falsely match `/opt/homebrew-evil`.
    public static func classify(
        pathBinary: String?,
        brewPrefix: String?
    ) -> Verdict {
        switch (pathBinary, brewPrefix) {
        case let (path?, prefix?):
            let normalizedPrefix = prefix.hasSuffix("/") ? prefix : prefix + "/"
            if path.hasPrefix(normalizedPrefix) {
                return .homebrewInstall(path: path, brewPrefix: prefix)
            }
            return .shadowed(path: path, brewPrefix: prefix)
        case let (path?, nil):
            return .noHomebrew(path: path)
        case (nil, let prefix):
            return .notOnPath(brewPrefix: prefix)
        }
    }

    public struct CheckResult {
        public let verdict: Verdict
        public let runningBinary: String

        public init(verdict: Verdict, runningBinary: String) {
            self.verdict = verdict
            self.runningBinary = runningBinary
        }

        /// `true` for verdicts that should not block other tooling — i.e.
        /// the install is sane (or, with no Homebrew, no comparison is
        /// possible). `false` for the dev-symlink-shadow case and for the
        /// "binary disappeared from PATH" case.
        public var isOK: Bool {
            switch verdict {
            case .homebrewInstall, .noHomebrew:
                return true
            case .shadowed, .notOnPath:
                return false
            }
        }
    }

    /// Runtime entry point. Resolves the on-PATH binary and Homebrew prefix
    /// via subprocess invocations, then classifies them.
    public static func run() -> CheckResult {
        let pathBinary = resolvedPathBinary()
        let brewPrefix = resolveBrewPrefix()
        let verdict = classify(pathBinary: pathBinary, brewPrefix: brewPrefix)
        return CheckResult(verdict: verdict, runningBinary: currentExecutablePath())
    }

    // MARK: - Subprocess helpers

    private static func resolvedPathBinary() -> String? {
        guard let raw = runCapturing(executable: "/usr/bin/which", arguments: ["testanyware"]) else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func resolveBrewPrefix() -> String? {
        let candidates = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        let brewPath = candidates.first { FileManager.default.isExecutableFile(atPath: $0) }
        guard let brewPath else { return nil }
        guard let raw = runCapturing(executable: brewPath, arguments: ["--prefix"]) else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func runCapturing(executable: String, arguments: [String]) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = arguments
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)
    }
}
