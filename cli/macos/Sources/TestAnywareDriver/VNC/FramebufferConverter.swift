import CoreGraphics
import Foundation
import ImageIO

// MARK: - Errors

public enum FramebufferConverterError: Error {
    case zeroDimensions
    case pixelCountMismatch(expected: Int, got: Int)
    case pngEncodingFailed
    case cgImageCreationFailed
}

// MARK: - FramebufferConverter

/// Converts raw framebuffer pixel data to CGImage and PNG.
public enum FramebufferConverter {

    /// Swap BGRA pixel data to RGBA in place.
    public static func bgraToRGBA(_ buffer: inout [UInt8]) {
        precondition(buffer.count % 4 == 0, "Buffer length must be a multiple of 4")
        for i in stride(from: 0, to: buffer.count, by: 4) {
            buffer.swapAt(i, i + 2)
        }
    }

    /// Create a CGImage from RGBA pixel data.
    public static func cgImage(fromRGBA rgba: [UInt8], width: Int, height: Int) throws -> CGImage {
        guard width > 0, height > 0 else {
            throw FramebufferConverterError.zeroDimensions
        }
        let expectedCount = width * height * 4
        guard rgba.count == expectedCount else {
            throw FramebufferConverterError.pixelCountMismatch(expected: expectedCount, got: rgba.count)
        }
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        guard let provider = CGDataProvider(data: Data(rgba) as CFData),
              let image = CGImage(
                width: width,
                height: height,
                bitsPerComponent: 8,
                bitsPerPixel: 32,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: bitmapInfo,
                provider: provider,
                decode: nil,
                shouldInterpolate: false,
                intent: .defaultIntent
              ) else {
            throw FramebufferConverterError.cgImageCreationFailed
        }
        return image
    }

    /// Encode a CGImage as PNG data.
    public static func pngData(from image: CGImage) throws -> Data {
        let mutableData = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            mutableData as CFMutableData,
            "public.png" as CFString,
            1,
            nil
        ) else {
            throw FramebufferConverterError.pngEncodingFailed
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else {
            throw FramebufferConverterError.pngEncodingFailed
        }
        return mutableData as Data
    }
}
