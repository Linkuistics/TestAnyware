import Testing
import TestAnywareAgentProtocol

// MARK: - Widget mappings

@Test func roleMapperBasicWidgets() {
    #expect(RoleMapper.map(role: "AXButton") == .button)
    #expect(RoleMapper.map(role: "AXTextField") == .textfield)
    #expect(RoleMapper.map(role: "AXCheckBox") == .checkbox)
    #expect(RoleMapper.map(role: "AXRadioButton") == .radio)
    #expect(RoleMapper.map(role: "AXSlider") == .slider)
    #expect(RoleMapper.map(role: "AXPopUpButton") == .comboBox)
}

@Test func roleMapperAdditionalWidgets() {
    #expect(RoleMapper.map(role: "AXTextArea") == .editableText)
    #expect(RoleMapper.map(role: "AXComboBox") == .comboBox)
    #expect(RoleMapper.map(role: "AXSwitch") == .switch)
    #expect(RoleMapper.map(role: "AXColorWell") == .colorWell)
    #expect(RoleMapper.map(role: "AXDateField") == .datePicker)
    #expect(RoleMapper.map(role: "AXProgressIndicator") == .progressIndicator)
    #expect(RoleMapper.map(role: "AXIncrementor") == .spinButton)
    #expect(RoleMapper.map(role: "AXDisclosureTriangle") == .disclosureTriangle)
    #expect(RoleMapper.map(role: "AXLink") == .link)
    #expect(RoleMapper.map(role: "AXList") == .list)
    #expect(RoleMapper.map(role: "AXOutline") == .tree)
    #expect(RoleMapper.map(role: "AXRow") == .row)
    #expect(RoleMapper.map(role: "AXBrowser") == .treeGrid)
    #expect(RoleMapper.map(role: "AXColumn") == .column)
    #expect(RoleMapper.map(role: "AXCell") == .cell)
}

// MARK: - Container mappings

@Test func roleMapperContainers() {
    #expect(RoleMapper.map(role: "AXWindow") == .window)
    #expect(RoleMapper.map(role: "AXGroup") == .group)
    #expect(RoleMapper.map(role: "AXToolbar") == .toolbar)
    #expect(RoleMapper.map(role: "AXScrollArea") == .scrollArea)
    #expect(RoleMapper.map(role: "AXSplitGroup") == .splitter)
    #expect(RoleMapper.map(role: "AXSheet") == .dialog)
    #expect(RoleMapper.map(role: "AXDrawer") == .dialog)
    #expect(RoleMapper.map(role: "AXScrollBar") == .scrollBar)
    #expect(RoleMapper.map(role: "AXSplitter") == .separator)
    #expect(RoleMapper.map(role: "AXTabGroup") == .tabList)
    #expect(RoleMapper.map(role: "AXLayoutArea") == .region)
    #expect(RoleMapper.map(role: "AXMatte") == .region)
}

// MARK: - Menu mappings

@Test func roleMapperMenus() {
    #expect(RoleMapper.map(role: "AXMenuBar") == .menuBar)
    #expect(RoleMapper.map(role: "AXMenu") == .menu)
    #expect(RoleMapper.map(role: "AXMenuItem") == .menuItem)
    #expect(RoleMapper.map(role: "AXMenuButton") == .menuItem)
    #expect(RoleMapper.map(role: "AXMenuBarItem") == .menuItem)
}

// MARK: - Content mappings

@Test func roleMapperContent() {
    #expect(RoleMapper.map(role: "AXStaticText") == .text)
    #expect(RoleMapper.map(role: "AXImage") == .image)
    #expect(RoleMapper.map(role: "AXHeading") == .heading)
    #expect(RoleMapper.map(role: "AXTable") == .table)
    #expect(RoleMapper.map(role: "AXValueIndicator") == .meter)
    #expect(RoleMapper.map(role: "AXLevelIndicator") == .meter)
    #expect(RoleMapper.map(role: "AXBusyIndicator") == .progressIndicator)
    #expect(RoleMapper.map(role: "AXRelevanceIndicator") == .meter)
    #expect(RoleMapper.map(role: "AXHelpTag") == .tooltip)
    #expect(RoleMapper.map(role: "AXGrowArea") == .generic)
    #expect(RoleMapper.map(role: "AXHandle") == .generic)
    #expect(RoleMapper.map(role: "AXRuler") == .toolbar)
}

// MARK: - Subrole disambiguation

@Test func roleMapperSubroleDialog() {
    #expect(RoleMapper.map(role: "AXGroup", subrole: "AXApplicationDialog") == .dialog)
    #expect(RoleMapper.map(role: "AXGroup", subrole: "AXSystemDialog") == .dialog)
}

@Test func roleMapperSubroleDescriptionList() {
    #expect(RoleMapper.map(role: "AXGroup", subrole: "AXDefinitionList") == .descriptionList)
}

@Test func roleMapperSubroleList() {
    #expect(RoleMapper.map(role: "AXGroup", subrole: "AXSectionListSubrole") == .list)
}

@Test func roleMapperSubroleTabPanel() {
    #expect(RoleMapper.map(role: "AXGroup", subrole: "AXTabPanelSubrole") == .tabPanel)
}

@Test func roleMapperRadioButtonAsTab() {
    #expect(RoleMapper.map(role: "AXRadioButton", subrole: "AXTabButtonSubrole") == .tab)
}

@Test func roleMapperGroupDefaultWithoutSubrole() {
    #expect(RoleMapper.map(role: "AXGroup") == .group)
    #expect(RoleMapper.map(role: "AXGroup", subrole: nil) == .group)
}

// MARK: - Unknown role

@Test func roleMapperUnknownRole() {
    #expect(RoleMapper.map(role: "AXSomethingNew") == .unknown)
    #expect(RoleMapper.map(role: "") == .unknown)
    #expect(RoleMapper.map(role: "NotAnAXRole") == .unknown)
}

// MARK: - Case sensitivity

@Test func roleMapperCaseSensitivity() {
    // macOS AX roles are PascalCase — lowercase should not match
    #expect(RoleMapper.map(role: "axbutton") == .unknown)
    #expect(RoleMapper.map(role: "AXBUTTON") == .unknown)
    #expect(RoleMapper.map(role: "axButton") == .unknown)
    #expect(RoleMapper.map(role: "AXbutton") == .unknown)
}
