import ApplicationServices
import AppKit
import CoreGraphics
import Foundation
import TestAnywareAgent
import TestAnywareAgentProtocol
import Hummingbird
import HTTPTypes
import NIOCore

// MARK: - Request types

struct ElementQuery: Decodable {
    let role: String?
    let label: String?
    let window: String?
    let id: String?
    let index: Int?
}

struct SnapshotRequest: Decodable {
    let mode: String?
    let window: String?
    let role: String?
    let label: String?
    let depth: Int?
}

struct SetValueRequest: Decodable {
    let role: String?
    let label: String?
    let window: String?
    let id: String?
    let index: Int?
    let value: String
}

struct WindowTarget: Decodable {
    let window: String
}

struct WindowResizeRequest: Decodable {
    let window: String
    let width: Int
    let height: Int
}

struct WindowMoveRequest: Decodable {
    let window: String
    let x: Int
    let y: Int
}

struct WaitRequest: Decodable {
    let window: String?
    let timeout: Int?
}

struct ExecRequest: Decodable {
    let command: String
    let timeout: Int?
    let detach: Bool?
}

struct UploadRequest: Decodable {
    let path: String
    let content: String
}

struct DownloadRequest: Decodable {
    let path: String
}


// MARK: - Router

func buildAgentRouter() -> Router<BasicRequestContext> {
    let router = Router()

    router.get("/health") { _, _ in handleHealth() }
    router.post("/windows") { req, _ in try await handleWindows(req) }
    router.post("/snapshot") { req, _ in try await handleSnapshot(req) }
    router.post("/inspect") { req, _ in try await handleInspect(req) }
    router.post("/press") { req, _ in try await handleAction(req, name: "press", perform: ActionPerformer.press) }
    router.post("/set-value") { req, _ in try await handleSetValue(req) }
    router.post("/focus") { req, _ in try await handleAction(req, name: "focus", perform: ActionPerformer.focus) }
    router.post("/show-menu") { req, _ in try await handleAction(req, name: "show-menu", perform: ActionPerformer.showMenu) }
    router.post("/window-focus") { req, _ in try await handleWindowFocus(req) }
    router.post("/window-resize") { req, _ in try await handleWindowResize(req) }
    router.post("/window-move") { req, _ in try await handleWindowMove(req) }
    router.post("/window-close") { req, _ in try await handleWindowClose(req) }
    router.post("/window-minimize") { req, _ in try await handleWindowMinimize(req) }
    router.post("/wait") { req, _ in try await handleWait(req) }
    router.post("/exec") { req, _ in try await handleExec(req) }
    router.post("/upload") { req, _ in try await handleUpload(req) }
    router.post("/download") { req, _ in try await handleDownload(req) }
    router.post("/shutdown") { _, _ in handleShutdown() }
    router.post("/debug/ax") { _, _ in handleDebugAX() }

    return router
}

// MARK: - Body decode helper

private func decode<T: Decodable>(_ request: Request) async throws -> T {
    let buffer = try await request.body.collect(upTo: 10_485_760)
    return try JSONDecoder().decode(T.self, from: Data(buffer: buffer))
}

// MARK: - Response helpers

private func jsonOK<T: Encodable>(_ value: T) -> Response {
    let data = try! JSONEncoder().encode(value)
    return Response(
        status: .ok,
        headers: [.contentType: "application/json"],
        body: .init(byteBuffer: ByteBuffer(bytes: Array(data)))
    )
}

private func jsonError(_ message: String, details: String? = nil, status: HTTPResponse.Status = .badRequest) -> Response {
    let err = ErrorResponse(error: message, details: details)
    let data = try! JSONEncoder().encode(err)
    return Response(
        status: status,
        headers: [.contentType: "application/json"],
        body: .init(byteBuffer: ByteBuffer(bytes: Array(data)))
    )
}

// MARK: - Health

private func handleHealth() -> Response {
    struct HealthResponse: Encodable {
        let accessible: Bool
        let platform: String
    }
    return jsonOK(HealthResponse(accessible: AXIsProcessTrusted(), platform: "macos"))
}

// MARK: - Windows

private func handleWindows(_ request: Request) async throws -> Response {
    let windows = enumerateWindows()
    return jsonOK(SnapshotResponse(windows: windows))
}

// MARK: - Snapshot

