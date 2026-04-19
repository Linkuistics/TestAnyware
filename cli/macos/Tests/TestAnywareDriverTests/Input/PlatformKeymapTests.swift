import Testing
import RoyalVNCKit
@testable import TestAnywareDriver

@Suite("PlatformKeymap")
struct PlatformKeymapTests {

    // MARK: - Base key resolution

    @Test func resolvesLetterKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "a").rawValue == UInt32(UInt8(ascii: "a")))
        #expect(try PlatformKeymap.keyCode(for: "z").rawValue == UInt32(UInt8(ascii: "z")))
    }

    @Test func resolvesNumberKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "0").rawValue == UInt32(UInt8(ascii: "0")))
        #expect(try PlatformKeymap.keyCode(for: "9").rawValue == UInt32(UInt8(ascii: "9")))
    }

    @Test func resolvesSpecialKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "return") == .return)
        #expect(try PlatformKeymap.keyCode(for: "enter") == .return)
        #expect(try PlatformKeymap.keyCode(for: "tab") == .tab)
        #expect(try PlatformKeymap.keyCode(for: "escape") == .escape)
        #expect(try PlatformKeymap.keyCode(for: "space") == .space)
        #expect(try PlatformKeymap.keyCode(for: "delete") == .delete)
        #expect(try PlatformKeymap.keyCode(for: "backspace") == .delete)
    }

    @Test func resolvesArrowKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "up") == .upArrow)
        #expect(try PlatformKeymap.keyCode(for: "down") == .downArrow)
        #expect(try PlatformKeymap.keyCode(for: "left") == .leftArrow)
        #expect(try PlatformKeymap.keyCode(for: "right") == .rightArrow)
    }

    @Test func resolvesFunctionKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "f1") == .f1)
        #expect(try PlatformKeymap.keyCode(for: "f12") == .f12)
    }

    @Test func resolvesExtendedFunctionKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "f13") == .f13)
        #expect(try PlatformKeymap.keyCode(for: "f14") == .f14)
        #expect(try PlatformKeymap.keyCode(for: "f15") == .f15)
        #expect(try PlatformKeymap.keyCode(for: "f16") == .f16)
        #expect(try PlatformKeymap.keyCode(for: "f17") == .f17)
        #expect(try PlatformKeymap.keyCode(for: "f18") == .f18)
        #expect(try PlatformKeymap.keyCode(for: "f19") == .f19)
    }

    @Test func extendedFunctionKeysCarryX11Keysyms() throws {
        // X11 keysyms XK_F13..XK_F19 = 0xFFCA..0xFFD0
        #expect(try PlatformKeymap.keyCode(for: "f13").rawValue == 0xFFCA)
        #expect(try PlatformKeymap.keyCode(for: "f18").rawValue == 0xFFCF)
        #expect(try PlatformKeymap.keyCode(for: "f19").rawValue == 0xFFD0)
    }

    @Test func resolvesNavigationKeys() throws {
        #expect(try PlatformKeymap.keyCode(for: "home") == .home)
        #expect(try PlatformKeymap.keyCode(for: "end") == .end)
        #expect(try PlatformKeymap.keyCode(for: "pageup") == .pageUp)
        #expect(try PlatformKeymap.keyCode(for: "pagedown") == .pageDown)
    }

    @Test func isCaseInsensitive() throws {
        #expect(try PlatformKeymap.keyCode(for: "Return") == .return)
        #expect(try PlatformKeymap.keyCode(for: "ESCAPE") == .escape)
    }

    @Test func throwsForUnknownKey() {
        #expect(throws: PlatformKeymapError.self) {
            try PlatformKeymap.keyCode(for: "nonexistent")
        }
    }

    // MARK: - Platform-specific modifiers

    @Test func macOSCmdMapsToAltL() {
        let code = PlatformKeymap.modifierKeyCode(for: "cmd", platform: .macos)
        #expect(code == .option)  // XK_Alt_L -> Cmd on Virtualization.framework
    }

    @Test func windowsCmdMapsToCtrl() {
        let code = PlatformKeymap.modifierKeyCode(for: "cmd", platform: .windows)
        #expect(code == .control)
    }

    @Test func linuxCmdMapsToCtrl() {
        let code = PlatformKeymap.modifierKeyCode(for: "cmd", platform: .linux)
        #expect(code == .control)
    }

    @Test func macOSAltMapsToMetaL() {
        let code = PlatformKeymap.modifierKeyCode(for: "alt", platform: .macos)
        #expect(code == .optionForARD)  // XK_Meta_L
    }

    @Test func windowsAltMapsToAltL() {
        let code = PlatformKeymap.modifierKeyCode(for: "alt", platform: .windows)
        #expect(code == .option)  // XK_Alt_L
    }

    @Test func shiftIsUniversal() {
        #expect(PlatformKeymap.modifierKeyCode(for: "shift", platform: .macos) == .shift)
        #expect(PlatformKeymap.modifierKeyCode(for: "shift", platform: .windows) == .shift)
        #expect(PlatformKeymap.modifierKeyCode(for: "shift", platform: .linux) == .shift)
    }

    @Test func ctrlIsUniversal() {
        #expect(PlatformKeymap.modifierKeyCode(for: "ctrl", platform: .macos) == .control)
        #expect(PlatformKeymap.modifierKeyCode(for: "ctrl", platform: .windows) == .control)
    }

    @Test func unknownModifierReturnsNil() {
        #expect(PlatformKeymap.modifierKeyCode(for: "unknown", platform: .macos) == nil)
    }

    @Test func defaultPlatformIsMacOS() {
        #expect(PlatformKeymap.modifierKeyCode(for: "cmd", platform: nil) == .option)
    }

    @Test func modifierIsCaseInsensitive() {
        #expect(PlatformKeymap.modifierKeyCode(for: "CMD", platform: .macos) == .option)
        #expect(PlatformKeymap.modifierKeyCode(for: "Shift", platform: .windows) == .shift)
    }

    // MARK: - Mouse buttons

    @Test func resolvesMouseButtons() throws {
        #expect(try PlatformKeymap.mouseButton(for: "left") == .left)
        #expect(try PlatformKeymap.mouseButton(for: "right") == .right)
        #expect(try PlatformKeymap.mouseButton(for: "middle") == .middle)
        #expect(try PlatformKeymap.mouseButton(for: "center") == .middle)
    }

    @Test func mouseButtonIsCaseInsensitive() throws {
        #expect(try PlatformKeymap.mouseButton(for: "Left") == .left)
        #expect(try PlatformKeymap.mouseButton(for: "RIGHT") == .right)
    }

    @Test func throwsForUnknownMouseButton() {
        #expect(throws: PlatformKeymapError.self) {
            try PlatformKeymap.mouseButton(for: "extra")
        }
    }

    // MARK: - Scroll decomposition

    @Test func decomposesScrollUp() {
        let components = PlatformKeymap.decomposeScroll(deltaX: 0, deltaY: -3)
        #expect(components == [.init(direction: .up, steps: 3)])
    }

    @Test func decomposesScrollDown() {
        let components = PlatformKeymap.decomposeScroll(deltaX: 0, deltaY: 3)
        #expect(components == [.init(direction: .down, steps: 3)])
    }

    @Test func decomposesScrollBothAxes() {
        let components = PlatformKeymap.decomposeScroll(deltaX: 2, deltaY: -1)
        #expect(components == [
            .init(direction: .up, steps: 1),
            .init(direction: .right, steps: 2),
        ])
    }

    @Test func decomposesScrollZero() {
        #expect(PlatformKeymap.decomposeScroll(deltaX: 0, deltaY: 0).isEmpty)
    }

    // MARK: - Batch modifier resolution

    @Test func resolvesMultipleModifiers() {
        let codes = PlatformKeymap.resolveModifiers(["cmd", "shift"], platform: .macos)
        #expect(codes.count == 2)
        #expect(codes.contains(.option))
        #expect(codes.contains(.shift))
    }

    @Test func resolveModifiersFiltersUnknown() {
        let codes = PlatformKeymap.resolveModifiers(["shift", "bogus", "ctrl"], platform: .macos)
        #expect(codes.count == 2)
    }
}
