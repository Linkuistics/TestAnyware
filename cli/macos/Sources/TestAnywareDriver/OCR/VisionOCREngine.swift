import CoreGraphics
import Foundation
import Vision

/// In-process OCR using Apple's Vision.framework.
/// Returns all detected text above the confidence threshold — no substring filtering.
public enum VisionOCREngine {

    public static func recognize(pngData: Data) -> [OCRDetection] {
        guard let provider = CGDataProvider(data: pngData as CFData),
              let image = CGImage(
                  pngDataProviderSource: provider,
                  decode: nil, shouldInterpolate: false,
                  intent: .defaultIntent
              ) else { return [] }

        let handler = VNImageRequestHandler(cgImage: image, options: [:])
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.usesLanguageCorrection = true

        do {
            try handler.perform([request])
        } catch {
            return []
        }

        guard let observations = request.results else { return [] }

        let imageWidth = Double(image.width)
        let imageHeight = Double(image.height)

        var detections: [OCRDetection] = []
        for observation in observations {
            guard observation.confidence >= 0.5,
                  let candidate = observation.topCandidates(1).first else { continue }

            let box = observation.boundingBox
            detections.append(OCRDetection(
                text: candidate.string,
                x: box.origin.x * imageWidth,
                y: (1.0 - box.origin.y - box.height) * imageHeight,
                width: box.width * imageWidth,
                height: box.height * imageHeight,
                confidence: candidate.confidence
            ))
        }

        return detections
    }
}
