import ArgumentParser
import Foundation
import TestAnywareDriver

/// VM lifecycle command group. All four subcommands delegate to
/// `VMLifecycle` / `TartRunner` / `QEMURunner` — bash wrappers in
/// `scripts/macos/vm-*.sh` simply `exec` through to this binary.
struct VMCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "vm",
        abstract: "VM lifecycle: start, stop, list, delete",
        subcommands: [
            Start.self,
            Stop.self,
            List.self,
            Delete.self,
        ]
    )

    struct Start: AsyncParsableCommand {
        static let configuration = CommandConfiguration(
            commandName: "start",
            abstract: "Start a VM and register its spec"
        )

        @Option(name: .long, help: "Target platform: macos, linux, windows")
        var platform: String = "macos"

        @Option(name: .long, help: "Base image to clone from")
        var base: String?

        @Option(name: .long, help: "VM instance id (default: testanyware-<hex8>)")
        var id: String?

        @Option(name: .long, help: "Display resolution (e.g. 1920x1080)")
        var display: String?

        @Flag(name: .long, help: "Open a VNC viewer after boot")
        var viewer: Bool = false

        @Flag(name: .long, help: "Skip waiting for SSH (accepted, ignored — deprecated)")
        var noSsh: Bool = false

        func run() async throws {
            if noSsh {
                FileHandle.standardError.write(Data(
                    "NOTE: --no-ssh is accepted but ignored; will be removed when SSH is disabled in the goldens.\n".utf8
                ))
            }
            guard let resolvedPlatform = Platform(rawValue: platform) else {
                throw ValidationError("Unknown platform '\(platform)'. Must be macos, linux, or windows.")
            }
            let options = VMStartOptions(
                platform: resolvedPlatform,
                base: base,
                id: id,
                display: display,
                openViewer: viewer
            )
            let result = try await VMLifecycle.start(options: options)
            print(result.id)
        }
    }

    struct Stop: AsyncParsableCommand {
        static let configuration = CommandConfiguration(
            commandName: "stop",
            abstract: "Stop a VM and remove its spec"
        )

        @Argument(help: "VM instance id (falls back to TESTANYWARE_VM_ID)")
        var id: String?

        func run() async throws {
            let resolved = id ?? ProcessInfo.processInfo.environment["TESTANYWARE_VM_ID"]
            guard let resolved, !resolved.isEmpty else {
                throw ValidationError(
                    "VM id required. Pass it as an argument or set TESTANYWARE_VM_ID."
                )
            }
            try VMLifecycle.stop(id: resolved)
        }
    }

    struct List: AsyncParsableCommand {
        static let configuration = CommandConfiguration(
            commandName: "list",
            abstract: "List golden images and running clones"
        )

        func run() async throws {
            let paths = VMPaths()
            let tartEntries = (try? TartRunner.runList()) ?? []
            var goldens = tartEntries.filter { $0.kind == .golden }
            var running = tartEntries.filter { $0.kind == .running }
            goldens.append(contentsOf: (try? QEMURunner.scanGoldenDir(path: paths.goldenDir)) ?? [])
            running.append(contentsOf: (try? QEMURunner.scanClonesDir(path: paths.clonesDir)) ?? [])
            print(VMListFormatter.render(goldens: goldens, running: running))
        }
    }

    struct Delete: AsyncParsableCommand {
        static let configuration = CommandConfiguration(
            commandName: "delete",
            abstract: "Delete a golden image"
        )

        @Argument(help: "Golden image name (run 'testanyware vm list' to see available images)")
        var name: String

        @Flag(name: .long, help: "Delete even if running clones appear to depend on the image")
        var force: Bool = false

        func run() async throws {
            try VMLifecycle.delete(name: name, force: force)
        }
    }
}
