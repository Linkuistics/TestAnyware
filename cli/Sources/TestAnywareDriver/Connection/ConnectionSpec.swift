import Foundation

// MARK: - Spec Types

/// VNC connection parameters. Required for all connections.
public struct VNCSpec: Codable, Equatable, Sendable {
    public let host: String
    public let port: Int
    public let password: String?

    public init(host: String, port: Int = 5900, password: String? = nil) {
        self.host = host
        self.port = port
        self.password = password
    }
}

/// Agent TCP service connection parameters. Optional — enables accessibility, exec, file transfer.
public struct AgentSpec: Codable, Equatable, Sendable {
    public let host: String
    public let port: Int

    public init(host: String, port: Int = 8648) {
        self.host = host
        self.port = port
    }
}

/// Complete connection specification for a target machine.
/// `vnc` is required. `agent` adds accessibility tree, exec, and file transfer.
public struct ConnectionSpec: Codable, Sendable {
    public let vnc: VNCSpec
    public let agent: AgentSpec?
    public let platform: Platform?

    public init(vnc: VNCSpec, agent: AgentSpec? = nil, platform: Platform? = nil) {
        self.vnc = vnc
        self.agent = agent
        self.platform = platform
    }
}

// MARK: - Loading

extension ConnectionSpec {
    /// Load a connection spec from a JSON file.
    public static func load(from path: String) throws -> ConnectionSpec {
        let expandedPath = NSString(string: path).expandingTildeInPath
        let url = URL(fileURLWithPath: expandedPath)
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(ConnectionSpec.self, from: data)
    }

    /// Construct a connection spec from CLI flag values.
    public static func from(
        vnc: String,
        agent: String? = nil,
        platform: String? = nil
    ) throws -> ConnectionSpec {
        let vncSpec = try parseVNCEndpoint(vnc)
        let agentSpec = try agent.map { try parseAgentEndpoint($0) }
        let platformValue = try platform.map { try parsePlatform($0) }
        return ConnectionSpec(vnc: vncSpec, agent: agentSpec, platform: platformValue)
    }

    /// Build a connection spec from TESTANYWARE_* environment variables.
    /// Returns nil if TESTANYWARE_VNC is not set; otherwise parses VNC,
    /// optional password, optional agent, and optional platform.
    public static func fromEnvironment(
        _ env: [String: String] = ProcessInfo.processInfo.environment
    ) throws -> ConnectionSpec? {
        guard let vncEndpoint = env["TESTANYWARE_VNC"] else { return nil }
        let (vncHost, vncPort) = try parseHostPort(vncEndpoint, defaultPort: 5900)
        let vncSpec = VNCSpec(
            host: vncHost, port: vncPort,
            password: env["TESTANYWARE_VNC_PASSWORD"]
        )
        let agentSpec = try env["TESTANYWARE_AGENT"].map { try parseAgentEndpoint($0) }
        let platformValue = try env["TESTANYWARE_PLATFORM"].map { try parsePlatform($0) }
        return ConnectionSpec(vnc: vncSpec, agent: agentSpec, platform: platformValue)
    }
}

// MARK: - Endpoint Parsing

extension ConnectionSpec {
    static func parseVNCEndpoint(_ endpoint: String) throws -> VNCSpec {
        let (host, port) = try parseHostPort(endpoint, defaultPort: 5900)
        return VNCSpec(host: host, port: port)
    }

    public static func parseAgentEndpoint(_ endpoint: String) throws -> AgentSpec {
        let (host, port) = try parseHostPort(endpoint, defaultPort: 8648)
        return AgentSpec(host: host, port: port)
    }

    static func parsePlatform(_ value: String) throws -> Platform {
        guard let platform = Platform(rawValue: value.lowercased()) else {
            throw ConnectionSpecError.invalidPlatform(value)
        }
        return platform
    }

    static func parseHostPort(_ endpoint: String, defaultPort: Int) throws -> (String, Int) {
        let parts = endpoint.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        let host = String(parts[0])
        if host.isEmpty {
            throw ConnectionSpecError.emptyHost
        }
        if parts.count == 2 {
            guard let port = Int(parts[1]), port > 0, port <= 65535 else {
                throw ConnectionSpecError.invalidPort(String(parts[1]))
            }
            return (host, port)
        }
        return (host, defaultPort)
    }
}

// MARK: - Errors

public enum ConnectionSpecError: LocalizedError {
    case invalidPlatform(String)
    case emptyHost
    case invalidPort(String)

    public var errorDescription: String? {
        switch self {
        case .invalidPlatform(let value):
            "Invalid platform '\(value)'. Expected: macos, windows, or linux"
        case .emptyHost:
            "Host cannot be empty"
        case .invalidPort(let port):
            "Invalid port '\(port)'. Expected a number between 1 and 65535"
        }
    }
}
