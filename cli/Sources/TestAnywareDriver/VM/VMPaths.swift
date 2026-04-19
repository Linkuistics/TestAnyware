import Foundation

/// XDG-compliant path helpers for VM lifecycle artefacts.
///
/// Mirrors `scripts/macos/_testanyware-paths.sh` so the Swift port and the
/// existing bash golden-image creators agree on every on-disk location.
/// Environment is injected for testability.
public struct VMPaths: Sendable {
    public let stateDir: String
    public let dataDir: String

    public init(env: [String: String] = ProcessInfo.processInfo.environment) {
        let home = env["HOME"] ?? NSHomeDirectory()
        let xdgState = env["XDG_STATE_HOME"] ?? ""
        self.stateDir = xdgState.isEmpty
            ? "\(home)/.local/state/testanyware"
            : "\(xdgState)/testanyware"
        let xdgData = env["XDG_DATA_HOME"] ?? ""
        self.dataDir = xdgData.isEmpty
            ? "\(home)/.local/share/testanyware"
            : "\(xdgData)/testanyware"
    }

    public var vmsDir: String { "\(stateDir)/vms" }
    public var goldenDir: String { "\(dataDir)/golden" }
    public var clonesDir: String { "\(dataDir)/clones" }

    public func specPath(forID id: String) -> String { "\(vmsDir)/\(id).json" }
    public func metaPath(forID id: String) -> String { "\(vmsDir)/\(id).meta.json" }
    public func cloneDir(forID id: String) -> String { "\(clonesDir)/\(id)" }
}
