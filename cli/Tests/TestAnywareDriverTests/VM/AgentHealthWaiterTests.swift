import Testing
import Foundation
import Darwin
@testable import TestAnywareDriver

@Suite("AgentHealthWaiter")
struct AgentHealthWaiterTests {

    @Test func reportsReadyOnceHealthReturns200() async throws {
        let server = TestHealthServer(responseCode: 200)
        try server.start()
        defer { server.stop() }

        let waiter = AgentHealthWaiter()
        let ready = try await waiter.waitForReady(
            host: "127.0.0.1",
            port: server.port,
            attempts: 20,
            intervalSeconds: 0.1
        )
        #expect(ready)
    }

    @Test func returnsFalseAfterExhaustingAttempts() async throws {
        let waiter = AgentHealthWaiter()
        let ready = try await waiter.waitForReady(
            host: "127.0.0.1",
            port: 1,
            attempts: 3,
            intervalSeconds: 0.05
        )
        #expect(!ready)
    }

    @Test func reportsReadyForAny2xxStatus() async throws {
        let server = TestHealthServer(responseCode: 204)
        try server.start()
        defer { server.stop() }

        let waiter = AgentHealthWaiter()
        let ready = try await waiter.waitForReady(
            host: "127.0.0.1",
            port: server.port,
            attempts: 20,
            intervalSeconds: 0.1
        )
        #expect(ready)
    }

    @Test func retriesOnNon2xxUntilExhausted() async throws {
        let server = TestHealthServer(responseCode: 503)
        try server.start()
        defer { server.stop() }

        let waiter = AgentHealthWaiter()
        let ready = try await waiter.waitForReady(
            host: "127.0.0.1",
            port: server.port,
            attempts: 3,
            intervalSeconds: 0.05
        )
        #expect(!ready)
    }
}

/// Minimal in-process HTTP server returning a canned status code.
///
/// Binds to 127.0.0.1 on an ephemeral port, accepts each connection in a
/// background thread, writes a canned HTTP/1.1 response, closes the
/// connection. `stop()` closes the listener FD — this unblocks the
/// thread's `accept()` and lets it exit cleanly.
final class TestHealthServer: @unchecked Sendable {
    private let responseCode: Int
    private var fd: Int32 = -1
    private(set) var port: Int = 0

    init(responseCode: Int) { self.responseCode = responseCode }

    func start() throws {
        fd = socket(AF_INET, SOCK_STREAM, 0)
        precondition(fd >= 0, "socket() failed")

        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))

        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bindResult = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        precondition(bindResult == 0, "bind() failed: errno=\(errno)")
        precondition(listen(fd, 16) == 0, "listen() failed")

        var bound = sockaddr_in()
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        withUnsafeMutablePointer(to: &bound) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                _ = getsockname(fd, $0, &len)
            }
        }
        port = Int(UInt16(bigEndian: bound.sin_port))

        let responseCode = self.responseCode
        let listenFD = fd
        Thread.detachNewThread {
            while true {
                let client = accept(listenFD, nil, nil)
                if client < 0 { return }
                let body = "\(responseCode)"
                let response = """
                HTTP/1.1 \(responseCode) OK\r
                Content-Length: \(body.count)\r
                Connection: close\r
                \r
                \(body)
                """
                _ = response.withCString { send(client, $0, strlen($0), 0) }
                close(client)
            }
        }
    }

    func stop() {
        if fd >= 0 {
            close(fd)
            fd = -1
        }
    }
}
