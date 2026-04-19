import CoreGraphics
import CryptoKit
import Darwin
import Foundation

// MARK: - Wire protocol types (client-side HTTP serialization)

/// A simple HTTP/1.1 request for Unix domain socket transport.
public struct WireRequest: Sendable {
    public let method: String
    public let path: String
    public let body: Data?

    public init(method: String, path: String, body: Data? = nil) {
        self.method = method
        self.path = path
        self.body = body
    }

    func serialize() -> Data {
        let bodyData = body ?? Data()
        var header = "\(method) \(path) HTTP/1.1\r\n"
        if !bodyData.isEmpty {
            header += "Content-Length: \(bodyData.count)\r\n"
        }
        header += "Connection: close\r\n\r\n"
        var result = Data(header.utf8)
        result.append(bodyData)
        return result
    }
}

/// A simple HTTP/1.1 response parsed from Unix domain socket transport.
public struct WireResponse: Sendable {
    public let statusCode: Int
    public let contentType: String
    public let body: Data

    private static let headerSeparator = Data("\r\n\r\n".utf8)

    static func parse(from data: Data) throws -> WireResponse {
        guard let separatorRange = data.range(of: headerSeparator) else {
            throw WireParseError.missingHeaderBlock
        }

        let headerBlock = data[data.startIndex..<separatorRange.lowerBound]
        let headerText = String(decoding: headerBlock, as: UTF8.self)
        let lines = headerText.components(separatedBy: "\r\n")

        let statusLine = lines[0]
        let parts = statusLine.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count >= 2, let statusCode = Int(parts[1]) else {
            throw WireParseError.malformedStatusLine(statusLine)
        }

        var contentType = "application/octet-stream"
        var contentLength: Int? = nil
        for line in lines.dropFirst() {
            let lower = line.lowercased()
            if lower.hasPrefix("content-type:") {
                contentType = line.dropFirst("content-type:".count)
                    .trimmingCharacters(in: .whitespaces)
            } else if lower.hasPrefix("content-length:") {
                let value = line.dropFirst("content-length:".count)
                    .trimmingCharacters(in: .whitespaces)
                contentLength = Int(value)
            }
        }

        let bodyStart = separatorRange.upperBound
        let body: Data
        if let length = contentLength, length > 0 {
            let available = data.distance(from: bodyStart, to: data.endIndex)
            guard available >= length else {
                throw WireParseError.bodyIncomplete
            }
            let bodyEnd = data.index(bodyStart, offsetBy: length)
            body = Data(data[bodyStart..<bodyEnd])
        } else {
            body = Data()
        }

        return WireResponse(statusCode: statusCode, contentType: contentType, body: body)
    }
}

enum WireParseError: Error {
    case missingHeaderBlock
    case malformedStatusLine(String)
    case bodyIncomplete
}

// MARK: - Transport errors

public enum ServerClientError: Error, Sendable {
    case socketCreateFailed(Int32)
    case connectFailed(Int32)
    case serverStartTimeout
    case serverStartFailed(String)
    case httpError(Int, String)
}

extension ServerClientError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .socketCreateFailed(let e):
            "Socket creation failed (errno \(e))"
        case .connectFailed(let e):
            "Connection refused (errno \(e))"
        case .serverStartTimeout:
            "Timed out waiting for server to become ready"
        case .serverStartFailed(let msg):
            "Server failed to start: \(msg)"
        case .httpError(let code, let msg):
            "HTTP \(code): \(msg)"
        }
    }
}

// MARK: - ServerClient

/// Client-side handle for a per-`ConnectionSpec` server instance.
///
/// Create instances via `ensure(spec:idleTimeout:)` which auto-starts the
/// server process if it isn't already running. Once you have a `ServerClient`
/// you can call the high-level API methods (screenshot, click, record, …).
public struct ServerClient: Sendable {

    private let socketPath: String

    private init(socketPath: String) {
        self.socketPath = socketPath
    }

