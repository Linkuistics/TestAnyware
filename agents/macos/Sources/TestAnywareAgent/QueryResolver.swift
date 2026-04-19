import TestAnywareAgentProtocol

/// The result of a query resolution attempt.
public enum QueryResult: Sendable {
    /// Exactly one element matched (or index disambiguation resolved to one).
    case found(ElementInfo)
    /// Multiple elements matched and no index was provided to disambiguate.
    case multiple([ElementInfo])
    /// No elements matched the query criteria.
    case notFound
}

/// Resolves element queries against a pre-walked `[ElementInfo]` tree.
public struct QueryResolver {

    /// Search `elements` (and their descendants) for elements matching the given criteria.
    ///
    /// - Parameters:
    ///   - elements: The top-level elements to search (TreeWalker output, or a window's children).
    ///   - role: If provided, only elements with this exact role are considered.
    ///   - label: If provided, only elements whose label contains this string (case-insensitive) are considered.
    ///   - id: If provided, only elements whose `id` exactly equals this string are considered.
    ///   - index: 1-based index for disambiguation when multiple elements match. If the Nth match
    ///     exists it is returned as `.found`; if out of range, `.notFound` is returned.
    /// - Returns: A `QueryResult` describing whether zero, one, or multiple elements matched.
    public static func resolve(
        in elements: [ElementInfo],
        role: UnifiedRole?,
        label: String?,
        id: String?,
        index: Int?
    ) -> QueryResult {
        var matches: [ElementInfo] = []
        collectMatches(in: elements, role: role, label: label, id: id, into: &matches)

        switch matches.count {
        case 0:
            return .notFound
        case 1:
            if let index {
                return index == 1 ? .found(matches[0]) : .notFound
            }
            return .found(matches[0])
        default:
            if let index {
                guard index >= 1 && index <= matches.count else { return .notFound }
                return .found(matches[index - 1])
            }
            return .multiple(matches)
        }
    }

    // MARK: - Private

    private static func collectMatches(
        in elements: [ElementInfo],
        role: UnifiedRole?,
        label: String?,
        id: String?,
        into matches: inout [ElementInfo]
    ) {
        for element in elements {
            if elementMatches(element, role: role, label: label, id: id) {
                matches.append(element)
            }
            if let children = element.children {
                collectMatches(in: children, role: role, label: label, id: id, into: &matches)
            }
        }
    }

    private static func elementMatches(
        _ element: ElementInfo,
        role: UnifiedRole?,
        label: String?,
        id: String?
    ) -> Bool {
        if let role, element.role != role {
            return false
        }
        if let label {
            guard let elementLabel = element.label,
                  elementLabel.localizedCaseInsensitiveContains(label) else {
                return false
            }
        }
        if let id, element.id != id {
            return false
        }
        return true
    }
}
