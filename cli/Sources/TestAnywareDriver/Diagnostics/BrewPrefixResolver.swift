import Foundation

/// Resolves the Homebrew prefix (the directory under which `bin/`,
/// `share/`, etc. live).
///
/// Shared by the diagnostics modules that need to inspect bundled
/// artefacts (`BundledAgentsCheck`, `BundledScriptsCheck`,
/// `InstallPathCheck`, `ProvisionerScriptsVersionCheck`). The resolver
/// runs `brew --prefix` against the first executable `brew` it finds at
/// `/opt/homebrew/bin/brew` (Apple Silicon) or `/usr/local/bin/brew`
/// (Intel), returning the trimmed stdout or `nil` if no `brew` is
/// installed.
public enum BrewPrefixResolver {

    /// Default `brew` candidate paths in priority order.
    public static let defaultBrewCandidates: [String] = [
        "/opt/homebrew/bin/brew",
        "/usr/local/bin/brew",
    ]

    /// Probe the standard candidate paths and return the resolved
    /// Homebrew prefix, or `nil` if no `brew` is installed.
    public static func resolve() -> String? {
        return resolve(candidates: defaultBrewCandidates)
    }

    /// Probe a specific candidate list. Used by tests to inject
    /// alternative brew paths without mutating the default list.
    public static func resolve(candidates: [String]) -> String? {
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
