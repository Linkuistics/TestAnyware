import Testing
import CoreGraphics
@testable import TestAnywareDriver

@Suite("CursorState")
struct CursorStateTests {
    @Test func defaultsToNil() {
        let state = CursorState()
        #expect(state.position == nil)
        #expect(state.hotspot == nil)
        #expect(state.size == nil)
    }

    @Test func updatesPosition() {
        var state = CursorState()
        state.update(position: CGPoint(x: 100, y: 200))
        #expect(state.position == CGPoint(x: 100, y: 200))
    }

    @Test func updatesCursorShape() {
        var state = CursorState()
        state.update(size: CGSize(width: 16, height: 16), hotspot: CGPoint(x: 8, y: 8))
        #expect(state.size == CGSize(width: 16, height: 16))
        #expect(state.hotspot == CGPoint(x: 8, y: 8))
    }
}