    // MARK: - Factory

    public static func ensure(
        spec: ConnectionSpec,
        idleTimeout: Int = 300
    ) async throws -> ServerClient {
        let path = Self.socketPath(for: spec)

        if await isHealthy(socketPath: path) {
            return ServerClient(socketPath: path)
        }

        let pidFilePath = Self.pidPath(for: spec)
        try? FileManager.default.removeItem(atPath: path)
        try? FileManager.default.removeItem(atPath: pidFilePath)

        let execPath = currentExecutablePath()

        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let jsonData = try encoder.encode(spec)
        guard let jsonString = String(data: jsonData, encoding: .utf8) else {
            throw ServerClientError.serverStartFailed("Could not encode ConnectionSpec as JSON")
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: execPath)
        process.arguments = [
            "_server",
            "--connect-json", jsonString,
            "--idle-timeout", "\(idleTimeout)",
        ]

        let outPipe = Pipe()
        process.standardOutput = outPipe
        process.standardError = FileHandle.standardError
        process.qualityOfService = .background

        do {
            try process.run()
        } catch {
            throw ServerClientError.serverStartFailed(error.localizedDescription)
        }

        let ready = await waitForReady(pipe: outPipe, timeoutSeconds: 10)
        guard ready else {
            process.terminate()
            throw ServerClientError.serverStartTimeout
        }

        guard await isHealthy(socketPath: path) else {
            process.terminate()
            throw ServerClientError.serverStartFailed("Server reported ready but health check failed")
        }

        return ServerClient(socketPath: path)
    }

    // MARK: - Path computation

    public static func socketPath(for spec: ConnectionSpec) -> String {
        "/tmp/testanyware-\(hexPrefix(for: spec)).sock"
    }

    public static func pidPath(for spec: ConnectionSpec) -> String {
        "/tmp/testanyware-\(hexPrefix(for: spec)).pid"
    }

    // MARK: - HTTP transport

