import AVFoundation
import CoreGraphics
import CoreMedia
import CoreVideo
import Foundation

/// Configuration for streaming video capture.
public struct StreamingCaptureConfig: Sendable {
    public let width: Int
    public let height: Int
    public let fps: Int
    public let codec: VideoCodec

    public init(width: Int, height: Int, fps: Int = 30, codec: VideoCodec = .h264) {
        self.width = width
        self.height = height
        self.fps = fps
        self.codec = codec
    }

    public enum VideoCodec: Sendable {
        case h264
        case hevc
    }
}

/// Records a stream of CGImage frames to a video file.
public actor StreamingCapture {
    public enum State: Sendable { case idle, recording }

    public private(set) var state: State = .idle

    private var assetWriter: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?
    private var frameCount: Int = 0
    private var fps: Int = 30
    private var config: StreamingCaptureConfig?

    public init() {}

    /// Start recording to the given file path.
    public func start(outputPath: String, config: StreamingCaptureConfig) throws {
        guard state == .idle else {
            throw StreamingCaptureError.alreadyRecording
        }

        let url = URL(fileURLWithPath: outputPath)
        try? FileManager.default.removeItem(at: url)

        let writer = try AVAssetWriter(outputURL: url, fileType: .mp4)

        let avCodec: AVVideoCodecType = config.codec == .hevc ? .hevc : .h264
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: avCodec,
            AVVideoWidthKey: config.width,
            AVVideoHeightKey: config.height,
        ]

        let input = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        input.expectsMediaDataInRealTime = true

        let sourcePixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: config.width,
            kCVPixelBufferHeightKey as String: config.height,
        ]

        let adaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: input,
            sourcePixelBufferAttributes: sourcePixelBufferAttributes
        )

        writer.add(input)
        writer.startWriting()
        writer.startSession(atSourceTime: .zero)

        self.assetWriter = writer
        self.videoInput = input
        self.pixelBufferAdaptor = adaptor
        self.frameCount = 0
        self.fps = config.fps
        self.config = config
        self.state = .recording
    }

    /// Append a frame to the recording.
    public func appendFrame(_ image: CGImage) throws {
        guard state == .recording,
              let adaptor = pixelBufferAdaptor,
              let input = videoInput else {
            throw StreamingCaptureError.notRecording
        }

        guard input.isReadyForMoreMediaData else { return }

        guard let pool = adaptor.pixelBufferPool else {
            throw StreamingCaptureError.pixelBufferPoolUnavailable
        }

        var pixelBuffer: CVPixelBuffer?
        let status = CVPixelBufferPoolCreatePixelBuffer(nil, pool, &pixelBuffer)
        guard status == kCVReturnSuccess, let buffer = pixelBuffer else {
            throw StreamingCaptureError.pixelBufferCreationFailed
        }

        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }

        let width = CVPixelBufferGetWidth(buffer)
        let height = CVPixelBufferGetHeight(buffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(buffer)

        guard let baseAddress = CVPixelBufferGetBaseAddress(buffer) else {
            throw StreamingCaptureError.pixelBufferCreationFailed
        }

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        guard let context = CGContext(
            data: baseAddress,
            width: width, height: height,
            bitsPerComponent: 8, bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            throw StreamingCaptureError.pixelBufferCreationFailed
        }

        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

        let presentationTime = CMTime(value: CMTimeValue(frameCount), timescale: CMTimeScale(fps))
        adaptor.append(buffer, withPresentationTime: presentationTime)
        frameCount += 1
    }

    /// Stop recording and finalize the video file.
    public func stop() async throws {
        guard state == .recording, let writer = assetWriter else {
            throw StreamingCaptureError.notRecording
        }

        videoInput?.markAsFinished()

        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            writer.finishWriting {
                continuation.resume()
            }
        }

        self.assetWriter = nil
        self.videoInput = nil
        self.pixelBufferAdaptor = nil
        self.config = nil
        self.state = .idle
    }
}

public enum StreamingCaptureError: Error, LocalizedError {
    case alreadyRecording
    case notRecording
    case pixelBufferPoolUnavailable
    case pixelBufferCreationFailed

    public var errorDescription: String? {
        switch self {
        case .alreadyRecording: "Already recording"
        case .notRecording: "Not currently recording"
        case .pixelBufferPoolUnavailable: "Pixel buffer pool not available"
        case .pixelBufferCreationFailed: "Failed to create pixel buffer"
        }
    }
}
