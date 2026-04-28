import Foundation

/// Reports whether the host has the third-party CLI tools that
/// `testanyware vm ...` shells out to: `tart` (macOS/Linux VMs),
/// `qemu-system-aarch64` (Windows VMs), and `swtpm` (TPM 2.0 for the
/// Windows VM). All three are advisory — a user who only runs macOS
/// tests has no need for QEMU or swtpm — so missing tools surface in
/// the doctor output but do not flip `isOK` to false.
///
/// In addition to presence, `tart` and `qemu-system-aarch64` carry a
/// minimum-version floor. Resolved binaries below the floor surface as
/// advisory warnings; the doctor still passes, because a below-floor
/// tool can still work for many workflows. Floors are deliberately
/// conservative — they flag obviously-old installs that are likely to
/// hit known-broken combinations, not the latest cutting edge.
public enum ToolAvailabilityCheck {

    public struct Tool: Equatable, Sendable {
        public let name: String
        public let purpose: String
        public let installHint: String
        /// Minimum supported version, dotted form (e.g., "2.0.0"). `nil`
        /// means presence-only: any resolved binary is acceptable.
        public let minimumVersion: String?

        public init(
            name: String,
            purpose: String,
            installHint: String,
            minimumVersion: String? = nil
        ) {
            self.name = name
            self.purpose = purpose
            self.installHint = installHint
            self.minimumVersion = minimumVersion
        }
    }

    /// Outcome of comparing a resolved tool's reported version against
    /// its `minimumVersion` floor.
    public enum VersionVerdict: Equatable, Sendable {
        /// Tool has no `minimumVersion` floor, or the resolved version
        /// is at or above the floor.
        case ok(detected: String?)
        /// Resolved version is below the floor.
        case belowFloor(detected: String, minimum: String)
        /// `--version` ran but the output could not be parsed.
        case unparseable(rawOutput: String, minimum: String)
        /// `--version` produced no output (probe returned `nil`). Treated
        /// as advisory rather than fatal so a transient probe failure
        /// doesn't block the doctor.
        case probeFailed(minimum: String)
    }

    public struct Status: Equatable, Sendable {
        public let tool: Tool
        /// Resolved absolute path, or `nil` if the binary is not on PATH.
        public let path: String?
        /// Verdict from comparing the reported version to `tool.minimumVersion`.
        /// Always `.ok(detected: nil)` when `path == nil` (we never probe a
        /// missing binary).
        public let versionVerdict: VersionVerdict

        public init(tool: Tool, path: String?, versionVerdict: VersionVerdict = .ok(detected: nil)) {
            self.tool = tool
            self.path = path
            self.versionVerdict = versionVerdict
        }

        public var isAvailable: Bool { path != nil }
    }

    public struct CheckResult: Equatable {
        public let statuses: [Status]

        public init(statuses: [Status]) {
            self.statuses = statuses
        }

        /// Tool availability is advisory, so the doctor does not fail on
        /// missing tools or below-floor versions. The verdict is
        /// informational only.
        public var isOK: Bool { true }
    }

    /// The three tools the doctor reports on, in display order. Floors are
    /// chosen as the oldest version known to support the features the
    /// provisioner scripts rely on — newer is always fine.
    public static let knownTools: [Tool] = [
        Tool(
            name: "tart",
            purpose: "macOS and Linux VMs",
            installHint: "brew install cirruslabs/cli/tart",
            // 2.0.0 is the baseline at which the `tart clone`/`tart run`
            // CLI surface stabilised; older releases lack flags the
            // provisioner scripts depend on (e.g. `--vnc-experimental`,
            // `--dir`).
            minimumVersion: "2.0.0"
        ),
        Tool(
            name: "qemu-system-aarch64",
            purpose: "Windows VMs",
            installHint: "brew install qemu",
            // 8.0.0 is the first release with stable Windows-11-on-ARM64
            // support across the swtpm/UEFI/virt-gic-3 combination the
            // Windows golden script uses; earlier QEMUs ship with
            // gic-version=3 quirks that intermittently fail boot.
            minimumVersion: "8.0.0"
        ),
        Tool(
            name: "swtpm",
            purpose: "TPM 2.0 emulation for Windows 11 VMs",
            installHint: "brew install swtpm"
        ),
    ]