    public static func send(_ request: WireRequest, to socketPath: String) async throws -> WireResponse {
        let fd = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Int32, Error>) in
            let thread = Thread {
                let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
                guard fd >= 0 else {
                    continuation.resume(throwing: ServerClientError.socketCreateFailed(errno))
                    return
                }

                var addr = sockaddr_un()
                addr.sun_family = sa_family_t(AF_UNIX)
                withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
                    ptr.withMemoryRebound(to: CChar.self, capacity: 104) { cptr in
                        _ = strlcpy(cptr, socketPath, 104)
                    }
                }
                let result = withUnsafePointer(to: &addr) { ptr in
                    ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sptr in
                        Darwin.connect(fd, sptr, socklen_t(MemoryLayout<sockaddr_un>.size))
                    }
                }
                guard result == 0 else {
                    Darwin.close(fd)
                    continuation.resume(throwing: ServerClientError.connectFailed(errno))
                    return
                }

                continuation.resume(returning: fd)
            }
            thread.start()
        }

        defer { Darwin.close(fd) }

        let requestData = request.serialize()
        await writeAll(fd: fd, data: requestData)

        var buffer = Data()
        let chunkSize = 65536
        let headerSeparator = Data("\r\n\r\n".utf8)

        while true {
            let chunk = await readChunk(fd: fd, maxBytes: chunkSize)
            if let data = chunk {
                buffer.append(data)
            }

            if buffer.range(of: headerSeparator) != nil {
                if let response = try? WireResponse.parse(from: buffer) {
                    return response
                }
            }

            if chunk == nil {
                return try WireResponse.parse(from: buffer)
            }
        }
    }

    // MARK: - Private socket helpers

    private static func readChunk(fd: Int32, maxBytes: Int) async -> Data? {
        await withCheckedContinuation { continuation in
            let thread = Thread {
                var buf = [UInt8](repeating: 0, count: maxBytes)
                let n = Darwin.read(fd, &buf, maxBytes)
                if n > 0 {
                    continuation.resume(returning: Data(buf[0..<n]))
                } else {
                    continuation.resume(returning: nil)
                }
            }
            thread.start()
        }
    }

    private static func writeAll(fd: Int32, data: Data) async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            let thread = Thread {
                data.withUnsafeBytes { rawBuf in
                    guard let base = rawBuf.baseAddress else {
                        continuation.resume()
                        return
                    }
                    var offset = 0
                    while offset < data.count {
                        let n = Darwin.write(fd, base.advanced(by: offset), data.count - offset)
                        if n <= 0 { break }
                        offset += n
                    }
                }
                continuation.resume()
            }
            thread.start()
        }
    }

    // MARK: - Private helpers

    private static func hexPrefix(for spec: ConnectionSpec) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = (try? encoder.encode(spec)) ?? Data()
        let digest = SHA256.hash(data: data)
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        return String(hex.prefix(16))
    }

    private static func isHealthy(socketPath: String) async -> Bool {
        let request = WireRequest(method: "GET", path: "/health")
        guard let response = try? await send(request, to: socketPath) else {
            return false
        }
        return response.statusCode == 200
    }

    private static func waitForReady(pipe: Pipe, timeoutSeconds: Int) async -> Bool {
        await withCheckedContinuation { continuation in
            let thread = Thread {
                let handle = pipe.fileHandleForReading
                let deadline = Date(timeIntervalSinceNow: Double(timeoutSeconds))
                var accumulated = Data()

                while Date() < deadline {
                    var buf = [UInt8](repeating: 0, count: 256)
                    let n = Darwin.read(handle.fileDescriptor, &buf, 256)
                    if n > 0 {
                        accumulated.append(contentsOf: buf[0..<n])
                        if let text = String(data: accumulated, encoding: .utf8),
                           text.contains("ready") {
                            continuation.resume(returning: true)
                            return
                        }
                    } else if n == 0 {
                        break
                    }
                    Thread.sleep(forTimeInterval: 0.05)
                }

                continuation.resume(returning: false)
            }
            thread.start()
        }
    }

    // MARK: - Response error checking

    private func errorMessage(from response: WireResponse) -> String {
        struct ErrorBody: Decodable { let error: String }
        if let body = try? JSONDecoder().decode(ErrorBody.self, from: response.body) {
            return body.error
        }
        return String(data: response.body, encoding: .utf8) ?? "HTTP \(response.statusCode)"
    }

    private func checkSuccess(_ response: WireResponse) throws {
        guard (200...299).contains(response.statusCode) else {
            throw ServerClientError.httpError(response.statusCode, errorMessage(from: response))
        }
    }

    private func jsonBody<T: Encodable>(_ value: T) throws -> Data {
        try JSONEncoder().encode(value)
    }
}

// MARK: - High-level API

extension ServerClient {

    // MARK: Screen

