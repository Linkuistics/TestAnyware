import ArgumentParser
import TestAnywareDriver

struct ScreenSizeCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "screen-size",
        abstract: "Query the VNC display dimensions"
    )

    @OptionGroup var connection: ConnectionOptions

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)
        let size = try await client.screenSize()
        print("\(Int(size.width))x\(Int(size.height))")
    }
}
