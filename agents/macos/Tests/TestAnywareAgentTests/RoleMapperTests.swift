import Testing
import TestAnywareAgentProtocol

// MARK: - Wire role queries

@Test func queryRoleReadsAUnifiedName() {
    #expect(RoleMapper.queryRole("button") == .button)
    #expect(RoleMapper.queryRole("editable-text") == .editableText)
}

@Test func queryRoleStillAcceptsAPlatformName() {
    #expect(RoleMapper.queryRole("AXButton") == .button)
    #expect(RoleMapper.queryRole("AXTextArea") == .editableText)
}

@Test func queryRoleFiltersOnUnknownForAnUnrecognisedName() {
    #expect(RoleMapper.queryRole("not-a-role") == .unknown)
}

@Test func everyUnifiedNameResolvesToItself() {
    for role in UnifiedRole.allCases {
        #expect(RoleMapper.queryRole(role.rawValue) == role)
    }
}
