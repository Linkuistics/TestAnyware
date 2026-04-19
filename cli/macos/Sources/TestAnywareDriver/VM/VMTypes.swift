import Foundation
import Security

/// VM backend used to host a given platform.
///
/// `tart` hosts macOS and Linux VMs; `qemu` hosts Windows VMs. Selected
/// automatically from `Platform.backend`.
public enum VMBackend: String, Codable, Equatable, Sendable {
    case tart
    case qemu
}

/// VM-lifecycle properties for each supported platform.
///
/// Extends the existing `Platform` enum (used for keysym mapping) rather than
/// introducing a parallel `VMPlatform` type, following the precedent set by
/// `VMSpec` which also reuses `Platform` directly.
extension Platform {
    /// Default golden-image name cloned by `testanyware vm start` when no
    /// `--base` is supplied. Matches the names produced by
    /// `scripts/macos/vm-create-golden-*.sh`.
    public var defaultBase: String {
        switch self {
        case .macos:   return "testanyware-golden-macos-tahoe"
        case .linux:   return "testanyware-golden-linux-24.04"
        case .windows: return "testanyware-golden-windows-11"
        }
    }

    /// VM backend that hosts this platform.
    public var backend: VMBackend {
        switch self {
        case .macos, .linux: return .tart
        case .windows:       return .qemu
        }
    }
}

/// Inputs for `VMLifecycle.start`.
///
/// `base` and `id` fall back to sensible defaults when the caller supplies
/// `nil` — the CLI surface exposes both as optional flags and lets this
/// struct fill them in.
public struct VMStartOptions: Equatable, Sendable {
    public var platform: Platform
    public var base: String
    public var id: String
    public var display: String?
    public var openViewer: Bool

    public init(
        platform: Platform,
        base: String?,
        id: String?,
        display: String?,
        openViewer: Bool
    ) {
        self.platform = platform
        self.base = base ?? platform.defaultBase
        self.id = id ?? VMStartOptions.generateID()
        self.display = display
        self.openViewer = openViewer
    }

    /// Generate a fresh `testanyware-<hex8>` identifier matching the format
    /// produced by `scripts/macos/vm-start.sh`.
    public static func generateID() -> String {
        var bytes = [UInt8](repeating: 0, count: 4)
        let status = bytes.withUnsafeMutableBytes { buffer -> Int32 in
            guard let base = buffer.baseAddress else { return errSecAllocate }
            return SecRandomCopyBytes(kSecRandomDefault, 4, base)
        }
        precondition(status == errSecSuccess, "SecRandomCopyBytes failed: \(status)")
        let hex = bytes.map { String(format: "%02x", $0) }.joined()
        return "testanyware-\(hex)"
    }
}

/// Result of a successful `VMLifecycle.start`.
///
/// Carries the written spec and meta sidecar so callers do not need to
/// re-read them from disk.
public struct VMStartResult: Equatable, Sendable {
    public var id: String
    public var spec: VMSpec
    public var meta: VMMeta

    public init(id: String, spec: VMSpec, meta: VMMeta) {
        self.id = id
        self.spec = spec
        self.meta = meta
    }
}

/// One row of `testanyware vm list` output.
///
/// String-typed `platform` and `backend` so entries whose image name cannot
/// be parsed (user-authored clones, non-testanyware tart VMs) can still be
/// surfaced with `"unknown"` rather than dropped. Running-only fields are
/// `nil` on golden rows; size is `nil` on running rows.
public struct VMListEntry: Equatable, Sendable {
    public enum Kind: String, Equatable, Sendable {
        case golden
        case running
    }

    public var kind: Kind
    public var name: String
    public var platform: String
    public var backend: String
    public var sizeGB: String?
    public var agent: String?
    public var vnc: String?
    public var pid: Int?

    public init(
        kind: Kind,
        name: String,
        platform: String,
        backend: String,
        sizeGB: String? = nil,
        agent: String? = nil,
        vnc: String? = nil,
        pid: Int? = nil
    ) {
        self.kind = kind
        self.name = name
        self.platform = platform
        self.backend = backend
        self.sizeGB = sizeGB
        self.agent = agent
        self.vnc = vnc
        self.pid = pid
    }
}
