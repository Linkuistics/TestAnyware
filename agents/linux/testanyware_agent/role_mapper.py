"""Map AT-SPI2 / ATK role names to the UnifiedRole vocabulary.

pyatspi2 exposes roles as pyatspi.ROLE_* constants (integers) and
Accessible.getRoleName() returns a human-readable string like
"push button". We map from the role name string to keep the code
independent of pyatspi constant values.
"""

# ATK role name (from getRoleName()) -> UnifiedRole kebab-case string
_ROLE_TABLE: dict[str, str] = {
    # Interactive widgets
    "push button":           "button",
    "toggle button":         "toggle-button",
    "check box":             "checkbox",
    "check menu item":       "menu-item-checkbox",
    "radio button":          "radio",
    "radio menu item":       "menu-item-radio",
    "text":                  "editable-text",
    "password text":         "textfield",
    "spin button":           "spin-button",
    "slider":                "slider",
    "scroll bar":            "scroll-bar",
    "combo box":             "combo-box",
    "link":                  "link",
    "entry":                 "textfield",
    "color chooser":         "color-well",
    "date editor":           "date-picker",
    "progress bar":          "progress-indicator",
    "split pane":            "splitter",
    "separator":             "separator",
    "tree table":            "tree-grid",

    # Tab / page
    "page tab":              "tab",
    "page tab list":         "tab-list",

    # Menus
    "menu bar":              "menu-bar",
    "menu":                  "menu",
    "menu item":             "menu-item",
    "popup menu":            "menu",
    "tear off menu item":    "menu-item",

    # Containers / structure
    "frame":                 "window",
    "dialog":                "dialog",
    "alert":                 "alert",
    "file chooser":          "dialog",
    "font chooser":          "dialog",
    "option pane":           "dialog",
    "panel":                 "group",
    "filler":                "group",
    "glass pane":            "group",
    "layered pane":          "group",
    "root pane":             "group",
    "viewport":              "group",
    "internal frame":        "group",
    "desktop frame":         "group",
    "tool bar":              "toolbar",
    "status bar":            "status",
    "scroll pane":           "scroll-area",
    "table":                 "table",
    "table cell":            "cell",
    "table column header":   "column-header",
    "table row header":      "row-header",
    "tree":                  "tree",
    "tree item":             "tree-item",
    "list":                  "list",
    "list item":             "list-item",
    "row":                   "row",
    "column":                "column",
    "column header":         "column-header",
    "row header":            "row-header",
    "form":                  "form",
    "canvas":                "canvas",
    "application":           "application",
    "window":                "window",
    "document frame":        "document",
    "document web":          "web-area",
    "document text":         "document",
    "document spreadsheet":  "document",
    "document presentation": "document",
    "section":               "section",
    "redundant object":      "none",
    "grouping":              "group",
    "notification":          "notification",
    "article":               "region",
    "landmark":              "region",
    "log":                   "log",
    "timer":                 "timer",
    "definition":            "definition",
    "block quote":           "blockquote",
    "comment":               "note",
    "math":                  "math",
    "content deletion":      "content-deletion",
    "content insertion":     "content-insertion",
    "mark":                  "mark",
    "suggestion":            "suggestion",
    "description list":      "description-list",
    "description term":      "description-list-term",
    "description value":     "description-list-detail",
    "footnote":              "note",
    "superscript":           "superscript",
    "subscript":             "subscript",

    # Content
    "label":                 "text",
    "static":                "text",
    "caption":               "caption",
    "heading":               "heading",
    "icon":                  "image",
    "image":                 "image",
    "animation":             "image",
    "paragraph":             "paragraph",
    "ruler":                 "toolbar",
    "tool tip":              "tooltip",
    "info bar":              "status",
    "level bar":             "meter",
    "video":                 "video",
    "audio":                 "audio",

    # Catch-all ARIA
    "embedded":              "iframe",
}


def map_role(atk_role_name: str) -> str:
    """Map an ATK role name string to a UnifiedRole kebab-case string."""
    return _ROLE_TABLE.get(atk_role_name, "unknown")


def map_from_string(role_string: str) -> str:
    """Accept a UnifiedRole string (kebab-case) and return it normalized.

    Also accepts camelCase input for convenience (e.g., "comboBox" -> "combo-box").
    """
    if not role_string:
        return "unknown"
    # Already kebab-case
    if "-" in role_string:
        return role_string
    # Convert camelCase to kebab-case
    result = []
    for char in role_string:
        if char.isupper() and result:
            result.append("-")
        result.append(char.lower())
    return "".join(result)
