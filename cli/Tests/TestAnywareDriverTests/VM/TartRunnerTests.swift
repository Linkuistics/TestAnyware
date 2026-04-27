import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("TartRunner")
struct TartRunnerTests {

    // MARK: - parseList

    @Test func parseListGoldenImagesFromJSON() throws {
        let json = """
        [
          {"Name": "testanyware-golden-macos-tahoe", "State": "stopped", "Disk": 50, "Source": "local"},
          {"Name": "testanyware-golden-linux-24.04", "State": "stopped", "Disk": 20, "Source": "local"},
          {"Name": "some-other-vm", "State": "stopped", "Disk": 10, "Source": "local"},
          {"Name": "testanyware-a1b2c3d4", "State": "running", "Disk": 50, "Source": "local"}
        ]
        """
        let entries = try TartRunner.parseList(tartJSON: json)
        let goldens = entries.filter { $0.kind == .golden }
        #expect(Set(goldens.map { $0.name }) == [
            "testanyware-golden-macos-tahoe",
            "testanyware-golden-linux-24.04",
        ])
        #expect(goldens.first { $0.name.contains("macos") }?.platform == "macos")
        #expect(goldens.first { $0.name.contains("linux") }?.platform == "linux")
        #expect(goldens.first { $0.name.contains("macos") }?.sizeGB == "50 GB")
        #expect(goldens.first { $0.name.contains("macos") }?.backend == "tart")
    }

    @Test func parseListRunningClonesSkipsGoldens() throws {
        let json = """
        [
          {"Name": "testanyware-golden-macos-tahoe", "State": "running", "Disk": 50, "Source": "local"},
          {"Name": "testanyware-a1b2c3d4", "State": "running", "Disk": 50, "Source": "local"},
          {"Name": "testanyware-b5c6d7e8", "State": "stopped", "Disk": 50, "Source": "local"}
        ]
        """
        let entries = try TartRunner.parseList(tartJSON: json)
        let running = entries.filter { $0.kind == .running }
        #expect(Set(running.map { $0.name }) == ["testanyware-a1b2c3d4"])
        let goldenRunning = entries.filter { $0.kind == .golden }
        #expect(goldenRunning.map { $0.name } == ["testanyware-golden-macos-tahoe"])
    }

    @Test func parseListReturnsEmptyForEmptyJSON() throws {
        let entries = try TartRunner.parseList(tartJSON: "[]")
        #expect(entries.isEmpty)
    }

    @Test func parseListReturnsEmptyOnMalformedJSON() throws {
        let entries = try TartRunner.parseList(tartJSON: "not json")
        #expect(entries.isEmpty)
    }

    @Test func parseListHandlesMissingDiskField() throws {
        let json = """
        [{"Name": "testanyware-golden-windows-11", "State": "stopped"}]
        """
        let entries = try TartRunner.parseList(tartJSON: json)
        let first = try #require(entries.first)
        #expect(first.kind == .golden)
        #expect(first.sizeGB == nil)
        #expect(first.platform == "windows")
    }

    // MARK: - parseAllVMNames
    //
    // `parseAllVMNames` underlies `vmExists`, which the lifecycle uses to
    // address user-supplied ids that may not follow the `testanyware-`
    // convention. parseList's prefix + state classification is intentionally
    // narrow for `vm list`; the broader query lives here.

    @Test func parseAllVMNamesReturnsEveryNameRegardlessOfStateOrPrefix() {
        let json = """
        [
          {"Name": "testanyware-golden-macos-tahoe", "State": "stopped"},
          {"Name": "testanyware-a1b2c3d4", "State": "running"},
          {"Name": "my-custom-vm", "State": "running"},
          {"Name": "another-vm", "State": "stopped"}
        ]
        """
        let names = TartRunner.parseAllVMNames(tartJSON: json)
        #expect(Set(names) == [
            "testanyware-golden-macos-tahoe",
            "testanyware-a1b2c3d4",
            "my-custom-vm",
            "another-vm",
        ])
    }

    @Test func parseAllVMNamesReturnsEmptyOnEmptyJSON() {
        #expect(TartRunner.parseAllVMNames(tartJSON: "[]").isEmpty)
        #expect(TartRunner.parseAllVMNames(tartJSON: "").isEmpty)
    }

    @Test func parseAllVMNamesReturnsEmptyOnMalformedJSON() {
        #expect(TartRunner.parseAllVMNames(tartJSON: "not json").isEmpty)
    }

    // MARK: - parseVNCURL

    @Test func parseVNCURLExtractsComponents() throws {
        let url = "vnc://:syrup-rotate@127.0.0.1:63530"
        let parsed = try TartRunner.parseVNCURL(url)
        #expect(parsed.host == "127.0.0.1")
        #expect(parsed.port == 63530)
        #expect(parsed.password == "syrup-rotate")
    }

    @Test func parseVNCURLStripsTrailingEllipsis() throws {
        let url = "vnc://:abc@127.0.0.1:5900..."
        let parsed = try TartRunner.parseVNCURL(url)
        #expect(parsed.port == 5900)
        #expect(parsed.password == "abc")
    }

    @Test func parseVNCURLWithoutPasswordReturnsNilPassword() throws {
        let url = "vnc://127.0.0.1:5900"
        let parsed = try TartRunner.parseVNCURL(url)
        #expect(parsed.host == "127.0.0.1")
        #expect(parsed.port == 5900)
        #expect(parsed.password == nil)
    }

    @Test func parseVNCURLMalformedThrows() {
        #expect(throws: TartRunnerError.self) {
            _ = try TartRunner.parseVNCURL("http://example.com")
        }
        #expect(throws: TartRunnerError.self) {
            _ = try TartRunner.parseVNCURL("vnc://no-port")
        }
    }

    // MARK: - pollVNCURL (file-driven; no tart invocation)

    @Test func pollVNCURLReturnsURLWhenLogContainsIt() throws {
        let logPath = NSTemporaryDirectory() + "tart-poll-\(UUID().uuidString).log"
        let body = "tart starting...\nVNC: vnc://:abc@127.0.0.1:54321\nready.\n"
        try body.write(toFile: logPath, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(atPath: logPath) }

        let parsed = TartRunner.pollVNCURL(logPath: logPath, attempts: 3, intervalSeconds: 0.01)
        let url = try #require(parsed, "expected pollVNCURL to find the URL in the log file")
        #expect(url.host == "127.0.0.1")
        #expect(url.port == 54321)
        #expect(url.password == "abc")
    }

    @Test func pollVNCURLReturnsNilWhenLogHasNoURL() throws {
        let logPath = NSTemporaryDirectory() + "tart-poll-\(UUID().uuidString).log"
        try "no url here".write(toFile: logPath, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(atPath: logPath) }

        let parsed = TartRunner.pollVNCURL(logPath: logPath, attempts: 2, intervalSeconds: 0.01)
        #expect(parsed == nil)
    }

    @Test func pollVNCURLReturnsNilWhenLogMissing() {
        let bogusPath = NSTemporaryDirectory() + "definitely-not-here-\(UUID().uuidString).log"
        let parsed = TartRunner.pollVNCURL(logPath: bogusPath, attempts: 2, intervalSeconds: 0.01)
        #expect(parsed == nil)
    }

    // MARK: - pollIP (live tart; degrades to nil on non-tart hosts)

    /// Exercises the private `runTart` plumbing end-to-end against the
    /// real `tart` binary when present. A non-existent id makes `tart ip`
    /// fail repeatedly, so the loop must time out and return `nil` rather
    /// than spinning or throwing. On hosts without `tart` the same `nil`
    /// is returned, so the assertion holds either way.
    @Test func pollIPReturnsNilForUnknownVM() {
        let bogusID = "testanyware-test-\(UUID().uuidString.prefix(8))"
        let ip = TartRunner.pollIP(id: bogusID, attempts: 2, intervalSeconds: 0.05)
        #expect(ip == nil)
    }
}
