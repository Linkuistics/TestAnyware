public struct RoleMapper {

    private static let roleTable: [String: UnifiedRole] = [
        // Widgets
        "AXButton":             .button,
        "AXCheckBox":           .checkbox,
        "AXRadioButton":        .radio,
        "AXTextField":          .textfield,
        "AXTextArea":           .editableText,
        "AXSlider":             .slider,
        "AXPopUpButton":        .comboBox,
        "AXComboBox":           .comboBox,
        "AXSwitch":             .switch,
        "AXColorWell":          .colorWell,
        "AXDateField":          .datePicker,
        "AXProgressIndicator":  .progressIndicator,
        "AXIncrementor":        .spinButton,
        "AXDisclosureTriangle": .disclosureTriangle,
        "AXLink":               .link,
        "AXList":               .list,
        "AXOutline":            .tree,
        "AXRow":                .row,
        "AXBrowser":            .treeGrid,
        "AXColumn":             .column,
        "AXCell":               .cell,

        // Menus
        "AXMenuBar":            .menuBar,
        "AXMenu":               .menu,
        "AXMenuItem":           .menuItem,
        "AXMenuButton":         .menuItem,
        "AXMenuBarItem":        .menuItem,

        // Containers / structure
        "AXWindow":             .window,
        "AXSheet":              .dialog,
        "AXDrawer":             .dialog,
        "AXGroup":              .group,
        "AXToolbar":            .toolbar,
        "AXScrollArea":         .scrollArea,
        "AXScrollBar":          .scrollBar,
        "AXSplitGroup":         .splitter,
        "AXSplitter":           .separator,
        "AXTabGroup":           .tabList,
        "AXLayoutArea":         .region,
        "AXMatte":              .region,

        // Content
        "AXStaticText":         .text,
        "AXImage":              .image,
        "AXHeading":            .heading,
        "AXTable":              .table,
        "AXValueIndicator":     .meter,
        "AXLevelIndicator":     .meter,
        "AXBusyIndicator":      .progressIndicator,
        "AXRelevanceIndicator": .meter,
        "AXHelpTag":            .tooltip,
        "AXGrowArea":           .generic,
        "AXHandle":             .generic,
        "AXRuler":              .toolbar,
    ]

    public static func map(role: String, subrole: String? = nil) -> UnifiedRole {
        if let subrole {
            switch (role, subrole) {
            case ("AXGroup", "AXApplicationDialog"), ("AXGroup", "AXSystemDialog"):
                return .dialog
            case ("AXGroup", "AXDefinitionList"):
                return .descriptionList
            case ("AXGroup", "AXSectionListSubrole"):
                return .list
            case ("AXGroup", "AXTabPanelSubrole"):
                return .tabPanel
            case ("AXRadioButton", "AXTabButtonSubrole"):
                return .tab
            default:
                break
            }
        }
        return roleTable[role] ?? .unknown
    }
}
