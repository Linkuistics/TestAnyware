import Testing
@testable import TestAnywareDriver

@Suite("InstallPathCheck.classify")
struct InstallPathCheckTests {

    @Test func homebrewInstallOnAppleSilicon() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/opt/homebrew/Cellar/testanyware/1.0.0/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        )
        #expect(verdict == .homebrewInstall(
            path: "/opt/homebrew/Cellar/testanyware/1.0.0/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        ))
    }

    @Test func homebrewInstallOnIntelMac() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/usr/local/Cellar/testanyware/1.0.0/bin/testanyware",
            brewPrefix: "/usr/local"
        )
        #expect(verdict == .homebrewInstall(
            path: "/usr/local/Cellar/testanyware/1.0.0/bin/testanyware",
            brewPrefix: "/usr/local"
        ))
    }

    @Test func devSymlinkShadowsHomebrew() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/usr/local/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        )
        #expect(verdict == .shadowed(
            path: "/usr/local/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        ))
    }

    @Test func swiftBuildArtifactShadowsHomebrew() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/Users/dev/TestAnyware/cli/.build/release/testanyware",
            brewPrefix: "/opt/homebrew"
        )
        #expect(verdict == .shadowed(
            path: "/Users/dev/TestAnyware/cli/.build/release/testanyware",
            brewPrefix: "/opt/homebrew"
        ))
    }

    @Test func adversarialPrefixDoesNotMatchByStringPrefix() {
        // `/opt/homebrew2` must NOT be classified as under `/opt/homebrew`.
        let verdict = InstallPathCheck.classify(
            pathBinary: "/opt/homebrew2/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        )
        #expect(verdict == .shadowed(
            path: "/opt/homebrew2/bin/testanyware",
            brewPrefix: "/opt/homebrew"
        ))
    }

    @Test func brewPrefixWithTrailingSlashStillMatches() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/opt/homebrew/bin/testanyware",
            brewPrefix: "/opt/homebrew/"
        )
        #expect(verdict == .homebrewInstall(
            path: "/opt/homebrew/bin/testanyware",
            brewPrefix: "/opt/homebrew/"
        ))
    }

    @Test func noHomebrewButBinaryOnPath() {
        let verdict = InstallPathCheck.classify(
            pathBinary: "/usr/local/bin/testanyware",
            brewPrefix: nil
        )
        #expect(verdict == .noHomebrew(path: "/usr/local/bin/testanyware"))
    }

    @Test func notOnPathWithHomebrewPresent() {
        let verdict = InstallPathCheck.classify(
            pathBinary: nil,
            brewPrefix: "/opt/homebrew"
        )
        #expect(verdict == .notOnPath(brewPrefix: "/opt/homebrew"))
    }

    @Test func notOnPathWithoutHomebrew() {
        let verdict = InstallPathCheck.classify(pathBinary: nil, brewPrefix: nil)
        #expect(verdict == .notOnPath(brewPrefix: nil))
    }

    @Test func checkResultIsOKForBenignVerdicts() {
        let homebrew = InstallPathCheck.CheckResult(
            verdict: .homebrewInstall(path: "/opt/homebrew/bin/testanyware", brewPrefix: "/opt/homebrew"),
            runningBinary: "/opt/homebrew/bin/testanyware"
        )
        #expect(homebrew.isOK)

        let noBrew = InstallPathCheck.CheckResult(
            verdict: .noHomebrew(path: "/usr/local/bin/testanyware"),
            runningBinary: "/usr/local/bin/testanyware"
        )
        #expect(noBrew.isOK)
    }

    @Test func checkResultIsNotOKForActionableVerdicts() {
        let shadowed = InstallPathCheck.CheckResult(
            verdict: .shadowed(path: "/usr/local/bin/testanyware", brewPrefix: "/opt/homebrew"),
            runningBinary: "/opt/homebrew/bin/testanyware"
        )
        #expect(!shadowed.isOK)

        let missing = InstallPathCheck.CheckResult(
            verdict: .notOnPath(brewPrefix: "/opt/homebrew"),
            runningBinary: "/opt/homebrew/bin/testanyware"
        )
        #expect(!missing.isOK)
    }
}
