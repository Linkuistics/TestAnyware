import Foundation

/// Private lifecycle metadata sidecar written alongside the public spec
/// at `<vmsDir>/<id>.meta.json`. The CLI never consumes this file;
/// `testanyware vm stop` reads it to tear the VM down cleanly.
///
/// Keys match the JSON produced by `scripts/macos/vm-start.sh` so either
/// side can read the other's output during the bash → Swift transition.
public struct VMMeta: Codable, Equatable, Sendable {
    public enum Tool: String, Codable, Equatable, Sendable {
        case tart
        case qemu
    }

    public var id: String
    public var tool: Tool
    public var pid: Int
    public var cloneDir: String?
    public var viewerWindowID: String?

    enum CodingKeys: String, CodingKey {
        case id
        case tool
        case pid
        case cloneDir = "clone_dir"
        case viewerWindowID = "viewer_window_id"
    }

    public init(
        id: String,
        tool: Tool,
        pid: Int,
        cloneDir: String?,
        viewerWindowID: String?
    ) {
        self.id = id
        self.tool = tool
        self.pid = pid
        self.cloneDir = cloneDir
        self.viewerWindowID = viewerWindowID
    }

    public static func load(from path: String) throws -> VMMeta {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        return try JSONDecoder().decode(VMMeta.self, from: data)
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
