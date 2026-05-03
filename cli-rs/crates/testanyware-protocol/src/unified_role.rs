use serde::{Deserialize, Serialize};

/// Cross-platform accessibility role taxonomy.
///
/// Mirrors `UnifiedRole` in `cli/Sources/TestAnywareAgentProtocol/UnifiedRole.swift`.
/// Multi-word variants serialise as kebab-case (`menu-item`, `combo-box`);
/// single-word variants stay as one word (`button`, `textfield`). The
/// kebab-case variants are explicitly renamed because Swift's source order
/// and naming conventions don't map cleanly onto `serde`'s rename-all.
///
/// Adding a variant here must be mirrored in the Swift enum *and* in any
/// `RoleMapper` table that produces the new role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnifiedRole {
    // MARK: - Interactive widgets
    #[serde(rename = "button")]
    Button,
    #[serde(rename = "checkbox")]
    Checkbox,
    #[serde(rename = "color-well")]
    ColorWell,
    #[serde(rename = "combo-box")]
    ComboBox,
    #[serde(rename = "date-picker")]
    DatePicker,
    #[serde(rename = "disclosure-triangle")]
    DisclosureTriangle,
    #[serde(rename = "editable-text")]
    EditableText,
    #[serde(rename = "grid")]
    Grid,
    #[serde(rename = "grid-cell")]
    GridCell,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "inline-text-box")]
    InlineTextBox,
    #[serde(rename = "input-time")]
    InputTime,
    #[serde(rename = "link")]
    Link,
    #[serde(rename = "list-box")]
    ListBox,
    #[serde(rename = "list-box-option")]
    ListBoxOption,
    #[serde(rename = "list-grid")]
    ListGrid,
    #[serde(rename = "list-marker")]
    ListMarker,
    #[serde(rename = "meter")]
    Meter,
    #[serde(rename = "progress-indicator")]
    ProgressIndicator,
    #[serde(rename = "radio")]
    Radio,
    #[serde(rename = "radio-group")]
    RadioGroup,
    #[serde(rename = "scroll-bar")]
    ScrollBar,
    #[serde(rename = "search")]
    Search,
    #[serde(rename = "search-input")]
    SearchInput,
    #[serde(rename = "slider")]
    Slider,
    #[serde(rename = "spin-button")]
    SpinButton,
    #[serde(rename = "splitter")]
    Splitter,
    #[serde(rename = "switch")]
    Switch,
    #[serde(rename = "tab")]
    Tab,
    #[serde(rename = "tab-list")]
    TabList,
    #[serde(rename = "tab-panel")]
    TabPanel,
    #[serde(rename = "textfield")]
    Textfield,
    #[serde(rename = "toggle-button")]
    ToggleButton,
    #[serde(rename = "tree")]
    Tree,
    #[serde(rename = "tree-item")]
    TreeItem,
    #[serde(rename = "tree-grid")]
    TreeGrid,
    #[serde(rename = "split-button")]
    SplitButton,

    // MARK: - Menus
    #[serde(rename = "menu")]
    Menu,
    #[serde(rename = "menu-bar")]
    MenuBar,
    #[serde(rename = "menu-item")]
    MenuItem,
    #[serde(rename = "menu-item-checkbox")]
    MenuItemCheckbox,
    #[serde(rename = "menu-item-radio")]
    MenuItemRadio,
    #[serde(rename = "menu-list-option")]
    MenuListOption,
    #[serde(rename = "menu-list-popup")]
    MenuListPopup,

    // MARK: - Containers / structure
    #[serde(rename = "alert")]
    Alert,
    #[serde(rename = "alert-dialog")]
    AlertDialog,
    #[serde(rename = "application")]
    Application,
    #[serde(rename = "banner")]
    Banner,
    #[serde(rename = "blockquote")]
    Blockquote,
    #[serde(rename = "cell")]
    Cell,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "column")]
    Column,
    #[serde(rename = "column-header")]
    ColumnHeader,
    #[serde(rename = "complementary")]
    Complementary,
    #[serde(rename = "content-deletion")]
    ContentDeletion,
    #[serde(rename = "content-insertion")]
    ContentInsertion,
    #[serde(rename = "content-info")]
    ContentInfo,
    #[serde(rename = "definition")]
    Definition,
    #[serde(rename = "description-list")]
    DescriptionList,
    #[serde(rename = "description-list-detail")]
    DescriptionListDetail,
    #[serde(rename = "description-list-term")]
    DescriptionListTerm,
    #[serde(rename = "dialog")]
    Dialog,
    #[serde(rename = "directory")]
    Directory,
    #[serde(rename = "document")]
    Document,
    #[serde(rename = "feed")]
    Feed,
    #[serde(rename = "figure")]
    Figure,
    #[serde(rename = "footer")]
    Footer,
    #[serde(rename = "footer-as-non-landmark")]
    FooterAsNonLandmark,
    #[serde(rename = "form")]
    Form,
    #[serde(rename = "generic")]
    Generic,
    #[serde(rename = "header-as-non-landmark")]
    HeaderAsNonLandmark,
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "mark")]
    Mark,
    #[serde(rename = "math")]
    Math,
    #[serde(rename = "math-expression")]
    MathExpression,
    #[serde(rename = "navigation")]
    Navigation,
    #[serde(rename = "none")]
    None_,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "paragraph")]
    Paragraph,
    #[serde(rename = "plugin-object")]
    PluginObject,
    #[serde(rename = "region")]
    Region,
    #[serde(rename = "row")]
    Row,
    #[serde(rename = "row-group")]
    RowGroup,
    #[serde(rename = "row-header")]
    RowHeader,
    #[serde(rename = "ruby")]
    Ruby,
    #[serde(rename = "ruby-annotation")]
    RubyAnnotation,
    #[serde(rename = "scroll-area")]
    ScrollArea,
    #[serde(rename = "section")]
    Section,
    #[serde(rename = "separator")]
    Separator,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "strong")]
    Strong,
    #[serde(rename = "subscript")]
    Subscript,
    #[serde(rename = "suggestion")]
    Suggestion,
    #[serde(rename = "superscript")]
    Superscript,
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "term")]
    Term,
    #[serde(rename = "time")]
    Time,
    #[serde(rename = "timer")]
    Timer,
    #[serde(rename = "toolbar")]
    Toolbar,
    #[serde(rename = "tooltip")]
    Tooltip,
    #[serde(rename = "window")]
    Window,

    // MARK: - Content
    #[serde(rename = "abbreviation")]
    Abbreviation,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "canvas")]
    Canvas,
    #[serde(rename = "caption")]
    Caption,
    #[serde(rename = "emphasis")]
    Emphasis,
    #[serde(rename = "graphics-document")]
    GraphicsDocument,
    #[serde(rename = "graphics-object")]
    GraphicsObject,
    #[serde(rename = "graphics-symbol")]
    GraphicsSymbol,
    #[serde(rename = "heading")]
    Heading,
    #[serde(rename = "iframe")]
    Iframe,
    #[serde(rename = "iframe-presentational")]
    IframePresentational,
    #[serde(rename = "img")]
    Img,
    #[serde(rename = "label-text")]
    LabelText,
    #[serde(rename = "legend")]
    Legend,
    #[serde(rename = "line-break")]
    LineBreak,
    #[serde(rename = "list")]
    List,
    #[serde(rename = "list-item")]
    ListItem,
    #[serde(rename = "log")]
    Log,
    #[serde(rename = "marquee")]
    Marquee,
    #[serde(rename = "pdf-actionable-highlight")]
    PdfActionableHighlight,
    #[serde(rename = "pdf-root")]
    PdfRoot,
    #[serde(rename = "presentation")]
    Presentation,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "text-run")]
    TextRun,
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "web-area")]
    WebArea,
    #[serde(rename = "word-break")]
    WordBreak,

    // MARK: - Transient surfaces
    #[serde(rename = "popover")]
    Popover,
    #[serde(rename = "notification")]
    Notification,
    #[serde(rename = "toast")]
    Toast,

    // MARK: - Catch-all
    #[serde(rename = "unknown")]
    Unknown,
}

