import CoreGraphics

/// Tracks cursor shape and position as reported by the VNC server.
public struct CursorState: Sendable {
    /// Current cursor position in framebuffer coordinates.
    public private(set) var position: CGPoint?
    /// Cursor hotspot offset within the cursor image.
    public private(set) var hotspot: CGPoint?
    /// Cursor image dimensions.
    public private(set) var size: CGSize?
    /// Raw cursor pixel data (RGBA).
    public private(set) var imageData: [UInt8]?

    public init() {}

    public mutating func update(position: CGPoint) {
        self.position = position
    }

    public mutating func update(size: CGSize, hotspot: CGPoint, imageData: [UInt8]? = nil) {
        self.size = size
        self.hotspot = hotspot
        self.imageData = imageData
    }
}
