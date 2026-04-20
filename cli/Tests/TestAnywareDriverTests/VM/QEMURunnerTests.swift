import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("QEMURunner")
struct QEMURunnerTests {

    // MARK: - Helpers

    private func tempDir() -> String {
        let path = NSTemporaryDirectory() + "qemu-scan-\(UUID().uuidString)"
        try? FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
        return path
    }

    // MARK: - platformFromName

    @Test func platformFromNameRecognisesKnownPrefixes() {
        #expect(QEMURunner.platformFromName("testanyware-golden-windows-11") == "windows")
        #expect(QEMURunner.platformFromName("testanyware-golden-macos-tahoe") == "macos")
        #expect(QEMURunner.platformFromName("testanyware-golden-linux-24.04") == "linux")
        #expect(QEMURunner.platformFromName("custom-image") == "unknown")
    }

    // MARK: - scanGoldenDir

    @Test func scanGoldenDirReturnsEmptyWhenMissing() throws {
        let path = NSTemporaryDirectory() + "qemu-missing-\(UUID().uuidString)"
        let entries = try QEMURunner.scanGoldenDir(path: path)
        #expect(entries.isEmpty)
    }

    @Test func scanGoldenDirListsQCOW2Files() throws {
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        FileManager.default.createFile(
            atPath: "\(path)/testanyware-golden-windows-11.qcow2",
            contents: Data()
        )
        FileManager.default.createFile(
            atPath: "\(path)/testanyware-golden-windows-11-efivars.fd",
            contents: Data()
        )
        FileManager.default.createFile(
            atPath: "\(path)/unrelated.txt",
            contents: Data()
        )

        let entries = try QEMURunner.scanGoldenDir(path: path)
        #expect(entries.count == 1)
        let entry = try #require(entries.first)
        #expect(entry.name == "testanyware-golden-windows-11")
        #expect(entry.kind == .golden)
        #expect(entry.backend == "qemu")
        #expect(entry.platform == "windows")
        #expect(entry.sizeGB == nil)
    }

    @Test func scanGoldenDirIgnoresSubdirectoriesAndNonQCOW2() throws {
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        try FileManager.default.createDirectory(
            atPath: "\(path)/subdir",
            withIntermediateDirectories: false
        )
        FileManager.default.createFile(atPath: "\(path)/file.img", contents: Data())

        let entries = try QEMURunner.scanGoldenDir(path: path)
        #expect(entries.isEmpty)
    }

    // MARK: - scanClonesDir

    @Test func scanClonesDirReturnsEmptyWhenMissing() throws {
        let path = NSTemporaryDirectory() + "qemu-clones-missing-\(UUID().uuidString)"
        let entries = try QEMURunner.scanClonesDir(path: path)
        #expect(entries.isEmpty)
    }

