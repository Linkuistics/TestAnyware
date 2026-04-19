import Foundation

/// Returns the absolute path of the currently running executable.
///
/// Uses `Bundle.main.executablePath`, which on macOS is backed by
/// `_NSGetExecutablePath` and yields the absolute path the binary was
/// launched with — independent of the current working directory and of
/// whether the binary was invoked by bare name via `$PATH` lookup.
///
/// Falls back to the legacy `argv[0] + cwd` join only if
/// `Bundle.main.executablePath` is `nil`. This shouldn't occur for a
/// CLI executable but the fallback keeps behaviour defined.
internal func currentExecutablePath() -> String {
    if let path = Bundle.main.executablePath {
        return (path as NSString).standardizingPath
    }
    var execPath = CommandLine.arguments.first ?? ""
    if !execPath.hasPrefix("/") {
        let cwd = FileManager.default.currentDirectoryPath
        execPath = (cwd as NSString).appendingPathComponent(execPath)
    }
    return (execPath as NSString).standardizingPath
}
