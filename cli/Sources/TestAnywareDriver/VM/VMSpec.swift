import Foundation

/// Public per-VM spec file written by `testanyware vm start`, consumed by
/// `ConnectionOptions.resolve()` via `ConnectionSpec.load`.
///
/// Reuses `VNCSpec`, `AgentSpec`, and `Platform` from `ConnectionSpec` so
/// the two stay in sync without parallel wrapper types.
public struct VMSpec: Codable, Equatable, Sendable {
    public var vnc: VNCSpec
    public var agent: AgentSpec?
    public var platform: Platform

    public init(
        vnc: VNCSpec,
        agent: AgentSpec?,
        platform: Platform
    ) {
        self.vnc = vnc
        self.agent = agent
        self.platform = platform
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
