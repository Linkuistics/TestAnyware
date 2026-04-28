import Foundation

/// Verifies that the provisioner scripts and helpers bundled with the
/// Homebrew install are present at `<brewPrefix>/share/testanyware/scripts/`
/// and `<brewPrefix>/share/testanyware/helpers/`.
///
/// `release-build.sh` stages two slots:
/// - `scripts/` — eight `.sh` files: `_testanyware-paths.sh`,
///   `vm-{start,stop,list,delete}.sh`, `vm-create-golden-{macos,linux,windows}.sh`.
///   All require the executable bit (the build sets it via `chmod +x`).
/// - `helpers/` — six mixed-format files: `autounattend.xml`,
///   `com.linkuistics.testanyware.agent.plist`, `desktop-setup.ps1`,
///   `set-wallpaper.ps1`, `set-wallpaper.swift`, `SetupComplete.cmd`.
///   Presence-only; their executable bits vary by file type and host.
///
/// A missing file means the brew install is incomplete — usually a
/// `brew reinstall` resolves it.
public enum BundledScriptsCheck {

    public enum Slot: String, Equatable, Sendable {
        case scripts
        case helpers
    }

    public enum FileIssue: Equatable, Sendable {
        case missing(path: String)
        case notExecutable(path: String)
    }

    public struct FileProbe: Equatable, Sendable {
        public let slot: Slot
        public let path: String
        public let exists: Bool
        public let isExecutable: Bool
        public let requiresExecutable: Bool

        public init(slot: Slot, path: String, exists: Bool, isExecutable: Bool, requiresExecutable: Bool) {
            self.slot = slot
            self.path = path
            self.exists = exists
            self.isExecutable = isExecutable
            self.requiresExecutable = requiresExecutable
        }
    }

    public enum Verdict: Equatable, Sendable {
        case allPresent(brewPrefix: String)
        case missing(brewPrefix: String, issues: [(slot: Slot, issue: FileIssue)])
        case noHomebrew

        public static func == (lhs: Verdict, rhs: Verdict) -> Bool {
            switch (lhs, rhs) {
            case let (.allPresent(a), .allPresent(b)):
                return a == b
            case (.noHomebrew, .noHomebrew):
                return true
            case let (.missing(aPrefix, aIssues), .missing(bPrefix, bIssues)):
                guard aPrefix == bPrefix, aIssues.count == bIssues.count else { return false }
                for (lhs, rhs) in zip(aIssues, bIssues) {
                    if lhs.slot != rhs.slot || lhs.issue != rhs.issue { return false }
                }
                return true
            default:
                return false
            }
        }
    }

    public struct CheckResult {
        public let verdict: Verdict

        public init(verdict: Verdict) {
            self.verdict = verdict
        }

        public var isOK: Bool {
            switch verdict {
            case .allPresent, .noHomebrew:
                return true
            case .missing:
                return false
            }
        }
    }

    /// Pure classifier. Takes the brew prefix and a flat list of file
    /// probes; returns the verdict. Probes carry their own
    /// `requiresExecutable` flag so the executable-bit invariant is a
    /// per-file property rather than a slot-wide policy embedded in the
    /// classifier.
    public static func classify(
        brewPrefix: String?,
        probes: [FileProbe]
    ) -> Verdict {
        guard let brewPrefix else { return .noHomebrew }
        var issues: [(slot: Slot, issue: FileIssue)] = []
        for probe in probes {
            if !probe.exists {
                issues.append((probe.slot, .missing(path: probe.path)))
                continue
            }
            if probe.requiresExecutable && !probe.isExecutable {
                issues.append((probe.slot, .notExecutable(path: probe.path)))
            }
        }
        if issues.isEmpty {
            return .allPresent(brewPrefix: brewPrefix)
        }
        return .missing(brewPrefix: brewPrefix, issues: issues)
    }

    /// Runtime entry point. Resolves the Homebrew prefix, probes every
    /// expected script and helper path, and classifies the result.
    public static func run() -> CheckResult {
        let brewPrefix = resolveBrewPrefix()
        let probes = brewPrefix.map(probeAll) ?? []
        return CheckResult(verdict: classify(brewPrefix: brewPrefix, probes: probes))
    }

    /// Filenames staged into `share/testanyware/scripts/` by
    /// `scripts/release-build.sh#stage_scripts`. All require the
    /// executable bit.
    public static let scriptFilenames: [String] = [
        "_testanyware-paths.sh",
        "vm-create-golden-linux.sh",
        "vm-create-golden-macos.sh",
        "vm-create-golden-windows.sh",
        "vm-delete.sh",
        "vm-list.sh",
        "vm-start.sh",
        "vm-stop.sh",
    ]

    /// Filenames staged into `share/testanyware/helpers/` by
    /// `scripts/release-build.sh#stage_helpers`. Presence-only —
    /// modes vary by file type.
    public static let helperFilenames: [String] = [
        "SetupComplete.cmd",
        "autounattend.xml",
        "com.linkuistics.testanyware.agent.plist",
        "desktop-setup.ps1",
        "set-wallpaper.ps1",
        "set-wallpaper.swift",
    ]

    public static func expectedScriptPaths(brewPrefix: String) -> [String] {
        let base = "\(brewPrefix)/share/testanyware/scripts"
        return scriptFilenames.map { "\(base)/\($0)" }
    }

    public static func expectedHelperPaths(brewPrefix: String) -> [String] {
        let base = "\(brewPrefix)/share/testanyware/helpers"
        return helperFilenames.map { "\(base)/\($0)" }
    }

    private static func probeAll(brewPrefix: String) -> [FileProbe] {
        var probes: [FileProbe] = []
        for path in expectedScriptPaths(brewPrefix: brewPrefix) {
            probes.append(probe(path: path, slot: .scripts, requiresExecutable: true))
        }
        for path in expectedHelperPaths(brewPrefix: brewPrefix) {
            probes.append(probe(path: path, slot: .helpers, requiresExecutable: false))
        }
        return probes
    }

    private static func probe(path: String, slot: Slot, requiresExecutable: Bool) -> FileProbe {
        let exists = FileManager.default.fileExists(atPath: path)
        let executable = exists && FileManager.default.isExecutableFile(atPath: path)
        return FileProbe(
            slot: slot,
            path: path,
            exists: exists,
            isExecutable: executable,
            requiresExecutable: requiresExecutable
        )
    }

    private static func resolveBrewPrefix() -> String? {
        let candidates = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        guard let brewPath = candidates.first(where: {
            FileManager.default.isExecutableFile(atPath: $0)
        }) else { return nil }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: brewPath)
        proc.arguments = ["--prefix"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let trimmed = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (trimmed?.isEmpty ?? true) ? nil : trimmed
    }
}
