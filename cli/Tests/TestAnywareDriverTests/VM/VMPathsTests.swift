import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("VMPaths")
struct VMPathsTests {

    // MARK: - stateDir

    @Test func stateDirUsesXDGStateHomeWhenSet() {
        let paths = VMPaths(env: ["XDG_STATE_HOME": "/tmp/xdg-state", "HOME": "/ignored"])
        #expect(paths.stateDir == "/tmp/xdg-state/testanyware")
    }

    @Test func stateDirFallsBackToHomeLocalStateWhenXDGUnset() {
        let paths = VMPaths(env: ["HOME": "/users/tester"])
        #expect(paths.stateDir == "/users/tester/.local/state/testanyware")
    }

    @Test func stateDirFallsBackToHomeLocalStateWhenXDGEmpty() {
        let paths = VMPaths(env: ["XDG_STATE_HOME": "", "HOME": "/users/tester"])
        #expect(paths.stateDir == "/users/tester/.local/state/testanyware")
    }

    // MARK: - dataDir

    @Test func dataDirUsesXDGDataHomeWhenSet() {
        let paths = VMPaths(env: ["XDG_DATA_HOME": "/tmp/xdg-data", "HOME": "/ignored"])
        #expect(paths.dataDir == "/tmp/xdg-data/testanyware")
    }

    @Test func dataDirFallsBackToHomeLocalShareWhenXDGUnset() {
        let paths = VMPaths(env: ["HOME": "/users/tester"])
        #expect(paths.dataDir == "/users/tester/.local/share/testanyware")
    }

    @Test func dataDirFallsBackToHomeLocalShareWhenXDGEmpty() {
        let paths = VMPaths(env: ["XDG_DATA_HOME": "", "HOME": "/users/tester"])
        #expect(paths.dataDir == "/users/tester/.local/share/testanyware")
    }

    // MARK: - Subdirectories

    @Test func vmsDirIsStateDirSlashVms() {
        let paths = VMPaths(env: ["HOME": "/users/tester"])
        #expect(paths.vmsDir == "/users/tester/.local/state/testanyware/vms")
    }

    @Test func goldenDirIsDataDirSlashGolden() {
        let paths = VMPaths(env: ["HOME": "/users/tester"])
        #expect(paths.goldenDir == "/users/tester/.local/share/testanyware/golden")
    }

    @Test func clonesDirIsDataDirSlashClones() {
        let paths = VMPaths(env: ["HOME": "/users/tester"])
        #expect(paths.clonesDir == "/users/tester/.local/share/testanyware/clones")
    }

    // MARK: - Per-id paths

    @Test func specPathForIdJoinsVMsDir() {
        let paths = VMPaths(env: ["HOME": "/h"])
        #expect(paths.specPath(forID: "testanyware-abc123") == "/h/.local/state/testanyware/vms/testanyware-abc123.json")
    }

    @Test func metaPathForIdJoinsVMsDir() {
        let paths = VMPaths(env: ["HOME": "/h"])
        #expect(paths.metaPath(forID: "testanyware-abc123") == "/h/.local/state/testanyware/vms/testanyware-abc123.meta.json")
    }

    @Test func cloneDirForIdJoinsClonesDir() {
        let paths = VMPaths(env: ["HOME": "/h"])
        #expect(paths.cloneDir(forID: "testanyware-abc123") == "/h/.local/share/testanyware/clones/testanyware-abc123")
    }

    // MARK: - XDG overrides apply under per-id paths

    @Test func specPathHonoursXDGStateHome() {
        let paths = VMPaths(env: ["XDG_STATE_HOME": "/tmp/state", "HOME": "/ignored"])
        #expect(paths.specPath(forID: "testanyware-xyz") == "/tmp/state/testanyware/vms/testanyware-xyz.json")
    }

    @Test func cloneDirHonoursXDGDataHome() {
        let paths = VMPaths(env: ["XDG_DATA_HOME": "/tmp/data", "HOME": "/ignored"])
        #expect(paths.cloneDir(forID: "testanyware-xyz") == "/tmp/data/testanyware/clones/testanyware-xyz")
    }
}
