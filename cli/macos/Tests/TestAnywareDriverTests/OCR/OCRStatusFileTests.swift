import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("OCRStatusFile")
struct OCRStatusFileTests {

    private func tempStatusPath() -> String {
        let dir = NSTemporaryDirectory() + "testanyware-test-\(UUID().uuidString)"
        return dir + "/ocr-status.json"
    }

    @Test func writeAndReadRoundTrip() throws {
        let path = tempStatusPath()
        defer { try? FileManager.default.removeItem(atPath: path) }

        let status = OCRStatusFile.Status(
            degraded: true,
            reason: "easyocr not installed",
            lastCheck: "2026-04-16T12:00:00Z"
        )
        try OCRStatusFile.write(status, to: path)
        let read = OCRStatusFile.read(from: path)
        #expect(read != nil)
        #expect(read?.degraded == true)
        #expect(read?.reason == "easyocr not installed")
        #expect(read?.lastCheck == "2026-04-16T12:00:00Z")
    }

    @Test func clearRemovesFile() throws {
        let path = tempStatusPath()
        let status = OCRStatusFile.Status(degraded: true, reason: "test", lastCheck: "now")
        try OCRStatusFile.write(status, to: path)
        #expect(FileManager.default.fileExists(atPath: path))
        OCRStatusFile.clear(at: path)
        #expect(!FileManager.default.fileExists(atPath: path))
    }

    @Test func readNonexistentReturnsNil() {
        let path = "/tmp/testanyware-nonexistent-\(UUID().uuidString)/ocr-status.json"
        let result = OCRStatusFile.read(from: path)
        #expect(result == nil)
    }

    @Test func readCorruptFileReturnsNil() throws {
        let path = tempStatusPath()
        defer { try? FileManager.default.removeItem(atPath: path) }

        let dir = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        try "garbage bytes!!!".write(toFile: path, atomically: true, encoding: .utf8)

        let result = OCRStatusFile.read(from: path)
        #expect(result == nil)
    }

    @Test func writeCreatesParentDirectory() throws {
        let path = tempStatusPath()
        let dir = (path as NSString).deletingLastPathComponent
        defer { try? FileManager.default.removeItem(atPath: dir) }

        #expect(!FileManager.default.fileExists(atPath: dir))
        let status = OCRStatusFile.Status(degraded: false, reason: "ok", lastCheck: "now")
        try OCRStatusFile.write(status, to: path)
        #expect(FileManager.default.fileExists(atPath: path))
    }

    @Test func defaultPathIsUnderApplicationSupport() {
        let path = OCRStatusFile.defaultPath
        #expect(path.contains("Application Support/testanyware"))
        #expect(path.hasSuffix("ocr-status.json"))
    }
}
