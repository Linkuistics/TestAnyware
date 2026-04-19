/// Errors thrown by `ActionPerformer` when an action is not available or fails.
public enum ActionPerformerError: Error, Sendable {
    case actionNotAvailable(action: String)
    case attributeNotSettable(attribute: String)
}

extension ActionPerformerError: CustomStringConvertible {
    public var description: String {
        switch self {
        case .actionNotAvailable(let action):
            return "Action '\(action)' is not available on this element"
        case .attributeNotSettable(let attribute):
            return "Attribute '\(attribute)' cannot be set on this element"
        }
    }
}

/// Performs accessibility actions on `AccessibleElement` values.
public struct ActionPerformer {

    /// Performs the AXPress action on the given element.
    /// - Throws: `ActionPerformerError.actionNotAvailable` if AXPress is not listed, or an underlying AX error.
    public static func press(element: any AccessibleElement) throws {
        let available = element.actionNames()
        guard available.contains("AXPress") else {
            throw ActionPerformerError.actionNotAvailable(action: "AXPress")
        }
        try element.performAction("AXPress")
    }

    /// Sets the AXValue attribute on the given element.
    /// - Throws: An underlying AX error if the attribute cannot be set.
    public static func setValue(element: any AccessibleElement, value: String) throws {
        try element.setAttribute("AXValue", value: value)
    }

    /// Sets AXFocused to true on the given element.
    /// - Throws: An underlying AX error if the attribute cannot be set.
    public static func focus(element: any AccessibleElement) throws {
        try element.setAttribute("AXFocused", value: true)
    }

    /// Performs the AXShowMenu action on the given element.
    /// - Throws: `ActionPerformerError.actionNotAvailable` if AXShowMenu is not listed, or an underlying AX error.
    public static func showMenu(element: any AccessibleElement) throws {
        let available = element.actionNames()
        guard available.contains("AXShowMenu") else {
            throw ActionPerformerError.actionNotAvailable(action: "AXShowMenu")
        }
        try element.performAction("AXShowMenu")
    }
}