    /// Pure classifier. `resolve` maps a binary name to its absolute
    /// path on PATH (or `nil` if absent); `versionProbe` maps a binary
    /// name to its raw `--version` stdout (or `nil` if the probe could
    /// not run). Returns one `Status` entry per known tool, in
    /// `knownTools` order.
    public static func classify(
        resolve: (String) -> String?,
        versionProbe: (String) -> String? = { _ in nil },
        tools: [Tool] = knownTools
    ) -> CheckResult {
        let statuses = tools.map { tool -> Status in
            let path = resolve(tool.name)
            let verdict = path == nil
                ? VersionVerdict.ok(detected: nil)
                : compareVersion(rawOutput: versionProbe(tool.name), minimum: tool.minimumVersion)
            return Status(tool: tool, path: path, versionVerdict: verdict)
        }
        return CheckResult(statuses: statuses)
    }

    /// Runtime entry point. Resolves each known tool via `/usr/bin/which`
    /// and probes its version via `<tool> --version`.
    public static func run() -> CheckResult {
        return classify(resolve: which, versionProbe: probeVersion)
    }

    // MARK: - Version comparison

    /// Compares a tool's `--version` output against an optional floor.
    /// Public for testing; behaviour matches `Status.versionVerdict`'s
    /// per-tool result.
    public static func compareVersion(
        rawOutput: String?,
        minimum: String?
    ) -> VersionVerdict {
        guard let minimum else {
            return .ok(detected: parseVersion(from: rawOutput ?? ""))
        }
        guard let rawOutput, !rawOutput.isEmpty else {
            return .probeFailed(minimum: minimum)
        }
        guard let detected = parseVersion(from: rawOutput) else {
            return .unparseable(rawOutput: rawOutput.trimmingCharacters(in: .whitespacesAndNewlines), minimum: minimum)
        }
        if detected.compare(minimum, options: .numeric) == .orderedAscending {
            return .belowFloor(detected: detected, minimum: minimum)
        }
        return .ok(detected: detected)
    }

    /// Pulls the first `MAJOR.MINOR[.PATCH]` token out of a `--version`
    /// blob. Tolerant of varied formats: `2.32.1`, `QEMU emulator
    /// version 11.0.0`, `swtpm version 0.10.0` all yield the right
    /// dotted token.
    static func parseVersion(from rawOutput: String) -> String? {
        // Find the first run of digits-and-dots that contains at least
        // one dot. Avoids matching standalone year tokens like "2026".
        var current = ""
        for ch in rawOutput {
            if ch.isNumber || ch == "." {
                current.append(ch)
            } else {
                if current.contains(".") && current.first?.isNumber == true {
                    return trimTrailingDots(current)
                }
                current.removeAll(keepingCapacity: true)
            }
        }
        if current.contains(".") && current.first?.isNumber == true {
            return trimTrailingDots(current)
        }
        return nil
    }

    private static func trimTrailingDots(_ s: String) -> String {
        var end = s.endIndex
        while end > s.startIndex {
            let prev = s.index(before: end)
            if s[prev] == "." { end = prev } else { break }
        }
        return String(s[..<end])
    }

    // MARK: - Subprocess probes

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

    private static func probeVersion(_ name: String) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        proc.arguments = [name, "--version"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        // Some tools write --version to stderr; we only consume stdout
        // because tart and qemu both print to stdout on supported
        // versions. If a future tool diverges, switch its probe rather
        // than merging streams (mixing them risks decoding garbage).
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let text = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return (text?.isEmpty ?? true) ? nil : text
    }
}
