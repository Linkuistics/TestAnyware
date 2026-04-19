import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("VMMeta")
struct VMMetaTests {

    // MARK: - Helpers

    private func tempMetaURL() -> URL {
        URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("vmmeta-\(UUID().uuidString).json")
    }

    // MARK: - Round-trip

    @Test func roundTripMinimalMeta() throws {
        let meta = VMMeta(
            id: "testanyware-abc",
            tool: .tart,
            pid: 12345,
            cloneDir: nil,
            viewerWindowID: nil
        )
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try meta.writeAtomic(to: url.path)
        let loaded = try VMMeta.load(from: url.path)
        #expect(loaded == meta)
    }

    @Test func roundTripFullMeta() throws {
        let meta = VMMeta(
            id: "testanyware-xyz",
            tool: .qemu,
            pid: 54321,
            cloneDir: "/some/clone/dir",
            viewerWindowID: "axid-1234"
        )
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try meta.writeAtomic(to: url.path)
        let loaded = try VMMeta.load(from: url.path)
        #expect(loaded == meta)
    }

    // MARK: - File permissions

    @Test func writeAtomicSetsMode0600() throws {
        let meta = VMMeta(id: "g", tool: .tart, pid: 1, cloneDir: nil, viewerWindowID: nil)
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try meta.writeAtomic(to: url.path)
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let perms = attrs[.posixPermissions] as? NSNumber
        #expect(perms?.int16Value == 0o600)
    }

    // MARK: - Atomic replace

    @Test func writeAtomicReplacesExisting() throws {
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let first = VMMeta(id: "a", tool: .tart, pid: 1, cloneDir: nil, viewerWindowID: nil)
        try first.writeAtomic(to: url.path)
        let second = VMMeta(id: "b", tool: .qemu, pid: 2, cloneDir: "/c", viewerWindowID: nil)
        try second.writeAtomic(to: url.path)
        let loaded = try VMMeta.load(from: url.path)
        #expect(loaded == second)
    }

    @Test func writeAtomicLeavesNoTempFileBehind() throws {
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let meta = VMMeta(id: "g", tool: .tart, pid: 1, cloneDir: nil, viewerWindowID: nil)
        try meta.writeAtomic(to: url.path)
        #expect(!FileManager.default.fileExists(atPath: url.path + ".tmp"))
    }

    // MARK: - JSON key compatibility with bash vm-start.sh

    @Test func encodedJSONUsesSnakeCaseKeys() throws {
        let meta = VMMeta(
            id: "testanyware-compat",
            tool: .qemu,
            pid: 42,
            cloneDir: "/tmp/clone",
            viewerWindowID: "win-1"
        )
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try meta.writeAtomic(to: url.path)
        let raw = try String(contentsOf: url, encoding: .utf8)
        #expect(raw.contains("\"clone_dir\""))
        #expect(raw.contains("\"viewer_window_id\""))
        #expect(raw.contains("\"tool\""))
        #expect(raw.contains("\"qemu\""))
    }

    @Test func decodesBashProducedMetaJSON() throws {
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let bashStyleJSON = """
        {
          "id": "testanyware-bash",
          "tool": "tart",
          "pid": 9999,
          "clone_dir": "/var/clone",
          "viewer_window_id": "axid-9"
        }
        """
        try bashStyleJSON.write(to: url, atomically: true, encoding: .utf8)
        let loaded = try VMMeta.load(from: url.path)
        #expect(loaded.id == "testanyware-bash")
        #expect(loaded.tool == .tart)
        #expect(loaded.pid == 9999)
        #expect(loaded.cloneDir == "/var/clone")
        #expect(loaded.viewerWindowID == "axid-9")
    }

    @Test func decodesBashMetaWithoutOptionalFields() throws {
        let url = tempMetaURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let bashStyleJSON = """
        {
          "id": "testanyware-minimal",
          "tool": "qemu",
          "pid": 1
        }
        """
        try bashStyleJSON.write(to: url, atomically: true, encoding: .utf8)
        let loaded = try VMMeta.load(from: url.path)
        #expect(loaded.cloneDir == nil)
        #expect(loaded.viewerWindowID == nil)
    }
}
