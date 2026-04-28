import Foundation

/// Reports whether the host has the third-party CLI tools that
/// `testanyware vm ...` shells out to: `tart` (macOS/Linux VMs),
/// `qemu-system-aarch64` (Windows VMs), and `swtpm` (TPM 2.0 for the
/// Windows VM). All three are advisory — a user who only runs macOS
/// tests has no need for QEMU or swtpm — so missing tools surface in
/// the doctor output but do not flip `isOK` to false.
public enum ToolAvailabilityCheck {

    public struct Tool: Equatable, Sendable {
        public let name: String
        public let purpose: String
        public let installHint: String

        public init(name: String, purpose: String, installHint: String) {
            self.name = name
            self.purpose = purpose
            self.installHint = installHint
        }
    }

    public struct Status: Equatable, Sendable {
        public let tool: Tool
        /// Resolved absolute path, or `nil` if the binary is not on PATH.
        public let path: String?

        public init(tool: Tool, path: String?) {
            self.tool = tool
            self.path = path
        }

        public var isAvailable: Bool { path != nil }
    }

    public struct CheckResult: Equatable {
        public let statuses: [Status]

        public init(statuses: [Status]) {
            self.statuses = statuses
        }

        /// Tool availability is advisory, so the doctor does not fail on
        /// missing tools. The verdict is informational only.
        public var isOK: Bool { true }
    }

    /// The three tools the doctor reports on, in display order.
    public static let knownTools: [Tool] = [
        Tool(
            name: "tart",
            purpose: "macOS and Linux VMs",
            installHint: "brew install cirruslabs/cli/tart"
        ),
        Tool(
            name: "qemu-system-aarch64",
            purpose: "Windows VMs",
            installHint: "brew install qemu"
        ),
        Tool(
            name: "swtpm",
            purpose: "TPM 2.0 emulation for Windows 11 VMs",
            installHint: "brew install swtpm"
        ),
    ]

    /// Pure classifier. Takes a resolver that maps a binary name to its
    /// absolute path on PATH (or `nil` if absent), and returns one
    /// `Status` entry per known tool, in `knownTools` order.
    public static func classify(
        resolve: (String) -> String?,
        tools: [Tool] = knownTools
    ) -> CheckResult {
        let statuses = tools.map { Status(tool: $0, path: resolve($0.name)) }
        return CheckResult(statuses: statuses)
    }

    /// Runtime entry point. Resolves each known tool via `/usr/bin/which`.
    public static func run() -> CheckResult {
        return classify(resolve: which)
    }

    private static func which(_ name: String) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        proc.arguments = [name]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let path = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (path?.isEmpty ?? true) ? nil : path
    }
}
