import ArgumentParser
import Foundation
import TestAnywareDriver

struct RecordCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "record",
        abstract: "Record VNC screen to a video file"
    )

    @OptionGroup var connection: ConnectionOptions

    @Option(name: .shortAndLong, help: "Output file path")
    var output: String = "recording.mp4"

    @Option(name: .long, help: "Frames per second")
    var fps: Int = 30

    @Option(name: .long, help: "Duration in seconds (0 = use max 300s)")
    var duration: Int = 0

    @Option(name: .long, help: "Crop region as x,y,width,height")
    var region: String?

    mutating func run() async throws {
        let spec = try connection.resolve()

        let cropRegion: CGRect?
        if let regionStr = region {
            let parts = regionStr.split(separator: ",").compactMap { Double($0) }
            guard parts.count == 4 else {
                throw ValidationError("Region must be x,y,width,height")
            }
            cropRegion = CGRect(x: parts[0], y: parts[1], width: parts[2], height: parts[3])
        } else {
            cropRegion = nil
        }

        let client = try await ServerClient.ensure(spec: spec)

        // duration 0 originally meant "until Ctrl+C"; map to server's max cap of 300s
        let effectiveDuration = duration > 0 ? duration : 300

        try await client.recordStart(output: output, fps: fps, duration: effectiveDuration, region: cropRegion)

        let screenSize = try await client.screenSize()
        let recordWidth = Int(cropRegion?.width ?? screenSize.width)
        let recordHeight = Int(cropRegion?.height ?? screenSize.height)

        print("Recording to \(output) at \(fps) fps (\(recordWidth)x\(recordHeight))...")
        print("Duration: \(effectiveDuration)s")

        try await Task.sleep(for: .seconds(effectiveDuration))

        try await client.recordStop()
        print("Recording saved to \(output)")
    }
}
