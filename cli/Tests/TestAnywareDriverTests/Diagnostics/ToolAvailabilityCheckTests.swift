import Testing
@testable import TestAnywareDriver

@Suite("ToolAvailabilityCheck.classify")
struct ToolAvailabilityCheckTests {

    // MARK: - Presence-only behaviour (preserved when no versionProbe is supplied)

    @Test func everyToolPresent() {
        let result = ToolAvailabilityCheck.classify { name in
            switch name {
            case "tart": return "/opt/homebrew/bin/tart"
            case "qemu-system-aarch64": return "/opt/homebrew/bin/qemu-system-aarch64"
            case "swtpm": return "/opt/homebrew/bin/swtpm"
            default: return nil
            }
        }
        #expect(result.statuses.count == 3)
        #expect(result.statuses.allSatisfy { $0.isAvailable })
        #expect(result.isOK)
    }

    @Test func everyToolMissing() {
        let result = ToolAvailabilityCheck.classify { _ in nil }
        #expect(result.statuses.count == 3)
        #expect(result.statuses.allSatisfy { !$0.isAvailable })
        // Missing tools are advisory — the doctor does not fail on them.
        #expect(result.isOK)
    }

    @Test func partialAvailabilityIsReportedPerTool() {
        let result = ToolAvailabilityCheck.classify { name in
            name == "tart" ? "/opt/homebrew/bin/tart" : nil
        }
        let byName = Dictionary(uniqueKeysWithValues: result.statuses.map { ($0.tool.name, $0) })
        #expect(byName["tart"]?.path == "/opt/homebrew/bin/tart")
        #expect(byName["qemu-system-aarch64"]?.path == nil)
        #expect(byName["swtpm"]?.path == nil)
        #expect(result.isOK)
    }

    @Test func reportingOrderMatchesKnownToolsOrder() {
        // Ensures the doctor's output order is stable and matches the order
        // declared in `knownTools` — tart first (most common), then the
        // Windows-only pair.
        let result = ToolAvailabilityCheck.classify { _ in nil }
        #expect(result.statuses.map(\.tool.name) == ["tart", "qemu-system-aarch64", "swtpm"])
    }

    @Test func eachToolCarriesAnInstallHint() {
        // The hints are what the doctor prints for users to fix a missing
        // tool, so they must be non-empty for every known tool.
        for tool in ToolAvailabilityCheck.knownTools {
            #expect(!tool.installHint.isEmpty)
            #expect(!tool.purpose.isEmpty)
        }
    }

    // MARK: - Version-floor metadata

    @Test func tartAndQemuCarryVersionFloors() {
        // The two tools the provisioner has version-specific contracts
        // with carry an explicit minimumVersion. swtpm doesn't, since
        // swtpm releases don't break the QEMU integration.
        let byName = Dictionary(
            uniqueKeysWithValues: ToolAvailabilityCheck.knownTools.map { ($0.name, $0) }
        )
        #expect(byName["tart"]?.minimumVersion != nil)
        #expect(byName["qemu-system-aarch64"]?.minimumVersion != nil)
        #expect(byName["swtpm"]?.minimumVersion == nil)
    }

    // MARK: - parseVersion

    @Test func parseVersionExtractsTartFormat() {
        #expect(ToolAvailabilityCheck.parseVersion(from: "2.32.1") == "2.32.1")
        #expect(ToolAvailabilityCheck.parseVersion(from: "2.32.1\n") == "2.32.1")
    }

    @Test func parseVersionExtractsQemuFormat() {
        let raw = "QEMU emulator version 11.0.0\nCopyright (c) 2003-2026 Fabrice Bellard"
        #expect(ToolAvailabilityCheck.parseVersion(from: raw) == "11.0.0")
    }

    @Test func parseVersionExtractsSwtpmFormat() {
        // swtpm prints `TPM emulator version 0.10.0, ...`. The parser
        // must skip prose words and grab the first dotted-numeric run.
        #expect(ToolAvailabilityCheck.parseVersion(from: "TPM emulator version 0.10.0, ...") == "0.10.0")
    }

    @Test func parseVersionIgnoresStandaloneYearTokens() {
        // Years like 2026 in a copyright line should not be returned as
        // a version (no dot), and should not prevent finding the real
        // dotted version that follows.
        let raw = "Copyright 2026 some vendor\nversion 3.4.5"
        #expect(ToolAvailabilityCheck.parseVersion(from: raw) == "3.4.5")
    }

    @Test func parseVersionReturnsNilWhenNoDottedNumber() {
        #expect(ToolAvailabilityCheck.parseVersion(from: "no version here") == nil)
        #expect(ToolAvailabilityCheck.parseVersion(from: "") == nil)
        #expect(ToolAvailabilityCheck.parseVersion(from: "2026") == nil)
    }

    // MARK: - compareVersion

    @Test func compareVersionAtFloorIsOK() {
        let verdict = ToolAvailabilityCheck.compareVersion(rawOutput: "2.0.0", minimum: "2.0.0")
        #expect(verdict == .ok(detected: "2.0.0"))
    }

