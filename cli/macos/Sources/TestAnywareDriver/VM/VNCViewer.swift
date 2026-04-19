import Foundation

/// AppleScript-driven macOS Screen Sharing viewer lifecycle.
///
/// The `--viewer` flag on `testanyware vm start` opens a live VNC session
/// through macOS Screen Sharing, then closes it on `vm stop`. AppleScript
/// is the only durable path for capturing and closing a specific window
/// — `open vnc://` doesn't return a window handle, and Screen Sharing
/// has no CLI interface. All AX operations require TCC Automation grant
/// for the parent `testanyware` binary.
///
/// No unit tests: AppleScript shell-outs are exercised by integration
/// tests (Task 21's `testViewerWindowCloses`). Pure unit-testing of
/// `osascript` invocation provides no meaningful coverage.
public enum VNCViewer {

    /// Probe TCC Automation permission with a no-op AppleScript.
    ///
    /// Returns `true` if `osascript` could drive System Events; `false`
    /// otherwise. Caller is responsible for surfacing the failure to the
    /// user (e.g. with instructions to grant Automation permission in
    /// System Settings → Privacy & Security → Automation).
    public static func tccPreflight() -> Bool {
        let result = runOsaScript(source: "tell application \"System Events\" to return name")
        return result.exitCode == 0
    }

    /// Open a `vnc://` URL in Screen Sharing, auto-type the password into
    /// the auth dialog, then capture the resulting window's AXIdentifier.
    ///
    /// Returns the AXIdentifier (to be stored in VMMeta for later close),
    /// or `nil` if any step failed. Silent on failure — this is best-
    /// effort convenience, not a critical path.
    public static func openAndCapture(vncURL: String, password: String?) -> String? {
        let openProc = Process()
        openProc.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        openProc.arguments = [vncURL]
        try? openProc.run()
        openProc.waitUntilExit()

        Thread.sleep(forTimeInterval: 2)
        if let pw = password, !pw.isEmpty {
            let typeScript = """
            tell application "System Events"
                keystroke "\(escapeAppleScript(pw))"
                keystroke return
            end tell
            """
            _ = runOsaScript(source: typeScript)
        }

        Thread.sleep(forTimeInterval: 1)
        let captureScript = """
        tell application "System Events"
            tell process "Screen Sharing"
                return value of attribute "AXIdentifier" of window 1
            end tell
        end tell
        """
        let result = runOsaScript(source: captureScript)
        guard result.exitCode == 0 else { return nil }
        let id = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        return id.isEmpty ? nil : id
    }

    /// Close the Screen Sharing window identified by `identifier` (pass 1),
    /// then sweep any remaining Screen Sharing windows whose AXDocument
    /// contains `<vncPort>.vncloc` (pass 2).
    ///
    /// The two-pass design exists because Screen Sharing may recycle an
    /// AXIdentifier after reconnect, or present its window with no
    /// identifier at all. The `.vncloc` URL pattern is a reliable
    /// secondary anchor. Leaves the Screen Sharing app itself running.
    public static func closeWindows(identifier: String?, vncPort: Int?) {
        guard screenSharingIsRunning() else { return }

        if let id = identifier, !id.isEmpty {
            let script = """
            tell application "System Events"
                tell process "Screen Sharing"
                    repeat with w in every window
                        if value of attribute "AXIdentifier" of w is "\(escapeAppleScript(id))" then
                            click (first button of w whose subrole is "AXCloseButton")
                            exit repeat
                        end if
                    end repeat
                end tell
            end tell
            """
            _ = runOsaScript(source: script)
            Thread.sleep(forTimeInterval: 0.3)
        }

        if let port = vncPort, screenSharingIsRunning() {
            let portMatch = "\(port).vncloc"
            let script = """
            tell application "System Events"
                tell process "Screen Sharing"
                    set targets to {}
                    repeat with w in every window
                        set doc to ""
                        try
                            set doc to (value of attribute "AXDocument" of w) as text
                        end try
                        if doc contains "\(escapeAppleScript(portMatch))" then
                            set end of targets to w
                        end if
                    end repeat
                    repeat with w in targets
                        try
                            click (first button of w whose subrole is "AXCloseButton")
                        end try
                    end repeat
                end tell
            end tell
            """
            _ = runOsaScript(source: script)
            Thread.sleep(forTimeInterval: 0.3)
        }
    }

    // MARK: - helpers

    private struct OsaResult {
        let exitCode: Int32
        let stdout: String
        let stderr: String
    }

    private static func runOsaScript(source: String) -> OsaResult {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        proc.arguments = ["-e", source]
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        do {
            try proc.run()
        } catch {
            return OsaResult(exitCode: -1, stdout: "", stderr: "\(error)")
        }
        proc.waitUntilExit()
        let out = String(
            data: outPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        ) ?? ""
        let err = String(
            data: errPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        ) ?? ""
        return OsaResult(exitCode: proc.terminationStatus, stdout: out, stderr: err)
    }

    private static func screenSharingIsRunning() -> Bool {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        proc.arguments = ["-q", "Screen Sharing"]
        do { try proc.run() } catch { return false }
        proc.waitUntilExit()
        return proc.terminationStatus == 0
    }

    private static func escapeAppleScript(_ s: String) -> String {
        s.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}