private func handleSnapshot(_ request: Request) async throws -> Response {
    let req: SnapshotRequest = try await decode(request)
    let mode = req.mode ?? "interact"
    let depth = req.depth ?? 3
    let roleFilter: UnifiedRole? = req.role.map { RoleMapper.map(role: $0) }

    var windows = enumerateWindows()
    if let filterStr = req.window {
        windows = windows.filter { windowMatches($0, filter: filterStr) }
    }

    var snapshotWindows = windows.map { win -> WindowInfo in
        guard let winElement = findWindowElement(matching: win) else { return win }

        let rawElements = TreeWalker.walk(
            root: winElement, depth: depth,
            roleFilter: roleFilter, labelFilter: req.label
        )

        let filteredElements: [ElementInfo]
        switch mode {
        case "interact": filteredElements = filterInteractive(rawElements)
        case "layout": filteredElements = filterLayout(rawElements)
        default: filteredElements = rawElements
        }

        return WindowInfo(
            title: win.title, windowType: win.windowType,
            size: win.size, position: win.position,
            appName: win.appName, focused: win.focused,
            elements: filteredElements
        )
    }

    // Include the focused app's menu bar as a pseudo-window.
    // macOS menu bar items (File, Edit, View, ...) are always visible at the
    // top of the screen but live outside the window hierarchy, so they need
    // separate enumeration.
    if let menuBarWin = focusedAppMenuBar(
        roleFilter: roleFilter, labelFilter: req.label, mode: mode, depth: depth
    ) {
        if let filterStr = req.window {
            if windowMatches(menuBarWin, filter: filterStr) {
                snapshotWindows.append(menuBarWin)
            }
        } else {
            snapshotWindows.append(menuBarWin)
        }
    }

    return jsonOK(SnapshotResponse(windows: snapshotWindows))
}

// MARK: - Inspect

private func handleInspect(_ request: Request) async throws -> Response {
    let req: ElementQuery = try await decode(request)
    let roleFilter: UnifiedRole? = req.role.map { RoleMapper.map(role: $0) }

    var windows = enumerateWindows()
    if let filterStr = req.window {
        windows = windows.filter { windowMatches($0, filter: filterStr) }
    }

    var allElements: [ElementInfo] = []
    for win in windows {
        let root = findWindowElement(matching: win) ?? AXElementWrapper.systemWide()
        allElements.append(contentsOf: TreeWalker.walk(root: root, depth: 10, roleFilter: roleFilter, labelFilter: req.label))
    }

    let result = QueryResolver.resolve(in: allElements, role: roleFilter, label: req.label, id: req.id, index: req.index)

    switch result {
    case .notFound:
        return jsonError("No element found matching query")
    case .multiple(let matches):
        return jsonError("Multiple elements matched — refine your query or use --index N",
                        details: matches.map { describeElement($0) }.joined(separator: "\n"))
    case .found(let info):
        guard let liveElement = findLiveElement(matching: info) else {
            return jsonError("Element found in snapshot but could not locate live AX element")
        }
        var fontFamily: String? = nil
        var fontSize: Double? = nil
        var fontWeight: String? = nil
        var textColor: String? = nil
        var bounds: CGRect? = nil

        if let pos = liveElement.position(), let sz = liveElement.size() {
            bounds = CGRect(origin: pos, size: sz)
        }

        if let axWrapper = liveElement as? AXElementWrapper {
            var fontRef: CFTypeRef?
            if AXUIElementCopyAttributeValue(axWrapper.element, "AXFont" as CFString, &fontRef) == .success,
               let fontDict = fontRef as? [String: Any] {
                fontFamily = fontDict["AXFontName"] as? String
                fontSize = fontDict["AXFontSize"] as? Double
                if let traits = fontDict["AXFontTraits"] as? Int {
                    fontWeight = (traits & 2) != 0 ? "bold" : "regular"
                }
            }
        }

        let response = InspectResponse(
            element: info,
            fontFamily: fontFamily, fontSize: fontSize,
            fontWeight: fontWeight, textColor: textColor,
            bounds: bounds
        )
        return jsonOK(response)
    }
}

// MARK: - Actions (press, focus, show-menu)

private func handleAction(
    _ request: Request,
    name: String,
    perform: (any AccessibleElement) throws -> Void
) async throws -> Response {
    let req: ElementQuery = try await decode(request)
    return resolveAndAct(query: req, actionName: name, perform: perform)
}

