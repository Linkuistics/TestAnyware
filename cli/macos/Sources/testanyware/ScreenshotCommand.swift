import ArgumentParser
import Foundation
import TestAnywareDriver
import TestAnywareAgentProtocol

struct ScreenshotCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "screenshot",
        abstract: "Capture a screenshot from the VNC server"
    )

    @OptionGroup var connection: ConnectionOptions

    @Option(name: .shortAndLong, help: "Output file path (default: screenshot.png)")
    var output: String = "screenshot.png"

    @Option(name: .long, help: "Crop region as x,y,width,height")
    var region: String?

    @Option(name: .long, help: "Window name for relative coordinates (crops to window bounds when no --region specified)")
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()

        let client = try await ServerClient.ensure(spec: spec)

        let cropRegion: CGRect?
        if let regionStr = region {
            cropRegion = try parseRegion(regionStr)
        } else if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            cropRegion = CGRect(
                x: win.position.x, y: win.position.y,
                width: win.size.width, height: win.size.height
            )
        } else {
            cropRegion = nil
        }

        let pngData = try await client.screenshot(region: cropRegion)
        let url = URL(fileURLWithPath: output)
        try pngData.write(to: url)
        print("Screenshot saved to \(output) (\(pngData.count) bytes)")
    }

    private func parseRegion(_ str: String) throws -> CGRect {
        let parts = str.split(separator: ",").compactMap { Double($0) }
        guard parts.count == 4 else {
            throw ValidationError("Region must be x,y,width,height (e.g. 0,0,800,600)")
        }
        return CGRect(x: parts[0], y: parts[1], width: parts[2], height: parts[3])
    }
}
