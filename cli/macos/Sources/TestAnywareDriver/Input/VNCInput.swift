import Foundation
@preconcurrency import RoyalVNCKit

/// VNC input: keyboard, mouse, and scroll operations.
public enum VNCInput {

    /// Whether to bypass RoyalVNCKit's ARD keysym remapping.
    private static let useRawKeysyms: Bool = {
        ProcessInfo.processInfo.environment["TESTANYWARE_VNC_ARD_REMAP"] != "1"
    }()

    // MARK: - Keyboard

    /// Press and release a key with optional modifiers.
    public static func pressKey(
        _ key: String,
        modifiers: [String] = [],
        platform: Platform?,
        connection: VNCConnection
    ) throws {
        let keyCode = try PlatformKeymap.keyCode(for: key)
        let modCodes = PlatformKeymap.resolveModifiers(modifiers, platform: platform)

        if useRawKeysyms && !modCodes.isEmpty {
            for mod in modCodes { connection.keyDownRaw(mod.rawValue) }
            Thread.sleep(forTimeInterval: 0.05)
            connection.keyDownRaw(keyCode.rawValue)
            connection.keyUpRaw(keyCode.rawValue)
            Thread.sleep(forTimeInterval: 0.05)
            for mod in modCodes.reversed() { connection.keyUpRaw(mod.rawValue) }
        } else {
            for mod in modCodes { connection.keyDown(mod) }
            if !modCodes.isEmpty { Thread.sleep(forTimeInterval: 0.05) }
            connection.keyDown(keyCode)
            connection.keyUp(keyCode)
            if !modCodes.isEmpty { Thread.sleep(forTimeInterval: 0.05) }
            for mod in modCodes.reversed() { connection.keyUp(mod) }
        }
    }

    /// Type a string by sending individual character key events.
    public static func typeText(_ text: String, connection: VNCConnection) {
        let shiftCode = VNCKeyCode.shift

        for char in text {
            if char.isUppercase, let lower = char.lowercased().first {
                let keyCodes = VNCKeyCode.withCharacter(lower)
                sendShifted(keyCodes: keyCodes, shiftCode: shiftCode, connection: connection)
            } else if let baseASCII = PlatformKeymap.shiftedCharToBase[char] {
                let keyCode = VNCKeyCode(asciiCharacter: baseASCII)
                sendShifted(keyCodes: [keyCode], shiftCode: shiftCode, connection: connection)
            } else {
                let keyCodes = VNCKeyCode.withCharacter(char)
                for code in keyCodes {
                    if useRawKeysyms {
                        connection.keyDownRaw(code.rawValue)
                        connection.keyUpRaw(code.rawValue)
                    } else {
                        connection.keyDown(code)
                        connection.keyUp(code)
                    }
                }
            }
        }
    }

    private static func sendShifted(keyCodes: [VNCKeyCode], shiftCode: VNCKeyCode, connection: VNCConnection) {
        if useRawKeysyms {
            connection.keyDownRaw(shiftCode.rawValue)
            Thread.sleep(forTimeInterval: 0.05)
            for code in keyCodes {
                connection.keyDownRaw(code.rawValue)
                connection.keyUpRaw(code.rawValue)
            }
            Thread.sleep(forTimeInterval: 0.05)
            connection.keyUpRaw(shiftCode.rawValue)
        } else {
            connection.keyDown(shiftCode)
            Thread.sleep(forTimeInterval: 0.05)
            for code in keyCodes {
                connection.keyDown(code)
                connection.keyUp(code)
            }
            Thread.sleep(forTimeInterval: 0.05)
            connection.keyUp(shiftCode)
        }
    }

    // MARK: - Key Down / Up

    /// Send a key-down event (without releasing).
    public static func keyDown(_ key: String, platform: Platform?, connection: VNCConnection) throws {
        let keyCode = try PlatformKeymap.keyCode(for: key)
        if useRawKeysyms {
            connection.keyDownRaw(keyCode.rawValue)
        } else {
            connection.keyDown(keyCode)
        }
    }

    /// Send a key-up event.
    public static func keyUp(_ key: String, platform: Platform?, connection: VNCConnection) throws {
        let keyCode = try PlatformKeymap.keyCode(for: key)
        if useRawKeysyms {
            connection.keyUpRaw(keyCode.rawValue)
        } else {
            connection.keyUp(keyCode)
        }
    }

    // MARK: - Mouse

    /// Move mouse pointer to absolute coordinates.
    public static func mouseMove(x: UInt16, y: UInt16, connection: VNCConnection) {
        connection.mouseMove(x: x, y: y)
    }

    /// Send a mouse-button-down event at coordinates.
    public static func mouseDown(
        x: UInt16, y: UInt16,
        button: String = "left",
        connection: VNCConnection
    ) throws {
        let btn = try PlatformKeymap.mouseButton(for: button)
        connection.mouseButtonDown(btn, x: x, y: y)
    }

    /// Send a mouse-button-up event at coordinates.
    public static func mouseUp(
        x: UInt16, y: UInt16,
        button: String = "left",
        connection: VNCConnection
    ) throws {
        let btn = try PlatformKeymap.mouseButton(for: button)
        connection.mouseButtonUp(btn, x: x, y: y)
    }

    /// Click at coordinates with optional button and count.
    public static func click(
        x: UInt16, y: UInt16,
        button: String = "left",
        count: Int = 1,
        connection: VNCConnection
    ) throws {
        let btn = try PlatformKeymap.mouseButton(for: button)
        for _ in 0..<count {
            connection.mouseButtonDown(btn, x: x, y: y)
            connection.mouseButtonUp(btn, x: x, y: y)
        }
    }

    /// Scroll at coordinates.
    public static func scroll(
        x: UInt16, y: UInt16,
        deltaX: Int, deltaY: Int,
        connection: VNCConnection
    ) {
        let components = PlatformKeymap.decomposeScroll(deltaX: deltaX, deltaY: deltaY)
        for component in components {
            connection.mouseWheel(component.direction, x: x, y: y, steps: component.steps)
        }
    }

    /// Drag from one point to another with interpolated steps.
    public static func drag(
        fromX: UInt16, fromY: UInt16,
        toX: UInt16, toY: UInt16,
        button: String = "left",
        steps: Int = 10,
        connection: VNCConnection
    ) throws {
        let btn = try PlatformKeymap.mouseButton(for: button)
        connection.mouseButtonDown(btn, x: fromX, y: fromY)

        let effectiveSteps = max(steps, 1)
        for i in 1...effectiveSteps {
            let t = Double(i) / Double(effectiveSteps)
            let x = UInt16(Double(fromX) + (Double(toX) - Double(fromX)) * t)
            let y = UInt16(Double(fromY) + (Double(toY) - Double(fromY)) * t)
            connection.mouseMove(x: x, y: y)
        }

        connection.mouseButtonUp(btn, x: toX, y: toY)
    }
}
