import Foundation
import CoreGraphics
@preconcurrency import RoyalVNCKit

/// Bridge between RoyalVNCKit delegate callbacks and Swift concurrency.
/// Thread-safe via `NSLock`. Stores framebuffer state, cursor state, and
/// signals readiness to waiting `connect()` calls via `CheckedContinuation`.
final class VNCCaptureDelegate: VNCConnectionDelegate, @unchecked Sendable {
    private let lock = NSLock()

    // Credentials
    private let password: String?

    // Framebuffer state (guarded by lock)
    private var _framebuffer: VNCFramebuffer?
    private var _isFramebufferReady = false

    // Cursor state (guarded by lock)
    private var _cursorState = CursorState()

    // Connect continuation (guarded by lock)
    private var connectContinuation: CheckedContinuation<Void, any Error>?
    private var disconnectError: (any Error)?

    init(password: String?) {
        self.password = password
    }

    // MARK: - Thread-safe accessors

    var framebuffer: VNCFramebuffer? {
        lock.withLock { _framebuffer }
    }

    var isFramebufferReady: Bool {
        lock.withLock { _isFramebufferReady }
    }

    var cursorState: CursorState {
        lock.withLock { _cursorState }
    }

    /// Register a continuation to be resumed on first framebuffer update or error.
    func setConnectContinuation(_ continuation: CheckedContinuation<Void, any Error>) {
        lock.lock()
        defer { lock.unlock() }
        if _isFramebufferReady {
            continuation.resume()
        } else if let error = disconnectError {
            continuation.resume(throwing: error)
        } else {
            connectContinuation = continuation
        }
    }

    /// Resume any pending continuation with an error (used by timeout timer).
    func resumeIfPending(with error: any Error) {
        lock.lock()
        let continuation = connectContinuation
        connectContinuation = nil
        lock.unlock()
        continuation?.resume(throwing: error)
    }

    // MARK: - VNCConnectionDelegate

    func connection(_ connection: VNCConnection,
                    stateDidChange connectionState: VNCConnection.ConnectionState) {
        switch connectionState.status {
        case .connected:
            break
        case .disconnected:
            lock.lock()
            let error = connectionState.error
            let continuation = connectContinuation
            connectContinuation = nil
            _isFramebufferReady = false
            disconnectError = error
            lock.unlock()

            if let continuation {
                continuation.resume(throwing: error ?? VNCCaptureError.disconnected)
            }
        case .connecting, .disconnecting:
            break
        @unknown default:
            break
        }
    }

    func connection(_ connection: VNCConnection,
                    credentialFor authenticationType: VNCAuthenticationType,
                    completion: @escaping (VNCCredential?) -> Void) {
        switch authenticationType {
        case .vnc:
            completion(VNCPasswordCredential(password: password ?? ""))
        case .appleRemoteDesktop:
            completion(VNCUsernamePasswordCredential(username: "", password: password ?? ""))
        default:
            completion(nil)
        }
    }

    func connection(_ connection: VNCConnection,
                    didCreateFramebuffer framebuffer: VNCFramebuffer) {
        lock.withLock { _framebuffer = framebuffer }
    }

    func connection(_ connection: VNCConnection,
                    didResizeFramebuffer framebuffer: VNCFramebuffer) {
        lock.withLock { _framebuffer = framebuffer }
    }

    func connection(_ connection: VNCConnection,
                    didUpdateFramebuffer framebuffer: VNCFramebuffer,
                    x: UInt16, y: UInt16,
                    width: UInt16, height: UInt16) {
        lock.lock()
        _framebuffer = framebuffer
        let wasReady = _isFramebufferReady
        _isFramebufferReady = true
        let continuation = connectContinuation
        connectContinuation = nil
        lock.unlock()

        if !wasReady, let continuation {
            continuation.resume()
        }
    }

    func connection(_ connection: VNCConnection,
                    didUpdateCursor cursor: VNCCursor) {
        lock.lock()
        _cursorState.update(
            size: CGSize(width: CGFloat(cursor.size.width), height: CGFloat(cursor.size.height)),
            hotspot: CGPoint(x: CGFloat(cursor.hotspot.x), y: CGFloat(cursor.hotspot.y))
        )
        lock.unlock()
    }
}
