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
}
