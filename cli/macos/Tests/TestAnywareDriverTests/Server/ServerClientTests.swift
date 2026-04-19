import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("ServerClient")
struct ServerClientTests {

    // MARK: - Socket path format

    @Test func socketPathIsUnderTmp() {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let path = ServerClient.socketPath(for: spec)
        #expect(path.hasPrefix("/tmp/testanyware-"))
        #expect(path.hasSuffix(".sock"))
    }

    @Test func socketPathHexSegment() {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let path = ServerClient.socketPath(for: spec)
        let stripped = path
            .replacingOccurrences(of: "/tmp/testanyware-", with: "")
            .replacingOccurrences(of: ".sock", with: "")
        #expect(stripped.count == 16)
        #expect(stripped.allSatisfy { $0.isHexDigit })
    }

    // MARK: - Determinism

    @Test func sameSpecProducesSamePath() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.1", port: 5901, password: "pass"))
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.1", port: 5901, password: "pass"))
        #expect(ServerClient.socketPath(for: spec1) == ServerClient.socketPath(for: spec2))
    }

    @Test func sameSpecWithAgentProducesSamePath() {
        let agent = AgentSpec(host: "10.0.0.1", port: 8648)
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.1", port: 5900), agent: agent, platform: .macos)
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.1", port: 5900), agent: agent, platform: .macos)
        #expect(ServerClient.socketPath(for: spec1) == ServerClient.socketPath(for: spec2))
    }

    // MARK: - Uniqueness

    @Test func differentHostProducesDifferentPath() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.1", port: 5900))
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "10.0.0.2", port: 5900))
        #expect(ServerClient.socketPath(for: spec1) != ServerClient.socketPath(for: spec2))
    }

    @Test func differentPortProducesDifferentPath() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5901))
        #expect(ServerClient.socketPath(for: spec1) != ServerClient.socketPath(for: spec2))
    }

    @Test func differentPasswordProducesDifferentPath() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900, password: "abc"))
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900, password: "xyz"))
        #expect(ServerClient.socketPath(for: spec1) != ServerClient.socketPath(for: spec2))
    }

    @Test func specWithAgentDiffersFromSpecWithout() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let spec2 = ConnectionSpec(
            vnc: VNCSpec(host: "localhost", port: 5900),
            agent: AgentSpec(host: "localhost", port: 8648)
        )
        #expect(ServerClient.socketPath(for: spec1) != ServerClient.socketPath(for: spec2))
    }

    @Test func differentPlatformProducesDifferentPath() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900), platform: .macos)
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900), platform: .linux)
        #expect(ServerClient.socketPath(for: spec1) != ServerClient.socketPath(for: spec2))
    }

    // MARK: - PID path

    @Test func pidPathFollowsSamePatternWithPidExtension() {
        let spec = ConnectionSpec(vnc: VNCSpec(host: "localhost", port: 5900))
        let socketPath = ServerClient.socketPath(for: spec)
        let pidPath = ServerClient.pidPath(for: spec)
        #expect(pidPath.hasPrefix("/tmp/testanyware-"))
        #expect(pidPath.hasSuffix(".pid"))
        let socketHex = socketPath
            .replacingOccurrences(of: "/tmp/testanyware-", with: "")
            .replacingOccurrences(of: ".sock", with: "")
        let pidHex = pidPath
            .replacingOccurrences(of: "/tmp/testanyware-", with: "")
            .replacingOccurrences(of: ".pid", with: "")
        #expect(socketHex == pidHex)
    }

    @Test func pidPathDifferentSpecsDifferentPaths() {
        let spec1 = ConnectionSpec(vnc: VNCSpec(host: "host-a", port: 5900))
        let spec2 = ConnectionSpec(vnc: VNCSpec(host: "host-b", port: 5900))
        #expect(ServerClient.pidPath(for: spec1) != ServerClient.pidPath(for: spec2))
    }

    // MARK: - OCR request serialization

    @Test func ocrRequestSerializesCorrectly() {
        let pngData = Data([0x89, 0x50, 0x4E, 0x47])
        let request = WireRequest(method: "POST", path: "/ocr", body: pngData)
        let serialized = request.serialize()
        let text = String(decoding: serialized, as: UTF8.self)
        #expect(text.hasPrefix("POST /ocr HTTP/1.1\r\n"))
        #expect(text.contains("Content-Length: \(pngData.count)\r\n"))
        #expect(serialized.suffix(pngData.count) == pngData)
    }

    // MARK: - Wire protocol serialization

    @Test func serializeRequestProducesCorrectHTTPFormat() {
        let request = WireRequest(method: "GET", path: "/health")
        let data = request.serialize()
        let text = String(decoding: data, as: UTF8.self)
        #expect(text.hasPrefix("GET /health HTTP/1.1\r\n"))
        #expect(text.contains("Connection: close\r\n"))
        #expect(text.hasSuffix("\r\n\r\n"))
    }

    @Test func serializePostRequestIncludesContentLength() {
        let body = Data(#"{"x":1}"#.utf8)
        let request = WireRequest(method: "POST", path: "/click", body: body)
        let data = request.serialize()
        let text = String(decoding: data, as: UTF8.self)
        #expect(text.hasPrefix("POST /click HTTP/1.1\r\n"))
        #expect(text.contains("Content-Length: \(body.count)\r\n"))
        #expect(data.suffix(body.count) == body)
    }
}
