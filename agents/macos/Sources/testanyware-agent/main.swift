import ApplicationServices
import CoreGraphics
import Foundation
import Hummingbird

let port = CommandLine.arguments.dropFirst().first.flatMap(Int.init) ?? 8648

// Dismiss the "App Background Activity" notification that macOS shows for our
// LaunchAgent on every boot. Retries for 30 seconds to handle timing.
Task.detached {
    for _ in 0..<6 {
        try? await Task.sleep(for: .seconds(5))
        if dismissBackgroundActivityNotification() { break }
    }
}

let router = buildAgentRouter()
let app = Application(
    router: router,
    configuration: .init(address: .hostname("0.0.0.0", port: port))
)

try await app.runService()

/// Finds the "App Background Activity" notification in Notification Center and
/// performs its "Close" action. Returns true if found and dismissed.
@discardableResult
func dismissBackgroundActivityNotification() -> Bool {
    // Find Notification Center's PID via CGWindowList (not NSWorkspace, which
    // has a stale cache without an AppKit run loop).
    guard let windowList = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID
    ) as? [[String: Any]] else { return false }

    var ncPid: pid_t? = nil
    for info in windowList {
        if let name = info[kCGWindowOwnerName as String] as? String,
           name == "Notification Center",
           let pid = info[kCGWindowOwnerPID as String] as? pid_t {
            ncPid = pid
            break
        }
    }
    guard let pid = ncPid else { return false }

    let ncApp = AXUIElementCreateApplication(pid)
    var windowsRef: CFTypeRef?
    guard AXUIElementCopyAttributeValue(ncApp, kAXChildrenAttribute as CFString, &windowsRef) == .success,
          let windows = windowsRef as? [AXUIElement] else { return false }

    for window in windows {
        if let notif = findNotificationGroup(in: window) {
            return closeNotification(notif)
        }
    }
    return false
}

private func findNotificationGroup(in element: AXUIElement) -> AXUIElement? {
    var childrenRef: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &childrenRef) == .success,
          let children = childrenRef as? [AXUIElement] else { return nil }

    for child in children {
        var labelRef: CFTypeRef?
        AXUIElementCopyAttributeValue(child, "AXLabel" as CFString, &labelRef)
        if let label = labelRef as? String, label.contains("Background Activity") {
            return child
        }
        if let found = findNotificationGroup(in: child) {
            return found
        }
    }
    return nil
}

private func closeNotification(_ element: AXUIElement) -> Bool {
    var actionNames: CFArray?
    guard AXUIElementCopyActionNames(element, &actionNames) == .success,
          let actions = actionNames as? [String] else { return false }
    for action in actions where action.contains("Close") {
        return AXUIElementPerformAction(element, action as CFString) == .success
    }
    return false
}
