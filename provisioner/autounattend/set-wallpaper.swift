import AppKit

guard CommandLine.arguments.count > 1 else {
    print("Usage: set-wallpaper <path>")
    exit(1)
}
let path = CommandLine.arguments[1]
let url = URL(fileURLWithPath: path)
for screen in NSScreen.screens {
    try! NSWorkspace.shared.setDesktopImageURL(url, for: screen)
}
print("Wallpaper set to \(path)")
