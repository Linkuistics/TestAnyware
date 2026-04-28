import Testing
import Foundation
@testable import TestAnywareDriver

private func tartAvailable() -> Bool {
    guard ProcessInfo.processInfo.environment["TESTANYWARE_SKIP_INTEGRATION"] != "1" else {
        return false
    }
    return TartRunner.which("tart") != nil
}

// Gates the Windows/QEMU round-trip. Requires qemu + the Windows golden
// qcow2 on disk; on a pure-tart host this suite disables itself at test
// discovery time (matches the memory note on .enabled(if:) semantics).
private func qemuWindowsAvailable() -> Bool {
    guard ProcessInfo.processInfo.environment["TESTANYWARE_SKIP_INTEGRATION"] != "1" else {
        return false
    }
    guard TartRunner.which("qemu-system-aarch64") != nil else { return false }
    let paths = VMPaths()
    let golden = "\(paths.goldenDir)/testanyware-golden-windows-11.qcow2"
    return FileManager.default.fileExists(atPath: golden)
}

// Gates the viewer-close test. `tccPreflight()` is the only reliable probe
// for the Automation grant on System Events — without it the AppleScript
// close path is a no-op. `TESTANYWARE_SKIP_VIEWER_TEST=1` lets CI opt out
// even on a workstation with the grant in place.
private func viewerTestEnabled() -> Bool {
    guard tartAvailable() else { return false }
    guard ProcessInfo.processInfo.environment["TESTANYWARE_SKIP_VIEWER_TEST"] != "1" else {
        return false
    }
    return VNCViewer.tccPreflight()
}

// Test-only AppleScript probe. Kept out of `VNCViewer` itself because it
// is a test assertion, not production behaviour — matching the coding-style
// guidance to not grow production API for test needs.
private func screenSharingHasWindow(withIdentifier identifier: String) -> Bool {
    let escaped = identifier
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
    let script = """
    tell application "System Events"
        if not (exists process "Screen Sharing") then return false
        tell process "Screen Sharing"
            repeat with w in every window
                try
                    if value of attribute "AXIdentifier" of w is "\(escaped)" then return true
                end try
            end repeat
        end tell
    end tell
    return false
    """
    let proc = Process()
    proc.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    proc.arguments = ["-e", script]
    let outPipe = Pipe()
    proc.standardOutput = outPipe
    proc.standardError = Pipe()
    do {
        try proc.run()
    } catch {
        return false
    }
    proc.waitUntilExit()
    let data = outPipe.fileHandleForReading.readDataToEndOfFile()
    let out = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return out == "true"
}

@Suite("VM Lifecycle Integration",
       .enabled(if: tartAvailable()),
       .serialized)
struct VMLifecycleTests {

    @Test(.timeLimit(.minutes(5)))
    func startStopRoundTripMacOS() async throws {
        let id = "testanyware-test-\(UUID().uuidString.prefix(8))".lowercased()
        let options = VMStartOptions(
            platform: .macos,
            base: nil,
            id: id,
            display: nil,
            openViewer: false
        )

        let cleanup = { try? VMLifecycle.stop(id: id) }

        let result: VMStartResult
        do {
            result = try await VMLifecycle.start(options: options)
        } catch {
            cleanup()
            throw error
        }

        #expect(result.id == id)

        let paths = VMPaths()
        #expect(FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))

        let spec = try ConnectionSpec.load(from: paths.specPath(forID: id))
        #expect(spec.platform?.rawValue == "macos")
        #expect(spec.vnc.port > 0)

        try VMLifecycle.stop(id: id)

        #expect(!FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(!FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))

        let entries = (try? TartRunner.runList()) ?? []
        #expect(!entries.contains { $0.name == id })
    }

    @Test(.timeLimit(.minutes(5)))
    func startStopRoundTripLinux() async throws {
        let id = "testanyware-test-\(UUID().uuidString.prefix(8))".lowercased()
        let options = VMStartOptions(
            platform: .linux,
            base: nil,
            id: id,
            display: nil,
            openViewer: false
        )

        let cleanup = { try? VMLifecycle.stop(id: id) }

        let result: VMStartResult
        do {
            result = try await VMLifecycle.start(options: options)
        } catch {
            cleanup()
            throw error
        }

        #expect(result.id == id)

        let paths = VMPaths()
        #expect(FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))

        let spec = try ConnectionSpec.load(from: paths.specPath(forID: id))
        #expect(spec.platform?.rawValue == "linux")
        #expect(spec.vnc.port > 0)

        try VMLifecycle.stop(id: id)

        #expect(!FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(!FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))

        let entries = (try? TartRunner.runList()) ?? []
        #expect(!entries.contains { $0.name == id })
    }

