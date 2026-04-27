import Testing
import Foundation

/// `TestAnywareAgentProtocol` is intentionally duplicated between
/// `cli/Sources/TestAnywareAgentProtocol/` and
/// `agents/macos/Sources/TestAnywareAgentProtocol/`. The cli will migrate
/// to Rust, so the host and agent share a wire-level contract (JSON-RPC
/// 2.0, port 8648) rather than Swift types — see `agents/macos/Package.swift`
/// for the rationale. This test guards the duplication while it stands:
/// it fails the moment the two directories diverge.
@Suite("AgentProtocolSync")
struct AgentProtocolSyncTests {

    @Test func agentProtocolDirectoriesContainSameFiles() throws {
        let cliFiles = try sortedSwiftFiles(at: cliCopyDirectory())
        let agentsFiles = try sortedSwiftFiles(at: agentsCopyDirectory())
        #expect(
            cliFiles == agentsFiles,
            "cli/Sources/TestAnywareAgentProtocol and agents/macos/Sources/TestAnywareAgentProtocol contain different files"
        )
    }

    @Test func agentProtocolFilesAreByteIdentical() throws {
        let cliDir = cliCopyDirectory()
        let agentsDir = agentsCopyDirectory()
        let names = try sortedSwiftFiles(at: cliDir)
        for name in names {
            let cliData = try Data(contentsOf: cliDir.appendingPathComponent(name))
            let agentsData = try Data(contentsOf: agentsDir.appendingPathComponent(name))
            #expect(
                cliData == agentsData,
                "TestAnywareAgentProtocol/\(name) differs between cli/ and agents/macos/"
            )
        }
    }

    // MARK: - Helpers

    private func cliCopyDirectory() -> URL {
        repoRoot()
            .appendingPathComponent("cli", isDirectory: true)
            .appendingPathComponent("Sources", isDirectory: true)
            .appendingPathComponent("TestAnywareAgentProtocol", isDirectory: true)
    }

    private func agentsCopyDirectory() -> URL {
        repoRoot()
            .appendingPathComponent("agents", isDirectory: true)
            .appendingPathComponent("macos", isDirectory: true)
            .appendingPathComponent("Sources", isDirectory: true)
            .appendingPathComponent("TestAnywareAgentProtocol", isDirectory: true)
    }

    /// `cli/Tests/TestAnywareAgentProtocolTests/AgentProtocolSyncTests.swift`
    /// → repo root is three directories up.
    private func repoRoot(file: String = #filePath) -> URL {
        URL(fileURLWithPath: file)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private func sortedSwiftFiles(at dir: URL) throws -> [String] {
        try FileManager.default
            .contentsOfDirectory(atPath: dir.path)
            .filter { $0.hasSuffix(".swift") }
            .sorted()
    }
}
