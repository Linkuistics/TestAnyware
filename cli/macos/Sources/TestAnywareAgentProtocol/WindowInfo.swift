import Foundation

public struct WindowInfo: Codable, Sendable, Equatable {
    public var title: String?
    public var windowType: String
    public var size: CGSize
    public var position: CGPoint
    public var appName: String
    public var focused: Bool
    public var elements: [ElementInfo]?

    public init(
        title: String?,
        windowType: String,
        size: CGSize,
        position: CGPoint,
        appName: String,
        focused: Bool,
        elements: [ElementInfo]?
    ) {
        self.title = title
        self.windowType = windowType
        self.size = size
        self.position = position
        self.appName = appName
        self.focused = focused
        self.elements = elements
    }

    enum CodingKeys: String, CodingKey {
        case title, windowType
        case sizeWidth, sizeHeight
        case positionX, positionY
        case appName, focused, elements
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        title = try c.decodeIfPresent(String.self, forKey: .title)
        windowType = try c.decode(String.self, forKey: .windowType)
        let w = try c.decode(Double.self, forKey: .sizeWidth)
        let h = try c.decode(Double.self, forKey: .sizeHeight)
        size = CGSize(width: w, height: h)
        let x = try c.decode(Double.self, forKey: .positionX)
        let y = try c.decode(Double.self, forKey: .positionY)
        position = CGPoint(x: x, y: y)
        appName = try c.decode(String.self, forKey: .appName)
        focused = try c.decode(Bool.self, forKey: .focused)
        elements = try c.decodeIfPresent([ElementInfo].self, forKey: .elements)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeIfPresent(title, forKey: .title)
        try c.encode(windowType, forKey: .windowType)
        try c.encode(size.width, forKey: .sizeWidth)
        try c.encode(size.height, forKey: .sizeHeight)
        try c.encode(position.x, forKey: .positionX)
        try c.encode(position.y, forKey: .positionY)
        try c.encode(appName, forKey: .appName)
        try c.encode(focused, forKey: .focused)
        try c.encodeIfPresent(elements, forKey: .elements)
    }

    public static func == (lhs: WindowInfo, rhs: WindowInfo) -> Bool {
        lhs.title == rhs.title &&
        lhs.windowType == rhs.windowType &&
        lhs.size.width == rhs.size.width &&
        lhs.size.height == rhs.size.height &&
        lhs.position.x == rhs.position.x &&
        lhs.position.y == rhs.position.y &&
        lhs.appName == rhs.appName &&
        lhs.focused == rhs.focused &&
        lhs.elements == rhs.elements
    }
}
