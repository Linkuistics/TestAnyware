import CoreGraphics
import TestAnywareAgentProtocol

/// Recursively walks an accessibility tree and converts elements to `ElementInfo`.
public struct TreeWalker {

    /// Walk the children of `root` up to the specified `depth`, optionally filtering by role or label.
    ///
    /// - Parameters:
    ///   - root: The root accessible element whose children will be walked.
    ///   - depth: Maximum depth to recurse (1 = only direct children, no grandchildren expanded).
    ///   - roleFilter: If set, only elements with this unified role are included. Ancestor elements
    ///     that contain matching descendants are preserved to maintain tree structure.
    ///   - labelFilter: If set, only elements whose label contains this substring (case-insensitive)
    ///     are included in the results.
    /// - Returns: An array of `ElementInfo` representing the filtered, depth-limited tree.
    public static func walk(
        root: any AccessibleElement,
        depth: Int,
        roleFilter: UnifiedRole? = nil,
        labelFilter: String? = nil
    ) -> [ElementInfo] {
        let rawChildren = root.children()
        return rawChildren.compactMap { child in
            walkElement(child, remainingDepth: depth, roleFilter: roleFilter, labelFilter: labelFilter)
        }
    }

    private static func walkElement(
        _ element: any AccessibleElement,
        remainingDepth: Int,
        roleFilter: UnifiedRole?,
        labelFilter: String?
    ) -> ElementInfo? {
        let rawChildren = element.children()
        let childCount = rawChildren.count
        let roleString = element.role() ?? ""
        let mappedRole = RoleMapper.map(role: roleString, subrole: element.subrole())

        // Determine expanded children based on depth
        let expandedChildren: [ElementInfo]?
        if remainingDepth <= 1 {
            // Depth exhausted: don't expand children
            expandedChildren = nil
        } else {
            let walked = rawChildren.compactMap { child in
                walkElement(
                    child,
                    remainingDepth: remainingDepth - 1,
                    roleFilter: roleFilter,
                    labelFilter: labelFilter
                )
            }
            expandedChildren = walked
        }

        // Apply filters
        let matchesSelf = elementMatchesFilters(
            role: mappedRole,
            label: element.label(),
            roleFilter: roleFilter,
            labelFilter: labelFilter
        )

        let hasMatchingDescendants = expandedChildren.map { !$0.isEmpty } ?? false

        // If neither this element matches nor any descendant matches, exclude it
        if !matchesSelf && !hasMatchingDescendants {
            if roleFilter != nil || labelFilter != nil {
                return nil
            }
        }

        let info = ElementInfo(
            role: mappedRole,
            label: element.label(),
            value: element.value(),
            description: element.descriptionText(),
            id: element.identifier(),
            enabled: element.isEnabled(),
            focused: element.isFocused(),
            position: element.position(),
            size: element.size(),
            childCount: childCount,
            actions: element.actionNames(),
            platformRole: roleString.isEmpty ? nil : roleString,
            children: expandedChildren
        )

        return info
    }

    private static func elementMatchesFilters(
        role: UnifiedRole,
        label: String?,
        roleFilter: UnifiedRole?,
        labelFilter: String?
    ) -> Bool {
        if let roleFilter, role != roleFilter {
            return false
        }
        if let labelFilter, let label {
            if !label.localizedCaseInsensitiveContains(labelFilter) {
                return false
            }
        } else if let _ = labelFilter {
            // labelFilter set but element has no label — no match
            return false
        }
        return true
    }
}
