import Darwin
import Foundation

// One open file version for `/download` (docs/specs/guest-command-lifecycle.md,
// "Agent download contract", and ADR-0026).
//
// The reader opens the file once and takes the length L from `fstat` of that
// handle. It reads with `pread` from the same handle and never past L. A
// rename replacement during the read therefore gives the version that was
// open at the start. An append gives the first L bytes. A truncation gives a
// short read, and the caller then closes the connection.
//
// The old handler looked the path up three times, and the length came from
// one lookup and the bytes from another (k7).

public final class SingleVersionFile: @unchecked Sendable {

    /// Why a file cannot be sent.
    public enum OpenError: Error, Equatable {
        /// No regular file is at the path. `/download` answers 404, as before.
        case notFound
        /// The file exists but cannot be opened or measured. The errno.
        case failed(Int32)
    }

    /// A read that ended before L bytes. The caller must close the connection.
    public struct ShortRead: Error, Equatable {
        public let offset: Int
        public let length: Int
        /// The errno of a failed read, or nil at end of file.
        public let errno: Int32?
    }

    /// The length that the response declares.
    public let length: Int
    private let lock = NSLock()
    private var fd: Int32
    private var offset = 0

    /// Opens `path` once and measures that handle.
    ///
    /// The open does not block. A blocking open of a FIFO without a writer
    /// waits for a writer, before the check below can reject it (review k237
    /// R2). `O_NOCTTY` keeps a terminal device from becoming the controlling
    /// terminal. A regular file then gets blocking reads again.
    public init(path: String) throws {
        let fd = Darwin.open(path, O_RDONLY | O_CLOEXEC | O_NONBLOCK | O_NOCTTY)
        guard fd >= 0 else {
            let code = errno
            throw code == ENOENT || code == ENOTDIR ? OpenError.notFound : OpenError.failed(code)
        }
        var info = stat()
        guard fstat(fd, &info) == 0 else {
            let code = errno
            Darwin.close(fd)
            throw OpenError.failed(code)
        }
        guard (info.st_mode & S_IFMT) == S_IFREG else {
            // A directory or another kind of file is not a downloadable file.
            Darwin.close(fd)
            throw OpenError.notFound
        }
        let flags = fcntl(fd, F_GETFL)
        guard flags >= 0, fcntl(fd, F_SETFL, flags & ~O_NONBLOCK) == 0 else {
            let code = errno
            Darwin.close(fd)
            throw OpenError.failed(code)
        }
        self.fd = fd
        length = Int(info.st_size)
    }

    deinit { close() }

    /// True after the reader sent all L bytes.
    public var finished: Bool {
        lock.lock()
        defer { lock.unlock() }
        return offset >= length
    }

    /// Reads the next chunk of at most `maxBytes`, never past L. Returns an
    /// empty value after L bytes. Throws `ShortRead` when the handle ends or
    /// fails first. The read blocks its thread.
    public func next(maxBytes: Int) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        let want = min(maxBytes, length - offset)
        guard want > 0 else { return Data() }
        guard fd >= 0 else { throw ShortRead(offset: offset, length: length, errno: EBADF) }
        var data = Data(count: want)
        while true {
            let count = data.withUnsafeMutableBytes { pread(fd, $0.baseAddress, want, off_t(offset)) }
            if count < 0, errno == EINTR { continue }
            guard count > 0 else {
                throw ShortRead(offset: offset, length: length, errno: count < 0 ? errno : nil)
            }
            offset += count
            return count == want ? data : data.prefix(count)
        }
    }

    /// Closes the handle. A later `next` throws.
    public func close() {
        lock.lock()
        defer { lock.unlock() }
        if fd >= 0 {
            Darwin.close(fd)
            fd = -1
        }
    }
}
