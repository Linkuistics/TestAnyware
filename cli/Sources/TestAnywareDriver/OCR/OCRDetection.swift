import Foundation

/// A single OCR text detection with bounding box in image-pixel coordinates.
public struct OCRDetection: Codable, Sendable, Equatable {
    public let text: String
    public let x: Double
    public let y: Double
    public let width: Double
    public let height: Double
    public let confidence: Float

    public init(text: String, x: Double, y: Double, width: Double, height: Double, confidence: Float) {
        self.text = text
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.confidence = confidence
    }
}

/// Response envelope for the `/ocr` server endpoint.
public struct OCRResponse: Codable, Sendable {
    public let engine: String
    public let detections: [OCRDetection]
    public let warning: String?

    public init(engine: String, detections: [OCRDetection], warning: String? = nil) {
        self.engine = engine
        self.detections = detections
        self.warning = warning
    }

    private enum CodingKeys: String, CodingKey {
        case engine, detections, warning
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(engine, forKey: .engine)
        try container.encode(detections, forKey: .detections)
        try container.encodeIfPresent(warning, forKey: .warning)
    }
}
