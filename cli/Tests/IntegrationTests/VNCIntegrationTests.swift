import Testing
import Foundation
import AVFoundation
import CoreGraphics
import CoreMedia
import RoyalVNCKit
import TestAnywareDriver

// MARK: - Environment-driven connection setup

/// Reads VNC/agent endpoints from environment variables.
/// No VM management — the caller is responsible for providing a running VM.
///
/// Resolution (first match wins):
///   TESTANYWARE_VM_ID=<id>             Load spec from ~/.testanyware/vms/<id>.json
///   TESTANYWARE_VNC=host:port          Direct VNC endpoint (with
///                                    TESTANYWARE_VNC_PASSWORD / TESTANYWARE_AGENT /
///                                    TESTANYWARE_PLATFORM as needed)
///
/// Opt-out:
///   TESTANYWARE_SKIP_INTEGRATION=1     Skip all integration tests
///
/// Typical workflow:
///   export TESTANYWARE_VM_ID=$(scripts/macos/vm-start.sh)
///   swift test --filter IntegrationTests
///   scripts/macos/vm-stop.sh "$TESTANYWARE_VM_ID"
enum TestEnv {
    static let spec: ConnectionSpec? = {
        let env = ProcessInfo.processInfo.environment

        if let id = env["TESTANYWARE_VM_ID"], !id.isEmpty {
            let path = ConnectionSpec.namedSpecPath(for: id)
            return try? ConnectionSpec.load(from: path)
        }

        guard let vnc = env["TESTANYWARE_VNC"] else { return nil }
        let password = env["TESTANYWARE_VNC_PASSWORD"]
        let agent = env["TESTANYWARE_AGENT"]
        let platform = env["TESTANYWARE_PLATFORM"]

        guard var spec = try? ConnectionSpec.from(vnc: vnc, agent: agent, platform: platform) else {
            return nil
        }

        if let password, !password.isEmpty {
            spec = ConnectionSpec(
                vnc: VNCSpec(host: spec.vnc.host, port: spec.vnc.port, password: password),
                agent: spec.agent,
                platform: spec.platform
            )
        }

        return spec
    }()

    static let agent: AgentTCPClient? = {
        guard let spec, let agentSpec = spec.agent else { return nil }
        return AgentTCPClient(spec: agentSpec)
    }()
}

private func integrationEnabled() -> Bool {
    let env = ProcessInfo.processInfo.environment
    guard env["TESTANYWARE_SKIP_INTEGRATION"] != "1" else { return false }
    return env["TESTANYWARE_VM_ID"] != nil || env["TESTANYWARE_VNC"] != nil
}

// MARK: - VNC Integration Tests

@Suite("VNC Integration",
       .enabled(if: integrationEnabled()),
       .serialized)
struct VNCIntegrationTests {

    // MARK: - Connection

