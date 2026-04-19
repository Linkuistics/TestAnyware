@preconcurrency import ApplicationServices
import CoreGraphics

/// Wraps an `AXUIElement` and conforms to `AccessibleElement`.
/// All attribute reads return `nil` on failure rather than throwing.
public struct AXElementWrapper: AccessibleElement {
    public let element: AXUIElement

    public init(_ element: AXUIElement) {
        self.element = element
    }

    // MARK: - Entry points

    /// Wraps `AXUIElementCreateSystemWide()`.
    public static func systemWide() -> AXElementWrapper {
        AXElementWrapper(AXUIElementCreateSystemWide())
    }

    /// Returns wrappers for all running applications via the system-wide element.
    public static func applicationElements() -> [AXElementWrapper] {
        let systemWideElement = AXUIElementCreateSystemWide()
        var value: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(
            systemWideElement,
            kAXFocusedApplicationAttribute as CFString,
            &value
        )
        // The system-wide element doesn't expose an application list directly.
        // Use NSRunningApplication to enumerate PIDs, then create per-app elements.
        guard result == .success, let appRef = value else {
            return []
        }
        // Return the single focused application as a fallback.
        // Full enumeration happens via NSWorkspace in higher-level code.
        return [AXElementWrapper(appRef as! AXUIElement)]
    }

    /// Creates a wrapper for a specific application by PID.
    public static func application(pid: pid_t) -> AXElementWrapper {
        AXElementWrapper(AXUIElementCreateApplication(pid))
    }

    // MARK: - Attribute helpers

    private func copyAttribute(_ attribute: String) -> CFTypeRef? {
        var value: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
        guard result == .success else { return nil }
        return value
    }

    private func stringAttribute(_ attribute: String) -> String? {
        guard let ref = copyAttribute(attribute) else { return nil }
        return ref as? String
    }

    private func boolAttribute(_ attribute: String) -> Bool {
        guard let ref = copyAttribute(attribute) else { return false }
        if let b = ref as? Bool { return b }
        // AXValue wrapping a CFBoolean
        if CFGetTypeID(ref) == CFBooleanGetTypeID() {
            return CFBooleanGetValue((ref as! CFBoolean))
        }
        return false
    }

    // MARK: - AccessibleElement conformance

    public func role() -> String? {
        stringAttribute(kAXRoleAttribute)
    }

    public func subrole() -> String? {
        stringAttribute(kAXSubroleAttribute)
    }

    public func label() -> String? {
        if let title = stringAttribute(kAXTitleAttribute), !title.isEmpty {
            return title
        }
        if let desc = stringAttribute(kAXDescriptionAttribute), !desc.isEmpty {
            return desc
        }
        // Placeholder text ("Enter URL", "Search…") disambiguates anonymous
        // text fields — commonly the only distinguishing text on NSTextField
        // instances inside NSStackView.
        return stringAttribute(kAXPlaceholderValueAttribute)
    }

    public func value() -> String? {
        guard let ref = copyAttribute(kAXValueAttribute) else { return nil }
        if let s = ref as? String { return s }
        return "\(ref)"
    }

    public func descriptionText() -> String? {
        if let help = stringAttribute(kAXHelpAttribute), !help.isEmpty {
            return help
        }
        return stringAttribute(kAXRoleDescriptionAttribute)
    }

    public func identifier() -> String? {
        stringAttribute(kAXIdentifierAttribute)
    }

    public func isEnabled() -> Bool {
        boolAttribute(kAXEnabledAttribute)
    }

    public func isFocused() -> Bool {
        boolAttribute(kAXFocusedAttribute)
    }

    public func position() -> CGPoint? {
        guard let ref = copyAttribute(kAXPositionAttribute) else { return nil }
        guard CFGetTypeID(ref) == AXValueGetTypeID() else { return nil }
        let axValue = ref as! AXValue
        var point = CGPoint.zero
        guard AXValueGetValue(axValue, .cgPoint, &point) else { return nil }
        return point
    }

    public func size() -> CGSize? {
        guard let ref = copyAttribute(kAXSizeAttribute) else { return nil }
        guard CFGetTypeID(ref) == AXValueGetTypeID() else { return nil }
        let axValue = ref as! AXValue
        var size = CGSize.zero
        guard AXValueGetValue(axValue, .cgSize, &size) else { return nil }
        return size
    }

    public func children() -> [any AccessibleElement] {
        guard let ref = copyAttribute(kAXChildrenAttribute) else { return [] }
        guard let array = ref as? [AXUIElement] else { return [] }
        return array.map { AXElementWrapper($0) }
    }

    public func actionNames() -> [String] {
        var names: CFArray?
        let result = AXUIElementCopyActionNames(element, &names)
        guard result == .success, let array = names as? [String] else { return [] }
        return array
    }

    public func performAction(_ name: String) throws {
        let result = AXUIElementPerformAction(element, name as CFString)
        guard result == .success else {
            throw AXWrapperError.actionFailed(action: name, axError: result.rawValue)
        }
    }

    public func setAttribute(_ name: String, value: Any) throws {
        let result = AXUIElementSetAttributeValue(
            element,
            name as CFString,
            value as CFTypeRef
        )
        guard result == .success else {
            throw AXWrapperError.setAttributeFailed(attribute: name, axError: result.rawValue)
        }
    }

    public func fontInfo() -> (family: String?, size: Double?, weight: String?)? {
        guard let ref = copyAttribute("AXFont") else { return nil }
        guard let dict = ref as? [String: Any] else { return nil }
        let family = dict["AXFontName"] as? String
        let size: Double?
        if let s = dict["AXFontSize"] as? Double {
            size = s
        } else if let s = dict["AXFontSize"] as? CGFloat {
            size = Double(s)
        } else if let s = dict["AXFontSize"] as? Float {
            size = Double(s)
        } else {
            size = nil
        }
        let weight = dict["AXFontWeight"].map { "\($0)" }
        return (family: family, size: size, weight: weight)
    }
}
