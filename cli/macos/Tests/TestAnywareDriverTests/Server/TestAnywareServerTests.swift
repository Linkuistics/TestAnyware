import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("Server")
struct ServerTests {

    // MARK: - Helpers

    private func makeServer(
        idleTimeout: Duration = .seconds(60),
        onShutdown: @escaping @Sendable () -> Void = {}
    ) -> TestAnywareServer {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        return TestAnywareServer(spec: spec, idleTimeout: idleTimeout, onShutdown: onShutdown)
    }

    // MARK: - Health handler

    @Test func healthReturnsOK() async {
        let server = makeServer()
        let response = await server.handleHealth()
        #expect(response.status == .ok)
    }

    // MARK: - Screen size without VNC

    @Test func screenSizeReturns503WithoutVNC() async {
        let server = makeServer()
        let response = await server.handleScreenSize()
        #expect(response.status == .serviceUnavailable)
    }

    // MARK: - Stop handler

    @Test func stopReturnsOK() async {
        let server = makeServer()
        let response = await server.handleStop()
        #expect(response.status == .ok)
    }

    // MARK: - Record stop returns OK

    @Test func recordStopReturnsOK() async {
        let server = makeServer()
        let response = await server.handleRecordStop()
        #expect(response.status == .ok)
    }

    // MARK: - OCR handler

    @Test func ocrWithMacosSpecUsesVisionEngine() async throws {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900), platform: .macos)
        let server = TestAnywareServer(spec: spec, idleTimeout: .seconds(60), onShutdown: {})
        // Read the hello.png fixture
        let testFile = URL(fileURLWithPath: #filePath)
        let fixturePath = testFile
            .deletingLastPathComponent()  // Server/
            .deletingLastPathComponent()  // TestAnywareDriverTests/
            .deletingLastPathComponent()  // Tests/
            .appendingPathComponent("Resources")
            .appendingPathComponent("hello.png")
            .path
        let pngData = try Data(contentsOf: URL(fileURLWithPath: fixturePath))
        let response = try await server.handleOCR(pngData: pngData)
        #expect(response.engine == "vision")
        #expect(response.warning == nil)
    }

    @Test func ocrWithNilPlatformUsesVisionEngine() async throws {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let server = TestAnywareServer(spec: spec, idleTimeout: .seconds(60), onShutdown: {})
        let testFile = URL(fileURLWithPath: #filePath)
        let fixturePath = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Resources")
            .appendingPathComponent("hello.png")
            .path
        let pngData = try Data(contentsOf: URL(fileURLWithPath: fixturePath))
        let response = try await server.handleOCR(pngData: pngData)
        #expect(response.engine == "vision")
    }

    @Test func ocrWithLinuxSpecUsesBridge() async throws {
        // Create a bridge pointing at the fake harness
        let testFile = URL(fileURLWithPath: #filePath)
        let fakeScript = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Resources")
            .appendingPathComponent("fake-ocr-daemon.sh")
            .path
        let bridge = OCRChildBridge(
            interpreterPath: "/bin/bash",
            daemonArguments: [fakeScript],
            environment: ["FAKE_OCR_BEHAVIOR": "ready_then_echo"]
        )
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900), platform: .linux)
        let server = TestAnywareServer(
            spec: spec,
            idleTimeout: .seconds(60),
            onShutdown: {},
            ocrBridge: bridge
        )
        let pngData = Data([0x89, 0x50])  // minimal bytes
        let response = try await server.handleOCR(pngData: pngData)
        #expect(response.engine == "easyocr_daemon")
        #expect(response.warning == nil)
        await bridge.shutdown()
    }

    // MARK: - Idle timer

    @Test func idleTimerFires() async {
        let shutdownCalled = LockIsolated(false)
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let server = TestAnywareServer(
            spec: spec,
            idleTimeout: .milliseconds(100),
            onShutdown: { shutdownCalled.withLock { $0 = true } }
        )
        let deadline = ContinuousClock().now + .seconds(1)
        while !shutdownCalled.withLock({ $0 }) {
            if ContinuousClock().now > deadline { break }
            try? await Task.sleep(for: .milliseconds(20))
        }
        _ = server
        #expect(shutdownCalled.withLock { $0 } == true)
    }

    // Behavioural cover for the idle-timer rewrite that fixes the -O-only
    // SIGABRT in swift_task_dealloc (backlog Task 7). The previous
    // implementation cancelled the init-armed Task.sleep on the first
    // request, which crashed in optimised builds; this exercises the same
    // hot path many times to ensure the epoch-based replacement survives.
    @Test func rapidActivityDoesNotShutDownOrCrash() async {
        let shutdownCalled = LockIsolated(false)
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let server = TestAnywareServer(
            spec: spec,
            idleTimeout: .seconds(60),
            onShutdown: { shutdownCalled.withLock { $0 = true } }
        )
        for _ in 0..<200 {
            _ = await server.handleHealth()
        }
        _ = server
        #expect(shutdownCalled.withLock { $0 } == false)
    }

    @Test func idleTimerResets() async throws {
        let shutdownCalled = LockIsolated(false)
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let server = TestAnywareServer(
            spec: spec,
            idleTimeout: .milliseconds(200),
            onShutdown: { shutdownCalled.withLock { $0 = true } }
        )
        for _ in 0..<10 {
            _ = await server.handleHealth()
            try await Task.sleep(for: .milliseconds(50))
        }
        #expect(shutdownCalled.withLock { $0 } == false)
        let deadline = ContinuousClock().now + .seconds(1)
        while !shutdownCalled.withLock({ $0 }) {
            if ContinuousClock().now > deadline { break }
            try? await Task.sleep(for: .milliseconds(20))
        }
        #expect(shutdownCalled.withLock { $0 } == true)
    }
}

// MARK: - LockIsolated

final class LockIsolated<Value>: @unchecked Sendable {
    private var value: Value
    private let lock = NSLock()

    init(_ value: Value) {
        self.value = value
    }

    @discardableResult
    func withLock<T>(_ body: (inout Value) -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body(&value)
    }
}