    public func screenshot(region: CGRect? = nil) async throws -> Data {
        struct ScreenshotRequest: Encodable { let region: String? }
        let regionStr = region.map { "\(Int($0.origin.x)),\(Int($0.origin.y)),\(Int($0.width)),\(Int($0.height))" }
        let body = try jsonBody(ScreenshotRequest(region: regionStr))
        let request = WireRequest(method: "POST", path: "/screenshot", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
        return response.body
    }

    public func screenSize() async throws -> CGSize {
        struct SizeResponse: Decodable { let width: Int; let height: Int }
        let request = WireRequest(method: "GET", path: "/screen-size")
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
        let parsed = try JSONDecoder().decode(SizeResponse.self, from: response.body)
        return CGSize(width: parsed.width, height: parsed.height)
    }

    // MARK: OCR

    public func ocr(pngData: Data) async throws -> OCRResponse {
        let request = WireRequest(method: "POST", path: "/ocr", body: pngData)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
        return try JSONDecoder().decode(OCRResponse.self, from: response.body)
    }

    // MARK: Keyboard

    public func pressKey(_ key: String, modifiers: [String] = []) async throws {
        struct KeyRequest: Encodable { let key: String; let modifiers: [String] }
        let body = try jsonBody(KeyRequest(key: key, modifiers: modifiers))
        let request = WireRequest(method: "POST", path: "/input/key", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func keyDown(_ key: String) async throws {
        struct KeyRequest: Encodable { let key: String }
        let body = try jsonBody(KeyRequest(key: key))
        let request = WireRequest(method: "POST", path: "/input/key-down", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func keyUp(_ key: String) async throws {
        struct KeyRequest: Encodable { let key: String }
        let body = try jsonBody(KeyRequest(key: key))
        let request = WireRequest(method: "POST", path: "/input/key-up", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func typeText(_ text: String) async throws {
        struct TypeRequest: Encodable { let text: String }
        let body = try jsonBody(TypeRequest(text: text))
        let request = WireRequest(method: "POST", path: "/input/type", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    // MARK: Mouse

    public func click(x: Int, y: Int, button: String = "left", count: Int = 1) async throws {
        struct ClickRequest: Encodable { let x: Int; let y: Int; let button: String; let count: Int }
        let body = try jsonBody(ClickRequest(x: x, y: y, button: button, count: count))
        let request = WireRequest(method: "POST", path: "/input/click", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func mouseDown(x: Int, y: Int, button: String = "left") async throws {
        struct MouseRequest: Encodable { let x: Int; let y: Int; let button: String }
        let body = try jsonBody(MouseRequest(x: x, y: y, button: button))
        let request = WireRequest(method: "POST", path: "/input/mouse-down", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func mouseUp(x: Int, y: Int, button: String = "left") async throws {
        struct MouseRequest: Encodable { let x: Int; let y: Int; let button: String }
        let body = try jsonBody(MouseRequest(x: x, y: y, button: button))
        let request = WireRequest(method: "POST", path: "/input/mouse-up", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func mouseMove(x: Int, y: Int) async throws {
        struct MoveRequest: Encodable { let x: Int; let y: Int }
        let body = try jsonBody(MoveRequest(x: x, y: y))
        let request = WireRequest(method: "POST", path: "/input/move", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func scroll(x: Int, y: Int, dx: Int, dy: Int) async throws {
        struct ScrollRequest: Encodable { let x: Int; let y: Int; let dx: Int; let dy: Int }
        let body = try jsonBody(ScrollRequest(x: x, y: y, dx: dx, dy: dy))
        let request = WireRequest(method: "POST", path: "/input/scroll", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func drag(
        fromX: Int, fromY: Int,
        toX: Int, toY: Int,
        button: String = "left",
        steps: Int = 10
    ) async throws {
        struct DragRequest: Encodable {
            let fromX: Int; let fromY: Int
            let toX: Int; let toY: Int
            let button: String; let steps: Int
        }
        let body = try jsonBody(DragRequest(fromX: fromX, fromY: fromY, toX: toX, toY: toY, button: button, steps: steps))
        let request = WireRequest(method: "POST", path: "/input/drag", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    // MARK: Recording

    public func recordStart(
        output: String,
        fps: Int = 30,
        duration: Int,
        region: CGRect? = nil
    ) async throws {
        struct RecordRequest: Encodable {
            let output: String; let fps: Int; let duration: Int; let region: String?
        }
        let regionStr = region.map { "\(Int($0.origin.x)),\(Int($0.origin.y)),\(Int($0.width)),\(Int($0.height))" }
        let body = try jsonBody(RecordRequest(output: output, fps: fps, duration: duration, region: regionStr))
        let request = WireRequest(method: "POST", path: "/record/start", body: body)
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    public func recordStop() async throws {
        let request = WireRequest(method: "POST", path: "/record/stop")
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }

    // MARK: Lifecycle

    public func stop() async throws {
        let request = WireRequest(method: "POST", path: "/stop")
        let response = try await Self.send(request, to: socketPath)
        try checkSuccess(response)
    }
}
