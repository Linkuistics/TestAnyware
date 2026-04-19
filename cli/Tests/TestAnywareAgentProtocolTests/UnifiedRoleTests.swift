import Testing
import Foundation
import TestAnywareAgentProtocol

@Test func unifiedRoleRoundTripCodable() throws {
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let roles: [UnifiedRole] = [.button, .textfield, .window, .dialog, .menuItem, .unknown]
    for role in roles {
        let data = try encoder.encode(role)
        let decoded = try decoder.decode(UnifiedRole.self, from: data)
        #expect(decoded == role)
    }
}

@Test func unifiedRoleRawValues() {
    #expect(UnifiedRole.button.rawValue == "button")
    #expect(UnifiedRole.textfield.rawValue == "textfield")
    #expect(UnifiedRole.window.rawValue == "window")
    #expect(UnifiedRole.dialog.rawValue == "dialog")
    #expect(UnifiedRole.menuItem.rawValue == "menu-item")
    #expect(UnifiedRole.unknown.rawValue == "unknown")
    #expect(UnifiedRole.splitButton.rawValue == "split-button")
    #expect(UnifiedRole.colorWell.rawValue == "color-well")
    #expect(UnifiedRole.comboBox.rawValue == "combo-box")
}

@Test func unifiedRoleUnknownCatchAll() {
    let role = UnifiedRole.unknown
    #expect(role.rawValue == "unknown")
    let decoded = UnifiedRole(rawValue: "not-a-real-role")
    #expect(decoded == nil)
}

@Test func unifiedRoleCaseIterableCount() {
    #expect(UnifiedRole.allCases.count == 133)
}
