import ArgumentParser
import Foundation
import TestAnywareDriver

struct FindTextCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "find-text",
        abstract: "Find text on screen using OCR (Vision on macOS, EasyOCR daemon on Linux/Windows)"
    )

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Text to search for (case-insensitive substring match)")
    var text: String?

    @Option(name: .long, help: "Wait up to N seconds for the text to appear")
    var timeout: Int?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        let deadline = timeout.map { Date().addingTimeInterval(Double($0)) }

        while true {
            let pngData = try await client.screenshot()
            let response = try await client.ocr(pngData: pngData)

            if let warning = response.warning {
                FileHandle.standardError.write(Data((warning + "\n").utf8))
            }

            let matches: [OCRDetection]
            if let searchText = text {
                matches = response.detections.filter {
                    $0.text.localizedCaseInsensitiveContains(searchText)
                }
            } else {
                matches = response.detections
            }

            if !matches.isEmpty {
                let encoder = JSONEncoder()
                encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                let data = try encoder.encode(matches)
                print(String(data: data, encoding: .utf8)!)
                return
            }

            if let deadline, Date() < deadline {
                try await Task.sleep(for: .milliseconds(500))
                continue
            }

            if text != nil {
                throw ValidationError("Text '\(text!)' not found on screen")
            } else {
                print("[]")
                return
            }
        }
    }
}