private func handleSetValue(_ request: Request) async throws -> Response {
    let req: SetValueRequest = try await decode(request)
    let query = ElementQuery(role: req.role, label: req.label, window: req.window, id: req.id, index: req.index)
    return resolveAndAct(query: query, actionName: "set-value") { element in
        try ActionPerformer.setValue(element: element, value: req.value)
    }
}

private func resolveAndAct(
    query: ElementQuery,
    actionName: String,
    perform: (any AccessibleElement) throws -> Void
) -> Response {
    let roleFilter: UnifiedRole? = query.role.map { RoleMapper.map(role: $0) }

    var windows = enumerateWindows()
    if let filterStr = query.window {
        windows = windows.filter { windowMatches($0, filter: filterStr) }
    }
    if windows.isEmpty {
        return jsonError("No matching windows found")
    }

    var allElements: [ElementInfo] = []
    for win in windows {
        let root = findWindowElement(matching: win) ?? AXElementWrapper.systemWide()
        allElements.append(contentsOf: TreeWalker.walk(root: root, depth: 10, roleFilter: roleFilter, labelFilter: query.label))
    }

    let result = QueryResolver.resolve(in: allElements, role: roleFilter, label: query.label, id: query.id, index: query.index)

    switch result {
    case .notFound:
        return jsonError("No element found matching query")
    case .multiple(let matches):
        return jsonError("Multiple elements matched — refine your query or use index",
                        details: matches.map { describeElement($0) }.joined(separator: "\n"))
    case .found(let info):
        guard let liveElement = findLiveElement(matching: info) else {
            return jsonError("Element found in snapshot but could not locate live AX element")
        }
        do {
            try perform(liveElement)
            return jsonOK(ActionResponse(success: true, message: "\(actionName) performed successfully"))
        } catch {
            return jsonOK(ActionResponse(success: false, message: "\(actionName) failed: \(error)"))
        }
    }
}

// MARK: - Window Management

private func handleWindowFocus(_ request: Request) async throws -> Response {
    let req: WindowTarget = try await decode(request)
    guard let (windowElement, _) = resolveWindowElement(filter: req.window) else {
        return jsonError("No window matching '\(req.window)'")
    }
    do {
        // Activate the owning app by extracting PID from the AX element.
        if let axWrapper = windowElement as? AXElementWrapper {
            var pid: pid_t = 0
            AXUIElementGetPid(axWrapper.element, &pid)
            NSRunningApplication(processIdentifier: pid)?.activate(options: [])
        }
        try windowElement.setAttribute("AXMain", value: true)
        try windowElement.setAttribute("AXFocused", value: true)
        return jsonOK(ActionResponse(success: true, message: "Window focused successfully"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "window-focus failed: \(error)"))
    }
}

private func handleWindowResize(_ request: Request) async throws -> Response {
    let req: WindowResizeRequest = try await decode(request)
    guard let (windowElement, _) = resolveWindowElement(filter: req.window) else {
        return jsonError("No window matching '\(req.window)'")
    }
    do {
        var sz = CGSize(width: req.width, height: req.height)
        let value = AXValueCreate(.cgSize, &sz)!
        try windowElement.setAttribute("AXSize", value: value)
        return jsonOK(ActionResponse(success: true, message: "Window resized to \(req.width)×\(req.height)"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "window-resize failed: \(error)"))
    }
}

private func handleWindowMove(_ request: Request) async throws -> Response {
    let req: WindowMoveRequest = try await decode(request)
    guard let (windowElement, _) = resolveWindowElement(filter: req.window) else {
        return jsonError("No window matching '\(req.window)'")
    }
    do {
        var pt = CGPoint(x: req.x, y: req.y)
        let value = AXValueCreate(.cgPoint, &pt)!
        try windowElement.setAttribute("AXPosition", value: value)
        return jsonOK(ActionResponse(success: true, message: "Window moved to (\(req.x), \(req.y))"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "window-move failed: \(error)"))
    }
}

