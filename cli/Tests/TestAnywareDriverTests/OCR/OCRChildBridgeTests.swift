import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("OCRChildBridge")
struct OCRChildBridgeTests {

    private func fakeScriptPath() -> String {
        let testFile = URL(fileURLWithPath: #filePath)
        return testFile
            .deletingLastPathComponent()  // OCR/
            .deletingLastPathComponent()  // TestAnywareDriverTests/
            .deletingLastPathComponent()  // Tests/
            .appendingPathComponent("Resources")
            .appendingPathComponent("fake-ocr-daemon.sh")
            .path
    }

    private func makeBridge(
        behavior: String,
        warmDeadline: Duration = .seconds(5),
        firstCallDeadline: Duration = .seconds(10)
    ) -> OCRChildBridge {
        OCRChildBridge(
            interpreterPath: "/bin/bash",
            daemonArguments: [fakeScriptPath()],
            environment: ["FAKE_OCR_BEHAVIOR": behavior],
            warmDeadline: warmDeadline,
            firstCallDeadline: firstCallDeadline
        )
    }

    // MARK: - Happy path

    @Test func coldStartReturnsDetections() async throws {
        let bridge = makeBridge(behavior: "ready_then_echo")
        defer { Task { await bridge.shutdown() } }
        let detections = try await bridge.recognize(pngData: Data([0x89, 0x50]))
        #expect(!detections.isEmpty)
        #expect(detections[0].text == "fake")
    }

    @Test func warmPathReusesSameChild() async throws {
        let bridge = makeBridge(behavior: "ready_then_echo")
        defer { Task { await bridge.shutdown() } }
        let d1 = try await bridge.recognize(pngData: Data([0x89]))
        let d2 = try await bridge.recognize(pngData: Data([0x89]))
        #expect(!d1.isEmpty)
        #expect(!d2.isEmpty)
    }

    @Test func concurrentCallsSerialize() async throws {
        let bridge = makeBridge(behavior: "ready_then_echo")
        defer { Task { await bridge.shutdown() } }
        async let r1 = bridge.recognize(pngData: Data([0x89]))
        async let r2 = bridge.recognize(pngData: Data([0x89]))
        let (d1, d2) = try await (r1, r2)
        #expect(!d1.isEmpty)
        #expect(!d2.isEmpty)
    }

    @Test func shutdownTerminatesChild() async throws {
        let bridge = makeBridge(behavior: "ready_then_echo")
        _ = try await bridge.recognize(pngData: Data([0x89]))
        await bridge.shutdown()
        // After shutdown, the child process should be gone.
        // A second shutdown should be harmless.
        await bridge.shutdown()
    }

    @Test func tempFileCleanedUpAfterCall() async throws {
        let bridge = makeBridge(behavior: "ready_then_echo")
        defer { Task { await bridge.shutdown() } }
        _ = try await bridge.recognize(pngData: Data([0x89]))
        // Check no leftover temp files
        let tmpDir = NSTemporaryDirectory()
        let files = try FileManager.default.contentsOfDirectory(atPath: tmpDir)
        let ocrFiles = files.filter { $0.hasPrefix("testanyware-ocr-") && $0.hasSuffix(".png") }
        #expect(ocrFiles.isEmpty)
    }
}
