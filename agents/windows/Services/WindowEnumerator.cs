using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Definitions;
using FlaUI.UIA3;
using TestAnywareAgent.Models;

namespace TestAnywareAgent.Services;

public sealed class WindowEnumerator : IDisposable
{
    private readonly UIA3Automation _automation = new();

    public List<WindowInfo> EnumerateWindows()
    {
        var desktop = _automation.GetDesktop();
        var results = new List<WindowInfo>();

        AutomationElement[] topLevelWindows;
        try
        {
            topLevelWindows = desktop.FindAllChildren(
                cf => cf.ByControlType(ControlType.Window));
        }
        catch
        {
            return results;
        }

        var foregroundHandle = NativeMethods.GetForegroundWindow();

        foreach (var win in topLevelWindows)
        {
            try
            {
                var name = win.Properties.Name.ValueOrDefault;
                if (string.IsNullOrEmpty(name)) continue;

                var bounds = win.BoundingRectangle;
                var processId = win.Properties.ProcessId.ValueOrDefault;
                var appName = GetProcessName(processId);
                var isFocused = win.Properties.NativeWindowHandle.ValueOrDefault == foregroundHandle;

                results.Add(new WindowInfo
                {
                    Title = name,
                    WindowType = ClassifyWindowType(win),
                    SizeWidth = bounds.Width,
                    SizeHeight = bounds.Height,
                    PositionX = bounds.X,
                    PositionY = bounds.Y,
                    AppName = appName,
                    Focused = isFocused,
                });
            }
            catch
            {
                continue;
            }
        }

        return results;
    }

    public AutomationElement? FindWindowElement(string filter)
    {
        var desktop = _automation.GetDesktop();
        AutomationElement[] windows;
        try
        {
            windows = desktop.FindAllChildren(
                cf => cf.ByControlType(ControlType.Window));
        }
        catch
        {
            return null;
        }

        foreach (var win in windows)
        {
            try
            {
                if (WindowMatches(win, filter)) return win;
            }
            catch
            {
                continue;
            }
        }
        return null;
    }

    public AutomationElement? FindWindowByInfo(WindowInfo info)
    {
        var desktop = _automation.GetDesktop();
        AutomationElement[] windows;
        try
        {
            windows = desktop.FindAllChildren(
                cf => cf.ByControlType(ControlType.Window));
        }
        catch
        {
            return null;
        }

        foreach (var win in windows)
        {
            try
            {
                var bounds = win.BoundingRectangle;
                var name = win.Properties.Name.ValueOrDefault;
                if (name == info.Title &&
                    Math.Abs(bounds.X - info.PositionX) < 1 &&
                    Math.Abs(bounds.Y - info.PositionY) < 1 &&
                    Math.Abs(bounds.Width - info.SizeWidth) < 1 &&
                    Math.Abs(bounds.Height - info.SizeHeight) < 1)
                {
                    return win;
                }
            }
            catch
            {
                continue;
            }
        }
        return null;
    }

    public AutomationElement? FindLiveElement(ElementInfo info)
    {
        var desktop = _automation.GetDesktop();
        AutomationElement[] windows;
        try
        {
            windows = desktop.FindAllChildren(
                cf => cf.ByControlType(ControlType.Window));
        }
        catch
        {
            return null;
        }

        foreach (var win in windows)
        {
            var found = SearchLiveTree(win, info);
            if (found != null) return found;
        }
        return null;
    }

    public UIA3Automation Automation => _automation;

    private static AutomationElement? SearchLiveTree(AutomationElement root, ElementInfo info)
    {
        AutomationElement[] children;
        try
        {
            children = root.FindAllChildren();
        }
        catch
        {
            return null;
        }

        foreach (var child in children)
        {
            if (LiveElementMatches(child, info)) return child;
            var found = SearchLiveTree(child, info);
            if (found != null) return found;
        }
        return null;
    }

    private static bool LiveElementMatches(AutomationElement element, ElementInfo info)
    {
        try
        {
            var controlType = element.Properties.ControlType.ValueOrDefault;
            var role = RoleMapper.Map(controlType);
            if (role != info.Role) return false;

            var name = element.Properties.Name.ValueOrDefault;
            if (info.Label != null)
            {
                if (name != info.Label) return false;
            }
            else if (!string.IsNullOrEmpty(name))
            {
                return false;
            }

            if (info.PositionX.HasValue && info.PositionY.HasValue)
            {
                var bounds = element.BoundingRectangle;
                if (Math.Abs(bounds.X - info.PositionX.Value) > 1 ||
                    Math.Abs(bounds.Y - info.PositionY.Value) > 1)
                    return false;
            }

            return true;
        }
        catch
        {
            return false;
        }
    }

    public static bool WindowMatches(AutomationElement window, string filter)
    {
        var name = window.Properties.Name.ValueOrDefault ?? "";
        var processId = window.Properties.ProcessId.ValueOrDefault;
        var appName = GetProcessName(processId);

        return name.Contains(filter, StringComparison.OrdinalIgnoreCase) ||
               appName.Contains(filter, StringComparison.OrdinalIgnoreCase);
    }

    public static bool WindowInfoMatches(WindowInfo window, string filter)
    {
        return (window.Title?.Contains(filter, StringComparison.OrdinalIgnoreCase) ?? false) ||
               window.AppName.Contains(filter, StringComparison.OrdinalIgnoreCase);
    }

    private static string ClassifyWindowType(AutomationElement window)
    {
        try
        {
            if (window.Patterns.Window.IsSupported)
            {
                return window.Patterns.Window.Pattern.IsModal.ValueOrDefault
                    ? "dialog"
                    : "standard";
            }
        }
        catch { }
        return "standard";
    }

    private static string GetProcessName(int processId)
    {
        try
        {
            var process = System.Diagnostics.Process.GetProcessById(processId);
            return process.ProcessName;
        }
        catch
        {
            return "Unknown";
        }
    }

    public void Dispose()
    {
        _automation.Dispose();
    }
}

internal static class NativeMethods
{
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
