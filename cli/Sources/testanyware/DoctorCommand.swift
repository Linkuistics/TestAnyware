import ArgumentParser
import Foundation
import TestAnywareDriver

struct DoctorCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "doctor",
        abstract: "Diagnose the testanyware install (PATH, bundled agents, host tools)"
    )

    func run() async throws {
        let install = InstallPathCheck.run()
        let bundled = BundledAgentsCheck.run()
        let tools = ToolAvailabilityCheck.run()

        print("testanyware doctor")
        print("")

        print("Install path")
        print("  running binary:  \(install.runningBinary)")
        printInstallVerdict(install.verdict)
        print("")

        print("Bundled agents")
        printBundledVerdict(bundled.verdict)
        print("")

        print("Host tools")
        printToolStatuses(tools.statuses)

        if !install.isOK || !bundled.isOK || !tools.isOK {
            throw ExitCode.failure
        }
    }

    private func printInstallVerdict(_ verdict: InstallPathCheck.Verdict) {
        switch verdict {
        case let .homebrewInstall(path, brewPrefix):
            print("  on PATH:         \(path)")
            print("  Homebrew prefix: \(brewPrefix)")
            print("  ✓ install path is under Homebrew prefix")
        case let .shadowed(path, brewPrefix):
            print("  on PATH:         \(path)")
            print("  Homebrew prefix: \(brewPrefix)")
            print("  ✗ \(path) shadows the Homebrew install at \(brewPrefix)/bin/testanyware")
            print("    remediation: sudo rm \(path)")
            print("                 (created during local dev; no longer needed)")
        case let .noHomebrew(path):
            print("  on PATH:         \(path)")
            print("  Homebrew prefix: not found")
            print("  ! Homebrew is not installed; cannot verify install layout")
            print("    install hint: /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"")
        case let .notOnPath(brewPrefix):
            print("  on PATH:         (not found)")
            if let brewPrefix {
                print("  Homebrew prefix: \(brewPrefix)")
                print("  ✗ testanyware is not on PATH; expected \(brewPrefix)/bin/testanyware")
                print("    remediation: brew install Linkuistics/taps/testanyware")
            } else {
                print("  Homebrew prefix: not found")
                print("  ✗ testanyware is not on PATH and Homebrew is not installed")
            }
        }
    }

    private func printBundledVerdict(_ verdict: BundledAgentsCheck.Verdict) {
        switch verdict {
        case let .allPresent(brewPrefix):
            print("  bundle root:     \(brewPrefix)/share/testanyware/agents")
            print("  ✓ macOS, Windows, and Linux agents all present")
        case .noHomebrew:
            print("  bundle root:     (skipped — Homebrew not installed)")
        case let .missing(brewPrefix, issues):
            print("  bundle root:     \(brewPrefix)/share/testanyware/agents")
            for (slot, issue) in issues {
                switch issue {
                case let .missing(path):
                    print("  ✗ \(slot.rawValue) agent missing: \(path)")
                case let .notExecutable(path):
                    print("  ✗ \(slot.rawValue) agent not executable: \(path)")
                }
            }
            print("    remediation: brew reinstall Linkuistics/taps/testanyware")
        }
    }

    private func printToolStatuses(_ statuses: [ToolAvailabilityCheck.Status]) {
        for status in statuses {
            if let path = status.path {
                print("  ✓ \(status.tool.name) — \(path)")
            } else {
                print("  ! \(status.tool.name) — not found (\(status.tool.purpose))")
                print("    install hint: \(status.tool.installHint)")
            }
        }
    }
}