    @Test func compareVersionAboveFloorIsOK() {
        let verdict = ToolAvailabilityCheck.compareVersion(rawOutput: "2.32.1", minimum: "2.0.0")
        #expect(verdict == .ok(detected: "2.32.1"))
        // Numeric comparison must not be lexicographic — "2.32.1" beats
        // "2.5.0" only when components are compared as numbers.
        let stricter = ToolAvailabilityCheck.compareVersion(rawOutput: "2.32.1", minimum: "2.5.0")
        #expect(stricter == .ok(detected: "2.32.1"))
    }

    @Test func compareVersionBelowFloorIsBelow() {
        let verdict = ToolAvailabilityCheck.compareVersion(rawOutput: "1.9.9", minimum: "2.0.0")
        #expect(verdict == .belowFloor(detected: "1.9.9", minimum: "2.0.0"))
    }

    @Test func compareVersionUnparseableSurfacesRawOutput() {
        let verdict = ToolAvailabilityCheck.compareVersion(rawOutput: "not a version", minimum: "2.0.0")
        #expect(verdict == .unparseable(rawOutput: "not a version", minimum: "2.0.0"))
    }

    @Test func compareVersionProbeFailureIsAdvisory() {
        let verdict = ToolAvailabilityCheck.compareVersion(rawOutput: nil, minimum: "2.0.0")
        #expect(verdict == .probeFailed(minimum: "2.0.0"))
    }

    @Test func compareVersionWithoutFloorAlwaysOK() {
        let withProse = ToolAvailabilityCheck.compareVersion(rawOutput: "TPM emulator version 0.10.0", minimum: nil)
        #expect(withProse == .ok(detected: "0.10.0"))
        let withGarbage = ToolAvailabilityCheck.compareVersion(rawOutput: "garbage", minimum: nil)
        #expect(withGarbage == .ok(detected: nil))
        let withNil = ToolAvailabilityCheck.compareVersion(rawOutput: nil, minimum: nil)
        #expect(withNil == .ok(detected: nil))
    }

    // MARK: - Wiring through classify

    @Test func classifyAttachesVersionVerdictToResolvedTools() {
        let result = ToolAvailabilityCheck.classify(
            resolve: { name in
                switch name {
                case "tart": return "/opt/homebrew/bin/tart"
                case "qemu-system-aarch64": return "/opt/homebrew/bin/qemu-system-aarch64"
                case "swtpm": return "/opt/homebrew/bin/swtpm"
                default: return nil
                }
            },
            versionProbe: { name in
                switch name {
                case "tart": return "2.32.1"
                case "qemu-system-aarch64": return "QEMU emulator version 11.0.0"
                case "swtpm": return "TPM emulator version 0.10.0"
                default: return nil
                }
            }
        )
        let byName = Dictionary(uniqueKeysWithValues: result.statuses.map { ($0.tool.name, $0) })
        #expect(byName["tart"]?.versionVerdict == .ok(detected: "2.32.1"))
        #expect(byName["qemu-system-aarch64"]?.versionVerdict == .ok(detected: "11.0.0"))
        // swtpm has no floor, so detected version is captured for display.
        #expect(byName["swtpm"]?.versionVerdict == .ok(detected: "0.10.0"))
    }

    @Test func classifyFlagsBelowFloorTart() {
        let result = ToolAvailabilityCheck.classify(
            resolve: { _ in "/usr/local/bin/tart" },
            versionProbe: { _ in "1.5.0" },
            tools: [ToolAvailabilityCheck.knownTools[0]]
        )
        #expect(result.statuses.count == 1)
        if case let .belowFloor(detected, minimum) = result.statuses[0].versionVerdict {
            #expect(detected == "1.5.0")
            #expect(minimum == "2.0.0")
        } else {
            #expect(Bool(false), "expected .belowFloor, got \(result.statuses[0].versionVerdict)")
        }
        // Below-floor is advisory — overall result still passes.
        #expect(result.isOK)
    }

    @Test func classifyDoesNotProbeMissingTools() {
        // If a tool is not on PATH, `versionProbe` must not be called
        // (probing a missing binary would either error or invent a
        // version). The verdict for a missing tool is .ok(detected:
        // nil), independent of probe behaviour.
        var probeCalls: [String] = []
        let result = ToolAvailabilityCheck.classify(
            resolve: { _ in nil },
            versionProbe: { name in
                probeCalls.append(name)
                return "9.9.9"
            }
        )
        #expect(probeCalls.isEmpty)
        #expect(result.statuses.allSatisfy { $0.versionVerdict == .ok(detected: nil) })
    }

    @Test func classifySurfacesProbeFailureForResolvedTool() {
        let result = ToolAvailabilityCheck.classify(
            resolve: { _ in "/usr/local/bin/tart" },
            versionProbe: { _ in nil },
            tools: [ToolAvailabilityCheck.knownTools[0]]
        )
        #expect(result.statuses[0].versionVerdict == .probeFailed(minimum: "2.0.0"))
        #expect(result.isOK)
    }
}
