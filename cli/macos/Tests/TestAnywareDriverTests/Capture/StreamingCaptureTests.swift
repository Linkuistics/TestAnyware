import Testing
import CoreGraphics
import Foundation
@testable import TestAnywareDriver

@Suite("StreamingCapture")
struct StreamingCaptureTests {

    @Test func initWithDefaults() {
        let config = StreamingCaptureConfig(width: 1920, height: 1080)
        #expect(config.width == 1920)
        #expect(config.height == 1080)
        #expect(config.fps == 30)
        #expect(config.codec == .h264)
    }

    @Test func initWithCustomFPS() {
        let config = StreamingCaptureConfig(width: 1280, height: 720, fps: 60)
        #expect(config.fps == 60)
    }

    @Test func stateStartsIdle() async {
        let capture = StreamingCapture()
        let state = await capture.state
        #expect(state == .idle)
    }

    @Test func recordsToFile() async throws {
        let outputPath = NSTemporaryDirectory() + "test_recording_\(UUID().uuidString).mp4"
        defer { try? FileManager.default.removeItem(atPath: outputPath) }

        let config = StreamingCaptureConfig(width: 100, height: 100, fps: 10, codec: .h264)
        let capture = StreamingCapture()

        try await capture.start(outputPath: outputPath, config: config)
        #expect(await capture.state == .recording)

        // Feed a few synthetic frames
        let image = try createTestImage(width: 100, height: 100)
        for _ in 0..<5 {
            try await capture.appendFrame(image)
            try await Task.sleep(for: .milliseconds(100))
        }

        try await capture.stop()
        #expect(await capture.state == .idle)

        // Verify file was created and has content
        let fileExists = FileManager.default.fileExists(atPath: outputPath)
        #expect(fileExists)
        let attrs = try FileManager.default.attributesOfItem(atPath: outputPath)
        let size = attrs[.size] as? Int ?? 0
        #expect(size > 0)
    }

    @Test func stopWhenIdleThrows() async {
        let capture = StreamingCapture()
        do {
            try await capture.stop()
            Issue.record("Expected error")
        } catch {
            #expect(error is StreamingCaptureError)
        }
    }

    @Test func doubleStartThrows() async throws {
        let path1 = NSTemporaryDirectory() + "test1_\(UUID().uuidString).mp4"
        let path2 = NSTemporaryDirectory() + "test2_\(UUID().uuidString).mp4"
        defer {
            try? FileManager.default.removeItem(atPath: path1)
            try? FileManager.default.removeItem(atPath: path2)
        }

        let config = StreamingCaptureConfig(width: 100, height: 100, fps: 10)
        let capture = StreamingCapture()
        try await capture.start(outputPath: path1, config: config)

        do {
            try await capture.start(outputPath: path2, config: config)
            Issue.record("Expected error")
        } catch {
            #expect(error is StreamingCaptureError)
        }

        try await capture.stop()
    }

    // MARK: - Helpers

    private func createTestImage(width: Int, height: Int) throws -> CGImage {
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { throw TestImageError.contextFailed }
        context.setFillColor(CGColor(red: 0.5, green: 0.5, blue: 0.5, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        guard let image = context.makeImage() else { throw TestImageError.imageFailed }
        return image
    }
}

private enum TestImageError: Error { case contextFailed, imageFailed }
