import Testing
@testable import TestAnywareDriver

@Suite("BundledScriptsCheck.classify")
struct BundledScriptsCheckTests {

    private func goodProbes(brewPrefix: String) -> [BundledScriptsCheck.FileProbe] {
        var probes: [BundledScriptsCheck.FileProbe] = []
        for path in BundledScriptsCheck.expectedScriptPaths(brewPrefix: brewPrefix) {
            probes.append(BundledScriptsCheck.FileProbe(
                slot: .scripts,
                path: path,
                exists: true,
                isExecutable: true,
                requiresExecutable: true
            ))
        }
        for path in BundledScriptsCheck.expectedHelperPaths(brewPrefix: brewPrefix) {
            probes.append(BundledScriptsCheck.FileProbe(
                slot: .helpers,
                path: path,
                exists: true,
                isExecutable: false,
                requiresExecutable: false
            ))
        }
        return probes
    }

    @Test func allPresentWhenEveryProbeIsHealthy() {
        let probes = goodProbes(brewPrefix: "/opt/homebrew")
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .allPresent(brewPrefix: "/opt/homebrew"))
    }

    @Test func noHomebrewSkipsTheCheck() {
        let verdict = BundledScriptsCheck.classify(brewPrefix: nil, probes: [])
        #expect(verdict == .noHomebrew)
    }

    @Test func missingScriptIsActionable() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        let originalPath = probes[0].path
        probes[0] = BundledScriptsCheck.FileProbe(
            slot: .scripts,
            path: originalPath,
            exists: false,
            isExecutable: false,
            requiresExecutable: true
        )
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [(.scripts, .missing(path: originalPath))]
        ))
    }

    @Test func scriptPresentButNotExecutableIsActionable() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        let originalPath = probes[0].path
        probes[0] = BundledScriptsCheck.FileProbe(
            slot: .scripts,
            path: originalPath,
            exists: true,
            isExecutable: false,
            requiresExecutable: true
        )
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [(.scripts, .notExecutable(path: originalPath))]
        ))
    }

    @Test func helperPresentWithoutExecutableBitIsAcceptable() {
        // Helpers (XML, plist, .ps1, .cmd) carry varying modes. The
        // check must not insist on the executable bit for them.
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        let helperIndex = probes.firstIndex { $0.slot == .helpers }!
        let originalPath = probes[helperIndex].path
        probes[helperIndex] = BundledScriptsCheck.FileProbe(
            slot: .helpers,
            path: originalPath,
            exists: true,
            isExecutable: false,
            requiresExecutable: false
        )
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .allPresent(brewPrefix: "/opt/homebrew"))
    }

    @Test func missingHelperIsActionable() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        let helperIndex = probes.firstIndex { $0.slot == .helpers }!
        let originalPath = probes[helperIndex].path
        probes[helperIndex] = BundledScriptsCheck.FileProbe(
            slot: .helpers,
            path: originalPath,
            exists: false,
            isExecutable: false,
            requiresExecutable: false
        )
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [(.helpers, .missing(path: originalPath))]
        ))
    }

    @Test func multipleIssuesAccumulateInSlotOrder() {
        var probes = goodProbes(brewPrefix: "/opt/homebrew")
        let scriptPath = probes[0].path
        probes[0] = BundledScriptsCheck.FileProbe(
            slot: .scripts,
            path: scriptPath,
            exists: false,
            isExecutable: false,
            requiresExecutable: true
        )
        let helperIndex = probes.firstIndex { $0.slot == .helpers }!
        let helperPath = probes[helperIndex].path
        probes[helperIndex] = BundledScriptsCheck.FileProbe(
            slot: .helpers,
            path: helperPath,
            exists: false,
            isExecutable: false,
            requiresExecutable: false
        )
        let verdict = BundledScriptsCheck.classify(brewPrefix: "/opt/homebrew", probes: probes)
        #expect(verdict == .missing(
            brewPrefix: "/opt/homebrew",
            issues: [
                (.scripts, .missing(path: scriptPath)),
                (.helpers, .missing(path: helperPath)),
            ]
        ))
    }

    @Test func expectedScriptPathsMatchTheReleaseBundleLayout() {
        // These filenames must stay in lockstep with
        // scripts/release-build.sh#stage_scripts, which copies
        // _testanyware-paths.sh and vm-*.sh into
        // share/testanyware/scripts/.
        let paths = BundledScriptsCheck.expectedScriptPaths(brewPrefix: "/opt/homebrew")
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/_testanyware-paths.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-start.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-stop.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-list.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-delete.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-create-golden-macos.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-create-golden-linux.sh"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/scripts/vm-create-golden-windows.sh"))
        #expect(paths.count == 8)
    }

    @Test func expectedHelperPathsMatchTheReleaseBundleLayout() {
        // Mirrors scripts/release-build.sh#stage_helpers
        // (cp -R provisioner/helpers/. → share/testanyware/helpers/).
        let paths = BundledScriptsCheck.expectedHelperPaths(brewPrefix: "/opt/homebrew")
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/autounattend.xml"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/com.linkuistics.testanyware.agent.plist"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/desktop-setup.ps1"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/set-wallpaper.ps1"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/set-wallpaper.swift"))
        #expect(paths.contains("/opt/homebrew/share/testanyware/helpers/SetupComplete.cmd"))
        #expect(paths.count == 6)
    }

    @Test func checkResultIsOKForBenignVerdicts() {
        let allPresent = BundledScriptsCheck.CheckResult(
            verdict: .allPresent(brewPrefix: "/opt/homebrew")
        )
        #expect(allPresent.isOK)

        let noBrew = BundledScriptsCheck.CheckResult(verdict: .noHomebrew)
        #expect(noBrew.isOK)
    }

    @Test func checkResultIsNotOKWhenAnyFileIsMissing() {
        let result = BundledScriptsCheck.CheckResult(
            verdict: .missing(
                brewPrefix: "/opt/homebrew",
                issues: [(.scripts, .missing(path: "/opt/homebrew/share/testanyware/scripts/vm-start.sh"))]
            )
        )
        #expect(!result.isOK)
    }
}