    @Test
    func stopMissingIdThrowsNotFound() throws {
        let missing = "testanyware-does-not-exist-\(UUID().uuidString.prefix(8))".lowercased()
        #expect(throws: VMLifecycleError.self) {
            try VMLifecycle.stop(id: missing)
        }
    }

    // P3 Task 21: verify `--viewer` opens a Screen Sharing window on start
    // and `vm stop` closes it. Gated on TCC Automation grant — the
    // AppleScript round-trip is the only way to capture/close a specific
    // Screen Sharing window, and it silently no-ops without the grant.
    @Test(.enabled(if: viewerTestEnabled()), .timeLimit(.minutes(5)))
    func startStopClosesViewerWindowMacOS() async throws {
        let id = "testanyware-test-\(UUID().uuidString.prefix(8))".lowercased()
        let options = VMStartOptions(
            platform: .macos,
            base: nil,
            id: id,
            display: nil,
            openViewer: true
        )

        let cleanup = { try? VMLifecycle.stop(id: id) }

        let result: VMStartResult
        do {
            result = try await VMLifecycle.start(options: options)
        } catch {
            cleanup()
            throw error
        }

        let capturedID: String
        do {
            guard let captured = result.meta.viewerWindowID, !captured.isEmpty else {
                #expect(Bool(false), "viewer window id should be captured when openViewer=true")
                try VMLifecycle.stop(id: id)
                return
            }
            capturedID = captured

            #expect(
                screenSharingHasWindow(withIdentifier: capturedID),
                "Screen Sharing should have viewer window \(capturedID) after start"
            )
        } catch {
            cleanup()
            throw error
        }

        try VMLifecycle.stop(id: id)

        #expect(
            !screenSharingHasWindow(withIdentifier: capturedID),
            "Screen Sharing should not have viewer window \(capturedID) after stop"
        )

        let paths = VMPaths()
        #expect(!FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(!FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))
    }
}

@Suite("VM Lifecycle Integration — Windows/QEMU",
       .enabled(if: qemuWindowsAvailable()),
       .serialized)
struct VMLifecycleQEMUTests {

    @Test(.timeLimit(.minutes(15)))
    func startStopRoundTripWindows() async throws {
        let id = "testanyware-test-\(UUID().uuidString.prefix(8))".lowercased()
        let options = VMStartOptions(
            platform: .windows,
            base: nil,
            id: id,
            display: nil,
            openViewer: false
        )

        let cleanup = { try? VMLifecycle.stop(id: id) }

        let result: VMStartResult
        do {
            result = try await VMLifecycle.start(options: options)
        } catch {
            cleanup()
            throw error
        }

        #expect(result.id == id)

        let paths = VMPaths()
        #expect(FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))
        #expect(FileManager.default.fileExists(atPath: paths.cloneDir(forID: id)))

        let spec = try ConnectionSpec.load(from: paths.specPath(forID: id))
        #expect(spec.platform?.rawValue == "windows")
        #expect(spec.vnc.port > 0)
        #expect(spec.vnc.password == "testanyware")

        try VMLifecycle.stop(id: id)

        #expect(!FileManager.default.fileExists(atPath: paths.specPath(forID: id)))
        #expect(!FileManager.default.fileExists(atPath: paths.metaPath(forID: id)))
        #expect(!FileManager.default.fileExists(atPath: paths.cloneDir(forID: id)))
    }

    @Test(.timeLimit(.minutes(15)))
    func qemuMonitorDiscoversAgentPort() async throws {
        let id = "testanyware-test-\(UUID().uuidString.prefix(8))".lowercased()
        let options = VMStartOptions(
            platform: .windows,
            base: nil,
            id: id,
            display: nil,
            openViewer: false
        )
        let paths = VMPaths()

        let artifacts: QEMURunner.StartArtifacts
        do {
            artifacts = try await QEMURunner.start(options: options, paths: paths)
        } catch {
            QEMURunner.stop(pid: 0, cloneDir: paths.cloneDir(forID: id))
            throw error
        }

        defer { QEMURunner.stop(pid: artifacts.pid, cloneDir: artifacts.cloneDir) }

        #expect((artifacts.agentPort ?? 0) > 0)
        #expect(artifacts.vncPort > 0)

        // monitor.sock lives in the TMPDIR session dir, not the clone dir,
        // so derive it via QEMURunner.sessionDir(forID:) — backlog item 12.
        let monitorSock = "\(QEMURunner.sessionDir(forID: id))/monitor.sock"
        let client = QEMUMonitorClient(socketPath: monitorSock)
        let rediscovered = try await client.agentPort(attempts: 2, intervalSeconds: 0.5)
        #expect(rediscovered == artifacts.agentPort)
    }
}
