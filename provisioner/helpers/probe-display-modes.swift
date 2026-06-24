// probe-display-modes.swift — spike for macos-guest-resolution (ADR-0014).
//
// Empirically resolves ADR-0014's load-bearing unknown #1: after
// `tart set --display 1920x1080px`, does a headless macOS Virtualization.framework
// guest advertise a *selectable* 1920×1080 CoreGraphics display mode at backing
// scale 1.0 (LoDPI) that `CGDisplaySetDisplayMode` will actually switch to?
//
// It dumps every advertised mode for the main display with pt + px + derived
// scale + refresh + IOKit flags (so we learn whether a true 1× mode exists, not
// only a Retina/2× one — see CONTEXT.md [[Framebuffer-pixel contract]]), then
// switches to the 1× 1920×1080 mode and re-reads the active mode to report
// whether it took.
//
// The spike found the bare `CGDisplaySetDisplayMode(_, _, nil)` that ADR-0014
// named is `.forAppOnly`-scoped — it reverts the instant the setting process
// exits — so this probe switches via a persistent configuration *transaction*
// (`CGBeginDisplayConfiguration` → `CGConfigureDisplayWithDisplayMode` →
// `CGCompleteDisplayConfiguration(.forSession)`); see ADR-0014's Verification.
//
// Host-compiled with `swiftc` and run inside the guest via the agent's
// /upload + /exec surface (NOT golden-baked). Its mode-enumeration and
// transaction calls seed the real switch helper that build leaf k3 will
// parameterize by target px.
//
// Output is a single JSON object on stdout so the result is both human-readable
// and machine-capturable for the ADR Verification section.

import CoreGraphics
import Foundation

// Target the 1× framebuffer-pixel contract: 1920×1080 px AND 1920×1080 pt.
let targetW = 1920
let targetH = 1080

let displayID = CGMainDisplayID()

// IOKit display-mode flag bits we care to decode (from IOGraphicsTypes.h).
// We print the raw hex regardless; these just annotate the common ones.
let flagNames: [(UInt32, String)] = [
    (0x0000_0001, "valid"),
    (0x0000_0002, "safe"),
    (0x0000_0004, "default"),
    (0x0200_0000, "native"),
    (0x0000_0080, "stretched"),
    (0x0000_0400, "simulscan"),
    (0x0000_0008, "alwaysShow"),
]

func decodeFlags(_ flags: UInt32) -> [String] {
    flagNames.compactMap { (bit, name) in (flags & bit) != 0 ? name : nil }
}

// Describe one CGDisplayMode as a plain dictionary for JSON emission.
func describe(_ mode: CGDisplayMode) -> [String: Any] {
    let ptW = mode.width
    let ptH = mode.height
    let pxW = mode.pixelWidth
    let pxH = mode.pixelHeight
    let scale = ptW > 0 ? Double(pxW) / Double(ptW) : 0
    let flags = mode.ioFlags
    return [
        "ptWidth": ptW,
        "ptHeight": ptH,
        "pxWidth": pxW,
        "pxHeight": pxH,
        "scale": scale,
        "refreshRate": mode.refreshRate,
        "ioFlags": String(format: "0x%08x", flags),
        "ioFlagsDecoded": decodeFlags(flags),
        "usableForDesktopGUI": mode.isUsableForDesktopGUI(),
        "modeID": mode.ioDisplayModeID,
    ]
}

// Pull modes both with and without the option that surfaces duplicate
// low-resolution (1×) modes — a 1× 1920×1080 can be hidden from the default
// list, and missing it would be a false REFUTE.
func copyModes(showDuplicates: Bool) -> [CGDisplayMode] {
    let options: CFDictionary?
    if showDuplicates {
        options = [kCGDisplayShowDuplicateLowResolutionModes as String: true] as CFDictionary
    } else {
        options = nil
    }
    return (CGDisplayCopyAllDisplayModes(displayID, options) as? [CGDisplayMode]) ?? []
}

let defaultModes = copyModes(showDuplicates: false)
let allModes = copyModes(showDuplicates: true)

// Match the 1× target: framebuffer pixels AND points both 1920×1080.
func isTarget(_ m: CGDisplayMode) -> Bool {
    m.pixelWidth == targetW && m.pixelHeight == targetH && m.width == targetW && m.height == targetH
}

let activeBefore = CGDisplayCopyDisplayMode(displayID)

// Prefer the expanded list for the switch (it's a superset).
let targetMode = allModes.first(where: isTarget)

// Switch via a persistent display-configuration *transaction*, NOT the
// one-shot `CGDisplaySetDisplayMode(_, _, nil)` that ADR-0014 named: the spike
// found that one-shot is `.forAppOnly`-scoped — the mode reverts to the golden's
// 1024×768 the instant the setting process exits, so the framebuffer never stays
// switched. `.forSession` survives the helper exiting and lasts the login session
// (the right scope for a per-instance runtime switch — not baked into prefs).
var switchAttempted = false
var switchCGError: Int32 = -1
var switchScope = "none"
if let target = targetMode {
    switchAttempted = true
    switchScope = "forSession (transaction)"
    var configRef: CGDisplayConfigRef?
    let beginErr = CGBeginDisplayConfiguration(&configRef)
    if beginErr == .success, let cfg = configRef {
        let setErr = CGConfigureDisplayWithDisplayMode(cfg, displayID, target, nil)
        if setErr == .success {
            switchCGError = CGCompleteDisplayConfiguration(cfg, .forSession).rawValue
        } else {
            CGCancelDisplayConfiguration(cfg)
            switchCGError = setErr.rawValue
        }
    } else {
        switchCGError = beginErr.rawValue
    }
}

let activeAfter = CGDisplayCopyDisplayMode(displayID)

// A 1× 1920×1080 mode exists (in either list) and switching to it left the
// active mode reporting 1920×1080 px ⇒ CONFIRMED.
let targetExists = targetMode != nil
let switchTook =
    switchAttempted && switchCGError == 0
    && activeAfter?.pixelWidth == targetW && activeAfter?.pixelHeight == targetH
let verdict = (targetExists && switchTook) ? "CONFIRMED" : "REFUTED"

var report: [String: Any] = [
    "displayID": displayID,
    "target": ["pxWidth": targetW, "pxHeight": targetH, "scale": 1.0],
    "defaultModeCount": defaultModes.count,
    "allModeCount": allModes.count,
    "defaultModes": defaultModes.map(describe),
    "allModes": allModes.map(describe),
    "targetModeExists": targetExists,
    "switchAttempted": switchAttempted,
    "switchScope": switchScope,
    "switchCGError": Int(switchCGError),
    "switchTook": switchTook,
    "verdict": verdict,
]
report["activeBefore"] = activeBefore.map(describe) ?? NSNull()
report["activeAfter"] = activeAfter.map(describe) ?? NSNull()

let data = try JSONSerialization.data(
    withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
