import Testing
import CoreGraphics
@testable import TestAnywareDriver

@Suite("FramebufferConverter")
struct FramebufferConverterTests {
    @Test func swapsBGRAtoRGBA() {
        var bgra: [UInt8] = [255, 0, 0, 255]  // B=255, G=0, R=0, A=255
        FramebufferConverter.bgraToRGBA(&bgra)
        #expect(bgra == [0, 0, 255, 255])  // R=0, G=0, B=255, A=255
    }

    @Test func swapsMultiplePixels() {
        var bgra: [UInt8] = [
            0, 0, 255, 255,    // BGRA red pixel
            0, 255, 0, 255,    // BGRA green pixel
        ]
        FramebufferConverter.bgraToRGBA(&bgra)
        #expect(bgra == [
            255, 0, 0, 255,    // RGBA red pixel
            0, 255, 0, 255,    // RGBA green pixel (G unchanged)
        ])
    }

    @Test func createsCGImageFromRGBA() throws {
        let rgba: [UInt8] = [
            255, 0, 0, 255,
            0, 255, 0, 255,
            0, 0, 255, 255,
            255, 255, 255, 255,
        ]
        let image = try FramebufferConverter.cgImage(fromRGBA: rgba, width: 2, height: 2)
        #expect(image.width == 2)
        #expect(image.height == 2)
    }

    @Test func encodesPNG() throws {
        let rgba: [UInt8] = Array(repeating: UInt8(128), count: 4 * 10 * 10)
        let image = try FramebufferConverter.cgImage(fromRGBA: rgba, width: 10, height: 10)
        let png = try FramebufferConverter.pngData(from: image)
        #expect(png[0] == 0x89)  // PNG magic
        #expect(png[1] == 0x50)  // P
        #expect(png[2] == 0x4E)  // N
        #expect(png[3] == 0x47)  // G
    }

    @Test func rejectsZeroDimensions() {
        #expect(throws: FramebufferConverterError.self) {
            try FramebufferConverter.cgImage(fromRGBA: [], width: 0, height: 0)
        }
    }

    @Test func rejectsMismatchedPixelCount() {
        let rgba: [UInt8] = [255, 0, 0, 255]  // 1 pixel
        #expect(throws: FramebufferConverterError.self) {
            try FramebufferConverter.cgImage(fromRGBA: rgba, width: 2, height: 2)
        }
    }
}