private func handleWindowClose(_ request: Request) async throws -> Response {
    let req: WindowTarget = try await decode(request)
    guard let (windowElement, _) = resolveWindowElement(filter: req.window) else {
        return jsonError("No window matching '\(req.window)'")
    }
    guard let axWrapper = windowElement as? AXElementWrapper else {
        return jsonOK(ActionResponse(success: false, message: "window-close: element is not an AXElementWrapper"))
    }
    var closeButtonRef: CFTypeRef?
    let axResult = AXUIElementCopyAttributeValue(axWrapper.element, "AXCloseButton" as CFString, &closeButtonRef)
    guard axResult == .success, let closeRef = closeButtonRef,
          CFGetTypeID(closeRef) == AXUIElementGetTypeID() else {
        return jsonOK(ActionResponse(success: false, message: "window-close: could not get AXCloseButton"))
    }
    let closeButton = AXElementWrapper(closeRef as! AXUIElement)
    do {
        try closeButton.performAction("AXPress")
        return jsonOK(ActionResponse(success: true, message: "Window closed successfully"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "window-close failed: \(error)"))
    }
}

private func handleWindowMinimize(_ request: Request) async throws -> Response {
    let req: WindowTarget = try await decode(request)
    guard let (windowElement, _) = resolveWindowElement(filter: req.window) else {
        return jsonError("No window matching '\(req.window)'")
    }
    do {
        try windowElement.setAttribute("AXMinimized", value: true)
        return jsonOK(ActionResponse(success: true, message: "Window minimized successfully"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "window-minimize failed: \(error)"))
    }
}

// MARK: - Wait

private func handleWait(_ request: Request) async throws -> Response {
    let req: WaitRequest = try await decode(request)
    let timeout = req.timeout ?? 10
    let deadline = Date().addingTimeInterval(Double(timeout))

    while Date() < deadline {
        var windows = enumerateWindows()
        if let filterStr = req.window {
            windows = windows.filter { windowMatches($0, filter: filterStr) }
        }
        if !windows.isEmpty {
            return jsonOK(ActionResponse(success: true, message: "Accessibility ready"))
        }
        try await Task.sleep(for: .milliseconds(500))
    }
    return jsonOK(ActionResponse(success: false, message: "Timed out waiting for accessibility"))
}

// MARK: - System: exec

private func handleExec(_ request: Request) async throws -> Response {
    struct ExecResult: Encodable {
        let exitCode: Int32
        let stdout: String
        let stderr: String
        let timedOut: Bool
    }
    let req: ExecRequest = try await decode(request)
    let timeout = req.timeout ?? 30
    let detach = req.detach ?? false

    if detach {
        try ProcessRunner.runShellDetached(command: req.command)
        return jsonOK(ExecResult(exitCode: 0, stdout: "", stderr: "", timedOut: false))
    }

    let result = try await ProcessRunner.runShell(
        command: req.command, timeoutSeconds: timeout
    )
    return jsonOK(ExecResult(
        exitCode: result.exitCode,
        stdout: result.stdout,
        stderr: result.stderr,
        timedOut: result.timedOut
    ))
}

// MARK: - System: upload/download

private func handleUpload(_ request: Request) async throws -> Response {
    let req: UploadRequest = try await decode(request)
    guard let data = Data(base64Encoded: req.content) else {
        return jsonError("Invalid base64 content")
    }
    do {
        let url = URL(fileURLWithPath: req.path)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url)
        return jsonOK(ActionResponse(success: true, message: "Uploaded to \(req.path)"))
    } catch {
        return jsonOK(ActionResponse(success: false, message: "Upload failed: \(error)"))
    }
}

private func handleDownload(_ request: Request) async throws -> Response {
    struct DownloadResponse: Encodable { let content: String }
    let req: DownloadRequest = try await decode(request)
    do {
        let data = try Data(contentsOf: URL(fileURLWithPath: req.path))
        return jsonOK(DownloadResponse(content: data.base64EncodedString()))
    } catch {
        return jsonError("Download failed: \(error.localizedDescription)")
    }
}

// MARK: - System: shutdown

private func handleShutdown() -> Response {
    Task {
        try? await Task.sleep(for: .milliseconds(100))
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", "tell app \"System Events\" to shut down"]
        try? process.run()
    }
    return jsonOK(ActionResponse(success: true, message: "Shutting down"))
}

// MARK: - Debug: AX Diagnostic

