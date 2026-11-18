import Darwin
import Foundation
import XCTest
@testable import TestAnywareAgent

/// The download reader of the spec's "Agent download contract" (ADR-0026).
/// Each writer action happens after the open, between the length and the
/// read, which is the gap that the old handler's three path lookups left.
/// Leaf k221.
final class SingleVersionFileTests: XCTestCase {

    static let version1 = Data(String(repeating: "version one of the evidence\n", count: 1000).utf8)
    static let version2 = Data(String(repeating: "VERSION TWO, A DIFFERENT LENGTH\n", count: 1500).utf8)

    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("testanyware-download-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
    }

    func testAnUnchangedFileGivesTheExactBytes() throws {
        let path = try write("evidence.txt", Self.version1)
        let file = try SingleVersionFile(path: path)
        XCTAssertEqual(file.length, Self.version1.count)
        XCTAssertEqual(try readAll(file), Self.version1)
    }

    /// The consistency row "Replacement by rename": the whole version that
    /// was open at the start.
    func testARenameReplacementGivesTheVersionOpenAtTheStart() throws {
        let path = try write("evidence.txt", Self.version1)
        let fresh = try write("evidence.txt.new", Self.version2)
        let file = try SingleVersionFile(path: path)
        XCTAssertEqual(rename(fresh, path), 0)
        XCTAssertEqual(file.length, Self.version1.count)
        XCTAssertEqual(try readAll(file), Self.version1)
        XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: path)), Self.version2)
    }

    /// The same, with the replacement after the first chunk.
    func testARenameReplacementDuringTheReadGivesTheVersionOpenAtTheStart() throws {
        let path = try write("evidence.txt", Self.version1)
        let fresh = try write("evidence.txt.new", Self.version2)
        let file = try SingleVersionFile(path: path)
        var body = try file.next(maxBytes: 1000)
        XCTAssertEqual(rename(fresh, path), 0)
        body += try readAll(file)
        XCTAssertEqual(body, Self.version1)
    }

    /// The consistency row "Append": the first L bytes, and never a byte past L.
    func testAnAppendGivesTheFirstLBytes() throws {
        let path = try write("evidence.txt", Self.version1)
        let file = try SingleVersionFile(path: path)
        let handle = try XCTUnwrap(FileHandle(forWritingAtPath: path))
        handle.seekToEndOfFile()
        handle.write(Data(String(repeating: "appended after the length was taken\n", count: 100).utf8))
        try handle.close()
        XCTAssertEqual(try readAll(file), Self.version1)
        XCTAssertTrue(file.finished)
        XCTAssertEqual(try file.next(maxBytes: 4096), Data(), "the reader went past L")
    }

    /// The consistency row "Truncation": a detected failure. The reader
    /// throws, and the server then closes the connection before L bytes.
    func testATruncationGivesAShortRead() throws {
        let path = try write("evidence.txt", Self.version1)
        let file = try SingleVersionFile(path: path)
        XCTAssertEqual(truncate(path, 100), 0)
        XCTAssertEqual(try file.next(maxBytes: 64 * 1024), Self.version1.prefix(100))
        XCTAssertThrowsError(try file.next(maxBytes: 64 * 1024)) { error in
            XCTAssertEqual(
                error as? SingleVersionFile.ShortRead,
                SingleVersionFile.ShortRead(offset: 100, length: Self.version1.count, errno: nil))
        }
    }

    func testAnEmptyFileHasNoBody() throws {
        let file = try SingleVersionFile(path: try write("empty", Data()))
        XCTAssertEqual(file.length, 0)
        XCTAssertTrue(file.finished)
        XCTAssertEqual(try file.next(maxBytes: 4096), Data())
    }

    func testAMissingFileOrADirectoryIsNotFound() {
        XCTAssertThrowsError(try SingleVersionFile(path: directory.appendingPathComponent("absent").path)) {
            XCTAssertEqual($0 as? SingleVersionFile.OpenError, .notFound)
        }
        XCTAssertThrowsError(try SingleVersionFile(path: directory.path)) {
            XCTAssertEqual($0 as? SingleVersionFile.OpenError, .notFound)
        }
    }

    /// Review k237 R2. A blocking open of a FIFO without a writer waits for a
    /// writer, so the rejection must come from a handle that opened at once.
    /// On failure, a writer opens the FIFO and ends the wait.
    func testAFIFOWithoutAWriterIsRejectedAtOnce() throws {
        let path = directory.appendingPathComponent("evidence.pipe").path
        XCTAssertEqual(mkfifo(path, 0o600), 0)
        let done = DispatchSemaphore(value: 0)
        let outcome = OpenOutcome()
        Thread {
            outcome.error = Result { try SingleVersionFile(path: path) }.failure
            done.signal()
        }.start()
        if done.wait(timeout: .now() + 1) == .timedOut {
            let writer = open(path, O_WRONLY | O_NONBLOCK)
            _ = done.wait(timeout: .now() + 5)
            if writer >= 0 { close(writer) }
            return XCTFail("the open of a FIFO without a writer blocked for 1 s")
        }
        XCTAssertEqual(outcome.error as? SingleVersionFile.OpenError, .notFound)
    }

    func testAnUnreadableFileFailsWithItsErrno() throws {
        let path = try write("secret", Data("x".utf8))
        XCTAssertEqual(chmod(path, 0), 0)
        defer { chmod(path, 0o600) }
        guard getuid() != 0 else { throw XCTSkip("root reads a file of mode 0") }
        XCTAssertThrowsError(try SingleVersionFile(path: path)) {
            XCTAssertEqual($0 as? SingleVersionFile.OpenError, .failed(EACCES))
        }
    }

    // MARK: - Helpers

    private func write(_ name: String, _ data: Data) throws -> String {
        let url = directory.appendingPathComponent(name)
        try data.write(to: url)
        return url.path
    }

    private func readAll(_ file: SingleVersionFile) throws -> Data {
        var body = Data()
        while !file.finished { body += try file.next(maxBytes: 4096) }
        return body
    }
}

/// The result of an open on another thread.
private final class OpenOutcome: @unchecked Sendable {
    var error: Error?
}

private extension Result {
    var failure: Failure? {
        if case .failure(let error) = self { return error }
        return nil
    }
}
