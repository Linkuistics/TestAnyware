import Foundation

/// Error types for the OCR child bridge.
public enum OCRBridgeError: Error, LocalizedError {
    case permanentlyUnavailable(reason: String)
    case childCrashed
    case responseTimeout

    public var errorDescription: String? {
        switch self {
        case .permanentlyUnavailable(let reason):
            "OCR daemon permanently unavailable: \(reason)"
        case .childCrashed:
            "OCR daemon child process crashed"
        case .responseTimeout:
            "OCR daemon did not respond in time"
        }
    }
}

/// Actor managing a long-lived Python child process that holds the EasyOCR
/// reader warm between calls. Communicates via temp-file PNG + JSON stdin/stdout.
public actor OCRChildBridge {

    // MARK: - Configuration

    private let interpreterPath: String
    private let daemonArguments: [String]
    private let environment: [String: String]
    private let warmDeadline: Duration
    private let firstCallDeadline: Duration

    // MARK: - Child state

    private var child: Process?
    private var childStdin: FileHandle?
    private var childStdout: FileHandle?
    private var isChildRunning: Bool = false
    private var stickyUnavailableReason: String?
    private var transientFailureCount: Int = 0

    // MARK: - Init

    public init(
        interpreterPath: String,
        daemonArguments: [String] = ["-m", "ocr_analyzer", "--daemon"],
        environment: [String: String] = [:],
        warmDeadline: Duration = .seconds(8),
        firstCallDeadline: Duration = .seconds(15)
    ) {
        self.interpreterPath = interpreterPath
        self.daemonArguments = daemonArguments
        self.environment = environment
        self.warmDeadline = warmDeadline
        self.firstCallDeadline = firstCallDeadline
    }

    // MARK: - Public API

    public func recognize(pngData: Data) async throws -> [OCRDetection] {
        if let reason = stickyUnavailableReason {
            throw OCRBridgeError.permanentlyUnavailable(reason: reason)
        }

        if !isChildRunning {
            try await spawnChild()
        }

        let tmpPath = NSTemporaryDirectory() + "testanyware-ocr-\(UUID().uuidString).png"
        try pngData.write(to: URL(fileURLWithPath: tmpPath))
        defer { try? FileManager.default.removeItem(atPath: tmpPath) }

        let request = "{\"image_path\":\"\(tmpPath)\"}\n"
        guard let requestData = request.data(using: .utf8) else {
            throw OCRBridgeError.childCrashed
        }

        childStdin?.write(requestData)

        guard let responseLine = readLineFromChild() else {
            throw OCRBridgeError.childCrashed
        }

        guard let responseData = responseLine.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: responseData) as? [String: Any]
        else {
            throw OCRBridgeError.childCrashed
        }

        if let error = json["error"] as? String {
            throw OCRBridgeError.permanentlyUnavailable(reason: error)
        }

        guard let detectionsArray = json["detections"] as? [[String: Any]] else {
            throw OCRBridgeError.childCrashed
        }

        return detectionsArray.compactMap { dict -> OCRDetection? in
            guard let text = dict["text"] as? String ?? dict["label"] as? String else { return nil }

            let (x, y, width, height): (Double, Double, Double, Double)
            if let bbox = dict["bbox"] as? [Double], bbox.count == 4 {
                x = bbox[0]
                y = bbox[1]
                width = bbox[2] - bbox[0]
                height = bbox[3] - bbox[1]
            } else {
                x = dict["x"] as? Double ?? 0
                y = dict["y"] as? Double ?? 0
                width = dict["width"] as? Double ?? 0
                height = dict["height"] as? Double ?? 0
            }
            let confidence = Float(dict["confidence"] as? Double ?? 0)
            return OCRDetection(text: text, x: x, y: y, width: width, height: height, confidence: confidence)
        }
    }

    public func shutdown() {
        guard let child, isChildRunning else { return }
        childStdin?.closeFile()
        child.terminate()
        child.waitUntilExit()
        self.child = nil
        self.childStdin = nil
        self.childStdout = nil
        self.isChildRunning = false
    }

    // MARK: - Private

    private func spawnChild() async throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: interpreterPath)
        process.arguments = daemonArguments

        var env = ProcessInfo.processInfo.environment
        for (key, value) in environment {
            env[key] = value
        }
        process.environment = env

        let stdinPipe = Pipe()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardInput = stdinPipe
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
        } catch {
            throw OCRBridgeError.permanentlyUnavailable(reason: "failed to spawn child: \(error.localizedDescription)")
        }

        self.child = process
        self.childStdin = stdinPipe.fileHandleForWriting
        self.childStdout = stdoutPipe.fileHandleForReading
        self.isChildRunning = true

        // Wait for {"ready": true}
        guard let readyLine = readLineFromChild() else {
            killChild()
            throw OCRBridgeError.permanentlyUnavailable(reason: "child exited before signaling ready")
        }

        guard let readyData = readyLine.data(using: .utf8),
              let readyJson = try? JSONSerialization.jsonObject(with: readyData) as? [String: Any],
              readyJson["ready"] as? Bool == true
        else {
            killChild()
            throw OCRBridgeError.permanentlyUnavailable(reason: "child did not send ready signal")
        }
    }

    private func readLineFromChild() -> String? {
        guard let stdout = childStdout else { return nil }

        var lineData = Data()
        while true {
            let chunk = stdout.readData(ofLength: 1)
            if chunk.isEmpty {
                // EOF — child died
                isChildRunning = false
                return lineData.isEmpty ? nil : String(data: lineData, encoding: .utf8)
            }
            if chunk[0] == UInt8(ascii: "\n") {
                return String(data: lineData, encoding: .utf8)
            }
            lineData.append(chunk)
        }
    }

    private func killChild() {
        child?.terminate()
        child?.waitUntilExit()
        child = nil
        childStdin = nil
        childStdout = nil
        isChildRunning = false
    }
}
