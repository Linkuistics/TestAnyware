import Foundation

/// Public per-VM spec file written by `testanyware vm start`, consumed by
/// `ConnectionOptions.resolve()` via `ConnectionSpec.load`.
///
/// Schema mirrors the JSON produced by `scripts/macos/vm-start.sh` so either
/// side can read the other's output during the bash → Swift transition.
/// Reuses `VNCSpec`, `AgentSpec`, and `Platform` from `ConnectionSpec` to
/// avoid divergence; adds `ssh` which `ConnectionSpec` does not carry.
public struct VMSpec: Codable, Equatable, Sendable {
    public var vnc: VNCSpec
    public var agent: AgentSpec?
    public var platform: Platform
    public var ssh: String?

    public init(
        vnc: VNCSpec,
        agent: AgentSpec?,
        platform: Platform,
        ssh: String?
    ) {
        self.vnc = vnc
        self.agent = agent
        self.platform = platform
        self.ssh = ssh
    }

    public static func load(from path: String) throws -> VMSpec {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        return try JSONDecoder().decode(VMSpec.self, from: data)
    }

    /// Atomically write `self` to `path` with mode 0600.
    /// Writes to a `<path>.tmp` sibling, chmods it, then atomically
    /// renames into place so readers never observe a partial file.
    public func writeAtomic(to path: String) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(self)

        let tmpPath = path + ".tmp"
        let tmpURL = URL(fileURLWithPath: tmpPath)
        let finalURL = URL(fileURLWithPath: path)

        try data.write(to: tmpURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: NSNumber(value: 0o600)],
            ofItemAtPath: tmpPath
        )

        if FileManager.default.fileExists(atPath: path) {
            _ = try FileManager.default.replaceItemAt(finalURL, withItemAt: tmpURL)
        } else {
            try FileManager.default.moveItem(at: tmpURL, to: finalURL)
        }
    }
}
