import Foundation
import TestAnywareAgentProtocol

// MARK: - Errors

public enum AgentTCPClientError: Error, LocalizedError, Sendable {
    case connectionFailed(String)
    case httpError(Int, String)
    case decodingFailed(String)

    public var errorDescription: String? {
        switch self {
        case .connectionFailed(let detail):
            "Agent connection failed: \(detail)"
        case .httpError(let code, let msg):
            "Agent HTTP \(code): \(msg)"
        case .decodingFailed(let detail):
            "Failed to decode agent response: \(detail)"
        }
    }
}

// MARK: - Client

/// HTTP client for the in-VM agent TCP service.
/// Uses URLSession to make requests to the agent on port 8648 (default).
public struct AgentTCPClient: Sendable {
    public let baseURL: URL

    public init(spec: AgentSpec) {
        self.baseURL = URL(string: "http://\(spec.host):\(spec.port)")!
    }

    public init(host: String, port: Int = 8648) {
        self.baseURL = URL(string: "http://\(host):\(port)")!
    }

    // MARK: - Health

    public func health() async throws -> Bool {
        let (_, response) = try await get("/health")
        return (response as? HTTPURLResponse)?.statusCode == 200
    }

    // MARK: - Accessibility

    public func windows() async throws -> SnapshotResponse {
        let data = try await postJSON("/windows", body: EmptyBody())
        return try decode(SnapshotResponse.self, from: data)
    }

    public func snapshot(
        mode: String? = nil,
        window: String? = nil,
        role: String? = nil,
        label: String? = nil,
        depth: Int? = nil
    ) async throws -> SnapshotResponse {
        struct Req: Encodable {
            let mode: String?; let window: String?
            let role: String?; let label: String?; let depth: Int?
        }
        let data = try await postJSON("/snapshot", body: Req(mode: mode, window: window, role: role, label: label, depth: depth))
        return try decode(SnapshotResponse.self, from: data)
    }

    public func inspect(
        role: String? = nil,
        label: String? = nil,
        window: String? = nil,
        id: String? = nil,
        index: Int? = nil
    ) async throws -> InspectResponse {
        let data = try await postJSON("/inspect", body: ElementQuery(role: role, label: label, window: window, id: id, index: index))
        return try decode(InspectResponse.self, from: data)
    }

    public func press(
        role: String? = nil,
        label: String? = nil,
        window: String? = nil,
        id: String? = nil,
        index: Int? = nil
    ) async throws -> ActionResponse {
        let data = try await postJSON("/press", body: ElementQuery(role: role, label: label, window: window, id: id, index: index))
        return try decode(ActionResponse.self, from: data)
    }

    public func setValue(
        role: String? = nil,
        label: String? = nil,
        window: String? = nil,
        id: String? = nil,
        index: Int? = nil,
        value: String
    ) async throws -> ActionResponse {
        struct Req: Encodable {
            let role: String?; let label: String?; let window: String?
            let id: String?; let index: Int?; let value: String
        }
        let data = try await postJSON("/set-value", body: Req(role: role, label: label, window: window, id: id, index: index, value: value))
        return try decode(ActionResponse.self, from: data)
    }

    public func focus(
        role: String? = nil,
        label: String? = nil,
        window: String? = nil,
        id: String? = nil,
        index: Int? = nil
    ) async throws -> ActionResponse {
        let data = try await postJSON("/focus", body: ElementQuery(role: role, label: label, window: window, id: id, index: index))
        return try decode(ActionResponse.self, from: data)
    }

    public func showMenu(
        role: String? = nil,
        label: String? = nil,
        window: String? = nil,
        id: String? = nil,
        index: Int? = nil
    ) async throws -> ActionResponse {
        let data = try await postJSON("/show-menu", body: ElementQuery(role: role, label: label, window: window, id: id, index: index))
        return try decode(ActionResponse.self, from: data)
    }

    // MARK: - Window Management

    public func windowFocus(window: String) async throws -> ActionResponse {
        let data = try await postJSON("/window-focus", body: WindowTarget(window: window))
        return try decode(ActionResponse.self, from: data)
    }

    public func windowResize(window: String, width: Int, height: Int) async throws -> ActionResponse {
        struct Req: Encodable { let window: String; let width: Int; let height: Int }
        let data = try await postJSON("/window-resize", body: Req(window: window, width: width, height: height))
        return try decode(ActionResponse.self, from: data)
    }

    public func windowMove(window: String, x: Int, y: Int) async throws -> ActionResponse {
        struct Req: Encodable { let window: String; let x: Int; let y: Int }
        let data = try await postJSON("/window-move", body: Req(window: window, x: x, y: y))
        return try decode(ActionResponse.self, from: data)
    }

