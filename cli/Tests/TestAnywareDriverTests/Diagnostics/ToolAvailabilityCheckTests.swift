import Testing
@testable import TestAnywareDriver

@Suite("ToolAvailabilityCheck.classify")
struct ToolAvailabilityCheckTests {

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
}
