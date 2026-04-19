import CoreGraphics
import TestAnywareAgent

/// A configurable mock for building fake accessibility trees in tests.
struct MockAccessibleElement: AccessibleElement, Sendable {
    var roleValue: String?
    var subroleValue: String?
    var labelValue: String?
    var valueValue: String?
    var descriptionValue: String?
    var identifierValue: String?
    var enabledValue: Bool
    var focusedValue: Bool
    var positionValue: CGPoint?
    var sizeValue: CGSize?
    var childElements: [MockAccessibleElement]
    var actionNamesValue: [String]

    init(
        role: String? = nil,
        subrole: String? = nil,
        label: String? = nil,
        value: String? = nil,
        description: String? = nil,
        identifier: String? = nil,
        enabled: Bool = true,
        focused: Bool = false,
        position: CGPoint? = nil,
        size: CGSize? = nil,
        children: [MockAccessibleElement] = [],
        actionNames: [String] = []
    ) {
        self.roleValue = role
        self.subroleValue = subrole
        self.labelValue = label
        self.valueValue = value
        self.descriptionValue = description
        self.identifierValue = identifier
        self.enabledValue = enabled
        self.focusedValue = focused
        self.positionValue = position
        self.sizeValue = size
        self.childElements = children
        self.actionNamesValue = actionNames
    }

    func role() -> String? { roleValue }
    func subrole() -> String? { subroleValue }
    func label() -> String? { labelValue }
    func value() -> String? { valueValue }
    func descriptionText() -> String? { descriptionValue }
    func identifier() -> String? { identifierValue }
    func isEnabled() -> Bool { enabledValue }
    func isFocused() -> Bool { focusedValue }
    func position() -> CGPoint? { positionValue }
    func size() -> CGSize? { sizeValue }
    func children() -> [any AccessibleElement] { childElements }
    func actionNames() -> [String] { actionNamesValue }
    func performAction(_ name: String) throws {}
    func setAttribute(_ name: String, value: Any) throws {}
    func fontInfo() -> (family: String?, size: Double?, weight: String?)? { nil }
}
