use crate::unified_role::UnifiedRole;

/// Maps macOS `AX*` roles (and optional subroles) to the cross-platform
/// `UnifiedRole` taxonomy.
///
/// Mirrors `RoleMapper` in
/// `cli/Sources/TestAnywareAgentProtocol/RoleMapper.swift`. The underlying
/// table is intentionally hand-written rather than table-driven from data
/// because it changes rarely and an exhaustive Swift-mirroring test catches
/// drift cheaply.
pub struct RoleMapper;

impl RoleMapper {
    /// Resolve `(role, subrole)` to a `UnifiedRole`, returning
    /// `UnifiedRole::Unknown` when the input is not in the table.
    pub fn map(role: &str, subrole: Option<&str>) -> UnifiedRole {
        if let Some(sub) = subrole {
            match (role, sub) {
                ("AXGroup", "AXApplicationDialog") | ("AXGroup", "AXSystemDialog") => {
                    return UnifiedRole::Dialog;
                }
                ("AXGroup", "AXDefinitionList") => return UnifiedRole::DescriptionList,
                ("AXGroup", "AXSectionListSubrole") => return UnifiedRole::List,
                ("AXGroup", "AXTabPanelSubrole") => return UnifiedRole::TabPanel,
                ("AXRadioButton", "AXTabButtonSubrole") => return UnifiedRole::Tab,
                _ => {}
            }
        }
        match role {
            // Widgets
            "AXButton" => UnifiedRole::Button,
            "AXCheckBox" => UnifiedRole::Checkbox,
            "AXRadioButton" => UnifiedRole::Radio,
            "AXTextField" => UnifiedRole::Textfield,
            "AXTextArea" => UnifiedRole::EditableText,
            "AXSlider" => UnifiedRole::Slider,
            "AXPopUpButton" | "AXComboBox" => UnifiedRole::ComboBox,
            "AXSwitch" => UnifiedRole::Switch,
            "AXColorWell" => UnifiedRole::ColorWell,
            "AXDateField" => UnifiedRole::DatePicker,
            "AXProgressIndicator" | "AXBusyIndicator" => UnifiedRole::ProgressIndicator,
            "AXIncrementor" => UnifiedRole::SpinButton,
            "AXDisclosureTriangle" => UnifiedRole::DisclosureTriangle,
            "AXLink" => UnifiedRole::Link,
            "AXList" => UnifiedRole::List,
            "AXOutline" => UnifiedRole::Tree,
            "AXRow" => UnifiedRole::Row,
            "AXBrowser" => UnifiedRole::TreeGrid,
            "AXColumn" => UnifiedRole::Column,
            "AXCell" => UnifiedRole::Cell,

            // Menus
            "AXMenuBar" => UnifiedRole::MenuBar,
            "AXMenu" => UnifiedRole::Menu,
            "AXMenuItem" | "AXMenuButton" | "AXMenuBarItem" => UnifiedRole::MenuItem,

            // Containers / structure
            "AXWindow" => UnifiedRole::Window,
            "AXSheet" | "AXDrawer" => UnifiedRole::Dialog,
            "AXGroup" => UnifiedRole::Group,
            "AXToolbar" | "AXRuler" => UnifiedRole::Toolbar,
            "AXScrollArea" => UnifiedRole::ScrollArea,
            "AXScrollBar" => UnifiedRole::ScrollBar,
            "AXSplitGroup" => UnifiedRole::Splitter,
            "AXSplitter" => UnifiedRole::Separator,
            "AXTabGroup" => UnifiedRole::TabList,
            "AXLayoutArea" | "AXMatte" => UnifiedRole::Region,

            // Web content
            "AXWebArea" => UnifiedRole::WebArea,

            // Content
            "AXStaticText" => UnifiedRole::Text,
            "AXImage" => UnifiedRole::Image,
            "AXHeading" => UnifiedRole::Heading,
            "AXTable" => UnifiedRole::Table,
            "AXValueIndicator" | "AXLevelIndicator" | "AXRelevanceIndicator" => UnifiedRole::Meter,
            "AXHelpTag" => UnifiedRole::Tooltip,
            "AXGrowArea" | "AXHandle" => UnifiedRole::Generic,

            _ => UnifiedRole::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_widgets() {
        assert_eq!(RoleMapper::map("AXButton", None), UnifiedRole::Button);
        assert_eq!(RoleMapper::map("AXTextField", None), UnifiedRole::Textfield);
        assert_eq!(RoleMapper::map("AXCheckBox", None), UnifiedRole::Checkbox);
        assert_eq!(RoleMapper::map("AXRadioButton", None), UnifiedRole::Radio);
        assert_eq!(RoleMapper::map("AXSlider", None), UnifiedRole::Slider);
        assert_eq!(RoleMapper::map("AXPopUpButton", None), UnifiedRole::ComboBox);
    }

    #[test]
    fn additional_widgets() {
        assert_eq!(RoleMapper::map("AXTextArea", None), UnifiedRole::EditableText);
        assert_eq!(RoleMapper::map("AXComboBox", None), UnifiedRole::ComboBox);
        assert_eq!(RoleMapper::map("AXSwitch", None), UnifiedRole::Switch);
        assert_eq!(RoleMapper::map("AXColorWell", None), UnifiedRole::ColorWell);
        assert_eq!(RoleMapper::map("AXDateField", None), UnifiedRole::DatePicker);
        assert_eq!(
            RoleMapper::map("AXProgressIndicator", None),
            UnifiedRole::ProgressIndicator
        );
        assert_eq!(RoleMapper::map("AXIncrementor", None), UnifiedRole::SpinButton);
        assert_eq!(
            RoleMapper::map("AXDisclosureTriangle", None),
            UnifiedRole::DisclosureTriangle
        );
        assert_eq!(RoleMapper::map("AXLink", None), UnifiedRole::Link);
        assert_eq!(RoleMapper::map("AXList", None), UnifiedRole::List);
        assert_eq!(RoleMapper::map("AXOutline", None), UnifiedRole::Tree);
        assert_eq!(RoleMapper::map("AXRow", None), UnifiedRole::Row);
        assert_eq!(RoleMapper::map("AXBrowser", None), UnifiedRole::TreeGrid);
        assert_eq!(RoleMapper::map("AXColumn", None), UnifiedRole::Column);
        assert_eq!(RoleMapper::map("AXCell", None), UnifiedRole::Cell);
    }

    #[test]
    fn containers() {
        assert_eq!(RoleMapper::map("AXWindow", None), UnifiedRole::Window);
        assert_eq!(RoleMapper::map("AXGroup", None), UnifiedRole::Group);
        assert_eq!(RoleMapper::map("AXToolbar", None), UnifiedRole::Toolbar);
        assert_eq!(RoleMapper::map("AXScrollArea", None), UnifiedRole::ScrollArea);
        assert_eq!(RoleMapper::map("AXSplitGroup", None), UnifiedRole::Splitter);
        assert_eq!(RoleMapper::map("AXSheet", None), UnifiedRole::Dialog);
        assert_eq!(RoleMapper::map("AXDrawer", None), UnifiedRole::Dialog);
        assert_eq!(RoleMapper::map("AXScrollBar", None), UnifiedRole::ScrollBar);
        assert_eq!(RoleMapper::map("AXSplitter", None), UnifiedRole::Separator);
        assert_eq!(RoleMapper::map("AXTabGroup", None), UnifiedRole::TabList);
        assert_eq!(RoleMapper::map("AXLayoutArea", None), UnifiedRole::Region);
        assert_eq!(RoleMapper::map("AXMatte", None), UnifiedRole::Region);
    }

    #[test]
    fn menus() {
        assert_eq!(RoleMapper::map("AXMenuBar", None), UnifiedRole::MenuBar);
        assert_eq!(RoleMapper::map("AXMenu", None), UnifiedRole::Menu);
        assert_eq!(RoleMapper::map("AXMenuItem", None), UnifiedRole::MenuItem);
        assert_eq!(RoleMapper::map("AXMenuButton", None), UnifiedRole::MenuItem);
        assert_eq!(RoleMapper::map("AXMenuBarItem", None), UnifiedRole::MenuItem);
    }

    #[test]
    fn content() {
        assert_eq!(RoleMapper::map("AXStaticText", None), UnifiedRole::Text);
        assert_eq!(RoleMapper::map("AXImage", None), UnifiedRole::Image);
        assert_eq!(RoleMapper::map("AXHeading", None), UnifiedRole::Heading);
        assert_eq!(RoleMapper::map("AXTable", None), UnifiedRole::Table);
        assert_eq!(RoleMapper::map("AXValueIndicator", None), UnifiedRole::Meter);
        assert_eq!(RoleMapper::map("AXLevelIndicator", None), UnifiedRole::Meter);
        assert_eq!(
            RoleMapper::map("AXBusyIndicator", None),
            UnifiedRole::ProgressIndicator
        );
        assert_eq!(RoleMapper::map("AXRelevanceIndicator", None), UnifiedRole::Meter);
        assert_eq!(RoleMapper::map("AXHelpTag", None), UnifiedRole::Tooltip);
        assert_eq!(RoleMapper::map("AXGrowArea", None), UnifiedRole::Generic);
        assert_eq!(RoleMapper::map("AXHandle", None), UnifiedRole::Generic);
        assert_eq!(RoleMapper::map("AXRuler", None), UnifiedRole::Toolbar);
    }

    #[test]
    fn subrole_dialog() {
        assert_eq!(
            RoleMapper::map("AXGroup", Some("AXApplicationDialog")),
            UnifiedRole::Dialog
        );
        assert_eq!(
            RoleMapper::map("AXGroup", Some("AXSystemDialog")),
            UnifiedRole::Dialog
        );
    }

    #[test]
    fn subrole_description_list() {
        assert_eq!(
            RoleMapper::map("AXGroup", Some("AXDefinitionList")),
            UnifiedRole::DescriptionList
        );
    }

    #[test]
    fn subrole_list() {
        assert_eq!(
            RoleMapper::map("AXGroup", Some("AXSectionListSubrole")),
            UnifiedRole::List
        );
    }

    #[test]
    fn subrole_tab_panel() {
        assert_eq!(
            RoleMapper::map("AXGroup", Some("AXTabPanelSubrole")),
            UnifiedRole::TabPanel
        );
    }

    #[test]
    fn radio_button_as_tab() {
        assert_eq!(
            RoleMapper::map("AXRadioButton", Some("AXTabButtonSubrole")),
            UnifiedRole::Tab
        );
    }

    #[test]
    fn group_default_without_subrole() {
        assert_eq!(RoleMapper::map("AXGroup", None), UnifiedRole::Group);
    }

    #[test]
    fn unknown_role() {
        assert_eq!(RoleMapper::map("AXSomethingNew", None), UnifiedRole::Unknown);
        assert_eq!(RoleMapper::map("", None), UnifiedRole::Unknown);
        assert_eq!(RoleMapper::map("NotAnAXRole", None), UnifiedRole::Unknown);
    }

    #[test]
    fn case_sensitivity() {
        // macOS AX roles are PascalCase — lowercase must not match.
        assert_eq!(RoleMapper::map("axbutton", None), UnifiedRole::Unknown);
        assert_eq!(RoleMapper::map("AXBUTTON", None), UnifiedRole::Unknown);
        assert_eq!(RoleMapper::map("axButton", None), UnifiedRole::Unknown);
        assert_eq!(RoleMapper::map("AXbutton", None), UnifiedRole::Unknown);
    }
}
