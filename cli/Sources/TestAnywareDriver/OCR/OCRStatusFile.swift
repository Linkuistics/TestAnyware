import Foundation

/// Persistent OCR daemon health status, written on permanent failure
/// and cleared by `testanyware doctor` on recovery.
public enum OCRStatusFile {

    public struct Status: Codable {
        public let degraded: Bool
        public let reason: String
        public let lastCheck: String

        public init(degraded: Bool, reason: String, lastCheck: String) {
            self.degraded = degraded
            self.reason = reason
            self.lastCheck = lastCheck
        }
    }

    public static var defaultPath: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return "\(home)/Library/Application Support/testanyware/ocr-status.json"
    }

    public static func read(from path: String = defaultPath) -> Status? {
        guard let data = FileManager.default.contents(atPath: path) else { return nil }
        return try? JSONDecoder().decode(Status.self, from: data)
    }

    public static func write(_ status: Status, to path: String = defaultPath) throws {
        let dir = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(
            atPath: dir, withIntermediateDirectories: true
        )
        let data = try JSONEncoder().encode(status)
        try data.write(to: URL(fileURLWithPath: path))
    }

    public static func clear(at path: String = defaultPath) {
        try? FileManager.default.removeItem(atPath: path)
    }
}
