// set-display-mode.swift — the macOS guest runtime display-mode switch
// (ADR-0014 / ADR-0016). Forces the guest's CoreGraphics main display to a
// mode of the requested *logical* (point) size at a given backing scale, so
// the host-side RFB framebuffer VF sizes to that mode is what the vision
// pipeline / viewer / input consume (see CONTEXT.md
// [[Framebuffer-pixel contract]] and [[HiDPI logical framebuffer]]).
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
// Usage:  set-display-mode <logical-width> <logical-height> [scale]
//   scale defaults to 1 (LoDPI, the ADR-0013/0014 default path): the mode
//   has px == pt == logical, landing on the vision distribution AND keeping
//   Linux/Windows layout parity. scale 2 is the HiDPI/Retina opt-in
//   (ADR-0016): the mode has the same *points* but 2× the *pixels*
//   (pixelWidth == 2·width), per k4 finding 3. At scale 1 the predicate
//   reduces to px == pt == logical — byte-identical to the original 1×
//   selector.
// Exit:   0  switched (and the active mode now reports the target pixels)
//         1  bad arguments
//         2  no matching mode advertised for that logical size + scale
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
guard args.count == 3 || args.count == 4,
    let logicalW = Int(args[1]), let logicalH = Int(args[2]),
    logicalW > 0, logicalH > 0
else {
    fail(1, "usage: set-display-mode <logical-width> <logical-height> [scale]")
}
// scale defaults to 1 so the 2-arg invocation is the unchanged LoDPI path.
let scale = args.count == 4 ? Int(args[3]) : 1
guard let scale, scale >= 1 else {
    fail(1, "scale must be a positive integer")
}
let pxW = logicalW * scale
let pxH = logicalH * scale

let displayID = CGMainDisplayID()

// Pull modes with the option that surfaces duplicate low-resolution (1×)
// modes — a 1× target can be hidden from the default list, and missing it
// would be a false "no matching mode". The spike found this list is a
// superset of the default one.
let options = [kCGDisplayShowDuplicateLowResolutionModes as String: true] as CFDictionary
let modes = (CGDisplayCopyAllDisplayModes(displayID, options) as? [CGDisplayMode]) ?? []

// Match by *both* axes: the point size (layout) AND the pixel size (scale).
// At scale 1 this is px == pt == logical (the LoDPI contract). At scale 2 it
// is the Retina mode (pt == logical, px == 2·logical) — and matching px too
// is load-bearing: at 2× the guest advertises BOTH a 1× and a 2× mode of the
// same logical size, so a pt-only match could pick the wrong one (k4
// finding 4).
func isTarget(_ m: CGDisplayMode) -> Bool {
    m.width == logicalW && m.height == logicalH
        && m.pixelWidth == pxW && m.pixelHeight == pxH
}

guard let target = modes.first(where: isTarget) else {
    fail(2, "no \(scale)x \(logicalW)x\(logicalH) (\(pxW)x\(pxH) px) mode advertised for the main display")
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

// Confirm the active mode now reports the target pixels before reporting
// success — VF resizes the host-side framebuffer to follow the guest's chosen
// mode, and the spike verified `screen size` reads the target after this.
let active = CGDisplayCopyDisplayMode(displayID)
guard active?.pixelWidth == pxW, active?.pixelHeight == pxH else {
    fail(3, "switch issued but active mode is "
        + "\(active?.pixelWidth ?? -1)x\(active?.pixelHeight ?? -1) px, not \(pxW)x\(pxH)")
}

print("set-display-mode: switched main display to \(logicalW)x\(logicalH) pt @ \(scale)x "
    + "(\(pxW)x\(pxH) px, modeID \(target.ioDisplayModeID))")
