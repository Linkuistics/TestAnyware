import Darwin

/// Chooses which processes the agent asks for the visible menu bar.
///
/// A background helper can hold the focus and have no menu bar. On macOS 27,
/// `AccessibilityUIServer` does this in some boots, while the screen still
/// shows the menu bar of Finder.
public enum MenuBarOwner {
    /// Returns the processes to try, in order, with no process twice.
    ///
    /// The focused process is first. Each regular app follows, in the order
    /// of `windowOrder`, which is front to back.
    public static func candidatePids(
        focused: pid_t?,
        windowOrder: [pid_t],
        isRegular: (pid_t) -> Bool
    ) -> [pid_t] {
        var seen = Set<pid_t>()
        var result: [pid_t] = []
        if let focused {
            seen.insert(focused)
            result.append(focused)
        }
        for pid in windowOrder where isRegular(pid) && seen.insert(pid).inserted {
            result.append(pid)
        }
        return result
    }
}
