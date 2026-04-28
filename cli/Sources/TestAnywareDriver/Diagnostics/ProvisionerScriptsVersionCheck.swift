import Foundation

/// Cross-checks the host tool versions reported by `ToolAvailabilityCheck`
/// against minimum-version sentinels declared inside the bundled
/// provisioner scripts.
///
/// Bundled scripts under `<brewPrefix>/share/testanyware/scripts/`
/// (and the release pipeline's `scripts/release-build.sh`) can declare
/// per-tool floors using a comment-style sentinel:
///
///     # testanyware-min-tool: <name> <version>
///
/// e.g. `# testanyware-min-tool: tart 2.5.0`. Floors collected from
/// every scanned script are aggregated by tool name, taking the
/// highest declared version. The check then compares each aggregate
/// floor to the host's resolved version (sourced from
/// `ToolAvailabilityCheck`).
///
/// Mismatches surface as advisory warnings — matching the
/// `ToolAvailabilityCheck` precedent — because a below-floor host
/// tool can still work for many workflows. The doctor stays green;
/// the verdict is informational.
///
/// Today's bundled scripts declare no sentinels, so the verdict is a
/// benign pass-through. The framework lets future scripts opt into
/// version coherence checks by adding a single comment line.
public enum ProvisionerScriptsVersionCheck {

    /// One sentinel-declared floor parsed from a script.
    public struct DeclaredFloor: Equatable, Sendable {
        /// Tool name as it would be resolved on PATH (e.g. `tart`).
        public let tool: String
        /// Required minimum version, dotted form (e.g. `2.5.0`).
        public let minimumVersion: String
        /// Path of the script that declared the floor; surfaced in
        /// remediation messages so the user knows where the constraint
        /// came from.
        public let source: String

        public init(tool: String, minimumVersion: String, source: String) {
            self.tool = tool
            self.minimumVersion = minimumVersion
            self.source = source
        }
    }

    /// Per-tool comparison verdict.
    public enum ToolVerdict: Equatable, Sendable {
        /// Host version meets or exceeds the declared floor.
        case ok(tool: String, hostVersion: String, declaredMinimum: String, source: String)
        /// Host version is below the declared floor.
        case belowFloor(tool: String, hostVersion: String, declaredMinimum: String, source: String)
        /// A floor was declared but the host did not resolve a version
        /// (tool missing or version probe failed). Advisory: surfaces
        /// the sentinel so the user knows the script needs the tool.
        case hostVersionUnknown(tool: String, declaredMinimum: String, source: String)
        /// A sentinel line was found but its version token was not
        /// dotted-numeric (e.g. `# testanyware-min-tool: tart latest`).
        /// Surfaced verbatim so the script author can fix the typo.
        case unparseable(tool: String, rawValue: String, source: String)
    }

    public struct CheckResult: Equatable, Sendable {
        public let perTool: [ToolVerdict]
        /// True when no scripts could be scanned (e.g. Homebrew not
        /// installed, or the bundled scripts directory is missing).
        /// Distinct from "scripts scanned, no sentinels found", which
        /// surfaces as `perTool.isEmpty && !skipped`.
        public let skipped: Bool

        public init(perTool: [ToolVerdict], skipped: Bool) {
            self.perTool = perTool
            self.skipped = skipped
        }

        /// Version coherence is advisory; a below-floor host does not
        /// fail the doctor. Matches `ToolAvailabilityCheck.isOK`
        /// semantics for the same reason.
        public var isOK: Bool { true }
    }

    /// Sentinel prefix recognised inside scanned scripts. Public for
    /// use by tests and by future script authors who want to grep
    /// the declaration syntax.
    public static let sentinelPrefix = "# testanyware-min-tool:"

    /// Pure classifier. Aggregates the highest declared floor per tool
    /// across all scanned `floors`, then compares each aggregate to
    /// the corresponding host version in `hostVersions` (`nil` means
    /// the host did not resolve a version for that tool).
    ///
    /// Aggregation uses string `compare(_:options:.numeric)` ordering
    /// so component values compare as numbers (`2.32` > `2.5`).
    public static func classify(
        floors: [DeclaredFloor],
        hostVersions: [String: String?]
    ) -> CheckResult {
        let aggregated = aggregateFloors(floors)
        var verdicts: [ToolVerdict] = []
        for declared in aggregated {
            if !isDottedVersion(declared.minimumVersion) {
                verdicts.append(.unparseable(
                    tool: declared.tool,
                    rawValue: declared.minimumVersion,
                    source: declared.source
                ))
                continue
            }
            let hostVersion = hostVersions[declared.tool] ?? nil
            guard let hostVersion else {
                verdicts.append(.hostVersionUnknown(
                    tool: declared.tool,
                    declaredMinimum: declared.minimumVersion,
                    source: declared.source
                ))
                continue
            }
            if hostVersion.compare(declared.minimumVersion, options: .numeric) == .orderedAscending {
                verdicts.append(.belowFloor(
                    tool: declared.tool,
                    hostVersion: hostVersion,
                    declaredMinimum: declared.minimumVersion,
                    source: declared.source
                ))
            } else {
                verdicts.append(.ok(
                    tool: declared.tool,
                    hostVersion: hostVersion,
                    declaredMinimum: declared.minimumVersion,
                    source: declared.source
                ))
            }
        }
        return CheckResult(perTool: verdicts, skipped: false)
    }

