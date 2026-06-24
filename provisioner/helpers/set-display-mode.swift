// set-display-mode.swift — the macOS guest runtime display-mode switch
// (ADR-0014). Forces the guest's CoreGraphics main display to a 1×
// (LoDPI) mode of the requested framebuffer-pixel size, so the host-side
// RFB framebuffer VF sizes to that mode comes onto the vision distribution
// (see CONTEXT.md [[Framebuffer-pixel contract]]).
//
// Host-compiled with `swiftc` and run inside the guest via the agent's
// /upload + /exec surface (NOT golden-baked) at `vm start`, after agent
// readiness. It is the production trim of the spike probe
// `probe-display-modes.swift`, which CONFIRMED ADR-0014's mechanism; the
// two material findings it bakes in:
//
//   * the switch must be a persistent configuration *transaction*
//     (`CGBeginDisplayConfiguration` → `CGConfigureDisplayWithDisplayMode`
//     → `CGCompleteDisplayConfiguration(.forSession)`), NOT the bare
//     `CGDisplaySetDisplayMode(_, _, nil)` the ADR first named — that one
//     is `.forAppOnly`-scoped and reverts the instant this process exits
//     (ADR-0014 Verification finding 3);
//   * `.forSession` is the right scope — per-instance runtime, not written
//     to prefs / not baked.
//
// Usage:  set-display-mode <width-px> <height-px>
// Exit:   0  switched (and the active mode now reports the target px)
//         1  bad arguments
//         2  no matching 1× mode advertised for that px size
//         3  the configuration transaction failed (CGError on stderr)
// A clear one-line reason goes to stderr on every non-zero exit so the
// host caller can warn.

import CoreGraphics
import Foundation

func fail(_ code: Int32, _ message: String) -> Never {
    FileHandle.standardError.write(Data("set-display-mode: \(message)\n".utf8))
    exit(code)
}

let args = CommandLine.arguments
guard args.count == 3, let targetW = Int(args[1]), let targetH = Int(args[2]),
    targetW > 0, targetH > 0
else {
    fail(1, "usage: set-display-mode <width-px> <height-px>")
}

let displayID = CGMainDisplayID()

// Pull modes with the option that surfaces duplicate low-resolution (1×)
// modes — a 1× target can be hidden from the default list, and missing it
// would be a false "no matching mode". The spike found this list is a
// superset of the default one.
let options = [kCGDisplayShowDuplicateLowResolutionModes as String: true] as CFDictionary
let modes = (CGDisplayCopyAllDisplayModes(displayID, options) as? [CGDisplayMode]) ?? []

// The 1× framebuffer-pixel contract: pixels AND points both == target, so
// the framebuffer lands on the vision distribution (pixelWidth) *and* keeps
// Linux/Windows layout parity (width pt). A Retina/2× mode (pixelWidth ==
// 2·width) is deliberately rejected — out of scope for this grove.
func isTarget(_ m: CGDisplayMode) -> Bool {
    m.pixelWidth == targetW && m.pixelHeight == targetH
        && m.width == targetW && m.height == targetH
}

guard let target = modes.first(where: isTarget) else {
    fail(2, "no 1x \(targetW)x\(targetH) mode advertised for the main display")
}

// Switch via a persistent configuration transaction (.forSession) so the
// mode outlives this helper exiting — the load-bearing correction from the
// spike (finding 3).
var configRef: CGDisplayConfigRef?
let beginErr = CGBeginDisplayConfiguration(&configRef)
guard beginErr == .success, let cfg = configRef else {
    fail(3, "CGBeginDisplayConfiguration failed (CGError \(beginErr.rawValue))")
}
let setErr = CGConfigureDisplayWithDisplayMode(cfg, displayID, target, nil)
guard setErr == .success else {
    CGCancelDisplayConfiguration(cfg)
    fail(3, "CGConfigureDisplayWithDisplayMode failed (CGError \(setErr.rawValue))")
}
let completeErr = CGCompleteDisplayConfiguration(cfg, .forSession)
guard completeErr == .success else {
    fail(3, "CGCompleteDisplayConfiguration failed (CGError \(completeErr.rawValue))")
}

// Confirm the active mode now reports the target px before reporting success
// — VF resizes the host-side framebuffer to follow the guest's chosen mode,
// and the spike verified `screen size` reads the target after this.
let active = CGDisplayCopyDisplayMode(displayID)
guard active?.pixelWidth == targetW, active?.pixelHeight == targetH else {
    fail(3, "switch issued but active mode is "
        + "\(active?.pixelWidth ?? -1)x\(active?.pixelHeight ?? -1), not \(targetW)x\(targetH)")
}

print("set-display-mode: switched main display to \(targetW)x\(targetH) px (modeID \(target.ioDisplayModeID))")