private func handleDebugAX() -> Response {
    struct AppEntry: Encodable {
        let pid: Int32
        let name: String?
        let bundleID: String?
        let activationPolicy: String
        let isActive: Bool
        let isHidden: Bool
        let ownsMenuBar: Bool
        let axChildrenCount: Int
        let axError: String?
    }
    struct CGWindowEntry: Encodable {
        let pid: Int32
        let name: String?
        let windowTitle: String?
        let layer: Int
        let bounds: [String: CGFloat]
        let axChildrenCount: Int
        let axError: String?
    }
    struct DebugResponse: Encodable {
        let nsWorkspaceApps: [AppEntry]
        let cgWindows: [CGWindowEntry]
        let cgUniquePIDs: Int
        let nsWorkspaceCount: Int
    }

    // Part 1: NSWorkspace.shared.runningApplications
    let runningApps = NSWorkspace.shared.runningApplications
    var appEntries: [AppEntry] = []
    for app in runningApps {
        guard app.activationPolicy != .prohibited else { continue }
        let pid = app.processIdentifier
        let axApp = AXUIElementCreateApplication(pid)
        var childrenRef: CFTypeRef?
        let axResult = AXUIElementCopyAttributeValue(axApp, kAXChildrenAttribute as CFString, &childrenRef)
        let childCount: Int
        let axErr: String?
        if axResult == .success, let arr = childrenRef as? [AXUIElement] {
            childCount = arr.count
            axErr = nil
        } else {
            childCount = 0
            axErr = "AXError(\(axResult.rawValue))"
        }
        let policyStr: String
        switch app.activationPolicy {
        case .regular: policyStr = "regular"
        case .accessory: policyStr = "accessory"
        case .prohibited: policyStr = "prohibited"
        @unknown default: policyStr = "unknown"
        }
        appEntries.append(AppEntry(
            pid: pid, name: app.localizedName,
            bundleID: app.bundleIdentifier,
            activationPolicy: policyStr,
            isActive: app.isActive, isHidden: app.isHidden,
            ownsMenuBar: app.ownsMenuBar,
            axChildrenCount: childCount, axError: axErr
        ))
    }

    // Part 2: CGWindowListCopyWindowInfo — bypasses NSWorkspace entirely
    var cgEntries: [CGWindowEntry] = []
    var seenPIDs = Set<Int32>()
    if let windowList = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] {
        for info in windowList {
            guard let pid = info[kCGWindowOwnerPID as String] as? Int32 else { continue }
            let name = info[kCGWindowOwnerName as String] as? String
            let title = info[kCGWindowName as String] as? String
            let layer = info[kCGWindowLayer as String] as? Int ?? -1
            let boundsDict = info[kCGWindowBounds as String] as? [String: CGFloat] ?? [:]

            var axChildCount = 0
            var axErr: String? = nil
            if !seenPIDs.contains(pid) {
                seenPIDs.insert(pid)
                let axApp = AXUIElementCreateApplication(pid)
                var childrenRef: CFTypeRef?
                let axResult = AXUIElementCopyAttributeValue(axApp, kAXChildrenAttribute as CFString, &childrenRef)
                if axResult == .success, let arr = childrenRef as? [AXUIElement] {
                    axChildCount = arr.count
                } else {
                    axErr = "AXError(\(axResult.rawValue))"
                }
            }

            cgEntries.append(CGWindowEntry(
                pid: pid, name: name, windowTitle: title,
                layer: layer, bounds: boundsDict,
                axChildrenCount: axChildCount, axError: axErr
            ))
        }
    }

    return jsonOK(DebugResponse(
        nsWorkspaceApps: appEntries,
        cgWindows: cgEntries,
        cgUniquePIDs: seenPIDs.count,
        nsWorkspaceCount: appEntries.count
    ))
}

// MARK: - App Discovery (CGWindowList-based)

/// Returns unique (pid, appName) pairs for all GUI applications.
/// Merges two sources to cover all cases:
///   - CGWindowListCopyWindowInfo: always current, catches SSH-launched apps
///   - NSWorkspace.shared.runningApplications: includes system services whose
///     windows don't appear in CGWindowList (e.g. Notification Center widgets)
/// NSWorkspace's list may be stale (no AppKit run loop), so CGWindowList is
/// the primary source. The union ensures nothing is missed.
private func onScreenApplications() -> [(pid: pid_t, name: String)] {
    var seen = Set<pid_t>()
    var result: [(pid: pid_t, name: String)] = []

    // Primary: CGWindowList — always current
    if let windowList = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID
    ) as? [[String: Any]] {
        for info in windowList {
            guard let pid = info[kCGWindowOwnerPID as String] as? pid_t,
                  let name = info[kCGWindowOwnerName as String] as? String,
                  !seen.contains(pid), pid > 0 else { continue }
            seen.insert(pid)
            result.append((pid: pid, name: name))
        }
    }

    // Secondary: NSWorkspace — may be stale but catches system services
    for app in NSWorkspace.shared.runningApplications {
        guard app.activationPolicy != .prohibited,
              !seen.contains(app.processIdentifier) else { continue }
        seen.insert(app.processIdentifier)
        result.append((pid: app.processIdentifier, name: app.localizedName ?? "Unknown"))
    }

    return result
}

