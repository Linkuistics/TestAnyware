// probe-hidpi-identity.swift — hidpi-vision spike (spike-hidpi-feasibility-k2,
// ADR-0015). Sibling of `probe-display-modes.swift` (the ADR-0014 spike probe).
//
// Reports the VF virtual main display's CoreGraphics identity (the keys the
// /Library/Displays override-plist mechanism is addressed by) AND enumerates
// every advertised mode with pt/px/scale/flags so we can (a) reproduce
// ADR-0014's "only 1× (LoDPI) modes" baseline on a given guest, and (b) detect
// whether any 2× (Retina) mode — pxWidth == 2·ptWidth — is present at all.
//
// Used by the spike to establish: the VF virtual display is identity-less
// (CGDisplayVendorNumber/ModelNumber == 0), advertises only scale-1.0 modes,
// and — combined with `tart set --display …pt` runs on hosts of differing
// NSScreen backing scale — that the backing scale of the advertised modes is
// governed *host-side* by the VF display configuration, not reachable from
// inside the guest. See ADR-0015's Verification.
//
// Host-compiled with `swiftc` and run inside the guest via the agent's
// /upload + /exec surface (NOT golden-baked). Single JSON object on stdout,
// machine-capturable for the ADR Verification.

import CoreGraphics
import Foundation

let displayID = CGMainDisplayID()

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

func describe(_ mode: CGDisplayMode) -> [String: Any] {
    let ptW = mode.width, ptH = mode.height
    let pxW = mode.pixelWidth, pxH = mode.pixelHeight
    let scale = ptW > 0 ? Double(pxW) / Double(ptW) : 0
    return [
        "ptWidth": ptW, "ptHeight": ptH,
        "pxWidth": pxW, "pxHeight": pxH,
        "scale": scale,
        "refreshRate": mode.refreshRate,
        "ioFlags": String(format: "0x%08x", mode.ioFlags),
        "ioFlagsDecoded": decodeFlags(mode.ioFlags),
        "usableForDesktopGUI": mode.isUsableForDesktopGUI(),
        "modeID": mode.ioDisplayModeID,
    ]
}

func copyModes(showDuplicates: Bool) -> [CGDisplayMode] {
    let options: CFDictionary? = showDuplicates
        ? ([kCGDisplayShowDuplicateLowResolutionModes as String: true] as CFDictionary)
        : nil
    return (CGDisplayCopyAllDisplayModes(displayID, options) as? [CGDisplayMode]) ?? []
}

let allModes = copyModes(showDuplicates: true)
let defaultModes = copyModes(showDuplicates: false)

// A Retina/2× mode: framebuffer pixels are exactly double the logical points.
func isRetina(_ m: CGDisplayMode) -> Bool { m.pixelWidth == 2 * m.width && m.width > 0 }
let retinaModes = allModes.filter(isRetina)

var report: [String: Any] = [
    "displayID": displayID,
    "vendorNumber": CGDisplayVendorNumber(displayID),
    "modelNumber": CGDisplayModelNumber(displayID),
    "serialNumber": CGDisplaySerialNumber(displayID),
    "unitNumber": CGDisplayUnitNumber(displayID),
    "isBuiltin": CGDisplayIsBuiltin(displayID),
    "vendorHex": String(format: "0x%x", CGDisplayVendorNumber(displayID)),
    "modelHex": String(format: "0x%x", CGDisplayModelNumber(displayID)),
    "defaultModeCount": defaultModes.count,
    "allModeCount": allModes.count,
    "retinaModeCount": retinaModes.count,
    "allModes": allModes.map(describe),
    "retinaModes": retinaModes.map(describe),
]
report["activeMode"] = CGDisplayCopyDisplayMode(displayID).map(describe) ?? NSNull()

let data = try JSONSerialization.data(
    withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
