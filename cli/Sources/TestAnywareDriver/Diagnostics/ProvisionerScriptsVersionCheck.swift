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

    /// Per-tool probe customisation. Used when a sentinel-declared tool
    /// is not covered by `ToolAvailabilityCheck` and the default
    /// `<tool> --version` + greedy-first-token parser would extract
    /// the wrong value (or when the tool's `--version` selector differs).
    public struct ProbeCustomization: Sendable {
        /// Arguments passed to the tool, e.g. `["--version"]`.
        public let arguments: [String]
        /// Extracts a dotted-version string from the probe's stdout,
        /// or `nil` if the output cannot be parsed.
        public let parser: @Sendable (String) -> String?

        public init(arguments: [String], parser: @escaping @Sendable (String) -> String?) {
            self.arguments = arguments
            self.parser = parser
        }
    }

    /// Per-tool probe overrides for sentinel-declared tools that aren't
    /// covered by `ToolAvailabilityCheck.knownTools`. Tools without an
    /// entry here fall back to `<tool> --version` + the greedy-first-token
    /// parser shared with `ToolAvailabilityCheck`.
    public static let probeCustomizations: [String: ProbeCustomization] = [
        // `swift --version` writes the compiler banner to stdout (the
        // `swift-driver version: ...` shim is on stderr and discarded by
        // the probe). Anchoring on "Apple Swift version" is defence-in-
        // depth against future format changes that might prepend other
        // dotted tokens — the explicit marker is more robust than
        // positional parsing.
        "swift": ProbeCustomization(
            arguments: ["--version"],
            parser: { ProvisionerScriptsVersionCheck.parseSwiftVersion(from: $0) }
        ),
        // `dotnet --version` already prints a clean dotted token like
        // `9.0.100`. Listed for explicitness so the table is the
        // canonical reference for sentinel-declared tools.
        "dotnet": ProbeCustomization(
            arguments: ["--version"],
            parser: { ToolAvailabilityCheck.parseVersion(from: $0) }
        ),
    ]

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

    /// Runtime entry point. Scans every `.sh` file under
    /// `<brewPrefix>/share/testanyware/scripts/` for sentinels, plus the
    /// source-tree's `scripts/release-build.sh` when the running binary
    /// is located inside a dev source tree (see
    /// `locateReleaseBuildScript(near:)`). Compares aggregated floors
    /// against the host versions reported by `ToolAvailabilityCheck`,
    /// supplemented by extra probes for sentinel-declared tools that
    /// `ToolAvailabilityCheck` doesn't cover (see `probeCustomizations`).
    public static func run() -> CheckResult {
        var scripts: [String] = []
        if let brewPrefix = BrewPrefixResolver.resolve() {
            let bundledDir = "\(brewPrefix)/share/testanyware/scripts"
            if let bundled = listShellScripts(in: bundledDir) {
                scripts.append(contentsOf: bundled)
            }
        }
        if let binaryPath = runningBinaryPath(),
           let releaseBuild = locateReleaseBuildScript(near: binaryPath) {
            scripts.append(releaseBuild)
        }
        if scripts.isEmpty {
            return skippedResult()
        }
        var floors: [DeclaredFloor] = []
        for path in scripts {
            guard let content = try? String(contentsOfFile: path, encoding: .utf8) else { continue }
            floors.append(contentsOf: parseSentinels(in: content, source: path))
        }
        let hostVersions = collectHostVersions(forFloors: floors)
        return classify(floors: floors, hostVersions: hostVersions)
    }

    /// Walks up from `binaryPath`'s containing directory, looking for a
    /// parent that contains `scripts/release-build.sh`. Returns the
    /// absolute path of the first hit, or `nil` if none is found before
    /// the filesystem root.
    ///
    /// Brew installs naturally return `nil`: the bundled binary at
    /// `<brewPrefix>/bin/testanyware` has no `scripts/release-build.sh`
    /// above it. Dev builds (e.g. binaries under `<repo>/cli/.build/`)
    /// resolve to `<repo>/scripts/release-build.sh`.
    public static func locateReleaseBuildScript(near binaryPath: String) -> String? {
        let target = "scripts/release-build.sh"
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: binaryPath)
            .resolvingSymlinksInPath()
            .deletingLastPathComponent()
        while dir.path != "/" && !dir.path.isEmpty {
            let candidate = dir.appendingPathComponent(target).path
            if fm.fileExists(atPath: candidate) {
                return candidate
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        return nil
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

    /// Collect host versions for every tool referenced by `floors`.
    /// Tools covered by `ToolAvailabilityCheck` reuse its result; any
    /// extra tools (e.g. `swift`, `dotnet`) are probed via the
    /// customisation table or the universal `<tool> --version` fallback.
    /// Maps each tool to its dotted version, or `nil` when the tool is
    /// missing / the probe failed / the output didn't parse.
    private static func collectHostVersions(forFloors floors: [DeclaredFloor]) -> [String: String?] {
        var hostVersions: [String: String?] = [:]
        let toolResult = ToolAvailabilityCheck.run()
        for status in toolResult.statuses {
            switch status.versionVerdict {
            case .ok(let detected):
                hostVersions[status.tool.name] = detected
            case .belowFloor(let detected, _):
                hostVersions[status.tool.name] = detected
            case .unparseable, .probeFailed:
                hostVersions[status.tool.name] = nil
            }
        }
        let coveredByToolCheck = Set(ToolAvailabilityCheck.knownTools.map(\.name))
        let extras = Set(floors.map(\.tool)).subtracting(coveredByToolCheck)
        for tool in extras {
            hostVersions[tool] = resolveExtraHostVersion(forTool: tool)
        }
        return hostVersions
    }

    /// Probe and parse the host's installed version of a sentinel-
    /// declared tool that isn't covered by `ToolAvailabilityCheck`.
    /// Uses the customisation table when present, otherwise falls back
    /// to `<tool> --version` + `ToolAvailabilityCheck.parseVersion`.
    static func resolveExtraHostVersion(forTool tool: String) -> String? {
        let custom = probeCustomizations[tool]
        let arguments = custom?.arguments ?? ["--version"]
        let parse: (String) -> String? = custom?.parser
            ?? { ToolAvailabilityCheck.parseVersion(from: $0) }
        guard let raw = runProbe(tool: tool, arguments: arguments) else {
            return nil
        }
        return parse(raw)
    }

    /// Extracts `MAJOR.MINOR[.PATCH]` from `Apple Swift version X.Y[.Z]`
    /// in `swift --version` stdout. The default greedy-first-token
    /// parser would otherwise attach itself to a `swift-driver version:`
    /// prefix if Apple ever moves that line to stdout — anchoring on
    /// the explicit "Apple Swift version" marker keeps the extraction
    /// stable across format drift.
    static func parseSwiftVersion(from rawOutput: String) -> String? {
        guard let range = rawOutput.range(of: "Apple Swift version ") else {
            return nil
        }
        return ToolAvailabilityCheck.parseVersion(from: String(rawOutput[range.upperBound...]))
    }

    /// Runs `/usr/bin/env <tool> <arguments>` and returns trimmed stdout,
    /// or `nil` on launch failure / empty output. Mirrors
    /// `ToolAvailabilityCheck.probeVersion` so all host probes share the
    /// same execution semantics (stderr discarded, stdout-only capture).
    private static func runProbe(tool: String, arguments: [String]) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        proc.arguments = [tool] + arguments
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let text = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (text?.isEmpty ?? true) ? nil : text
    }

    /// Returns the path of the currently-running executable, preferring
    /// `Bundle.main.executablePath` (Foundation-resolved) and falling
    /// back to `CommandLine.arguments.first`. Used by `run()` to locate
    /// the source tree above a dev-built binary.
    private static func runningBinaryPath() -> String? {
        if let bundlePath = Bundle.main.executablePath, !bundlePath.isEmpty {
            return bundlePath
        }
        return CommandLine.arguments.first
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
