import Foundation
@preconcurrency import RoyalVNCKit

/// Platform-aware keysym mapping for VNC input.
public enum PlatformKeymap {

    // MARK: - Key resolution

    public static func keyCode(for key: String) throws -> VNCKeyCode {
        guard let code = keyCodes[key.lowercased()] else {
            throw PlatformKeymapError.unknownKey(key)
        }
        return code
    }

    public static func modifierKeyCode(for modifier: String, platform: Platform?) -> VNCKeyCode? {
        modifierKeyCodes(for: platform ?? .macos)[modifier.lowercased()]
    }

    public static func resolveModifiers(_ modifiers: [String], platform: Platform?) -> [VNCKeyCode] {
        modifiers.compactMap { modifierKeyCode(for: $0, platform: platform) }
    }

    // MARK: - Mouse

    public static func mouseButton(for name: String) throws -> VNCMouseButton {
        guard let button = mouseButtons[name.lowercased()] else {
            throw PlatformKeymapError.unknownButton(name)
        }
        return button
    }

    // MARK: - Scroll

    public struct ScrollComponent: Equatable, Sendable {
        public let direction: VNCMouseWheel
        public let steps: UInt32

        public init(direction: VNCMouseWheel, steps: UInt32) {
            self.direction = direction
            self.steps = steps
        }
    }

    public static func decomposeScroll(deltaX: Int, deltaY: Int) -> [ScrollComponent] {
        var components: [ScrollComponent] = []
        if deltaY < 0 {
            components.append(.init(direction: .up, steps: UInt32(abs(deltaY))))
        } else if deltaY > 0 {
            components.append(.init(direction: .down, steps: UInt32(deltaY)))
        }
        if deltaX < 0 {
            components.append(.init(direction: .left, steps: UInt32(abs(deltaX))))
        } else if deltaX > 0 {
            components.append(.init(direction: .right, steps: UInt32(deltaX)))
        }
        return components
    }

    // MARK: - Shifted character map

    /// Characters requiring Shift on a US keyboard mapped to their unshifted base ASCII value.
    static let shiftedCharToBase: [Character: UInt8] = [
        "!": 0x31, "@": 0x32, "#": 0x33, "$": 0x34, "%": 0x35,
        "^": 0x36, "&": 0x37, "*": 0x38, "(": 0x39, ")": 0x30,
        "~": 0x60, "_": 0x2d, "+": 0x3d,
        "{": 0x5b, "}": 0x5d, "|": 0x5c,
        ":": 0x3b, "\"": 0x27,
        "<": 0x2c, ">": 0x2e, "?": 0x2f,
    ]

    // MARK: - Key tables

    private nonisolated(unsafe) static let keyCodes: [String: VNCKeyCode] = {
        var map: [String: VNCKeyCode] = [:]
        for c: UInt8 in UInt8(ascii: "a")...UInt8(ascii: "z") {
            map[String(UnicodeScalar(c))] = VNCKeyCode(asciiCharacter: c)
        }
        for c: UInt8 in UInt8(ascii: "0")...UInt8(ascii: "9") {
            map[String(UnicodeScalar(c))] = VNCKeyCode(asciiCharacter: c)
        }
        map["return"] = .return
        map["enter"] = .return
        map["tab"] = .tab
        map["escape"] = .escape
        map["esc"] = .escape
        map["space"] = .space
        map["delete"] = .delete
        map["backspace"] = .delete
        map["forwarddelete"] = .forwardDelete
        map["up"] = .upArrow
        map["down"] = .downArrow
        map["left"] = .leftArrow
        map["right"] = .rightArrow
        map["home"] = .home
        map["end"] = .end
        map["pageup"] = .pageUp
        map["pagedown"] = .pageDown
        map["f1"] = .f1;  map["f2"] = .f2;  map["f3"] = .f3;  map["f4"] = .f4
        map["f5"] = .f5;  map["f6"] = .f6;  map["f7"] = .f7;  map["f8"] = .f8
        map["f9"] = .f9;  map["f10"] = .f10; map["f11"] = .f11; map["f12"] = .f12
        map["f13"] = .f13; map["f14"] = .f14; map["f15"] = .f15; map["f16"] = .f16
        map["f17"] = .f17; map["f18"] = .f18; map["f19"] = .f19
        return map
    }()

    private static func modifierKeyCodes(for platform: Platform) -> [String: VNCKeyCode] {
        switch platform {
        case .macos:
            return [
                "cmd": .option, "command": .option,       // XK_Alt_L -> Cmd
                "alt": .optionForARD, "option": .optionForARD,  // XK_Meta_L -> Option
                "shift": .shift,
                "ctrl": .control, "control": .control,
            ]
        case .windows, .linux:
            return [
                "cmd": .control, "command": .control,
                "alt": .option, "option": .option,        // XK_Alt_L
                "shift": .shift,
                "ctrl": .control, "control": .control,
                "super": .command, "win": .command,       // XK_Super_L
            ]
        }
    }

    private nonisolated(unsafe) static let mouseButtons: [String: VNCMouseButton] = [
        "left": .left, "right": .right,
        "middle": .middle, "center": .middle,
    ]
}

public enum PlatformKeymapError: Error, LocalizedError {
    case unknownKey(String)
    case unknownButton(String)

    public var errorDescription: String? {
        switch self {
        case .unknownKey(let key): "Unknown key: '\(key)'"
        case .unknownButton(let button): "Unknown mouse button: '\(button)'"
        }
    }
}
