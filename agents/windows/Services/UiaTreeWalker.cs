using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using TestAnywareAgent.Models;

namespace TestAnywareAgent.Services;

public static class UiaTreeWalker
{
    public static List<ElementInfo> Walk(
        AutomationElement root,
        int depth,
        UnifiedRole? roleFilter,
        string? labelFilter)
    {
        var results = new List<ElementInfo>();
        WalkRecursive(root, depth, roleFilter, labelFilter, results);
        return results;
    }

    private static void WalkRecursive(
        AutomationElement element,
        int remainingDepth,
        UnifiedRole? roleFilter,
        string? labelFilter,
        List<ElementInfo> results)
    {
        if (remainingDepth < 0) return;

        AutomationElement[] children;
        try
        {
            children = element.FindAllChildren();
        }
        catch
        {
            return;
        }

        foreach (var child in children)
        {
            var info = BuildElementInfo(child, remainingDepth - 1, roleFilter, labelFilter);
            if (info == null) continue;

            var matchesRole = roleFilter == null || info.Role == roleFilter;
            var matchesLabel = labelFilter == null ||
                (info.Label?.Contains(labelFilter, StringComparison.OrdinalIgnoreCase) ?? false);

            if (matchesRole && matchesLabel)
            {
                results.Add(info);
            }
            else if (info.Children is { Count: > 0 })
            {
                results.Add(info);
            }
        }
    }

    public static ElementInfo? BuildElementInfo(
        AutomationElement element,
        int remainingDepth,
        UnifiedRole? roleFilter,
        string? labelFilter)
    {
        try
        {
            var controlType = element.Properties.ControlType.ValueOrDefault;
            var role = RoleMapper.Map(controlType);

            // Detect web content: Chromium-based apps (Chrome, Edge, Electron,
            // CEF, ElectroBun, Tauri/WebView2) all set AutomationId="RootWebArea"
            // on their web content Document element. Non-Chromium web engines
            // (Firefox) are caught by FrameworkId not being a native framework.
            if (role == UnifiedRole.Document)
            {
                var docAutomationId = element.Properties.AutomationId.ValueOrDefault;
                if (docAutomationId == "RootWebArea")
                {
                    role = UnifiedRole.WebArea;
                }
                else
                {
                    var frameworkId = element.Properties.FrameworkId.ValueOrDefault;
                    if (frameworkId is not ("Win32" or "WPF" or "XAML" or "WinForm" or "" or null))
                        role = UnifiedRole.WebArea;
                }
            }

            var name = element.Properties.Name.ValueOrDefault;
            var automationId = element.Properties.AutomationId.ValueOrDefault;
            var isEnabled = element.Properties.IsEnabled.ValueOrDefault;
            var hasKeyboardFocus = element.Properties.HasKeyboardFocus.ValueOrDefault;
            var isOffscreen = element.Properties.IsOffscreen.ValueOrDefault;

            string? value = null;
            try
            {
                if (element.Patterns.Value.IsSupported)
                    value = element.Patterns.Value.Pattern.Value.ValueOrDefault;
                else if (element.Patterns.RangeValue.IsSupported)
                    value = element.Patterns.RangeValue.Pattern.Value.ValueOrDefault.ToString();
                else if (element.Patterns.Toggle.IsSupported)
                    value = element.Patterns.Toggle.Pattern.ToggleState.ValueOrDefault.ToString();
                else if (element.Patterns.SelectionItem.IsSupported)
                    value = element.Patterns.SelectionItem.Pattern.IsSelected.ValueOrDefault.ToString();
            }
            catch { }

            var actions = new List<string>();
            try
            {
                if (element.Patterns.Invoke.IsSupported) actions.Add("press");
                if (element.Patterns.Toggle.IsSupported) actions.Add("toggle");
                if (element.Patterns.ExpandCollapse.IsSupported) actions.Add("expand-collapse");
                if (element.Patterns.SelectionItem.IsSupported) actions.Add("select");
                if (element.Patterns.Value.IsSupported) actions.Add("set-value");
                if (element.Patterns.ScrollItem.IsSupported) actions.Add("scroll-into-view");
            }
            catch { }

            double? posX = null, posY = null, sizeW = null, sizeH = null;
            try
            {
                var bounds = element.BoundingRectangle;
                if (!bounds.IsEmpty)
                {
                    posX = bounds.X;
                    posY = bounds.Y;
                    sizeW = bounds.Width;
                    sizeH = bounds.Height;
                }
            }
            catch { }

            int childCount;
            List<ElementInfo>? children = null;
            try
            {
                var childElements = element.FindAllChildren();
                childCount = childElements.Length;

                if (remainingDepth > 0 && childCount > 0)
                {
                    children = [];
                    foreach (var child in childElements)
                    {
                        var childInfo = BuildElementInfo(child, remainingDepth - 1, roleFilter, labelFilter);
                        if (childInfo != null) children.Add(childInfo);
                    }
                    if (children.Count == 0) children = null;
                }
            }
            catch
            {
                childCount = 0;
            }

            return new ElementInfo
            {
                Role = role,
                Label = string.IsNullOrEmpty(name) ? null : name,
                Value = string.IsNullOrEmpty(value) ? null : value,
                Description = null,
                Id = string.IsNullOrEmpty(automationId) ? null : automationId,
                Enabled = isEnabled,
                Focused = hasKeyboardFocus,
                Showing = !isOffscreen,
                PositionX = posX,
                PositionY = posY,
                SizeWidth = sizeW,
                SizeHeight = sizeH,
                ChildCount = childCount,
                Actions = actions,
                PlatformRole = role == UnifiedRole.Unknown ? controlType.ToString() : null,
                Children = children,
            };
        }
        catch
        {
            return null;
        }
    }
}