    @Test func connectsAndReportsScreenSize() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let size = await capture.screenSize()
        #expect(size != nil, "Screen size should be available after connect")
        #expect(size!.width >= 1024, "Screen width should be at least 1024")
        #expect(size!.height >= 768, "Screen height should be at least 768")
    }

    @Test func reconnectsAfterDisconnect() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)

        try await capture.connect(timeout: .seconds(30))
        let size1 = await capture.screenSize()
        #expect(size1 != nil)
        await capture.disconnect()

        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }
        let size2 = await capture.screenSize()
        #expect(size2 != nil)
        #expect(size1!.width == size2!.width, "Screen size should be consistent across reconnects")
    }

    // MARK: - Screenshot Capture

    @Test func capturesFullScreenshot() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let image = try await capture.captureImage()
        let size = await capture.screenSize()!
        #expect(image.width == Int(size.width), "Image width should match screen width")
        #expect(image.height == Int(size.height), "Image height should match screen height")
    }

    @Test func capturesValidPNG() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let png = try await capture.screenshot()
        #expect(png.count > 1000, "PNG should have substantial data")
        #expect(png[0] == 0x89)
        #expect(png[1] == 0x50) // P
        #expect(png[2] == 0x4E) // N
        #expect(png[3] == 0x47) // G

        let tmpPath = NSTemporaryDirectory() + "testanyware-test-\(UUID().uuidString).png"
        defer { try? FileManager.default.removeItem(atPath: tmpPath) }
        try png.write(to: URL(fileURLWithPath: tmpPath))
        let readBack = try Data(contentsOf: URL(fileURLWithPath: tmpPath))
        #expect(readBack.count == png.count)
    }

    @Test func capturesCroppedRegion() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let region = CGRect(x: 10, y: 10, width: 200, height: 150)
        let image = try await capture.captureImage(region: region)
        #expect(image.width == 200)
        #expect(image.height == 150)
    }

    @Test func consecutiveScreenshotsSameSize() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let img1 = try await capture.captureImage()
        try await Task.sleep(for: .milliseconds(200))
        let img2 = try await capture.captureImage()
        #expect(img1.width == img2.width)
        #expect(img1.height == img2.height)
    }

    // MARK: - Mouse Input

    @Test func mouseMoveAndCaptureWorkTogether() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        for (x, y) in [(UInt16(10), UInt16(10)), (500, 400), (800, 600)] {
            try await capture.withConnection { conn in
                VNCInput.mouseMove(x: x, y: y, connection: conn)
            }
            try await Task.sleep(for: .milliseconds(100))
            let img = try await capture.captureImage()
            #expect(img.width > 0)
        }
    }

    @Test func mouseClickAccepted() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            try VNCInput.click(x: 500, y: 400, button: "left", count: 1, connection: conn)
        }
        try await capture.withConnection { conn in
            try VNCInput.click(x: 500, y: 400, button: "left", count: 2, connection: conn)
        }
        try await capture.withConnection { conn in
            try VNCInput.click(x: 500, y: 400, button: "right", count: 1, connection: conn)
        }
        try await Task.sleep(for: .milliseconds(500))
        try await capture.withConnection { conn in
            try VNCInput.pressKey("escape", platform: spec.platform, connection: conn)
        }
    }

    @Test func mouseDragCompletesWithoutError() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            try VNCInput.drag(fromX: 200, fromY: 200, toX: 400, toY: 400,
                              button: "left", steps: 20, connection: conn)
        }
        let img = try await capture.captureImage()
        #expect(img.width > 0)
    }

    @Test func scrollAccepted() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            VNCInput.scroll(x: 500, y: 400, deltaX: 0, deltaY: -3, connection: conn)
            VNCInput.scroll(x: 500, y: 400, deltaX: 0, deltaY: 3, connection: conn)
            VNCInput.scroll(x: 500, y: 400, deltaX: -2, deltaY: 0, connection: conn)
            VNCInput.scroll(x: 500, y: 400, deltaX: 2, deltaY: 0, connection: conn)
        }
    }

    // MARK: - Keyboard Input

    @Test func specialKeysAccepted() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            try VNCInput.pressKey("escape", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("tab", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("return", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("space", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("delete", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("up", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("down", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("left", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("right", platform: spec.platform, connection: conn)
            try VNCInput.pressKey("f1", platform: spec.platform, connection: conn)
        }
    }

    @Test func modifierCombinationsAccepted() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            try VNCInput.pressKey("a", modifiers: ["cmd"], platform: spec.platform, connection: conn)
            try VNCInput.pressKey("z", modifiers: ["cmd", "shift"], platform: spec.platform, connection: conn)
            try VNCInput.pressKey("c", modifiers: ["ctrl"], platform: spec.platform, connection: conn)
        }
    }

    @Test func typeTextExercisesShiftedChars() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            VNCInput.typeText("Hello World! @#$ Test_123", connection: conn)
        }
        let img = try await capture.captureImage()
        #expect(img.width > 0)
    }

    // MARK: - Cursor State

    @Test func cursorStateAccessibleAfterMovement() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        try await capture.withConnection { conn in
            VNCInput.mouseMove(x: 500, y: 400, connection: conn)
        }
        try await Task.sleep(for: .seconds(1))

        let cursor = await capture.cursorState
        if let size = cursor.size {
            #expect(size.width > 0 && size.height > 0)
        }
    }

    // MARK: - Streaming Capture

    @Test func recordsVideoFromLiveVNC() async throws {
        let spec = try #require(TestEnv.spec, "TESTANYWARE_VM_ID or TESTANYWARE_VNC not set")
        let capture = VNCCapture(spec: spec.vnc)
        try await capture.connect(timeout: .seconds(30))
        defer { Task { await capture.disconnect() } }

        let outputPath = NSTemporaryDirectory() + "testanyware-integration-\(UUID().uuidString).mp4"
        defer { try? FileManager.default.removeItem(atPath: outputPath) }

        guard let screenSize = await capture.screenSize() else {
            Issue.record("Could not determine screen size")
            return
        }

        let config = StreamingCaptureConfig(
            width: Int(screenSize.width),
            height: Int(screenSize.height),
            fps: 10
        )
        let recorder = StreamingCapture()
        try await recorder.start(outputPath: outputPath, config: config)

        for i in 0..<10 {
            let image = try await capture.captureImage()
            try await recorder.appendFrame(image)
            try await capture.withConnection { conn in
                VNCInput.mouseMove(x: UInt16(100 + i * 30), y: UInt16(100 + i * 20), connection: conn)
            }
            try await Task.sleep(for: .milliseconds(100))
        }

        try await recorder.stop()

        let url = URL(fileURLWithPath: outputPath)
        let asset = AVURLAsset(url: url)
        let tracks = try await asset.loadTracks(withMediaType: .video)
        #expect(!tracks.isEmpty, "MP4 should contain a video track")

        let track = tracks[0]
        let naturalSize = try await track.load(.naturalSize)
        #expect(Int(naturalSize.width) == Int(screenSize.width))
        #expect(Int(naturalSize.height) == Int(screenSize.height))

        let duration = try await asset.load(.duration)
        let durationSeconds = CMTimeGetSeconds(duration)
        #expect(durationSeconds > 0.5)
        #expect(durationSeconds < 5.0)
    }
}

// MARK: - Agent Integration Tests (placeholder — requires agent running in VM)
// Agent integration tests will be added once golden images with the agent are built.
