import CoreGraphics
import Foundation
import Hummingbird
import HTTPTypes
import NIOCore
@preconcurrency import RoyalVNCKit
import Vision

// MARK: - Server

/// VNC-only HTTP server using Hummingbird, listening on a Unix domain socket.
/// Routes VNC operations (screenshots, input, recording). No SSH, no OCR.
public actor TestAnywareServer {

    // MARK: - Properties

    private let spec: ConnectionSpec
    private let idleTimeout: Duration
    public let onShutdown: @Sendable () -> Void
    private let capture: VNCCapture
    private var recordingCapture: StreamingCapture?
    private var recordingTask: Task<Void, Never>?
    private var currentSocketPath: String?
    private var currentPidPath: String?
    private var ocrBridge: OCRChildBridge?

    // Idle timer is implemented as fire-and-forget tasks tagged with a
    // monotonic epoch. Each call to bumpIdleTimer increments the epoch and
    // schedules a fresh task; older tasks observe the stale epoch on wake
    // and exit without acting. Cancellation is deliberately avoided because
    // cancelling Task.sleep(for: Duration) hits a Swift Concurrency runtime
    // bug in -O builds (swift_task_dealloc → SIGABRT, "freed pointer was not
    // the last allocation"). See backlog Task 7 / memory note.
    private var idleTimerEpoch: UInt64 = 0
    // Recording loop uses the same epoch pattern as idleTimerEpoch: the
    // detached task captures the epoch value at start; finishRecording bumps
    // the counter so the in-flight task observes a stale epoch on its next
    // iteration and exits. The cancel-Task.sleep crash applies here too —
    // recordingTask is Task.detached, which is by definition a different
    // executor.
    private var recordingEpoch: UInt64 = 0
    private var hasShutDown: Bool = false

    // MARK: - Init

    public init(
        spec: ConnectionSpec,
        idleTimeout: Duration,
        onShutdown: @escaping @Sendable () -> Void,
        ocrBridge: OCRChildBridge? = nil
    ) {
        self.spec = spec
        self.idleTimeout = idleTimeout
        self.onShutdown = onShutdown
        self.capture = VNCCapture(spec: spec.vnc)
        self.ocrBridge = ocrBridge
        Self.scheduleIdleShutdown(epoch: 0, timeout: idleTimeout, server: self)
    }

    // MARK: - Connect

    public func connect() async throws {
        try await capture.connect()
    }

    // MARK: - Start

    /// Start the Hummingbird server on a Unix domain socket. Blocks until shutdown.
    public func start(
        socketPath: String,
        pidPath: String,
        onReady: @escaping @Sendable () async -> Void = {}
    ) async throws {
        currentSocketPath = socketPath
        currentPidPath = pidPath

        try? FileManager.default.removeItem(atPath: socketPath)
        let pid = ProcessInfo.processInfo.processIdentifier
        try "\(pid)\n".write(toFile: pidPath, atomically: true, encoding: .utf8)

        let router = buildRouter()
        let app = Application(
            router: router,
            configuration: .init(address: .unixDomainSocket(path: socketPath)),
            onServerRunning: { _ in await onReady() }
        )
        try await app.runService()
    }

    // MARK: - Shutdown

    public func shutdown() {
        if hasShutDown { return }
        hasShutDown = true

        recordingEpoch &+= 1
        recordingTask = nil

        if let sc = recordingCapture {
            recordingCapture = nil
            Task { try? await sc.stop() }
        }

        if let path = currentSocketPath {
            try? FileManager.default.removeItem(atPath: path)
            currentSocketPath = nil
        }
        if let path = currentPidPath {
            try? FileManager.default.removeItem(atPath: path)
            currentPidPath = nil
        }

        if let bridge = ocrBridge {
            ocrBridge = nil
            Task { await bridge.shutdown() }
        }

        let captureRef = capture
        Task { await captureRef.disconnect() }
        onShutdown()
    }

    // MARK: - Router

    public func buildRouter() -> Router<BasicRequestContext> {
        let router = Router()
        let server = self

        // Health
        router.get("/health") { _, _ in
            await server.handleHealth()
        }

        // Screen
        router.get("/screen-size") { _, _ in
            await server.handleScreenSize()
        }
        router.post("/screenshot") { request, _ in
            await server.handleScreenshot(request)
        }

        // Input
        router.post("/input/key") { request, _ in
            await server.handleInputKey(request)
        }
        router.post("/input/key-down") { request, _ in
            await server.handleInputKeyDown(request)
        }
        router.post("/input/key-up") { request, _ in
            await server.handleInputKeyUp(request)
        }
        router.post("/input/type") { request, _ in
            await server.handleInputType(request)
        }
        router.post("/input/click") { request, _ in
            await server.handleInputClick(request)
        }
        router.post("/input/mouse-down") { request, _ in
            await server.handleInputMouseDown(request)
        }
        router.post("/input/mouse-up") { request, _ in
            await server.handleInputMouseUp(request)
        }
        router.post("/input/move") { request, _ in
            await server.handleInputMove(request)
        }
        router.post("/input/scroll") { request, _ in
            await server.handleInputScroll(request)
        }
        router.post("/input/drag") { request, _ in
            await server.handleInputDrag(request)
        }

        // Recording
        router.post("/record/start") { request, _ in
            await server.handleRecordStart(request)
        }
        router.post("/record/stop") { _, _ in
            await server.handleRecordStop()
        }

        // OCR
        router.post("/ocr") { request, _ in
            await server.handleOCRRequest(request)
        }

        // Stop
        router.post("/stop") { _, _ in
            await server.handleStop()
        }

        return router
    }

    // MARK: - Idle Timer

    private func armIdleTimerIfNeeded() {
        if recordingTask == nil {
            bumpIdleTimer()
        }
    }

    private func bumpIdleTimer() {
        idleTimerEpoch &+= 1
        Self.scheduleIdleShutdown(epoch: idleTimerEpoch, timeout: idleTimeout, server: self)
    }

    private nonisolated static func scheduleIdleShutdown(
        epoch: UInt64,
        timeout: Duration,
        server: TestAnywareServer
    ) {
        Task { [weak server] in
            try? await Task.sleep(for: timeout)
            guard let server else { return }
            await server.fireIdleShutdownIfCurrent(epoch: epoch)
        }
    }

    private func fireIdleShutdownIfCurrent(epoch: UInt64) {
        guard !hasShutDown, idleTimerEpoch == epoch else { return }
        shutdown()
    }

    // MARK: - Handlers

    func handleHealth() -> Response {
        armIdleTimerIfNeeded()
        return jsonResponse(#"{"status":"ok"}"#)
    }

    func handleScreenSize() async -> Response {
        armIdleTimerIfNeeded()
        guard let size = await capture.screenSize() else {
            return jsonResponse(#"{"error":"screen size unavailable — VNC not connected"}"#, status: HTTPResponse.Status.serviceUnavailable)
        }
        return jsonResponse("{\"width\":\(Int(size.width)),\"height\":\(Int(size.height))}")
    }

    func handleScreenshot(_ request: Request) async -> Response {
        armIdleTimerIfNeeded()
        do {
            let region = try await parseRegion(from: request)
            let pngData = try await capture.screenshot(region: region)
            return pngResponse(pngData)
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputKey(_ request: Request) async -> Response {
        struct KeyRequest: Decodable {
            let key: String
            let modifiers: [String]?
        }
        armIdleTimerIfNeeded()
        do {
            let req: KeyRequest = try await decodeBody(request)
            let platform = spec.platform
            let key = req.key
            let modifiers = req.modifiers ?? []
            try await capture.withConnection { conn in
                try VNCInput.pressKey(key, modifiers: modifiers, platform: platform, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputKeyDown(_ request: Request) async -> Response {
        struct SingleKeyRequest: Decodable { let key: String }
        armIdleTimerIfNeeded()
        do {
            let req: SingleKeyRequest = try await decodeBody(request)
            let platform = spec.platform
            let key = req.key
            try await capture.withConnection { conn in
                try VNCInput.keyDown(key, platform: platform, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputKeyUp(_ request: Request) async -> Response {
        struct SingleKeyRequest: Decodable { let key: String }
        armIdleTimerIfNeeded()
        do {
            let req: SingleKeyRequest = try await decodeBody(request)
            let platform = spec.platform
            let key = req.key
            try await capture.withConnection { conn in
                try VNCInput.keyUp(key, platform: platform, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputType(_ request: Request) async -> Response {
        struct TypeRequest: Decodable { let text: String }
        armIdleTimerIfNeeded()
        do {
            let req: TypeRequest = try await decodeBody(request)
            let text = req.text
            try await capture.withConnection { conn in
                VNCInput.typeText(text, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputClick(_ request: Request) async -> Response {
        struct ClickRequest: Decodable {
            let x: UInt16; let y: UInt16
            let button: String?; let count: Int?
        }
        armIdleTimerIfNeeded()
        do {
            let req: ClickRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                try VNCInput.click(
                    x: req.x, y: req.y,
                    button: req.button ?? "left",
                    count: req.count ?? 1,
                    connection: conn
                )
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputMouseDown(_ request: Request) async -> Response {
        struct MouseRequest: Decodable {
            let x: UInt16; let y: UInt16; let button: String?
        }
        armIdleTimerIfNeeded()
        do {
            let req: MouseRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                try VNCInput.mouseDown(x: req.x, y: req.y, button: req.button ?? "left", connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputMouseUp(_ request: Request) async -> Response {
        struct MouseRequest: Decodable {
            let x: UInt16; let y: UInt16; let button: String?
        }
        armIdleTimerIfNeeded()
        do {
            let req: MouseRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                try VNCInput.mouseUp(x: req.x, y: req.y, button: req.button ?? "left", connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputMove(_ request: Request) async -> Response {
        struct MoveRequest: Decodable { let x: UInt16; let y: UInt16 }
        armIdleTimerIfNeeded()
        do {
            let req: MoveRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                VNCInput.mouseMove(x: req.x, y: req.y, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputScroll(_ request: Request) async -> Response {
        struct ScrollRequest: Decodable {
            let x: UInt16; let y: UInt16; let dx: Int; let dy: Int
        }
        armIdleTimerIfNeeded()
        do {
            let req: ScrollRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                VNCInput.scroll(x: req.x, y: req.y, deltaX: req.dx, deltaY: req.dy, connection: conn)
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleInputDrag(_ request: Request) async -> Response {
        struct DragRequest: Decodable {
            let fromX: UInt16; let fromY: UInt16
            let toX: UInt16; let toY: UInt16
            let button: String?; let steps: Int?
        }
        armIdleTimerIfNeeded()
        do {
            let req: DragRequest = try await decodeBody(request)
            try await capture.withConnection { conn in
                try VNCInput.drag(
                    fromX: req.fromX, fromY: req.fromY,
                    toX: req.toX, toY: req.toY,
                    button: req.button ?? "left",
                    steps: req.steps ?? 10,
                    connection: conn
                )
            }
            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleRecordStart(_ request: Request) async -> Response {
        struct RecordRequest: Decodable {
            let output: String; let fps: Int?; let duration: Int; let region: String?
        }
        armIdleTimerIfNeeded()
        if recordingTask != nil {
            return jsonResponse(#"{"error":"recording already active"}"#, status: HTTPResponse.Status.badRequest)
        }
        do {
            let req: RecordRequest = try await decodeBody(request)
            let cappedDuration = min(req.duration, 300)
            guard cappedDuration > 0 else {
                return jsonResponse(#"{"error":"duration must be > 0"}"#, status: HTTPResponse.Status.badRequest)
            }

            let region = try parseRegionFromString(req.region)

            let (width, height): (Int, Int)
            if let r = region {
                width = Int(r.width); height = Int(r.height)
            } else if let size = await capture.screenSize() {
                width = Int(size.width); height = Int(size.height)
            } else {
                return jsonResponse(
                    #"{"error":"cannot determine screen size — VNC not connected"}"#,
                    status: HTTPResponse.Status.serviceUnavailable
                )
            }

            let fps = req.fps ?? 30
            let sc = StreamingCapture()
            let config = StreamingCaptureConfig(width: width, height: height, fps: fps)
            try await sc.start(outputPath: req.output, config: config)
            self.recordingCapture = sc

            self.recordingEpoch &+= 1
            let myEpoch = self.recordingEpoch
            let captureSelf = self
            let captureSC = sc
            let task = Task.detached {
                let interval = Duration.nanoseconds(1_000_000_000 / max(fps, 1))
                let deadline = ContinuousClock.now + .seconds(cappedDuration)
                while ContinuousClock.now < deadline {
                    guard await captureSelf.isRecordingEpochCurrent(myEpoch) else { break }
                    do {
                        let image = try await captureSelf.captureImage(region: region)
                        try await captureSC.appendFrame(image)
                    } catch {}
                    try? await Task.sleep(for: interval)
                }
                await captureSelf.finishRecording()
            }
            self.recordingTask = task

            return okResponse()
        } catch {
            return errorResponse(error)
        }
    }

    func handleRecordStop() async -> Response {
        armIdleTimerIfNeeded()
        await finishRecording()
        return okResponse()
    }

    fileprivate func captureImage(region: CGRect?) async throws -> CGImage {
        try await capture.captureImage(region: region)
    }

    private func finishRecording() async {
        recordingEpoch &+= 1
        recordingTask = nil
        if let sc = recordingCapture {
            recordingCapture = nil
            try? await sc.stop()
        }
        bumpIdleTimer()
    }

    fileprivate func isRecordingEpochCurrent(_ epoch: UInt64) -> Bool {
        recordingEpoch == epoch
    }

    // MARK: - OCR handler

    func handleOCRRequest(_ request: Request) async -> Response {
        armIdleTimerIfNeeded()
        do {
            let buffer = try await request.body.collect(upTo: 50_000_000)
            let pngData = Data(buffer: buffer)
            let response = try await handleOCR(pngData: pngData)
            let encoder = JSONEncoder()
            encoder.outputFormatting = .sortedKeys
            let data = try encoder.encode(response)
            return jsonResponse(String(data: data, encoding: .utf8)!)
        } catch {
            return errorResponse(error)
        }
    }

    /// Dispatch OCR: macOS uses Vision.framework in-process,
    /// Linux/Windows use the Python daemon via OCRChildBridge.
    public func handleOCR(pngData: Data) async throws -> OCRResponse {
        armIdleTimerIfNeeded()

        let platform = spec.platform

        if platform == .macos || platform == nil {
            let detections = VisionOCREngine.recognize(pngData: pngData)
            return OCRResponse(engine: "vision", detections: detections)
        }

        // Linux/Windows — use bridge
        if ocrBridge == nil {
            ocrBridge = OCRChildBridge(interpreterPath: resolveOCRInterpreterPath())
        }

        do {
            let detections = try await ocrBridge!.recognize(pngData: pngData)
            return OCRResponse(engine: "easyocr_daemon", detections: detections)
        } catch let error as OCRBridgeError {
            if case .permanentlyUnavailable = error {
                // Check for fallback mode
                let fallbackEnabled = ProcessInfo.processInfo.environment["TESTANYWARE_OCR_FALLBACK"] == "1"
                if fallbackEnabled {
                    let detections = VisionOCREngine.recognize(pngData: pngData)
                    return OCRResponse(
                        engine: "vision",
                        detections: detections,
                        warning: "OCR daemon unavailable (\(error.localizedDescription)). Using Vision fallback."
                    )
                }
                // Write status file
                let status = OCRStatusFile.Status(
                    degraded: true,
                    reason: error.localizedDescription,
                    lastCheck: ISO8601DateFormatter().string(from: Date())
                )
                try? OCRStatusFile.write(status)
            }
            throw error
        }
    }

    private func resolveOCRInterpreterPath() -> String {
        // 1. Environment variable override
        if let envPath = ProcessInfo.processInfo.environment["TESTANYWARE_OCR_PYTHON"],
           FileManager.default.fileExists(atPath: envPath) {
            return envPath
        }

        // 2. Cellar-relative (Homebrew)
        var execPath = currentExecutablePath()
        // Resolve symlinks to find real path (Cellar layout discovery)
        if let realPath = try? FileManager.default.destinationOfSymbolicLink(atPath: execPath) {
            execPath = realPath
        }
        let cellarPython = (execPath as NSString)
            .deletingLastPathComponent  // bin/
            .appending("/../libexec/venv/bin/python")
        let cellarResolved = (cellarPython as NSString).standardizingPath
        if FileManager.default.fileExists(atPath: cellarResolved) {
            return cellarResolved
        }

        // 3. Dev fallback — pipeline/.venv/bin/python relative to repo root
        let repoRelative = (execPath as NSString)
            .deletingLastPathComponent  // Sources/testanyware/ or .build/debug/
        // Walk up until we find pipeline/.venv
        var dir = repoRelative
        for _ in 0..<10 {
            let candidate = (dir as NSString).appendingPathComponent("pipeline/.venv/bin/python")
            if FileManager.default.fileExists(atPath: candidate) {
                return candidate
            }
            dir = (dir as NSString).deletingLastPathComponent
        }

        // Fallback: return the system python (will likely fail but gives a clear error)
        return "/usr/bin/python3"
    }

    func handleStop() -> Response {
        Task { self.shutdown() }
        return jsonResponse(#"{"ok":true}"#)
    }

    // MARK: - Body Parsing

    private func decodeBody<T: Decodable>(_ request: Request) async throws -> T {
        let buffer = try await request.body.collect(upTo: 10_485_760)
        let data = Data(buffer: buffer)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private func parseRegion(from request: Request) async throws -> CGRect? {
        struct RegionRequest: Decodable { let region: String? }
        let buffer = try await request.body.collect(upTo: 1024)
        guard buffer.readableBytes > 0 else { return nil }
        let data = Data(buffer: buffer)
        let req = try JSONDecoder().decode(RegionRequest.self, from: data)
        return try parseRegionFromString(req.region)
    }

    private func parseRegionFromString(_ regionString: String?) throws -> CGRect? {
        guard let regionString else { return nil }
        let parts = regionString.split(separator: ",").compactMap {
            Double($0.trimmingCharacters(in: .whitespaces))
        }
        guard parts.count == 4 else {
            throw RegionParseError.invalid(regionString)
        }
        return CGRect(x: parts[0], y: parts[1], width: parts[2], height: parts[3])
    }

    // MARK: - Response Helpers

    nonisolated func jsonResponse(_ body: String, status: HTTPResponse.Status = .ok) -> Response {
        Response(
            status: status,
            headers: [.contentType: "application/json"],
            body: .init(byteBuffer: ByteBuffer(string: body))
        )
    }

    nonisolated func okResponse() -> Response {
        jsonResponse(#"{"ok":true}"#)
    }

    nonisolated func pngResponse(_ data: Data) -> Response {
        Response(
            status: .ok,
            headers: [.contentType: "image/png"],
            body: .init(byteBuffer: ByteBuffer(bytes: Array(data)))
        )
    }

    nonisolated func errorResponse(_ error: Error) -> Response {
        jsonResponse(
            "{\"error\":\(escapeJSON(error.localizedDescription))}",
            status: .internalServerError
        )
    }

    nonisolated func escapeJSON(_ string: String) -> String {
        var escaped = ""
        escaped.reserveCapacity(string.utf16.count + 2)
        for scalar in string.unicodeScalars {
            switch scalar {
            case "\\": escaped += "\\\\"
            case "\"": escaped += "\\\""
            case "\n": escaped += "\\n"
            case "\r": escaped += "\\r"
            case "\t": escaped += "\\t"
            default:
                if scalar.value < 0x20 {
                    escaped += String(format: "\\u%04X", scalar.value)
                } else {
                    escaped.unicodeScalars.append(scalar)
                }
            }
        }
        return "\"\(escaped)\""
    }
}

// MARK: - Region Parse Error

private enum RegionParseError: LocalizedError {
    case invalid(String)
    var errorDescription: String? {
        switch self {
        case .invalid(let s):
            "Invalid region '\(s)'. Expected format: x,y,w,h"
        }
    }
}
