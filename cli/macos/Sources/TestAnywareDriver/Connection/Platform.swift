import Foundation

/// Target platform hint, used for keysym mapping.
public enum Platform: String, Codable, Sendable {
    case macos
    case windows
    case linux
}
