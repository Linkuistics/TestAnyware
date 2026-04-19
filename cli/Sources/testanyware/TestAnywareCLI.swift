import ArgumentParser
import Foundation
import TestAnywareDriver

@main
struct TestAnywareCLI: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "testanyware",
        abstract: "VNC + agent driver for virtual machine automation",
        version: "0.2.0",
        subcommands: [
            ScreenshotCommand.self,
            ScreenSizeCommand.self,
            InputCommand.self,
            ExecCommand.self,
            UploadCommand.self,
            DownloadCommand.self,
            RecordCommand.self,
            FindTextCommand.self,
            ServerCommand.self,
            AgentCommand.self,
            VMCommand.self,
        ]
    )
}

/// Shared connection options used by all subcommands.
struct ConnectionOptions: ParsableArguments {
    @Option(name: .long, help: "Path to connection spec JSON file")
    var connect: String?

    @Option(name: .long, help: "VM instance id (resolves to $XDG_STATE_HOME/testanyware/vms/<id>.json)")
    var vm: String?

    @Option(name: .long, help: "VNC endpoint (host:port)")
    var vnc: String?

    @Option(name: .long, help: "Agent endpoint (host:port, default port 8648)")
    var agent: String?

    @Option(name: .long, help: "Target platform (macos, windows, linux)")
    var platform: String?

    /// Resolution chain, highest priority first:
    ///   1. `--connect <path>` — explicit spec file
    ///   2. `--vm <id>` — per-VM spec at $XDG_STATE_HOME/testanyware/vms/<id>.json
    ///   3. `--vnc` (plus `--agent`, `--platform`) — explicit flags
    ///   4. `TESTANYWARE_VM_ID` env var — resolves like `--vm`
    ///   5. `TESTANYWARE_VNC` / `TESTANYWARE_VNC_PASSWORD` / `TESTANYWARE_AGENT`
    ///      / `TESTANYWARE_PLATFORM` — direct env vars
    ///   6. Error
    func resolve() throws -> ConnectionSpec {
        if let connectPath = connect {
            return try ConnectionSpec.load(from: connectPath)
        }
        if let id = vm {
            return try loadSpec(forVMID: id)
        }
        if let vncEndpoint = vnc {
            return try ConnectionSpec.from(vnc: vncEndpoint, agent: agent, platform: platform)
        }
        if let envID = ProcessInfo.processInfo.environment["TESTANYWARE_VM_ID"], !envID.isEmpty {
            return try loadSpec(forVMID: envID)
        }
        if let envSpec = try ConnectionSpec.fromEnvironment() {
            return envSpec
        }
        throw ValidationError(
            "No connection specified. Provide --connect <path>, --vm <id>, "
            + "--vnc <host:port>, or set TESTANYWARE_VM_ID / TESTANYWARE_VNC. "
            + "Start a VM with scripts/macos/vm-start.sh to create a spec."
        )
    }

    func resolveAgent() throws -> AgentTCPClient {
        if let agentEndpoint = agent {
            let agentSpec = try ConnectionSpec.parseAgentEndpoint(agentEndpoint)
            return AgentTCPClient(spec: agentSpec)
        }
        if let agentEnv = ProcessInfo.processInfo.environment["TESTANYWARE_AGENT"] {
            let agentSpec = try ConnectionSpec.parseAgentEndpoint(agentEnv)
            return AgentTCPClient(spec: agentSpec)
        }
        if let spec = try? resolve(), let agentSpec = spec.agent {
            return AgentTCPClient(spec: agentSpec)
        }
        throw ValidationError(
            "Agent endpoint required. Provide --agent <host:port>, set TESTANYWARE_AGENT, "
            + "or ensure the resolved connect spec includes an agent section."
        )
    }

    private func loadSpec(forVMID id: String) throws -> ConnectionSpec {
        let path = ConnectionSpec.namedSpecPath(for: id)
        guard FileManager.default.fileExists(atPath: path) else {
            throw ValidationError(
                "No spec found for VM id '\(id)' at \(path). "
                + "Start it with scripts/macos/vm-start.sh, or check the id."
            )
        }
        return try ConnectionSpec.load(from: path)
    }
}