impl UnifiedRole {
    /// Returns the wire-format string (the same value used in JSON).
    ///
    /// This is the inverse of the `#[serde(rename = ...)]` attributes; both
    /// must be kept in sync. The role-mapper test crate exercises both
    /// directions to catch drift.
    pub fn as_wire_str(self) -> &'static str {
        match self {
            UnifiedRole::Button => "button",
            UnifiedRole::Checkbox => "checkbox",
            UnifiedRole::ColorWell => "color-well",
            UnifiedRole::ComboBox => "combo-box",
            UnifiedRole::DatePicker => "date-picker",
            UnifiedRole::DisclosureTriangle => "disclosure-triangle",
            UnifiedRole::EditableText => "editable-text",
            UnifiedRole::Grid => "grid",
            UnifiedRole::GridCell => "grid-cell",
            UnifiedRole::Group => "group",
            UnifiedRole::Image => "image",
            UnifiedRole::InlineTextBox => "inline-text-box",
            UnifiedRole::InputTime => "input-time",
            UnifiedRole::Link => "link",
            UnifiedRole::ListBox => "list-box",
            UnifiedRole::ListBoxOption => "list-box-option",
            UnifiedRole::ListGrid => "list-grid",
            UnifiedRole::ListMarker => "list-marker",
            UnifiedRole::Meter => "meter",
            UnifiedRole::ProgressIndicator => "progress-indicator",
            UnifiedRole::Radio => "radio",
            UnifiedRole::RadioGroup => "radio-group",
            UnifiedRole::ScrollBar => "scroll-bar",
            UnifiedRole::Search => "search",
            UnifiedRole::SearchInput => "search-input",
            UnifiedRole::Slider => "slider",
            UnifiedRole::SpinButton => "spin-button",
            UnifiedRole::Splitter => "splitter",
            UnifiedRole::Switch => "switch",
            UnifiedRole::Tab => "tab",
            UnifiedRole::TabList => "tab-list",
            UnifiedRole::TabPanel => "tab-panel",
            UnifiedRole::Textfield => "textfield",
            UnifiedRole::ToggleButton => "toggle-button",
            UnifiedRole::Tree => "tree",
            UnifiedRole::TreeItem => "tree-item",
            UnifiedRole::TreeGrid => "tree-grid",
            UnifiedRole::SplitButton => "split-button",
            UnifiedRole::Menu => "menu",
            UnifiedRole::MenuBar => "menu-bar",
            UnifiedRole::MenuItem => "menu-item",
            UnifiedRole::MenuItemCheckbox => "menu-item-checkbox",
            UnifiedRole::MenuItemRadio => "menu-item-radio",
            UnifiedRole::MenuListOption => "menu-list-option",
            UnifiedRole::MenuListPopup => "menu-list-popup",
            UnifiedRole::Alert => "alert",
            UnifiedRole::AlertDialog => "alert-dialog",
            UnifiedRole::Application => "application",
            UnifiedRole::Banner => "banner",
            UnifiedRole::Blockquote => "blockquote",
            UnifiedRole::Cell => "cell",
            UnifiedRole::Code => "code",
            UnifiedRole::Column => "column",
            UnifiedRole::ColumnHeader => "column-header",
            UnifiedRole::Complementary => "complementary",
            UnifiedRole::ContentDeletion => "content-deletion",
            UnifiedRole::ContentInsertion => "content-insertion",
            UnifiedRole::ContentInfo => "content-info",
            UnifiedRole::Definition => "definition",
            UnifiedRole::DescriptionList => "description-list",
            UnifiedRole::DescriptionListDetail => "description-list-detail",
            UnifiedRole::DescriptionListTerm => "description-list-term",
            UnifiedRole::Dialog => "dialog",
            UnifiedRole::Directory => "directory",
            UnifiedRole::Document => "document",
            UnifiedRole::Feed => "feed",
            UnifiedRole::Figure => "figure",
            UnifiedRole::Footer => "footer",
            UnifiedRole::FooterAsNonLandmark => "footer-as-non-landmark",
            UnifiedRole::Form => "form",
            UnifiedRole::Generic => "generic",
            UnifiedRole::HeaderAsNonLandmark => "header-as-non-landmark",
            UnifiedRole::Main => "main",
            UnifiedRole::Mark => "mark",
            UnifiedRole::Math => "math",
            UnifiedRole::MathExpression => "math-expression",
            UnifiedRole::Navigation => "navigation",
            UnifiedRole::None_ => "none",
            UnifiedRole::Note => "note",
            UnifiedRole::Paragraph => "paragraph",
            UnifiedRole::PluginObject => "plugin-object",
            UnifiedRole::Region => "region",
            UnifiedRole::Row => "row",
            UnifiedRole::RowGroup => "row-group",
            UnifiedRole::RowHeader => "row-header",
            UnifiedRole::Ruby => "ruby",
            UnifiedRole::RubyAnnotation => "ruby-annotation",
            UnifiedRole::ScrollArea => "scroll-area",
            UnifiedRole::Section => "section",
            UnifiedRole::Separator => "separator",
            UnifiedRole::Status => "status",
            UnifiedRole::Strong => "strong",
            UnifiedRole::Subscript => "subscript",
            UnifiedRole::Suggestion => "suggestion",
            UnifiedRole::Superscript => "superscript",
            UnifiedRole::Table => "table",
            UnifiedRole::Term => "term",
            UnifiedRole::Time => "time",
            UnifiedRole::Timer => "timer",
            UnifiedRole::Toolbar => "toolbar",
            UnifiedRole::Tooltip => "tooltip",
            UnifiedRole::Window => "window",
            UnifiedRole::Abbreviation => "abbreviation",
            UnifiedRole::Audio => "audio",
            UnifiedRole::Canvas => "canvas",
            UnifiedRole::Caption => "caption",
            UnifiedRole::Emphasis => "emphasis",
            UnifiedRole::GraphicsDocument => "graphics-document",
            UnifiedRole::GraphicsObject => "graphics-object",
            UnifiedRole::GraphicsSymbol => "graphics-symbol",
            UnifiedRole::Heading => "heading",
            UnifiedRole::Iframe => "iframe",
            UnifiedRole::IframePresentational => "iframe-presentational",
            UnifiedRole::Img => "img",
            UnifiedRole::LabelText => "label-text",
            UnifiedRole::Legend => "legend",
            UnifiedRole::LineBreak => "line-break",
            UnifiedRole::List => "list",
            UnifiedRole::ListItem => "list-item",
            UnifiedRole::Log => "log",
            UnifiedRole::Marquee => "marquee",
            UnifiedRole::PdfActionableHighlight => "pdf-actionable-highlight",
            UnifiedRole::PdfRoot => "pdf-root",
            UnifiedRole::Presentation => "presentation",
            UnifiedRole::Text => "text",
            UnifiedRole::TextRun => "text-run",
            UnifiedRole::Video => "video",
            UnifiedRole::WebArea => "web-area",
            UnifiedRole::WordBreak => "word-break",
            UnifiedRole::Popover => "popover",
            UnifiedRole::Notification => "notification",
            UnifiedRole::Toast => "toast",
            UnifiedRole::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_codable() {
        for role in [
            UnifiedRole::Button,
            UnifiedRole::Textfield,
            UnifiedRole::Window,
            UnifiedRole::Dialog,
            UnifiedRole::MenuItem,
            UnifiedRole::Unknown,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: UnifiedRole = serde_json::from_str(&json).unwrap();
            assert_eq!(back, role);
        }
    }

    #[test]
    fn raw_values() {
        let cases = [
            (UnifiedRole::Button, "\"button\""),
            (UnifiedRole::Textfield, "\"textfield\""),
            (UnifiedRole::Window, "\"window\""),
            (UnifiedRole::Dialog, "\"dialog\""),
            (UnifiedRole::MenuItem, "\"menu-item\""),
            (UnifiedRole::Unknown, "\"unknown\""),
            (UnifiedRole::SplitButton, "\"split-button\""),
            (UnifiedRole::ColorWell, "\"color-well\""),
            (UnifiedRole::ComboBox, "\"combo-box\""),
        ];
        for (role, expected) in cases {
            assert_eq!(serde_json::to_string(&role).unwrap(), expected);
        }
    }

    #[test]
    fn unknown_role_string_does_not_decode() {
        let result: Result<UnifiedRole, _> = serde_json::from_str("\"not-a-real-role\"");
        assert!(result.is_err());
    }
}
