import Foundation

/// Renders `[VMListEntry]` into the two-section text format that
/// `scripts/macos/vm-list.sh` has historically produced.
///
/// Column widths mirror the bash `printf` format strings byte-for-byte so
/// existing callers that grep or awk the output continue to work after the
/// bash-to-Swift flip (`printf "  %-8s %-40s %-8s %s\n"` for goldens,
/// `printf "  %-20s %-8s %-30s %-24s PID %s\n"` for running rows).
public enum VMListFormatter {

    public static func render(goldens: [VMListEntry], running: [VMListEntry]) -> String {
        var lines: [String] = ["Golden images:"]
        if goldens.isEmpty {
            lines.append("  (none)")
        } else {
            for entry in goldens {
                lines.append(renderGolden(entry))
            }
        }
        lines.append("")
        lines.append("Running clones:")
        if running.isEmpty {
            lines.append("  (none)")
        } else {
            for entry in running {
                lines.append(renderRunning(entry))
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func renderGolden(_ entry: VMListEntry) -> String {
        let platform = pad(entry.platform, width: 8)
        let name = pad(entry.name, width: 40)
        let backend = pad(entry.backend, width: 8)
        let size = entry.sizeGB ?? "? GB"
        return "  \(platform) \(name) \(backend) \(size)"
    }

    private static func renderRunning(_ entry: VMListEntry) -> String {
        let name = pad(entry.name, width: 20)
        let platform = pad(entry.platform, width: 8)
        let agent = pad(entry.agent ?? "", width: 30)
        let vnc = pad(entry.vnc ?? "", width: 24)
        let pid = entry.pid.map(String.init) ?? "?"
        return "  \(name) \(platform) \(agent) \(vnc) PID \(pid)"
    }

    private static func pad(_ s: String, width: Int) -> String {
        guard s.count < width else { return s }
        return s + String(repeating: " ", count: width - s.count)
    }
}