    @Test func scanClonesDirListsDirectoriesWithMonitorSock() throws {
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        // Use unique ids so a stray session dir from another test run can't
        // mark the "stale" clone as running and flake the assertion.
        let runID = "testanyware-scan-\(UUID().uuidString.prefix(8))".lowercased()
        let staleID = "testanyware-stale-\(UUID().uuidString.prefix(8))".lowercased()
        let running = "\(path)/\(runID)"
        let stale = "\(path)/\(staleID)"
        try FileManager.default.createDirectory(atPath: running, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(atPath: stale, withIntermediateDirectories: true)

        // monitor.sock now lives in the TMPDIR session dir, not the clone
        // dir — see QEMURunner.sessionDir(forID:). Drop a fake socket file
        // there for the running id and ensure the stale id has none.
        let runSession = QEMURunner.sessionDir(forID: runID)
        let staleSession = QEMURunner.sessionDir(forID: staleID)
        try FileManager.default.createDirectory(atPath: runSession, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(atPath: runSession)
            try? FileManager.default.removeItem(atPath: staleSession)
        }
        FileManager.default.createFile(atPath: "\(runSession)/monitor.sock", contents: Data())

        let entries = try QEMURunner.scanClonesDir(path: path)
        #expect(entries.count == 1)
        let entry = try #require(entries.first)
        #expect(entry.name == runID)
        #expect(entry.kind == .running)
        #expect(entry.backend == "qemu")
    }

    @Test func scanClonesDirIgnoresStaleMonitorSockInCloneDir() throws {
        // Pre-fix path layout placed monitor.sock under cloneDir/. After
        // the move, such a file must not be interpreted as "running" — the
        // session-dir absence is the authoritative liveness signal.
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        let id = "testanyware-stale-clone-\(UUID().uuidString.prefix(8))".lowercased()
        let cloneDir = "\(path)/\(id)"
        try FileManager.default.createDirectory(atPath: cloneDir, withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: "\(cloneDir)/monitor.sock", contents: Data())

        // Also clean up any stale TMPDIR session dir so we don't accidentally
        // mark ourselves running via leftover state from a previous run.
        try? FileManager.default.removeItem(atPath: QEMURunner.sessionDir(forID: id))

        let entries = try QEMURunner.scanClonesDir(path: path)
        #expect(entries.isEmpty)
    }

    // MARK: - sessionDir

    @Test func sessionDirIsDerivedDeterministicallyFromVMID() {
        let a1 = QEMURunner.sessionDir(forID: "testanyware-test-deadbeef")
        let a2 = QEMURunner.sessionDir(forID: "testanyware-test-deadbeef")
        #expect(a1 == a2)
        #expect(a1.hasSuffix("/testanyware-testanyware-test-deadbeef"))
    }

    @Test func sessionDirStaysUnderSunPathLimitForLongestVMID() {
        // macOS struct sockaddr_un.sun_path is 104 bytes (incl. NUL). The
        // longest VM id we generate is `testanyware-test-<hex8>` (25 chars).
        // The longest socket name we stage is `swtpm-sock` (10 chars). The
        // session dir prepends a slash + the file name, so the worst-case
        // path is sessionDir + "/swtpm-sock". Assert it fits with margin.
        let id = "testanyware-test-deadbeef"
        let socket = "\(QEMURunner.sessionDir(forID: id))/swtpm-sock"
        // 104 includes the NUL terminator, so the byte string itself must
        // be < 104. We assert <= 100 to leave a small cushion for $TMPDIR
        // variance across user sessions.
        #expect(socket.utf8.count <= 100, "socket path \(socket) is \(socket.utf8.count) bytes — too close to sun_path 104")
    }

    @Test func scanClonesDirSkipsLooseFiles() throws {
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        FileManager.default.createFile(atPath: "\(path)/loose.txt", contents: Data())

        let entries = try QEMURunner.scanClonesDir(path: path)
        #expect(entries.isEmpty)
    }

    // MARK: - deleteGolden

    @Test func deleteGoldenRemovesQcow2AndEfivarsAndTpmDir() throws {
        let home = tempDir()
        defer { try? FileManager.default.removeItem(atPath: home) }
        let paths = VMPaths(env: ["HOME": home])
        try FileManager.default.createDirectory(
            atPath: paths.goldenDir,
            withIntermediateDirectories: true
        )

        let name = "testanyware-golden-windows-11"
        FileManager.default.createFile(
            atPath: "\(paths.goldenDir)/\(name).qcow2",
            contents: Data()
        )
        FileManager.default.createFile(
            atPath: "\(paths.goldenDir)/\(name)-efivars.fd",
            contents: Data()
        )
        try FileManager.default.createDirectory(
            atPath: "\(paths.goldenDir)/\(name)-tpm",
            withIntermediateDirectories: true
        )
        FileManager.default.createFile(
            atPath: "\(paths.goldenDir)/\(name)-tpm/state",
            contents: Data()
        )
        FileManager.default.createFile(
            atPath: "\(paths.goldenDir)/unrelated.qcow2",
            contents: Data()
        )

        QEMURunner.deleteGolden(name: name, paths: paths)

        let fm = FileManager.default
        #expect(!fm.fileExists(atPath: "\(paths.goldenDir)/\(name).qcow2"))
        #expect(!fm.fileExists(atPath: "\(paths.goldenDir)/\(name)-efivars.fd"))
        #expect(!fm.fileExists(atPath: "\(paths.goldenDir)/\(name)-tpm"))
        #expect(fm.fileExists(atPath: "\(paths.goldenDir)/unrelated.qcow2"))
    }

    @Test func deleteGoldenIsIdempotentOnMissingArtefacts() throws {
        let home = tempDir()
        defer { try? FileManager.default.removeItem(atPath: home) }
        let paths = VMPaths(env: ["HOME": home])
        try FileManager.default.createDirectory(
            atPath: paths.goldenDir,
            withIntermediateDirectories: true
        )

        QEMURunner.deleteGolden(name: "nonexistent", paths: paths)
    }

    // MARK: - teardown / stop

    @Test func stopRemovesSessionDirAlongsideCloneDir() {
        // Verifies the new contract: stop must clear the TMPDIR session
        // dir for the VM as well as the clone dir. Both are populated as
        // a successful start would, then stop is invoked with pid=0
        // (no qemu running) — files-only path of the teardown helper.
        let path = tempDir()
        defer { try? FileManager.default.removeItem(atPath: path) }

        let id = "testanyware-stop-\(UUID().uuidString.prefix(8))".lowercased()
        let cloneDir = "\(path)/\(id)"
        try? FileManager.default.createDirectory(atPath: cloneDir, withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: "\(cloneDir)/\(id).qcow2", contents: Data())

        let session = QEMURunner.sessionDir(forID: id)
        try? FileManager.default.createDirectory(atPath: session, withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: "\(session)/monitor.sock", contents: Data())
        FileManager.default.createFile(atPath: "\(session)/swtpm-sock", contents: Data())

        QEMURunner.stop(pid: 0, cloneDir: cloneDir)

        #expect(!FileManager.default.fileExists(atPath: cloneDir))
        #expect(!FileManager.default.fileExists(atPath: session))
    }

    @Test func teardownIsIdempotentWhenDirsAreMissing() {
        // Calling teardown when nothing is on disk must not throw or
        // crash — start-failure paths invoke it before any files exist.
        let id = "testanyware-missing-\(UUID().uuidString.prefix(8))".lowercased()
        let cloneDir = NSTemporaryDirectory() + "ta-missing-\(UUID().uuidString)"
        let session = QEMURunner.sessionDir(forID: id)
        // Both paths intentionally do not exist.
        QEMURunner.teardown(pid: 0, cloneDir: cloneDir, sessionDir: session)
        #expect(!FileManager.default.fileExists(atPath: cloneDir))
        #expect(!FileManager.default.fileExists(atPath: session))
    }

    // MARK: - runningClonesBacked

    @Test func runningClonesBackedReturnsEmptyWhenClonesDirMissing() {
        let home = tempDir()
        defer { try? FileManager.default.removeItem(atPath: home) }
        let paths = VMPaths(env: ["HOME": home])

        let pids = QEMURunner.runningClonesBacked(
            byGoldenName: "testanyware-golden-windows-11",
            paths: paths
        )
        #expect(pids.isEmpty)
    }
}
