import Foundation
import TestAnywareAgentProtocol

/// Pure helpers for the `agent snapshot --open-menu <label>` orchestration:
/// locate a menu-bar element by label across the snapshot's window list,
/// then derive a screen-absolute click point from its frame.
///
/// Kept side-effect-free so the search/centering logic is unit-testable
/// without a live VM. The actual VNC click and re-snapshot live in the
/// caller (`AgentSnapshotCmd`).
public enum MenuBarLocator {

    /// Depth-first search for the first element whose `label` matches
    /// `target` (case-insensitive). Returns `nil` if no match is found.
    public static func findElement(
        byLabel target: String,
        in windows: [WindowInfo]
    ) -> ElementInfo? {
        for window in windows {
            if let hit = search(target: target, in: window.elements) { return hit }
        }
        return nil
    }

    /// Center point of the element's frame, rounded to integer screen
    /// coordinates. Returns `nil` if either `position` or `size` is
    /// unavailable — without both, no click target can be derived.
    public static func centerPoint(of element: ElementInfo) -> (x: Int, y: Int)? {
        guard let p = element.position, let s = element.size else { return nil }
        return (Int((p.x + s.width / 2).rounded()), Int((p.y + s.height / 2).rounded()))
    }

    private static func search(target: String, in elements: [ElementInfo]?) -> ElementInfo? {
        guard let elements else { return nil }
        for elem in elements {
            if let label = elem.label, label.caseInsensitiveCompare(target) == .orderedSame {
                return elem
            }
            if let hit = search(target: target, in: elem.children) { return hit }
        }
        return nil
    }
}
