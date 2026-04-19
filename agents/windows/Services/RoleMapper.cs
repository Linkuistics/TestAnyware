using FlaUI.Core.Definitions;
using TestAnywareAgent.Models;

namespace TestAnywareAgent.Services;

public static class RoleMapper
{
    private static readonly Dictionary<ControlType, UnifiedRole> ControlTypeMap = new()
    {
        // Interactive widgets
        [ControlType.Button] = UnifiedRole.Button,
        [ControlType.CheckBox] = UnifiedRole.Checkbox,
        [ControlType.ComboBox] = UnifiedRole.ComboBox,
        [ControlType.Edit] = UnifiedRole.Textfield,
        [ControlType.Hyperlink] = UnifiedRole.Link,
        [ControlType.Slider] = UnifiedRole.Slider,
        [ControlType.Spinner] = UnifiedRole.SpinButton,
        [ControlType.SplitButton] = UnifiedRole.SplitButton,
        [ControlType.RadioButton] = UnifiedRole.Radio,
        [ControlType.ProgressBar] = UnifiedRole.ProgressIndicator,
        [ControlType.ScrollBar] = UnifiedRole.ScrollBar,

        // Containers / structure
        [ControlType.Window] = UnifiedRole.Window,
        [ControlType.Pane] = UnifiedRole.Region,
        [ControlType.Group] = UnifiedRole.Group,
        [ControlType.Tab] = UnifiedRole.TabList,
        [ControlType.TabItem] = UnifiedRole.Tab,
        [ControlType.ToolBar] = UnifiedRole.Toolbar,
        [ControlType.StatusBar] = UnifiedRole.Status,
        [ControlType.Separator] = UnifiedRole.Separator,
        [ControlType.Document] = UnifiedRole.Document,

        // Menus
        [ControlType.Menu] = UnifiedRole.Menu,
        [ControlType.MenuBar] = UnifiedRole.MenuBar,
        [ControlType.MenuItem] = UnifiedRole.MenuItem,

        // Tables / grids
        [ControlType.Table] = UnifiedRole.Table,
        [ControlType.DataGrid] = UnifiedRole.Grid,
        [ControlType.DataItem] = UnifiedRole.Row,
        [ControlType.Header] = UnifiedRole.RowGroup,
        [ControlType.HeaderItem] = UnifiedRole.ColumnHeader,

        // Content
        [ControlType.Text] = UnifiedRole.Text,
        [ControlType.Image] = UnifiedRole.Image,
        [ControlType.ToolTip] = UnifiedRole.Tooltip,
        [ControlType.List] = UnifiedRole.List,
        [ControlType.ListItem] = UnifiedRole.ListItem,
        [ControlType.Tree] = UnifiedRole.Tree,
        [ControlType.TreeItem] = UnifiedRole.TreeItem,
        [ControlType.Calendar] = UnifiedRole.DatePicker,
        [ControlType.Thumb] = UnifiedRole.Generic,
        [ControlType.TitleBar] = UnifiedRole.Toolbar,
        [ControlType.SemanticZoom] = UnifiedRole.Region,
        [ControlType.AppBar] = UnifiedRole.Toolbar,
    };

    public static UnifiedRole Map(ControlType controlType)
    {
        return ControlTypeMap.GetValueOrDefault(controlType, UnifiedRole.Unknown);
    }

    public static UnifiedRole MapFromString(string roleName)
    {
        if (Enum.TryParse<UnifiedRole>(roleName, ignoreCase: true, out var role))
            return role;

        var normalized = roleName.Replace("-", "");
        if (Enum.TryParse<UnifiedRole>(normalized, ignoreCase: true, out var normalizedRole))
            return normalizedRole;

        return UnifiedRole.Unknown;
    }
}