/// Returns the PID of the currently focused application via the AX system-wide element.
private func focusedApplicationPid() -> pid_t? {
    let sysWide = AXUIElementCreateSystemWide()
    var ref: CFTypeRef?
    guard AXUIElementCopyAttributeValue(sysWide, kAXFocusedApplicationAttribute as CFString, &ref) == .success,
          let appRef = ref else { return nil }
    var pid: pid_t = 0
    guard AXUIElementGetPid(appRef as! AXUIElement, &pid) == .success else { return nil }
    return pid
}

// MARK: - Menu Bar

/// Returns the focused app's menu bar as a pseudo-window with its
/// AXMenuBarItem children as elements.
///
/// `depth` mirrors the request's snapshot depth. With a closed menu the
/// AXMenuBarItem has no children, so deeper walks are cheap. When a menu is
/// open via `--open-menu`, depth ≥ 3 exposes the AXMenu and its AXMenuItem
/// children — required for submenu drill-down to locate the next segment.
private func focusedAppMenuBar(
    roleFilter: UnifiedRole?,
    labelFilter: String?,
    mode: String,
    depth: Int
) -> WindowInfo? {
    guard let frontPid = focusedApplicationPid() else { return nil }
    let appWrapper = AXElementWrapper.application(pid: frontPid)
    let appName = onScreenApplications()
        .first(where: { $0.pid == frontPid })?.name ?? "Unknown"

    for child in appWrapper.children() {
        guard child.role() == "AXMenuBar" else { continue }

        let rawElements = TreeWalker.walk(
            root: child, depth: depth,
            roleFilter: roleFilter, labelFilter: labelFilter
        )

        let filteredElements: [ElementInfo]
        switch mode {
        case "interact": filteredElements = filterInteractive(rawElements)
        case "layout": filteredElements = filterLayout(rawElements)
        default: filteredElements = rawElements
        }

        let pos = child.position() ?? .zero
        let sz = child.size() ?? .zero

        return WindowInfo(
            title: "Menu Bar",
            windowType: "menuBar",
            size: CGSize(width: sz.width, height: sz.height),
            position: CGPoint(x: pos.x, y: pos.y),
            appName: appName,
            focused: false,
            elements: filteredElements
        )
    }
    return nil
}

// MARK: - Window Enumeration

func enumerateWindows() -> [WindowInfo] {
    var result: [WindowInfo] = []
    let apps = onScreenApplications()
    let frontPid = focusedApplicationPid()

    for (pid, appName) in apps {
        let appWrapper = AXElementWrapper.application(pid: pid)
        let isFrontApp = (pid == frontPid)

        for child in appWrapper.children() {
            guard child.role() == "AXWindow" else { continue }
            let title = child.label()
            let pos = child.position() ?? .zero
            let sz = child.size() ?? .zero
            let subrole = child.subrole()
            let windowType = windowTypeFromSubrole(subrole)
            var focused = false
            if isFrontApp, let axChild = child as? AXElementWrapper {
                var mainRef: CFTypeRef?
                if AXUIElementCopyAttributeValue(axChild.element, "AXMain" as CFString, &mainRef) == .success {
                    focused = (mainRef as? Bool) ?? false
                }
            }

            result.append(WindowInfo(
                title: title, windowType: windowType,
                size: CGSize(width: sz.width, height: sz.height),
                position: CGPoint(x: pos.x, y: pos.y),
                appName: appName, focused: focused,
                elements: nil
            ))
        }
    }
    return result
}

func windowMatches(_ window: WindowInfo, filter: String) -> Bool {
    (window.title?.localizedCaseInsensitiveContains(filter) ?? false) ||
    window.appName.localizedCaseInsensitiveContains(filter)
}

private func windowTypeFromSubrole(_ subrole: String?) -> String {
    switch subrole {
    case "AXStandardWindow": return "standard"
    case "AXDialog": return "dialog"
    case "AXFloatingWindow": return "floating"
    case "AXSheet": return "sheet"
    case "AXSystemDialog": return "systemDialog"
    default: return "standard"
    }
}

