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
        let scripts = BundledScriptsCheck.run()
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

        print("Bundled scripts and helpers")
        printScriptsVerdict(scripts.verdict)
        print("")

        print("Host tools")
        printToolStatuses(tools.statuses)

        if !install.isOK || !bundled.isOK || !scripts.isOK || !tools.isOK {
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

    private func printScriptsVerdict(_ verdict: BundledScriptsCheck.Verdict) {
        switch verdict {
        case let .allPresent(brewPrefix):
            print("  scripts root:    \(brewPrefix)/share/testanyware/scripts")
            print("  helpers root:    \(brewPrefix)/share/testanyware/helpers")
            print("  ✓ all 8 provisioner scripts and 6 helpers present")
        case .noHomebrew:
            print("  scripts root:    (skipped — Homebrew not installed)")
        case let .missing(brewPrefix, issues):
            print("  scripts root:    \(brewPrefix)/share/testanyware/scripts")
            print("  helpers root:    \(brewPrefix)/share/testanyware/helpers")
            for (slot, issue) in issues {
                switch issue {
                case let .missing(path):
                    print("  ✗ \(slot.rawValue) file missing: \(path)")
                case let .notExecutable(path):
                    print("  ✗ \(slot.rawValue) file not executable: \(path)")
                }
            }
            print("    remediation: brew reinstall Linkuistics/taps/testanyware")
        }
    }

    private func printToolStatuses(_ statuses: [ToolAvailabilityCheck.Status]) {
        for status in statuses {
            guard let path = status.path else {
                print("  ! \(status.tool.name) — not found (\(status.tool.purpose))")
                print("    install hint: \(status.tool.installHint)")
                continue
            }
            switch status.versionVerdict {
            case .ok(let detected):
                if let detected {
                    print("  ✓ \(status.tool.name) \(detected) — \(path)")
                } else {
                    print("  ✓ \(status.tool.name) — \(path)")
                }
            case let .belowFloor(detected, minimum):
                print("  ! \(status.tool.name) \(detected) — \(path)")
                print("    below supported floor (\(minimum)); upgrade with: \(status.tool.installHint)")
            case let .unparseable(rawOutput, minimum):
                print("  ! \(status.tool.name) — \(path)")
                print("    could not parse --version output (expected ≥ \(minimum)); raw: \(rawOutput)")
            case let .probeFailed(minimum):
                print("  ! \(status.tool.name) — \(path)")
                print("    --version probe produced no output; cannot verify ≥ \(minimum)")
            }
        }
    }
}
