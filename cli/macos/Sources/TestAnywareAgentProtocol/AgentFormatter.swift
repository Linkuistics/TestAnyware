import Foundation

/// Formats agent JSON responses into LLM-optimized indented text.
public enum AgentFormatter {

    // MARK: - Public API

    /// Formats a SnapshotResponse (windows with element trees) into indented text.
    public static func formatSnapshot(_ data: Data) throws -> String {
        formatSnapshot(try JSONDecoder().decode(SnapshotResponse.self, from: data))
    }

    public static func formatSnapshot(_ response: SnapshotResponse) -> String {
        var lines: [String] = []
        for window in response.windows {
            lines.append(formatWindowLine(window))
            if let elements = window.elements {
                for element in elements {
                    formatElement(element, indent: 1, into: &lines)
                }
            }
        }
        return lines.joined(separator: "\n")
    }

    /// Formats a SnapshotResponse as a window list (no element trees).
    public static func formatWindows(_ data: Data) throws -> String {
        formatWindows(try JSONDecoder().decode(SnapshotResponse.self, from: data))
    }

    public static func formatWindows(_ response: SnapshotResponse) -> String {
        response.windows.map { formatWindowLine($0) }.joined(separator: "\n")
    }

    /// Formats an ActionResponse into a concise success/failure message.
    public static func formatAction(_ data: Data) throws -> String {
        formatAction(try JSONDecoder().decode(ActionResponse.self, from: data))
    }

    public static func formatAction(_ response: ActionResponse) -> String {
        let prefix = response.success ? "OK" : "FAILED"
        if let message = response.message {
            return "\(prefix): \(message)"
        }
        return prefix
    }

    /// Formats an ErrorResponse into a clear error message.
    public static func formatError(_ data: Data) throws -> String {
        let response = try JSONDecoder().decode(ErrorResponse.self, from: data)
        if let details = response.details {
            return "Error: \(response.error) \u{2014} \(details)"
        }
        return "Error: \(response.error)"
    }

    /// Formats an InspectResponse with element details and metadata.
    public static func formatInspect(_ data: Data) throws -> String {
        formatInspect(try JSONDecoder().decode(InspectResponse.self, from: data))
    }

    public static func formatInspect(_ response: InspectResponse) -> String {
        var lines: [String] = []
        lines.append(formatElementLine(response.element, indent: 0))
        if let bounds = response.bounds {
            let x = formatCoordinate(bounds.origin.x)
            let y = formatCoordinate(bounds.origin.y)
            let w = formatCoordinate(bounds.size.width)
            let h = formatCoordinate(bounds.size.height)
            lines.append("  bounds: \(x),\(y) \(w)x\(h)")
        }
        let fontParts = buildFontParts(response)
        if !fontParts.isEmpty {
            lines.append("  font: \(fontParts.joined(separator: " "))")
        }
        return lines.joined(separator: "\n")
    }

    // MARK: - Window Formatting

    private static func formatWindowLine(_ window: WindowInfo) -> String {
        var parts: [String] = []
        if let title = window.title {
            parts.append("\"\(title)\"")
        }
        let w = formatCoordinate(window.size.width)
        let h = formatCoordinate(window.size.height)
        parts.append("(\(window.windowType))")
        parts.append("\(w)x\(h)")
        if window.focused {
            parts.append("[focused]")
        }
        parts.append("app:\"\(window.appName)\"")
        return parts.joined(separator: " ")
    }

    // MARK: - Element Formatting

    private static func formatElement(_ element: ElementInfo, indent: Int, into lines: inout [String]) {
        lines.append(formatElementLine(element, indent: indent))
        if let children = element.children {
            for child in children {
                formatElement(child, indent: indent + 1, into: &lines)
            }
        }
    }

    private static func formatElementLine(_ element: ElementInfo, indent: Int) -> String {
        let prefix = String(repeating: "  ", count: indent)
        var parts: [String] = []
        parts.append(element.role.rawValue)
        if let label = element.label {
            parts.append("\"\(label)\"")
        }
        if !element.enabled {
            parts.append("[disabled]")
        }
        if element.focused {
            parts.append("[focused]")
        }
        if showChildCount(for: element.role), element.childCount > 0 {
            parts.append("\(element.childCount) items")
        }
        if let value = element.value {
            parts.append("value=\"\(value)\"")
        }
        return prefix + parts.joined(separator: " ")
    }

    /// Show child count annotation for list/tree container roles.
    private static func showChildCount(for role: UnifiedRole) -> Bool {
        switch role {
        case .list, .tree, .treeGrid, .listBox, .listGrid:
            return true
        default:
            return false
        }
    }

    // MARK: - Inspect Helpers

    private static func buildFontParts(_ response: InspectResponse) -> [String] {
        var parts: [String] = []
        if let family = response.fontFamily {
            parts.append(family)
        }
        if let size = response.fontSize {
            let formatted = formatCoordinate(size)
            parts.append("\(formatted)pt")
        }
        if let weight = response.fontWeight {
            parts.append(weight)
        }
        return parts
    }

    // MARK: - Number Formatting

    /// Formats a coordinate/size value: integer when whole, otherwise decimal.
    private static func formatCoordinate(_ value: Double) -> String {
        if value == value.rounded() && !value.isNaN && !value.isInfinite {
            return String(Int(value))
        }
        return String(value)
    }
}
