import Testing
import CoreGraphics
@testable import TestAnywareDriver

@Suite("VNCCapture")
struct VNCCaptureTests {
    @Test func initFromSpec() async {
        let spec = VNCSpec(host: "testhost", port: 5901, password: "pass")
        let capture = VNCCapture(spec: spec)
        let size = await capture.screenSize()
        #expect(size == nil)  // not connected yet
    }

    @Test func initFromHostPort() async {
        let capture = VNCCapture(host: "localhost", port: 5900)
        let size = await capture.screenSize()
        #expect(size == nil)
    }

    @Test func captureImageThrowsWhenNotConnected() async {
        let capture = VNCCapture(host: "localhost")
        do {
            _ = try await capture.captureImage()
            Issue.record("Expected VNCCaptureError.notConfigured")
        } catch {
            #expect(error is VNCCaptureError)
        }
    }

    @Test func withConnectionThrowsWhenNotConnected() async {
        let capture = VNCCapture(host: "localhost")
        do {
            _ = try await capture.withConnection { _ in 42 }
            Issue.record("Expected VNCCaptureError.notConfigured")
        } catch {
            #expect(error is VNCCaptureError)
        }
    }

    @Test func cursorStateStartsEmpty() async {
        let capture = VNCCapture(host: "localhost")
        let cursor = await capture.cursorState
        #expect(cursor.position == nil)
        #expect(cursor.size == nil)
    }
}