// MARK: - Window Element Lookup

func findWindowElement(matching win: WindowInfo) -> (any AccessibleElement)? {
    for (pid, appName) in onScreenApplications() {
        guard appName == win.appName else { continue }
        let appWrapper = AXElementWrapper.application(pid: pid)
        for child in appWrapper.children() {
            guard child.role() == "AXWindow" else { continue }
            let pos = child.position() ?? .zero
            let sz = child.size() ?? .zero
            if pos.x == win.position.x && pos.y == win.position.y &&
               sz.width == win.size.width && sz.height == win.size.height {
                return child
            }
        }
    }
    return nil
}

func resolveWindowElement(filter: String) -> (window: any AccessibleElement, info: WindowInfo)? {
    let windows = enumerateWindows()
    let matches = windows.filter { windowMatches($0, filter: filter) }
    guard let info = matches.first else { return nil }

    for (pid, appName) in onScreenApplications() {
        guard appName == info.appName else { continue }
        let appWrapper = AXElementWrapper.application(pid: pid)
        for child in appWrapper.children() {
            guard child.role() == "AXWindow" else { continue }
            let pos = child.position() ?? .zero
            let sz = child.size() ?? .zero
            if pos.x == info.position.x && pos.y == info.position.y &&
               sz.width == info.size.width && sz.height == info.size.height {
                return (window: child, info: info)
            }
        }
    }
    return nil
}

// MARK: - Live Element Lookup

func findLiveElement(matching info: ElementInfo) -> (any AccessibleElement)? {
    var windowRoots: [any AccessibleElement] = []
    for (pid, _) in onScreenApplications() {
        let appWrapper = AXElementWrapper.application(pid: pid)
        for winElement in appWrapper.children() where winElement.role() == "AXWindow" {
            windowRoots.append(winElement)
        }
    }
    return LiveElementMatcher.find(in: windowRoots, matching: info)
}

// MARK: - Mode Filters

func filterInteractive(_ elements: [ElementInfo]) -> [ElementInfo] {
    elements.compactMap { filterInteractiveElement($0) }
}

private func filterInteractiveElement(_ element: ElementInfo) -> ElementInfo? {
    let filteredChildren = element.children.map { filterInteractive($0) } ?? []
    let selfInteractive = isInteractive(element)
    if !selfInteractive && filteredChildren.isEmpty { return nil }
    return ElementInfo(
        role: element.role, label: element.label, value: element.value,
        description: element.description, id: element.id,
        enabled: element.enabled, focused: element.focused,
        showing: element.showing,
        position: element.position, size: element.size,
        childCount: element.childCount, actions: element.actions,
        platformRole: element.platformRole,
        children: filteredChildren.isEmpty ? nil : filteredChildren
    )
}

private func isInteractive(_ element: ElementInfo) -> Bool {
    if !element.actions.isEmpty { return true }
    if element.focused { return true }
    switch element.role {
    case .button, .checkbox, .radio, .textfield, .editableText, .slider,
         .comboBox, .switch, .link, .menuItem, .tab, .disclosureTriangle,
         .colorWell, .datePicker, .spinButton:
        return true
    default: return false
    }
}

func filterLayout(_ elements: [ElementInfo]) -> [ElementInfo] {
    elements.compactMap { filterLayoutElement($0) }
}

private func filterLayoutElement(_ element: ElementInfo) -> ElementInfo? {
    let filteredChildren = element.children.map { filterLayout($0) } ?? []
    let hasGeometry = element.position != nil && element.size != nil
    if !hasGeometry && filteredChildren.isEmpty { return nil }
    return ElementInfo(
        role: element.role, label: element.label, value: element.value,
        description: element.description, id: element.id,
        enabled: element.enabled, focused: element.focused,
        showing: element.showing,
        position: element.position, size: element.size,
        childCount: element.childCount, actions: element.actions,
        platformRole: element.platformRole,
        children: filteredChildren.isEmpty ? nil : filteredChildren
    )
}

// MARK: - Describe helpers

private func describeElement(_ info: ElementInfo) -> String {
    var parts: [String] = [info.role.rawValue]
    if let label = info.label { parts.append("label=\(label)") }
    if let id = info.id { parts.append("id=\(id)") }
    if let pos = info.position { parts.append("pos=(\(Int(pos.x)),\(Int(pos.y)))") }
    return parts.joined(separator: " ")
}
