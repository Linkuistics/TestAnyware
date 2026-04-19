import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("OCRDetection")
struct OCRDetectionTests {

    // MARK: - OCRDetection round-trip

    @Test func detectionEncodesAndDecodesIdentically() throws {
        let detection = OCRDetection(
            text: "Hello",
            x: 10.5, y: 20.0,
            width: 100.0, height: 15.5,
            confidence: 0.95
        )
        let data = try JSONEncoder().encode(detection)
        let decoded = try JSONDecoder().decode(OCRDetection.self, from: data)
        #expect(decoded.text == detection.text)
        #expect(decoded.x == detection.x)
        #expect(decoded.y == detection.y)
        #expect(decoded.width == detection.width)
        #expect(decoded.height == detection.height)
        #expect(decoded.confidence == detection.confidence)
    }

    // MARK: - OCRResponse round-trip

    @Test func responseWithWarningEncodesAndDecodes() throws {
        let response = OCRResponse(
            engine: "vision",
            detections: [
                OCRDetection(text: "Hello", x: 0, y: 0, width: 50, height: 12, confidence: 0.99),
            ],
            warning: "daemon unavailable, using Vision fallback"
        )
        let data = try JSONEncoder().encode(response)
        let decoded = try JSONDecoder().decode(OCRResponse.self, from: data)
        #expect(decoded.engine == "vision")
        #expect(decoded.detections.count == 1)
        #expect(decoded.detections[0].text == "Hello")
        #expect(decoded.warning == "daemon unavailable, using Vision fallback")
    }

    @Test func responseWithNilWarningOmitsKey() throws {
        let response = OCRResponse(
            engine: "easyocr_daemon",
            detections: [],
            warning: nil
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8)!
        #expect(!json.contains("warning"))
    }

    @Test func emptyDetectionsArrayRoundTrips() throws {
        let response = OCRResponse(engine: "vision", detections: [])
        let data = try JSONEncoder().encode(response)
        let decoded = try JSONDecoder().decode(OCRResponse.self, from: data)
        #expect(decoded.detections.isEmpty)
        #expect(decoded.engine == "vision")
        #expect(decoded.warning == nil)
    }
}
