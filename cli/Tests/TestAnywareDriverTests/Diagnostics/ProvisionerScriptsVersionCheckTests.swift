import Foundation
import Testing
@testable import TestAnywareDriver

@Suite("ProvisionerScriptsVersionCheck")
struct ProvisionerScriptsVersionCheckTests {

    typealias Check = ProvisionerScriptsVersionCheck
    typealias Floor = ProvisionerScriptsVersionCheck.DeclaredFloor

    // MARK: - parseSentinels

    @Test func parsesSingleSentinel() {
        let content = """
        #!/usr/bin/env bash
        # testanyware-min-tool: tart 2.5.0
        echo hi
        """
        let floors = Check.parseSentinels(in: content, source: "/path/to/foo.sh")
        #expect(floors == [
            Floor(tool: "tart", minimumVersion: "2.5.0", source: "/path/to/foo.sh"),
        ])
    }

    @Test func parsesMultipleSentinelsInOneScript() {
        let content = """
        # testanyware-min-tool: tart 2.5.0
        # testanyware-min-tool: qemu-system-aarch64 9.0.0
        # testanyware-min-tool: swtpm 0.10.0
        """
        let floors = Check.parseSentinels(in: content, source: "/x/foo.sh")
        #expect(floors.count == 3)
        #expect(floors[0].tool == "tart")
        #expect(floors[1].tool == "qemu-system-aarch64")
        #expect(floors[2].tool == "swtpm")
    }

    @Test func ignoresLinesWithoutSentinelPrefix() {
        let content = """
        # this is just a comment
        echo "# testanyware-min-tool: tart 2.5.0"
        # min-tool: tart 2.5.0
        # testanyware-min-tool: tart 2.5.0
        """
        // Only the literal sentinel-prefixed comment counts. The echo
        // line has the prefix only inside a quoted string, but the
        // parser is line-oriented and trims leading whitespace — so
        // a non-`#` first character disqualifies the line.
        let floors = Check.parseSentinels(in: content, source: "/x/foo.sh")
        #expect(floors == [
            Floor(tool: "tart", minimumVersion: "2.5.0", source: "/x/foo.sh"),
        ])
    }

    @Test func tolerantOfLeadingWhitespace() {
        // Indented sentinels still count — script authors might tab
        // them under a section heading.
        let content = "    # testanyware-min-tool: tart 2.5.0"
        let floors = Check.parseSentinels(in: content, source: "/x/foo.sh")
        #expect(floors.first?.tool == "tart")
    }

    @Test func malformedSentinelMissingVersionIsRecorded() {
        // A single-token sentinel (just the tool name) preserves the
        // empty version string so the user sees the typo via the
        // `.unparseable` verdict downstream.
        let content = "# testanyware-min-tool: tart"
        let floors = Check.parseSentinels(in: content, source: "/x/foo.sh")
        #expect(floors.count == 1)
        #expect(floors[0].tool == "tart")
        #expect(floors[0].minimumVersion == "")
    }

    // MARK: - aggregateFloors

    @Test func aggregateKeepsHighestPerTool() {
        let floors = [
            Floor(tool: "tart", minimumVersion: "2.0.0", source: "a.sh"),
            Floor(tool: "tart", minimumVersion: "2.5.0", source: "b.sh"),
            Floor(tool: "tart", minimumVersion: "2.32.1", source: "c.sh"),
            Floor(tool: "qemu-system-aarch64", minimumVersion: "8.0.0", source: "a.sh"),
        ]
        let aggregated = Check.aggregateFloors(floors)
        let byTool = Dictionary(uniqueKeysWithValues: aggregated.map { ($0.tool, $0) })
        // Numeric comparison must beat lexicographic — "2.32.1" vs
        // "2.5.0" only resolves correctly when components compare as
        // integers.
        #expect(byTool["tart"]?.minimumVersion == "2.32.1")
        #expect(byTool["tart"]?.source == "c.sh")
        #expect(byTool["qemu-system-aarch64"]?.minimumVersion == "8.0.0")
    }

    @Test func aggregateIsStable() {
        // Result must be ordered by tool name so doctor output is
        // deterministic across runs.
        let floors = [
            Floor(tool: "swtpm", minimumVersion: "0.10.0", source: "a.sh"),
            Floor(tool: "tart", minimumVersion: "2.0.0", source: "b.sh"),
            Floor(tool: "qemu-system-aarch64", minimumVersion: "8.0.0", source: "c.sh"),
        ]
        let aggregated = Check.aggregateFloors(floors)
        #expect(aggregated.map(\.tool) == ["qemu-system-aarch64", "swtpm", "tart"])
    }

    // MARK: - classify

    @Test func classifyAtFloorIsOK() {
        let floors = [Floor(tool: "tart", minimumVersion: "2.5.0", source: "/x/foo.sh")]
        let result = Check.classify(floors: floors, hostVersions: ["tart": "2.5.0"])
        #expect(result.perTool.count == 1)
        if case let .ok(tool, hostVersion, declaredMinimum, _) = result.perTool[0] {
            #expect(tool == "tart")
            #expect(hostVersion == "2.5.0")
            #expect(declaredMinimum == "2.5.0")
        } else {
            #expect(Bool(false), "expected .ok, got \(result.perTool[0])")
        }
        #expect(result.isOK)
    }

    @Test func classifyAboveFloorIsOK() {
        let floors = [Floor(tool: "tart", minimumVersion: "2.5.0", source: "/x/foo.sh")]
        // "2.32.1" must be treated as > "2.5.0" via numeric component
        // comparison; lexicographic would call it less-than.
        let result = Check.classify(floors: floors, hostVersions: ["tart": "2.32.1"])
        guard case .ok = result.perTool[0] else {
            #expect(Bool(false), "expected .ok, got \(result.perTool[0])")
            return
        }
    }

    @Test func classifyBelowFloorIsBelow() {
        let floors = [Floor(tool: "tart", minimumVersion: "2.5.0", source: "/x/foo.sh")]
        let result = Check.classify(floors: floors, hostVersions: ["tart": "2.0.0"])
        if case let .belowFloor(tool, hostVersion, declaredMinimum, source) = result.perTool[0] {
            #expect(tool == "tart")
            #expect(hostVersion == "2.0.0")
            #expect(declaredMinimum == "2.5.0")
            #expect(source == "/x/foo.sh")
        } else {
            #expect(Bool(false), "expected .belowFloor, got \(result.perTool[0])")
        }
        // Below-floor stays advisory: doctor still passes.
        #expect(result.isOK)
    }

    @Test func classifyHostVersionAbsentSurfacesUnknown() {
        let floors = [Floor(tool: "tart", minimumVersion: "2.5.0", source: "/x/foo.sh")]
        // Two ways the host version can be absent: missing key, or
        // explicit nil for the key. Both must produce the same verdict.
        let resultMissing = Check.classify(floors: floors, hostVersions: [:])
        let resultExplicitNil = Check.classify(floors: floors, hostVersions: ["tart": nil])
        for result in [resultMissing, resultExplicitNil] {
            if case let .hostVersionUnknown(tool, declaredMinimum, _) = result.perTool[0] {
                #expect(tool == "tart")
                #expect(declaredMinimum == "2.5.0")
            } else {
                #expect(Bool(false), "expected .hostVersionUnknown, got \(result.perTool[0])")
            }
        }
    }

    @Test func classifyUnparseableFloorSurfacesRawValue() {
        let floors = [Floor(tool: "tart", minimumVersion: "latest", source: "/x/foo.sh")]
        let result = Check.classify(floors: floors, hostVersions: ["tart": "2.5.0"])
        if case let .unparseable(tool, rawValue, source) = result.perTool[0] {
            #expect(tool == "tart")
            #expect(rawValue == "latest")
            #expect(source == "/x/foo.sh")
        } else {
            #expect(Bool(false), "expected .unparseable, got \(result.perTool[0])")
        }
        #expect(result.isOK)
    }

    @Test func classifyEmptyFloorsListProducesEmptyResult() {
        // No sentinels anywhere = no floors to enforce. Result is
        // empty (not skipped — that's the no-bundle case).
        let result = Check.classify(floors: [], hostVersions: ["tart": "2.5.0"])
        #expect(result.perTool.isEmpty)
        #expect(!result.skipped)
        #expect(result.isOK)
    }

    @Test func classifyWiresMultipleToolsIndependently() {
        // Mix of below/at/above floors across different tools — each
        // tool's verdict must be evaluated on its own merits, not
        // contaminated by another tool's outcome.
        let floors = [
            Floor(tool: "tart", minimumVersion: "2.5.0", source: "/a.sh"),
            Floor(tool: "qemu-system-aarch64", minimumVersion: "8.0.0", source: "/b.sh"),
            Floor(tool: "swtpm", minimumVersion: "0.10.0", source: "/c.sh"),
        ]
        let hostVersions: [String: String?] = [
            "tart": "2.0.0",                  // below
            "qemu-system-aarch64": "11.0.0",  // above
            "swtpm": "0.10.0",                // at
        ]
        let result = Check.classify(floors: floors, hostVersions: hostVersions)
        let byTool = Dictionary(uniqueKeysWithValues: result.perTool.compactMap { v -> (String, Check.ToolVerdict)? in
            switch v {
            case let .ok(tool, _, _, _),
                 let .belowFloor(tool, _, _, _),
                 let .hostVersionUnknown(tool, _, _),
                 let .unparseable(tool, _, _):
                return (tool, v)
            }
        })
        if case .belowFloor = byTool["tart"]! { /* ok */ } else {
            #expect(Bool(false), "tart expected belowFloor")
        }
        if case .ok = byTool["qemu-system-aarch64"]! { /* ok */ } else {
            #expect(Bool(false), "qemu expected ok")
        }
        if case .ok = byTool["swtpm"]! { /* ok */ } else {
            #expect(Bool(false), "swtpm expected ok")
        }
    }

    // MARK: - skippedResult

    @Test func skippedResultIsOK() {
        let result = Check.skippedResult()
        #expect(result.skipped)
        #expect(result.perTool.isEmpty)
        #expect(result.isOK)
    }

    // MARK: - isDottedVersion

    @Test func isDottedVersionAcceptsDottedNumeric() {
        #expect(Check.isDottedVersion("2.0.0"))
        #expect(Check.isDottedVersion("2.32.1"))
        #expect(Check.isDottedVersion("2.5"))
    }

    @Test func isDottedVersionRejectsNonDotted() {
        // Pure-number tokens like a copyright year must be rejected
        // so the parser never confuses them with versions.
        #expect(!Check.isDottedVersion("2026"))
        #expect(!Check.isDottedVersion("latest"))
        #expect(!Check.isDottedVersion(""))
        #expect(!Check.isDottedVersion(".5.0"))
    }

    // MARK: - parseSwiftVersion

    @Test func parseSwiftVersionExtractsCompilerVersionFromStdoutOnly() {
        // What the swift probe actually sees (stderr discarded by the
        // probe pipe). Anchoring on "Apple Swift version" extracts the
        // compiler version even though it's the first dotted token.
        let raw = """
        Apple Swift version 6.3.1 (swiftlang-6.3.1.1.2 clang-2100.0.123.102)
        Target: arm64-apple-macosx26.0
        """
        #expect(Check.parseSwiftVersion(from: raw) == "6.3.1")
    }

    @Test func parseSwiftVersionIgnoresSwiftDriverPrefix() {
        // Defence against a future swift release that moves the
        // `swift-driver version: ...` shim back to stdout — the marker
        // anchor still picks the compiler version, not the driver one.
        let raw = "swift-driver version: 1.95 Apple Swift version 6.1.2 (swift-6.1.2-RELEASE)"
        #expect(Check.parseSwiftVersion(from: raw) == "6.1.2")
    }

    @Test func parseSwiftVersionAcceptsTwoComponentVersion() {
        // Some toolchain previews omit the patch component; the
        // underlying greedy parser handles this once the marker has
        // anchored the search.
        let raw = "Apple Swift version 6.0 (...)"
        #expect(Check.parseSwiftVersion(from: raw) == "6.0")
    }

    @Test func parseSwiftVersionReturnsNilWhenMarkerAbsent() {
        // No "Apple Swift version" anchor in output → no extraction;
        // the caller will surface this as `hostVersionUnknown` rather
        // than misreport a stale token from a different prefix.
        #expect(Check.parseSwiftVersion(from: "swift-driver version: 1.95") == nil)
        #expect(Check.parseSwiftVersion(from: "") == nil)
    }

    // MARK: - probeCustomizations

    @Test func probeCustomizationsCoverSwiftAndDotnet() {
        // The table is the canonical place where sentinel-declared
        // tools opt out of the universal `<tool> --version` + greedy
        // parser path. Tools covered by ToolAvailabilityCheck
        // (tart/qemu/swtpm) intentionally do NOT appear here.
        let table = Check.probeCustomizations
        #expect(table["swift"] != nil)
        #expect(table["dotnet"] != nil)
        #expect(table["tart"] == nil)
        #expect(table["qemu-system-aarch64"] == nil)
    }

    @Test func swiftCustomizationParsesActualSwiftOutput() {
        // End-to-end sanity check that the table entry is wired to the
        // anchored parser, not the greedy default.
        guard let custom = Check.probeCustomizations["swift"] else {
            #expect(Bool(false), "swift customization missing")
            return
        }
        #expect(custom.arguments == ["--version"])
        let raw = "Apple Swift version 6.3.1 (swiftlang-6.3.1.1.2)"
        #expect(custom.parser(raw) == "6.3.1")
    }

    @Test func dotnetCustomizationUsesGreedyParser() {
        // dotnet's `--version` already prints a clean dotted token —
        // the customization is a thin wrapper around the shared parser.
        guard let custom = Check.probeCustomizations["dotnet"] else {
            #expect(Bool(false), "dotnet customization missing")
            return
        }
        #expect(custom.arguments == ["--version"])
        #expect(custom.parser("9.0.100") == "9.0.100")
        #expect(custom.parser("10.0.203\n") == "10.0.203")
    }

    // MARK: - locateReleaseBuildScript

    /// Build a disposable directory tree mirroring a dev source tree:
    ///   <root>/scripts/release-build.sh
    ///   <root>/cli/.build/release/testanyware
    /// Returns (root, binaryPath, scriptPath). Caller is responsible
    /// for cleaning up `root` via `try? FileManager.default.removeItem(at: root)`.
    private func makeFakeSourceTree() throws -> (root: URL, binary: URL, script: URL) {
        let fm = FileManager.default
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("provisioner-scripts-version-check-\(UUID().uuidString)")
        let scriptsDir = root.appendingPathComponent("scripts")
        let binDir = root.appendingPathComponent("cli/.build/release")
        try fm.createDirectory(at: scriptsDir, withIntermediateDirectories: true)
        try fm.createDirectory(at: binDir, withIntermediateDirectories: true)
        let script = scriptsDir.appendingPathComponent("release-build.sh")
        try "# stub".write(to: script, atomically: true, encoding: .utf8)
        let binary = binDir.appendingPathComponent("testanyware")
        try Data().write(to: binary)
        return (root, binary, script)
    }

    @Test func locateReleaseBuildScriptFindsItAboveBinary() throws {
        let tree = try makeFakeSourceTree()
        defer { try? FileManager.default.removeItem(at: tree.root) }

        let resolved = Check.locateReleaseBuildScript(near: tree.binary.path)
        // Compare via standardized URL paths so /var vs /private/var
        // symlinks on macOS don't false-positive a mismatch.
        let expected = tree.script.resolvingSymlinksInPath().path
        let actual = resolved.map { URL(fileURLWithPath: $0).resolvingSymlinksInPath().path }
        #expect(actual == expected)
    }

    @Test func locateReleaseBuildScriptReturnsNilWhenAbsent() throws {
        let fm = FileManager.default
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("provisioner-scripts-version-check-empty-\(UUID().uuidString)")
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        let binary = root.appendingPathComponent("testanyware")
        try Data().write(to: binary)

        // No `scripts/release-build.sh` exists above this binary.
        // Without the bound, the walk would otherwise climb out of the
        // tmpdir into /private/tmp → /private → / and could pick up an
        // unrelated file outside the tmpdir.
        let resolved = Check.locateReleaseBuildScript(near: binary.path)
        #expect(resolved == nil)
    }

    @Test func locateReleaseBuildScriptStopsAtFilesystemRoot() {
        // Root-adjacent path with no script anywhere above must not
        // hang or return a stale value.
        let resolved = Check.locateReleaseBuildScript(near: "/nonexistent/testanyware")
        #expect(resolved == nil)
    }
}
