import ArgumentParser
import Foundation
import TestAnywareDriver

struct ExecCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "exec", abstract: "Execute a command on the VM via agent")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Command to execute")
    var command: String

    @Flag(name: .long, help: "Launch process detached (return immediately without waiting)")
    var detach: Bool = false

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let result = try await agent.exec(command, detach: detach)
        if !result.stdout.isEmpty { print(result.stdout) }
        if !result.stderr.isEmpty { FileHandle.standardError.write(Data((result.stderr + "\n").utf8)) }
        if !result.succeeded {
            throw ExitCode(result.exitCode)
        }
    }
}

struct UploadCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "upload", abstract: "Upload a file to the VM via agent")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Local file path")
    var localPath: String

    @Argument(help: "Remote file path")
    var remotePath: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let data = try Data(contentsOf: URL(fileURLWithPath: localPath))
        try await agent.upload(path: remotePath, content: data)
        print("Uploaded \(localPath) → \(remotePath)")
    }
}

struct DownloadCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "download", abstract: "Download a file from the VM via agent")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Remote file path")
    var remotePath: String

    @Argument(help: "Local file path")
    var localPath: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let data = try await agent.download(path: remotePath)
        try data.write(to: URL(fileURLWithPath: localPath))
        print("Downloaded \(remotePath) → \(localPath)")
    }
}
