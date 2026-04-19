import Testing
import RoyalVNCKit
@testable import TestAnywareDriver

@Suite("VNCInput")
struct VNCInputTests {

    // MARK: - Shifted character detection

    @Test func identifiesShiftedSymbols() {
        let shifted: [Character] = ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")",
                                     "~", "_", "+", "{", "}", "|", ":", "\"", "<", ">", "?"]
        for char in shifted {
            #expect(PlatformKeymap.shiftedCharToBase[char] != nil, "Expected \(char) to be shifted")
        }
    }

    @Test func unshiftedSymbolsNotInMap() {
        let unshifted: [Character] = ["-", "=", "[", "]", "\\", ";", "'", ",", ".", "/"]
        for char in unshifted {
            #expect(PlatformKeymap.shiftedCharToBase[char] == nil, "Expected \(char) to NOT be shifted")
        }
    }

    // MARK: - Drag interpolation math

    @Test func dragInterpolatesLinearly() {
        let steps = 10
        for i in 1...steps {
            let t = Double(i) / Double(steps)
            let x = UInt16(Double(0) + (Double(100) - Double(0)) * t)
            let y = UInt16(Double(0) + (Double(200) - Double(0)) * t)
            if i == steps {
                #expect(x == 100)
                #expect(y == 200)
            }
            if i == 5 {
                #expect(x == 50)
                #expect(y == 100)
            }
        }
    }

    // MARK: - API surface exists

    @Test func pressKeySignatureExists() {
        let _: (String, [String], Platform?, VNCConnection) throws -> Void = VNCInput.pressKey
    }

    @Test func typeTextSignatureExists() {
        let _: (String, VNCConnection) -> Void = VNCInput.typeText
    }

    @Test func clickSignatureExists() {
        let _: (UInt16, UInt16, String, Int, VNCConnection) throws -> Void = VNCInput.click
    }

    @Test func mouseMoveSignatureExists() {
        let _: (UInt16, UInt16, VNCConnection) -> Void = VNCInput.mouseMove
    }

    @Test func scrollSignatureExists() {
        let _: (UInt16, UInt16, Int, Int, VNCConnection) -> Void = VNCInput.scroll
    }

    @Test func dragSignatureExists() {
        let _: (UInt16, UInt16, UInt16, UInt16, String, Int, VNCConnection) throws -> Void = VNCInput.drag
    }
}
