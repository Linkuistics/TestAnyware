import CoreGraphics
import Foundation
@preconcurrency import RoyalVNCKit

/// Persistent VNC connection for screen capture, input, and cursor tracking.
public actor VNCCapture {

    private let spec: VNCSpec

    private nonisolated(unsafe) var connection: VNCConnection?
    private nonisolated(unsafe) var delegate: VNCCaptureDelegate?

    // MARK: - Init

    public init(spec: VNCSpec) {
        self.spec = spec
    }

    public init(host: String, port: Int = 5900, password: String? = nil) {
        self.spec = VNCSpec(host: host, port: port, password: password)
    }

    // MARK: - Connect / Disconnect

    /// Connect to the VNC server and wait for the first framebuffer update.
    public func connect(timeout: Duration = .seconds(30)) async throws {
        disconnect()

        let del = VNCCaptureDelegate(password: spec.password)

        let debugVNC = ProcessInfo.processInfo.environment["TESTANYWARE_VNC_DEBUG"] == "1"
        let settings = VNCConnection.Settings(
            isDebugLoggingEnabled: debugVNC,
            hostname: spec.host,
            port: UInt16(spec.port),
            isShared: true,
            isScalingEnabled: false,
            useDisplayLink: false,
            inputMode: .forwardAllKeyboardShortcutsAndHotKeys,
            isClipboardRedirectionEnabled: false,
            colorDepth: .depth24Bit,
            frameEncodings: .default
        )

        let conn = VNCConnection(settings: settings)
        conn.delegate = del

        self.delegate = del
        self.connection = conn

        conn.connect()

        let timeoutSeconds = Int(timeout.components.seconds)
        let timeoutItem = DispatchWorkItem { [weak del] in
            del?.resumeIfPending(with: VNCCaptureError.timeout)
        }
        DispatchQueue.global().asyncAfter(
            deadline: .now() + .seconds(timeoutSeconds),
            execute: timeoutItem
        )

        do {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, any Error>) in
                del.setConnectContinuation(continuation)
            }
            timeoutItem.cancel()
        } catch {
            timeoutItem.cancel()
            throw error
        }
    }

    /// Disconnect from the VNC server and release resources.
    public func disconnect() {
        connection?.disconnect()
        connection = nil
        delegate = nil
    }

    // MARK: - Screen Capture

    /// Capture the current framebuffer as a CGImage.
    public func captureImage(region: CGRect? = nil) async throws -> CGImage {
        guard let currentDelegate = delegate else {
            throw VNCCaptureError.notConfigured
        }

        if !currentDelegate.isFramebufferReady {
            try await connect()
        }

        guard let activeDelegate = self.delegate,
              let framebuffer = activeDelegate.framebuffer else {
            throw VNCCaptureError.framebufferNotReady
        }

        let fullImage = try Self.cgImageFromFramebuffer(framebuffer)

        if let region {
            guard let cropped = fullImage.cropping(to: region) else {
                throw VNCCaptureError.captureFailed
            }
            return cropped
        }
        return fullImage
    }

    /// Capture a screenshot as PNG data.
    public func screenshot(region: CGRect? = nil) async throws -> Data {
        let image = try await captureImage(region: region)
        return try FramebufferConverter.pngData(from: image)
    }

    // MARK: - Screen Metadata

    /// Return framebuffer dimensions, or nil if not connected.
    public func screenSize() -> CGSize? {
        guard let framebuffer = delegate?.framebuffer else { return nil }
        return framebuffer.cgSize
    }

    // MARK: - Cursor

    /// Current cursor state (shape, position, hotspot).
    public var cursorState: CursorState {
        delegate?.cursorState ?? CursorState()
    }

    // MARK: - Low-level Connection Access

    /// Run a closure with direct access to the VNCConnection.
    public func withConnection<T: Sendable>(_ body: (VNCConnection) throws -> T) throws -> T {
        guard let conn = connection else {
            throw VNCCaptureError.notConfigured
        }
        return try body(conn)
    }

    // MARK: - Framebuffer -> CGImage

    private static func cgImageFromFramebuffer(_ framebuffer: VNCFramebuffer) throws -> CGImage {
        let width = Int(framebuffer.size.width)
        let height = Int(framebuffer.size.height)
        let bytesPerPixel = 4
        let bytesPerRow = width * bytesPerPixel
        let totalBytes = width * height * bytesPerPixel

        guard framebuffer.surfaceByteCount == totalBytes else {
            throw VNCCaptureError.captureFailed
        }

        framebuffer.allocator.lockReadOnly()
        let rgbaData = NSMutableData(length: totalBytes)!
        let src = framebuffer.surfaceAddress.assumingMemoryBound(to: UInt8.self)
        let dst = rgbaData.mutableBytes.assumingMemoryBound(to: UInt8.self)
        for i in stride(from: 0, to: totalBytes, by: bytesPerPixel) {
            dst[i + 0] = src[i + 2]  // R <- B
            dst[i + 1] = src[i + 1]  // G <- G
            dst[i + 2] = src[i + 0]  // B <- R
            dst[i + 3] = 255         // A
        }
        framebuffer.allocator.unlockReadOnly()

        guard let provider = CGDataProvider(data: rgbaData) else {
            throw VNCCaptureError.captureFailed
        }

        guard let image = CGImage(
            width: width, height: height,
            bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue),
            provider: provider, decode: nil,
            shouldInterpolate: false, intent: .defaultIntent
        ) else {
            throw VNCCaptureError.captureFailed
        }
        return image
    }
}
