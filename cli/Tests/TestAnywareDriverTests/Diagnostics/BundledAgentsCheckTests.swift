import Testing
@testable import TestAnywareDriver

@Suite("BundledAgentsCheck.classify")
struct BundledAgentsCheckTests {

    private func goodProbes(brewPrefix: String) -> [BundledAgentsCheck.SlotProbe] {
        return [BundledAgentsCheck.AgentSlot.macos, .windows, .linux].map { slot in
            BundledAgentsCheck.SlotProbe(
                slot: slot,
                expectedPath: BundledAgentsCheck.expectedPath(brewPrefix: brewPrefix, slot: slot),
                exists: true,
                isExecutable: slot == .macos
            )
        }
    }

    @Test func allPresentWhenEveryProbeIsHealthy() {
        let probes = goodProbes(brewPrefix: "/opt/homebrew")
        let verdict = BundledAgentsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .allPresent(brewPrefix: "/opt/homebrew"))
    }

    @Test func noHomebrewSkipsTheCheck() {
        let verdict = BundledAgentsCheck.classify(brewPrefix: nil, probes: [])
        #expect(verdict == .noHomebrew)
    }

    @Test func missingMacosBinaryIsActionable() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        probes[0] = BundledAgentsCheck.SlotProbe(
            slot: .macos,
            expectedPath: probes[0].expectedPath,
            exists: false,
            isExecutable: false
        )
        let verdict = BundledAgentsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        let expectedPath = BundledAgentsCheck.expectedPath(brewPrefix: "/opt/homebrew", slot: .macos)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [(.macos, .missing(path: expectedPath))]
        ))
    }

    @Test func macosBinaryPresentButNotExecutableIsActionable() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        probes[0] = BundledAgentsCheck.SlotProbe(
            slot: .macos,
            expectedPath: probes[0].expectedPath,
            exists: true,
            isExecutable: false
        )
        let verdict = BundledAgentsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        let expectedPath = BundledAgentsCheck.expectedPath(brewPrefix: "/opt/homebrew", slot: .macos)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [(.macos, .notExecutable(path: expectedPath))]
        ))
    }

    @Test func windowsExePresenceIsCheckedButNotItsExecutableBit() {
        // The Windows .exe is a win-arm64 binary; it won't carry the macOS
        // executable bit and we should not assert one. Presence-only.
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        probes[1] = BundledAgentsCheck.SlotProbe(
            slot: .windows,
            expectedPath: probes[1].expectedPath,
            exists: true,
            isExecutable: false
        )
        let verdict = BundledAgentsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .allPresent(brewPrefix: "/opt/homebrew"))
    }

    @Test func linuxPackageMarkerPresenceIsCheckedButNotItsExecutableBit() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        probes[2] = BundledAgentsCheck.SlotProbe(
            slot: .linux,
            expectedPath: probes[2].expectedPath,
            exists: true,
            isExecutable: false
        )
        let verdict = BundledAgentsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .allPresent(brewPrefix: "/opt/homebrew"))
    }

    @Test func multipleMissingPlatformsAccumulate() {
        let prefix = "/opt/homebrew"
        let probes: [BundledAgentsCheck.SlotProbe] = [
            BundledAgentsCheck.SlotProbe(
                slot: .macos,
                expectedPath: BundledAgentsCheck.expectedPath(brewPrefix: prefix, slot: .macos),
                exists: false,
                isExecutable: false
            ),
            BundledAgentsCheck.SlotProbe(
                slot: .windows,
                expectedPath: BundledAgentsCheck.expectedPath(brewPrefix: prefix, slot: .windows),
                exists: true,
                isExecutable: false
            ),
            BundledAgentsCheck.SlotProbe(
                slot: .linux,
                expectedPath: BundledAgentsCheck.expectedPath(brewPrefix: prefix, slot: .linux),
                exists: false,
                isExecutable: false
            ),
        ]
        let verdict = BundledAgentsCheck.classify(brewPrefix: prefix, probes: probes)
        #expect(verdict == .missing(
            brewPrefix: prefix,
            issues: [
                (.macos, .missing(path: BundledAgentsCheck.expectedPath(brewPrefix: prefix, slot: .macos))),
                (.linux, .missing(path: BundledAgentsCheck.expectedPath(brewPrefix: prefix, slot: .linux))),
            ]
        ))
    }

    @Test func expectedPathsMatchTheReleaseBundleLayout() {
        // These paths must stay in lockstep with scripts/release-build.sh,
        // which stages the bundle under share/testanyware/agents/<platform>/.
        #expect(
            BundledAgentsCheck.expectedPath(brewPrefix: "/opt/homebrew", slot: .macos)
                == "/opt/homebrew/share/testanyware/agents/macos/testanyware-agent"
        )
        #expect(
            BundledAgentsCheck.expectedPath(brewPrefix: "/opt/homebrew", slot: .windows)
                == "/opt/homebrew/share/testanyware/agents/windows/testanyware-agent.exe"
        )
        #expect(
            BundledAgentsCheck.expectedPath(brewPrefix: "/opt/homebrew", slot: .linux)
                == "/opt/homebrew/share/testanyware/agents/linux/testanyware_agent/__main__.py"
        )
    }

    @Test func checkResultIsOKForBenignVerdicts() {
        let allPresent = BundledAgentsCheck.CheckResult(
            verdict: .allPresent(brewPrefix: "/opt/homebrew")
        )
        #expect(allPresent.isOK)

        let noBrew = BundledAgentsCheck.CheckResult(verdict: .noHomebrew)
        #expect(noBrew.isOK)
    }

    @Test func checkResultIsNotOKWhenAnyAgentIsMissing() {
        let result = BundledAgentsCheck.CheckResult(
            verdict: .missing(
                brewPrefix: "/opt/homebrew",
                issues: [(.macos, .missing(path: "/opt/homebrew/share/testanyware/agents/macos/testanyware-agent"))]
            )
        )
        #expect(!result.isOK)
    }
}
