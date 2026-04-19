public enum UnifiedRole: String, Codable, Sendable, CaseIterable, Equatable {

    // MARK: - Interactive widgets

    case button
    case checkbox
    case colorWell = "color-well"
    case comboBox = "combo-box"
    case datePicker = "date-picker"
    case disclosureTriangle = "disclosure-triangle"
    case editableText = "editable-text"
    case grid
    case gridCell = "grid-cell"
    case group
    case image
    case inlineTextBox = "inline-text-box"
    case inputTime = "input-time"
    case link
    case listBox = "list-box"
    case listBoxOption = "list-box-option"
    case listGrid = "list-grid"
    case listMarker = "list-marker"
    case meter
    case progressIndicator = "progress-indicator"
    case radio
    case radioGroup = "radio-group"
    case scrollBar = "scroll-bar"
    case search
    case searchInput = "search-input"
    case slider
    case spinButton = "spin-button"
    case splitter
    case `switch`
    case tab
    case tabList = "tab-list"
    case tabPanel = "tab-panel"
    case textfield
    case toggleButton = "toggle-button"
    case tree
    case treeItem = "tree-item"
    case treeGrid = "tree-grid"
    case splitButton = "split-button"

    // MARK: - Menus

    case menu
    case menuBar = "menu-bar"
    case menuItem = "menu-item"
    case menuItemCheckbox = "menu-item-checkbox"
    case menuItemRadio = "menu-item-radio"
    case menuListOption = "menu-list-option"
    case menuListPopup = "menu-list-popup"

    // MARK: - Containers / structure

    case alert
    case alertDialog = "alert-dialog"
    case application
    case banner
    case blockquote
    case cell
    case code
    case column
    case columnHeader = "column-header"
    case complementary
    case contentDeletion = "content-deletion"
    case contentInsertion = "content-insertion"
    case contentInfo = "content-info"
    case definition
    case descriptionList = "description-list"
    case descriptionListDetail = "description-list-detail"
    case descriptionListTerm = "description-list-term"
    case dialog
    case directory
    case document
    case feed
    case figure
    case footer
    case footerAsNonLandmark = "footer-as-non-landmark"
    case form
    case generic
    case headerAsNonLandmark = "header-as-non-landmark"
    case main
    case mark
    case math
    case mathExpression = "math-expression"
    case navigation
    case none
    case note
    case paragraph
    case pluginObject = "plugin-object"
    case region
    case row
    case rowGroup = "row-group"
    case rowHeader = "row-header"
    case ruby
    case rubyAnnotation = "ruby-annotation"
    case scrollArea = "scroll-area"
    case section
    case separator
    case status
    case strong
    case `subscript`
    case suggestion
    case superscript
    case table
    case term
    case time
    case timer
    case toolbar
    case tooltip
    case window

    // MARK: - Content

    case abbreviation
    case audio
    case canvas
    case caption
    case emphasis
    case graphicsDocument = "graphics-document"
    case graphicsObject = "graphics-object"
    case graphicsSymbol = "graphics-symbol"
    case heading
    case iframe
    case iframePresentational = "iframe-presentational"
    case img
    case labelText = "label-text"
    case legend
    case lineBreak = "line-break"
    case list
    case listItem = "list-item"
    case log
    case marquee
    case pdfActionableHighlight = "pdf-actionable-highlight"
    case pdfRoot = "pdf-root"
    case presentation
    case text
    case textRun = "text-run"
    case video
    case webArea = "web-area"
    case wordBreak = "word-break"

    // MARK: - Transient surfaces

    case popover
    case notification
    case toast

    // MARK: - Catch-all

    case unknown
}
