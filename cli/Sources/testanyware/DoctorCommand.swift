import ArgumentParser
import Foundation
import TestAnywareDriver

struct DoctorCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "doctor",
        abstract: "Diagnose the testanyware install (PATH shadowing, Homebrew layout)"
    )

    func run() async throws {
        let check = InstallPathCheck.run()

        print("testanyware doctor")
        print("  running binary:  \(check.runningBinary)")
        printVerdict(check.verdict)

        if !check.isOK {
            throw ExitCode.failure
        }
    }

    private func printVerdict(_ verdict: InstallPathCheck.Verdict) {
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
}
