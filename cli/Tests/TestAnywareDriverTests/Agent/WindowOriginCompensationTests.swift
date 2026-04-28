import Testing
import Foundation
@testable import TestAnywareDriver
import TestAnywareAgentProtocol

@Suite("WindowOriginCompensation")
struct WindowOriginCompensationTests {

    private func makeWindow(x: Double, y: Double) -> WindowInfo {
        WindowInfo(
            title: "Settings",
            windowType: "standard",
            size: CGSize(width: 800, height: 600),
            position: CGPoint(x: x, y: y),
            appName: "TestApp",
            focused: true,
            elements: nil
        )
    }

    @Test func macosSubtractsDefaultTopInset() {
        let win = makeWindow(x: 100, y: 200)
        let off = WindowOriginCompensation.offset(for: win, platform: .macos, environment: [:])
        #expect(off.x == 100)
        #expect(off.y == 160)  // 200 - 40
    }

    @Test func linuxLeavesOriginUnchanged() {
        let win = makeWindow(x: 100, y: 200)
        let off = WindowOriginCompensation.offset(for: win, platform: .linux, environment: [:])
        #expect(off.x == 100)
        #expect(off.y == 200)
    }

    @Test func windowsLeavesOriginUnchanged() {
        let win = makeWindow(x: 100, y: 200)
        let off = WindowOriginCompensation.offset(for: win, platform: .windows, environment: [:])
        #expect(off.x == 100)
        #expect(off.y == 200)
    }

    @Test func nilPlatformLeavesOriginUnchanged() {
        let win = makeWindow(x: 100, y: 200)
        let off = WindowOriginCompensation.offset(for: win, platform: nil, environment: [:])
        #expect(off.x == 100)
        #expect(off.y == 200)
    }

    @Test func envOverrideSetsCustomInset() {
        let win = makeWindow(x: 50, y: 100)
        let off = WindowOriginCompensation.offset(
            for: win, platform: .macos,
            environment: ["TESTANYWARE_WINDOW_TOP_INSET": "25"]
        )
        #expect(off.x == 50)
        #expect(off.y == 75)  // 100 - 25
    }

    @Test func envOverrideZeroDisablesCompensation() {
        let win = makeWindow(x: 0, y: 100)
        let off = WindowOriginCompensation.offset(
            for: win, platform: .macos,
            environment: ["TESTANYWARE_WINDOW_TOP_INSET": "0"]
        )
        #expect(off.y == 100)
    }

    @Test func envOverrideOnlyAffectsMacos() {
        let win = makeWindow(x: 0, y: 100)
        let off = WindowOriginCompensation.offset(
            for: win, platform: .linux,
            environment: ["TESTANYWARE_WINDOW_TOP_INSET": "999"]
        )
        #expect(off.y == 100)  // env var ignored on non-macos
    }

    @Test func nonIntegerEnvFallsBackToDefault() {
        let win = makeWindow(x: 0, y: 100)
        let off = WindowOriginCompensation.offset(
            for: win, platform: .macos,
            environment: ["TESTANYWARE_WINDOW_TOP_INSET": "not-a-number"]
        )
        #expect(off.y == 60)  // 100 - 40 default
    }

    @Test func fractionalAxOriginTruncatesToInt() {
        let win = makeWindow(x: 10.7, y: 200.9)
        let off = WindowOriginCompensation.offset(for: win, platform: .macos, environment: [:])
        #expect(off.x == 10)
        #expect(off.y == 160)  // Int(200.9) - 40 = 200 - 40
    }

    @Test func negativeYAfterCompensationIsAllowed() {
        let win = makeWindow(x: 10, y: 20)
        let off = WindowOriginCompensation.offset(for: win, platform: .macos, environment: [:])
        #expect(off.y == -20)  // 20 - 40; caller may clamp if needed
    }
}
