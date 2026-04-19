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

        let running = "\(path)/testanyware-a1b2c3d4"
        let stale = "\(path)/testanyware-b5c6d7e8"
        try FileManager.default.createDirectory(atPath: running, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(atPath: stale, withIntermediateDirectories: true)

        FileManager.default.createFile(atPath: "\(running)/monitor.sock", contents: Data())

        let entries = try QEMURunner.scanClonesDir(path: path)
        #expect(entries.count == 1)
        let entry = try #require(entries.first)
        #expect(entry.name == "testanyware-a1b2c3d4")
        #expect(entry.kind == .running)
        #expect(entry.backend == "qemu")
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