    public func windowClose(window: String) async throws -> ActionResponse {
        let data = try await postJSON("/window-close", body: WindowTarget(window: window))
        return try decode(ActionResponse.self, from: data)
    }

    public func windowMinimize(window: String) async throws -> ActionResponse {
        let data = try await postJSON("/window-minimize", body: WindowTarget(window: window))
        return try decode(ActionResponse.self, from: data)
    }

    // MARK: - Wait

    public func wait(window: String? = nil, timeout: Int? = nil) async throws -> ActionResponse {
        struct Req: Encodable { let window: String?; let timeout: Int? }
        let data = try await postJSON("/wait", body: Req(window: window, timeout: timeout))
        return try decode(ActionResponse.self, from: data)
    }

    // MARK: - System Commands

    public func exec(_ command: String, timeout: Int = 30, detach: Bool = false) async throws -> ExecResult {
        struct Req: Encodable { let command: String; let timeout: Int; let detach: Bool }
        // Give the agent a few seconds of headroom past its own deadline
        // so the URLSession layer never aborts a still-progressing exec
        // when the user explicitly asks for a long timeout.
        let httpTimeout = TimeInterval(timeout + 10)
        let data = try await postJSON(
            "/exec",
            body: Req(command: command, timeout: timeout, detach: detach),
            timeoutSeconds: httpTimeout
        )
        return try decode(ExecResult.self, from: data)
    }

    public func upload(path: String, content: Data) async throws {
        struct Req: Encodable { let path: String; let content: String }
        let body = Req(path: path, content: content.base64EncodedString())
        let data = try await postJSON("/upload", body: body)
        let result = try decode(ActionResponse.self, from: data)
        guard result.success else {
            throw AgentTCPClientError.httpError(500, result.message ?? "upload failed")
        }
    }

    public func download(path: String) async throws -> Data {
        struct Req: Encodable { let path: String }
        struct Resp: Decodable { let content: String }
        let data = try await postJSON("/download", body: Req(path: path))
        let resp = try decode(Resp.self, from: data)
        guard let decoded = Data(base64Encoded: resp.content) else {
            throw AgentTCPClientError.decodingFailed("invalid base64 content")
        }
        return decoded
    }

    public func shutdown() async throws {
        _ = try await postJSON("/shutdown", body: EmptyBody())
    }

    // MARK: - Transport

    private func get(_ path: String) async throws -> (Data, URLResponse) {
        let url = baseURL.appendingPathComponent(path)
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.timeoutInterval = 10
        do {
            return try await URLSession.shared.data(for: request)
        } catch {
            throw AgentTCPClientError.connectionFailed(error.localizedDescription)
        }
    }

    private func postJSON<T: Encodable>(
        _ path: String,
        body: T,
        timeoutSeconds: TimeInterval = 60
    ) async throws -> Data {
        let url = baseURL.appendingPathComponent(path)
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = timeoutSeconds
        request.httpBody = try JSONEncoder().encode(body)

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await URLSession.shared.data(for: request)
        } catch {
            throw AgentTCPClientError.connectionFailed(error.localizedDescription)
        }

        if let httpResponse = response as? HTTPURLResponse,
           !(200...299).contains(httpResponse.statusCode) {
            let message = parseErrorMessage(from: data) ?? "HTTP \(httpResponse.statusCode)"
            throw AgentTCPClientError.httpError(httpResponse.statusCode, message)
        }

        return data
    }

    private func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try JSONDecoder().decode(type, from: data)
        } catch {
            throw AgentTCPClientError.decodingFailed(error.localizedDescription)
        }
    }

    private func parseErrorMessage(from data: Data) -> String? {
        struct ErrorBody: Decodable { let error: String }
        return (try? JSONDecoder().decode(ErrorBody.self, from: data))?.error
    }
}

// MARK: - Request Types

private struct EmptyBody: Encodable {}

private struct ElementQuery: Encodable {
    let role: String?
    let label: String?
    let window: String?
    let id: String?
    let index: Int?
}

private struct WindowTarget: Encodable {
    let window: String
}

// MARK: - Response Types

/// Result of a remote command execution via the agent.
public struct ExecResult: Codable, Sendable {
    public let exitCode: Int32
    public let stdout: String
    public let stderr: String
    /// True when the agent had to terminate the process because it
    /// exceeded the requested timeout. Optional for compatibility with
    /// agents built before this field was added.
    public let timedOut: Bool?
    public var succeeded: Bool { exitCode == 0 }
}
