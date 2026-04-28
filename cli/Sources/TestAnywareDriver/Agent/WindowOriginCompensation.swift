import Foundation
import TestAnywareAgentProtocol

/// Translates a window-relative coordinate origin into a screen-absolute one
/// using the AX-reported window position, compensating for the macOS Tahoe
/// drop-shadow inset.
///
/// On Tahoe, `kAXPositionAttribute` for an `AXWindow` is offset below the
/// visible top of the window by the structural drop-shadow inset, so taking
/// the AX origin as the offset for `--window`-relative coordinates makes
/// every click land ~40 px below the intended target. Subtracting the inset
/// from the y origin restores intent. The default value is empirical (Tahoe
/// `AXStandardWindow`); set `TESTANYWARE_WINDOW_TOP_INSET` to an integer to
/// override it for tuning across other macOS versions or window subroles.
public enum WindowOriginCompensation {
    public static let defaultMacosTopInset: Int = 40

    public static func offset(
        for window: WindowInfo,
        platform: Platform?,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> (x: Int, y: Int) {
        let baseX = Int(window.position.x)
        let baseY = Int(window.position.y)
        guard platform == .macos else {
            return (baseX, baseY)
        }
        let topInset = environment["TESTANYWARE_WINDOW_TOP_INSET"]
            .flatMap(Int.init) ?? defaultMacosTopInset
        return (baseX, baseY - topInset)
    }
}
