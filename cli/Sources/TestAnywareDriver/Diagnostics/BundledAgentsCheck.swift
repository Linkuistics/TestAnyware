import Foundation

/// Verifies that the per-platform agent payloads bundled with the
/// Homebrew install are present at `<brewPrefix>/share/testanyware/agents/`.
///
/// The release tarball stages three artefacts:
/// - `macos/testanyware-agent`           — Swift binary (must be executable)
/// - `windows/testanyware-agent.exe`     — .NET 9 win-arm64 binary (presence only;
///                                         won't carry the macOS executable bit)
/// - `linux/testanyware_agent/__main__.py` — Python package entry point
///
/// A missing payload means the brew install is incomplete or an override
/// env-var pointed at a stale path; either is actionable.
public enum BundledAgentsCheck {

    public enum AgentSlot: String, Equatable, Sendable {
        case macos
        case windows
        case linux
    }

    public enum SlotIssue: Equatable, Sendable {
        /// The expected file or directory does not exist at all.
        case missing(path: String)
        /// The macOS agent binary exists but is not marked executable.
        case notExecutable(path: String)
    }

    public struct SlotProbe: Equatable, Sendable {
        public let slot: AgentSlot
        public let expectedPath: String
        public let exists: Bool
        public let isExecutable: Bool

        public init(slot: AgentSlot, expectedPath: String, exists: Bool, isExecutable: Bool) {
            self.slot = slot
            self.expectedPath = expectedPath
            self.exists = exists
            self.isExecutable = isExecutable
        }
    }

    public enum Verdict: Equatable, Sendable {
        /// All three agent payloads are present and (where applicable) executable.
        case allPresent(brewPrefix: String)
        /// At least one slot is missing or non-executable.
        case missing(brewPrefix: String, issues: [(slot: AgentSlot, issue: SlotIssue)])
        /// Homebrew is not installed; the bundled-agent directory has no
        /// canonical location to check. Treated as a benign skip — the
        /// `InstallPathCheck` already flags the no-Homebrew case.
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

    /// Pure classifier. Takes the brew prefix and three slot probes; returns
    /// the verdict. Slot probes carry exists/executable booleans so the
    /// runtime layer can be a thin wrapper around `FileManager`.
    public static func classify(
        brewPrefix: String?,
        probes: [SlotProbe]
    ) -> Verdict {
        guard let brewPrefix else { return .noHomebrew }
        var issues: [(slot: AgentSlot, issue: SlotIssue)] = []
        let bySlot = Dictionary(uniqueKeysWithValues: probes.map { ($0.slot, $0) })
        for slot in [AgentSlot.macos, .windows, .linux] {
            guard let probe = bySlot[slot] else {
                issues.append((slot, .missing(path: "(unprobed)")))
                continue
            }
            if !probe.exists {
                issues.append((slot, .missing(path: probe.expectedPath)))
                continue
            }
            // Only the macOS agent is run on this host, so it's the only one
            // whose executable bit we can meaningfully assert. The .exe is
            // a Windows binary and the Linux Python package isn't executable
            // by file mode either way.
            if slot == .macos && !probe.isExecutable {
                issues.append((slot, .notExecutable(path: probe.expectedPath)))
            }
        }
        if issues.isEmpty {
            return .allPresent(brewPrefix: brewPrefix)
        }
        return .missing(brewPrefix: brewPrefix, issues: issues)
    }

    /// Runtime entry point. Resolves the Homebrew prefix, probes the three
    /// expected agent paths, and classifies the result.
    public static func run() -> CheckResult {
        let brewPrefix = BrewPrefixResolver.resolve()
        let probes = brewPrefix.map(probeAll) ?? []
        return CheckResult(verdict: classify(brewPrefix: brewPrefix, probes: probes))
    }

    /// Where each slot's payload is expected to live under a brew prefix.
    public static func expectedPath(brewPrefix: String, slot: AgentSlot) -> String {
        let base = "\(brewPrefix)/share/testanyware/agents"
        switch slot {
        case .macos:
            return "\(base)/macos/testanyware-agent"
        case .windows:
            return "\(base)/windows/testanyware-agent.exe"
        case .linux:
            return "\(base)/linux/testanyware_agent/__main__.py"
        }
    }

    private static func probeAll(brewPrefix: String) -> [SlotProbe] {
        return [AgentSlot.macos, .windows, .linux].map { slot in
            let path = expectedPath(brewPrefix: brewPrefix, slot: slot)
            let exists = FileManager.default.fileExists(atPath: path)
            let executable = exists && FileManager.default.isExecutableFile(atPath: path)
            return SlotProbe(slot: slot, expectedPath: path, exists: exists, isExecutable: executable)
        }
    }

}
