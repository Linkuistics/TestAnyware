import Foundation

public struct SnapshotResponse: Codable, Sendable, Equatable {
    public var windows: [WindowInfo]

    public init(windows: [WindowInfo]) {
        self.windows = windows
    }
}

public struct ActionResponse: Codable, Sendable, Equatable {
    public var success: Bool
    public var message: String?

    public init(success: Bool, message: String?) {
        self.success = success
        self.message = message
    }
}

public struct ErrorResponse: Codable, Sendable, Equatable {
    public var error: String
    public var details: String?

    public init(error: String, details: String?) {
        self.error = error
        self.details = details
    }
}

public struct InspectResponse: Codable, Sendable, Equatable {
    public var element: ElementInfo
    public var fontFamily: String?
    public var fontSize: Double?
    public var fontWeight: String?
    public var textColor: String?
    public var bounds: CGRect?

    public init(
        element: ElementInfo,
        fontFamily: String?,
        fontSize: Double?,
        fontWeight: String?,
        textColor: String?,
        bounds: CGRect?
    ) {
        self.element = element
        self.fontFamily = fontFamily
        self.fontSize = fontSize
        self.fontWeight = fontWeight
        self.textColor = textColor
        self.bounds = bounds
    }

    enum CodingKeys: String, CodingKey {
        case element, fontFamily, fontSize, fontWeight, textColor
        case boundsX, boundsY, boundsWidth, boundsHeight
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        element = try c.decode(ElementInfo.self, forKey: .element)
        fontFamily = try c.decodeIfPresent(String.self, forKey: .fontFamily)
        fontSize = try c.decodeIfPresent(Double.self, forKey: .fontSize)
        fontWeight = try c.decodeIfPresent(String.self, forKey: .fontWeight)
        textColor = try c.decodeIfPresent(String.self, forKey: .textColor)
        if let bx = try c.decodeIfPresent(Double.self, forKey: .boundsX),
           let by = try c.decodeIfPresent(Double.self, forKey: .boundsY),
           let bw = try c.decodeIfPresent(Double.self, forKey: .boundsWidth),
           let bh = try c.decodeIfPresent(Double.self, forKey: .boundsHeight) {
            bounds = CGRect(origin: CGPoint(x: bx, y: by), size: CGSize(width: bw, height: bh))
        } else {
            bounds = nil
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(element, forKey: .element)
        try c.encodeIfPresent(fontFamily, forKey: .fontFamily)
        try c.encodeIfPresent(fontSize, forKey: .fontSize)
        try c.encodeIfPresent(fontWeight, forKey: .fontWeight)
        try c.encodeIfPresent(textColor, forKey: .textColor)
        if let b = bounds {
            try c.encode(b.origin.x, forKey: .boundsX)
            try c.encode(b.origin.y, forKey: .boundsY)
            try c.encode(b.size.width, forKey: .boundsWidth)
            try c.encode(b.size.height, forKey: .boundsHeight)
        }
    }

    public static func == (lhs: InspectResponse, rhs: InspectResponse) -> Bool {
        lhs.element == rhs.element &&
        lhs.fontFamily == rhs.fontFamily &&
        lhs.fontSize == rhs.fontSize &&
        lhs.fontWeight == rhs.fontWeight &&
        lhs.textColor == rhs.textColor &&
        cgRectEqual(lhs.bounds, rhs.bounds)
    }
}

private func cgRectEqual(_ lhs: CGRect?, _ rhs: CGRect?) -> Bool {
    switch (lhs, rhs) {
    case (nil, nil): return true
    case (nil, _), (_, nil): return false
    case let (l?, r?):
        return l.origin.x == r.origin.x && l.origin.y == r.origin.y &&
               l.size.width == r.size.width && l.size.height == r.size.height
    }
}
