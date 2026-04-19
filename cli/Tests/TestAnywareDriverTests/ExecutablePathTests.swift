import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("ExecutablePath")
struct ExecutablePathTests {

    @Test func currentExecutablePathIsAbsolute() {
        let path = currentExecutablePath()
        #expect(path.hasPrefix("/"), "expected absolute path, got: \(path)")
    }

    @Test func currentExecutablePathIsNonEmpty() {
        let path = currentExecutablePath()
        #expect(!path.isEmpty)
    }

    @Test func currentExecutablePathPointsAtExistingFile() {
        // Under `swift test`, the running executable is the test runner
        // (xctest helper). It should exist on disk regardless of CWD.
        let path = currentExecutablePath()
        #expect(FileManager.default.fileExists(atPath: path),
                "executable path does not exist on disk: \(path)")
    }

    @Test func currentExecutablePathIsCWDIndependent() {
        // The path must be resolved from the binary's true location, not
        // from `argv[0]` joined to CWD. Changing CWD must not change it.
        let original = FileManager.default.currentDirectoryPath
        defer { _ = FileManager.default.changeCurrentDirectoryPath(original) }

        let pathBefore = currentExecutablePath()
        #expect(FileManager.default.changeCurrentDirectoryPath("/tmp"))
        let pathAfter = currentExecutablePath()
        #expect(pathBefore == pathAfter,
                "executable path changed with CWD: \(pathBefore) vs \(pathAfter)")
    }
}
