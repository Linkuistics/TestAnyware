import CoreGraphics

/// Abstraction over a platform accessibility element.
/// `public` so test targets can declare `MockAccessibleElement: AccessibleElement`.
public protocol AccessibleElement: Sendable {
    func role() -> String?
    func subrole() -> String?
    func label() -> String?
    func value() -> String?
    func descriptionText() -> String?
    func identifier() -> String?
    func isEnabled() -> Bool
    func isFocused() -> Bool
    func position() -> CGPoint?
    func size() -> CGSize?
    func children() -> [any AccessibleElement]
    func actionNames() -> [String]
    func performAction(_ name: String) throws
    func setAttribute(_ name: String, value: Any) throws
    func fontInfo() -> (family: String?, size: Double?, weight: String?)?
}

/// Errors thrown by `AXElementWrapper` write/action methods.
public enum AXWrapperError: Error, Sendable {
    case actionFailed(action: String, axError: Int32)
    case setAttributeFailed(attribute: String, axError: Int32)
}
