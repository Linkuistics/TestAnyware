// The fixture window of the input live checks (scripts/input-live.sh).
//
// One window, "Input Fixture", with a text field above a scroll view that is taller than its
// window. The text field is labelled "Fixture field" for the accessibility tree. The scroll view
// holds 40 rows, each a label "row-N", and it keeps a legacy scroll bar so that the thumb is
// always on screen. The script reads the value of the field and the position of "row-20"
// through `agent snapshot`.
//
// The script compiles this file on the host with swiftc and uploads the binary, so the golden
// image needs no change.

import AppKit

let rowHeight: CGFloat = 24
let rowCount = 40

final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(
            contentRect: NSRect(x: 200, y: 200, width: 560, height: 420),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "Input Fixture"
        let content = window.contentView!

        let field = NSTextField(frame: NSRect(x: 20, y: 370, width: 520, height: 28))
        field.placeholderString = "type here"
        field.setAccessibilityLabel("Fixture field")
        content.addSubview(field)

        let scroll = NSScrollView(frame: NSRect(x: 20, y: 20, width: 520, height: 330))
        scroll.hasVerticalScroller = true
        scroll.scrollerStyle = .legacy
        scroll.borderType = .lineBorder
        let documentHeight = rowHeight * CGFloat(rowCount)
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 500, height: documentHeight))
        for index in 0..<rowCount {
            // The first row sits at the top of the document, which is flipped here by hand.
            let y = documentHeight - rowHeight * CGFloat(index + 1)
            let label = NSTextField(labelWithString: "row-\(index + 1)")
            label.frame = NSRect(x: 8, y: y, width: 200, height: rowHeight)
            label.setAccessibilityLabel("row-\(index + 1)")
            document.addSubview(label)
        }
        scroll.documentView = document
        scroll.contentView.scroll(to: NSPoint(x: 0, y: documentHeight - scroll.contentSize.height))
        scroll.reflectScrolledClipView(scroll.contentView)
        content.addSubview(scroll)

        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        window.makeFirstResponder(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