    /// Result for the no-bundle case (Homebrew not installed, or the
    /// scripts directory is absent). Distinct from a scan that simply
    /// found no sentinels.
    public static func skippedResult() -> CheckResult {
        return CheckResult(perTool: [], skipped: true)
    }

    /// Parse all sentinel lines out of `scriptContent`, attaching
    /// `source` to each floor. Lines that do not start with the
    /// sentinel prefix are ignored. Lines that match the prefix but
    /// don't tokenise as `<tool> <version>` produce an `.unparseable`
    /// floor (with the malformed value preserved in `minimumVersion`)
    /// so the user sees the typo instead of silent drop.
    public static func parseSentinels(in scriptContent: String, source: String) -> [DeclaredFloor] {
        var floors: [DeclaredFloor] = []
        for rawLine in scriptContent.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard line.hasPrefix(sentinelPrefix) else { continue }
            let payload = line.dropFirst(sentinelPrefix.count).trimmingCharacters(in: .whitespaces)
            let tokens = payload.split(whereSeparator: { $0.isWhitespace }).map(String.init)
            guard tokens.count >= 2 else {
                floors.append(DeclaredFloor(
                    tool: tokens.first ?? "(unknown)",
                    minimumVersion: tokens.dropFirst().joined(separator: " "),
                    source: source
                ))
                continue
            }
            floors.append(DeclaredFloor(
                tool: tokens[0],
                minimumVersion: tokens[1],
                source: source
            ))
        }
        return floors
    }

    /// Runtime entry point. Resolves the Homebrew prefix, scans every
    /// `.sh` file under `<brewPrefix>/share/testanyware/scripts/` for
    /// sentinels, and compares aggregated floors against the host
    /// versions reported by `ToolAvailabilityCheck`.
    public static func run() -> CheckResult {
        guard let brewPrefix = BrewPrefixResolver.resolve() else {
            return skippedResult()
        }
        let scriptDir = "\(brewPrefix)/share/testanyware/scripts"
        guard let scripts = listShellScripts(in: scriptDir), !scripts.isEmpty else {
            return skippedResult()
        }
        var floors: [DeclaredFloor] = []
        for path in scripts {
            guard let content = try? String(contentsOfFile: path, encoding: .utf8) else { continue }
            floors.append(contentsOf: parseSentinels(in: content, source: path))
        }
        let hostVersions = collectHostVersions()
        return classify(floors: floors, hostVersions: hostVersions)
    }

    // MARK: - Helpers

    /// Aggregate floors by tool name, keeping the highest declared
    /// version. The `source` attached to the result is the script that
    /// declared the winning version, so the doctor can point users at
    /// the right file.
    static func aggregateFloors(_ floors: [DeclaredFloor]) -> [DeclaredFloor] {
        var byTool: [String: DeclaredFloor] = [:]
        for floor in floors {
            guard let existing = byTool[floor.tool] else {
                byTool[floor.tool] = floor
                continue
            }
            // Numeric comparison preserves "2.32" > "2.5"; for
            // unparseable strings the comparison falls back to
            // lexicographic, which is fine — we surface unparseables
            // as their own verdict regardless of which won here.
            if floor.minimumVersion.compare(existing.minimumVersion, options: .numeric) == .orderedDescending {
                byTool[floor.tool] = floor
            }
        }
        return byTool.keys.sorted().compactMap { byTool[$0] }
    }

    /// Returns true if `value` is a dotted-numeric version like
    /// `2.0.0` or `2.5`. Pure-number tokens (no dot) are rejected so
    /// `latest` and `2026` don't sneak through as "valid".
    static func isDottedVersion(_ value: String) -> Bool {
        guard value.contains(".") else { return false }
        for ch in value where !(ch.isNumber || ch == ".") { return false }
        guard value.first?.isNumber == true else { return false }
        return true
    }

    /// Collect the host's resolved tool versions from
    /// `ToolAvailabilityCheck`. Maps each known tool's verdict to a
    /// dotted-version string when the host reported one, or `nil`
    /// when the tool is missing / unparseable / the probe failed.
    private static func collectHostVersions() -> [String: String?] {
        let result = ToolAvailabilityCheck.run()
        var hostVersions: [String: String?] = [:]
        for status in result.statuses {
            switch status.versionVerdict {
            case .ok(let detected):
                hostVersions[status.tool.name] = detected
            case .belowFloor(let detected, _):
                hostVersions[status.tool.name] = detected
            case .unparseable, .probeFailed:
                hostVersions[status.tool.name] = nil
            }
        }
        return hostVersions
    }

    private static func listShellScripts(in directory: String) -> [String]? {
        let fm = FileManager.default
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: directory, isDirectory: &isDir), isDir.boolValue else {
            return nil
        }
        guard let entries = try? fm.contentsOfDirectory(atPath: directory) else {
            return nil
        }
        return entries
            .filter { $0.hasSuffix(".sh") }
            .map { "\(directory)/\($0)" }
            .sorted()
    }
}
