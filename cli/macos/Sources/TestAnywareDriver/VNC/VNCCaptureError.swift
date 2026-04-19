import Foundation

public enum VNCCaptureError: Error, CustomStringConvertible, Sendable {
    case notConfigured
    case connectionFailed(String)
    case disconnected
    case framebufferNotReady
    case captureFailed
    case encodingFailed
    case timeout

    public var description: String {
        switch self {
        case .notConfigured:
            "VNC not configured: call connect() first"
        case .connectionFailed(let detail):
            "VNC connection failed: \(detail)"
        case .disconnected:
            "VNC connection lost"
        case .framebufferNotReady:
            "VNC framebuffer not ready"
        case .captureFailed:
            "Failed to capture VNC framebuffer"
        case .encodingFailed:
            "Failed to encode PNG"
        case .timeout:
            "VNC connection timed out"
        }
    }
}
