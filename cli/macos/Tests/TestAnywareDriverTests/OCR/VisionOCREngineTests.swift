import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("VisionOCREngine")
struct VisionOCREngineTests {

    private func fixturePath(_ name: String) -> String {
        let testFile = URL(fileURLWithPath: #filePath)
        return testFile
            .deletingLastPathComponent()  // OCR/
            .deletingLastPathComponent()  // TestAnywareDriverTests/
            .deletingLastPathComponent()  // Tests/
            .appendingPathComponent("Resources")
            .appendingPathComponent(name)
            .path
    }

    @Test func emptyDataReturnsEmptyArray() {
        let detections = VisionOCREngine.recognize(pngData: Data())
        #expect(detections.isEmpty)
    }

    @Test func corruptDataReturnsEmptyArray() {
        let detections = VisionOCREngine.recognize(pngData: Data([0xFF, 0xFE, 0x00]))
        #expect(detections.isEmpty)
    }

    @Test func helloFixtureReturnsDetection() throws {
        let path = fixturePath("hello.png")
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let detections = VisionOCREngine.recognize(pngData: data)
        #expect(!detections.isEmpty)
        let hasHello = detections.contains { $0.text.localizedCaseInsensitiveContains("Hello") }
        #expect(hasHello)
    }

    @Test func detectionsHaveReasonableConfidence() throws {
        let path = fixturePath("hello.png")
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let detections = VisionOCREngine.recognize(pngData: data)
        for detection in detections {
            #expect(detection.confidence >= 0.5)
        }
    }

    @Test func boundingBoxesAreInPixelSpace() throws {
        let path = fixturePath("hello.png")
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let detections = VisionOCREngine.recognize(pngData: data)
        // Image is 200x50
        for detection in detections {
            #expect(detection.x >= 0)
            #expect(detection.y >= 0)
            #expect(detection.width > 0)
            #expect(detection.height > 0)
            #expect(detection.x + detection.width <= 200)
            #expect(detection.y + detection.height <= 50)
        }
    }
}
