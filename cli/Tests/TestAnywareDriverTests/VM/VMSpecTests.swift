import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("VMSpec")
struct VMSpecTests {

    // MARK: - Helpers

    private func tempSpecURL() -> URL {
        URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("vmspec-\(UUID().uuidString).json")
    }

    private func decodeJSON(at url: URL) throws -> [String: Any] {
        let data = try Data(contentsOf: url)
        guard let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            Issue.record("Spec file did not decode as a JSON object")
            return [:]
        }
        return obj
    }

    // MARK: - Round-trip

    @Test func roundTripFullSpec() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "127.0.0.1", port: 63530, password: "secret"),
            agent: AgentSpec(host: "192.168.64.2", port: 8648),
            platform: .macos
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)
        let loaded = try VMSpec.load(from: url.path)
        #expect(loaded == spec)
    }

    @Test func roundTripMinimalSpec() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "h", port: 1, password: nil),
            agent: nil,
            platform: .linux
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)
        let loaded = try VMSpec.load(from: url.path)
        #expect(loaded == spec)
    }

    // MARK: - Field presence / omission

    @Test func writesAllFieldsWhenPresent() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "127.0.0.1", port: 63530, password: "secret"),
            agent: AgentSpec(host: "192.168.64.2", port: 8648),
            platform: .macos
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)

        let json = try decodeJSON(at: url)
        let vnc = json["vnc"] as? [String: Any]
        #expect(vnc?["host"] as? String == "127.0.0.1")
        #expect(vnc?["port"] as? Int == 63530)
        #expect(vnc?["password"] as? String == "secret")
        let agent = json["agent"] as? [String: Any]
        #expect(agent?["host"] as? String == "192.168.64.2")
        #expect(agent?["port"] as? Int == 8648)
        #expect(json["platform"] as? String == "macos")
        #expect(json["ssh"] == nil)
    }

    @Test func omitsOptionalFieldsWhenNil() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "h", port: 1, password: nil),
            agent: nil,
            platform: .linux
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)

        let json = try decodeJSON(at: url)
        let vnc = json["vnc"] as? [String: Any]
        #expect(vnc?["password"] == nil)
        #expect(json["agent"] == nil)
    }

    // MARK: - File permissions

    @Test func writeAtomicSetsMode0600() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "h", port: 1, password: nil),
            agent: nil,
            platform: .macos
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let perms = attrs[.posixPermissions] as? NSNumber
        #expect(perms?.int16Value == 0o600)
    }

    // MARK: - Atomic replace

    @Test func writeAtomicReplacesExisting() throws {
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let first = VMSpec(
            vnc: VNCSpec(host: "a", port: 1, password: nil),
            agent: nil,
            platform: .macos
        )
        try first.writeAtomic(to: url.path)
        let second = VMSpec(
            vnc: VNCSpec(host: "b", port: 2, password: "pw"),
            agent: AgentSpec(host: "b", port: 8648),
            platform: .windows
        )
        try second.writeAtomic(to: url.path)
        let loaded = try VMSpec.load(from: url.path)
        #expect(loaded == second)
    }

    @Test func writeAtomicLeavesNoTempFileBehind() throws {
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let spec = VMSpec(
            vnc: VNCSpec(host: "h", port: 1, password: nil),
            agent: nil,
            platform: .macos
        )
        try spec.writeAtomic(to: url.path)
        #expect(!FileManager.default.fileExists(atPath: url.path + ".tmp"))
    }

    // MARK: - Interop with ConnectionSpec reader

    @Test func connectionSpecCanLoadVMSpecOutput() throws {
        let spec = VMSpec(
            vnc: VNCSpec(host: "127.0.0.1", port: 63530, password: "pw"),
            agent: AgentSpec(host: "192.168.64.2", port: 8648),
            platform: .macos
        )
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        try spec.writeAtomic(to: url.path)
        let loaded = try ConnectionSpec.load(from: url.path)
        #expect(loaded.vnc.host == "127.0.0.1")
        #expect(loaded.vnc.port == 63530)
        #expect(loaded.vnc.password == "pw")
        #expect(loaded.agent?.host == "192.168.64.2")
        #expect(loaded.agent?.port == 8648)
        #expect(loaded.platform == .macos)
    }

    // MARK: - Forward compatibility with legacy spec files

    @Test func decodesLegacySpecWithSshField() throws {
        // Older spec files (pre-SSH-disable, written by running VMs that
        // span this upgrade) may still contain an `ssh` field. JSONDecoder
        // ignores unknown keys by default, so decode must succeed.
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let legacyJSON = """
        {
          "vnc": { "host": "127.0.0.1", "port": 63530, "password": "secret" },
          "agent": { "host": "192.168.64.2", "port": 8648 },
          "platform": "macos",
          "ssh": "admin@192.168.64.2"
        }
        """
        try legacyJSON.write(to: url, atomically: true, encoding: .utf8)
        let loaded = try VMSpec.load(from: url.path)
        #expect(loaded.vnc.host == "127.0.0.1")
        #expect(loaded.vnc.port == 63530)
        #expect(loaded.vnc.password == "secret")
        #expect(loaded.agent?.host == "192.168.64.2")
        #expect(loaded.agent?.port == 8648)
        #expect(loaded.platform == .macos)
    }

    @Test func decodesMinimalSpecWithoutOptionalFields() throws {
        let url = tempSpecURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let minimalJSON = """
        {
          "vnc": { "host": "h", "port": 1 },
          "platform": "linux"
        }
        """
        try minimalJSON.write(to: url, atomically: true, encoding: .utf8)
        let loaded = try VMSpec.load(from: url.path)
        #expect(loaded.vnc.password == nil)
        #expect(loaded.agent == nil)
        #expect(loaded.platform == .linux)
    }
}
