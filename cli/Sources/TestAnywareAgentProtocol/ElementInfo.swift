import Foundation

public struct ElementInfo: Codable, Sendable, Equatable {
    public var role: UnifiedRole
    public var label: String?
    public var value: String?
    public var description: String?
    public var id: String?
    public var enabled: Bool
    public var focused: Bool
    public var showing: Bool?
    public var position: CGPoint?
    public var size: CGSize?
    public var childCount: Int
    public var actions: [String]
    public var platformRole: String?
    public var children: [ElementInfo]?

    public init(
        role: UnifiedRole,
        label: String?,
        value: String?,
        description: String?,
        id: String?,
        enabled: Bool,
        focused: Bool,
        showing: Bool? = nil,
        position: CGPoint?,
        size: CGSize?,
        childCount: Int,
        actions: [String],
        platformRole: String?,
        children: [ElementInfo]?
    ) {
        self.role = role
        self.label = label
        self.value = value
        self.description = description
        self.id = id
        self.enabled = enabled
        self.focused = focused
        self.showing = showing
        self.position = position
        self.size = size
        self.childCount = childCount
        self.actions = actions
        self.platformRole = platformRole
        self.children = children
    }

    enum CodingKeys: String, CodingKey {
        case role, label, value, description, id
        case enabled, focused, showing
        case positionX, positionY
        case sizeWidth, sizeHeight
        case childCount, actions, platformRole, children
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        role = try c.decode(UnifiedRole.self, forKey: .role)
        label = try c.decodeIfPresent(String.self, forKey: .label)
        value = try c.decodeIfPresent(String.self, forKey: .value)
        description = try c.decodeIfPresent(String.self, forKey: .description)
        id = try c.decodeIfPresent(String.self, forKey: .id)
        enabled = try c.decode(Bool.self, forKey: .enabled)
        focused = try c.decode(Bool.self, forKey: .focused)
        showing = try c.decodeIfPresent(Bool.self, forKey: .showing)
        if let x = try c.decodeIfPresent(Double.self, forKey: .positionX),
           let y = try c.decodeIfPresent(Double.self, forKey: .positionY) {
            position = CGPoint(x: x, y: y)
        } else {
            position = nil
        }
        if let w = try c.decodeIfPresent(Double.self, forKey: .sizeWidth),
           let h = try c.decodeIfPresent(Double.self, forKey: .sizeHeight) {
            size = CGSize(width: w, height: h)
        } else {
            size = nil
        }
        childCount = try c.decode(Int.self, forKey: .childCount)
        actions = try c.decode([String].self, forKey: .actions)
        platformRole = try c.decodeIfPresent(String.self, forKey: .platformRole)
        children = try c.decodeIfPresent([ElementInfo].self, forKey: .children)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(role, forKey: .role)
        try c.encodeIfPresent(label, forKey: .label)
        try c.encodeIfPresent(value, forKey: .value)
        try c.encodeIfPresent(description, forKey: .description)
        try c.encodeIfPresent(id, forKey: .id)
        try c.encode(enabled, forKey: .enabled)
        try c.encode(focused, forKey: .focused)
        try c.encodeIfPresent(showing, forKey: .showing)
        if let p = position {
            try c.encode(p.x, forKey: .positionX)
            try c.encode(p.y, forKey: .positionY)
        }
        if let s = size {
            try c.encode(s.width, forKey: .sizeWidth)
            try c.encode(s.height, forKey: .sizeHeight)
        }
        try c.encode(childCount, forKey: .childCount)
        try c.encode(actions, forKey: .actions)
        try c.encodeIfPresent(platformRole, forKey: .platformRole)
        try c.encodeIfPresent(children, forKey: .children)
    }

    public static func == (lhs: ElementInfo, rhs: ElementInfo) -> Bool {
        lhs.role == rhs.role &&
        lhs.label == rhs.label &&
        lhs.value == rhs.value &&
        lhs.description == rhs.description &&
        lhs.id == rhs.id &&
        lhs.enabled == rhs.enabled &&
        lhs.focused == rhs.focused &&
        lhs.showing == rhs.showing &&
        cgPointEqual(lhs.position, rhs.position) &&
        cgSizeEqual(lhs.size, rhs.size) &&
        lhs.childCount == rhs.childCount &&
        lhs.actions == rhs.actions &&
        lhs.platformRole == rhs.platformRole &&
        lhs.children == rhs.children
    }
}

private func cgPointEqual(_ lhs: CGPoint?, _ rhs: CGPoint?) -> Bool {
    switch (lhs, rhs) {
    case (nil, nil): return true
    case (nil, _), (_, nil): return false
    case let (l?, r?): return l.x == r.x && l.y == r.y
    }
}

private func cgSizeEqual(_ lhs: CGSize?, _ rhs: CGSize?) -> Bool {
    switch (lhs, rhs) {
    case (nil, nil): return true
    case (nil, _), (_, nil): return false
    case let (l?, r?): return l.width == r.width && l.height == r.height
    }
}
